use futures_util::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use smolpc_engine_core::inference::backend::CheckModelResponse;
use smolpc_engine_core::models::registry::ModelDefinition;
use smolpc_engine_core::{GenerationConfig, GenerationMetrics};
use std::time::{Duration, Instant};

use crate::{
    EngineClientError, EngineMeta, EngineStatus, StartupMode, StartupPolicy, WaitReadyOptions,
    LOAD_REQUEST_TIMEOUT, NON_STREAMING_REQUEST_TIMEOUT,
};

#[derive(Debug, Clone)]
pub struct EngineClient {
    pub(crate) base_url: String,
    pub(crate) token: String,
    pub(crate) http: Client,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EngineChatMessage {
    pub role: String,
    pub content: String,
}

impl EngineClient {
    pub(crate) fn new(base_url: String, token: String) -> Self {
        Self {
            base_url,
            token,
            http: Client::builder()
                .connect_timeout(Duration::from_secs(2))
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    pub(crate) fn auth_header(&self) -> String {
        format!("Bearer {}", self.token)
    }

    pub(crate) fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    pub async fn health(&self) -> Result<bool, EngineClientError> {
        let response = self
            .http
            .get(self.url("/engine/health"))
            .header(AUTHORIZATION, self.auth_header())
            .timeout(NON_STREAMING_REQUEST_TIMEOUT)
            .send()
            .await;

        match response {
            Ok(r) => Ok(r.status().is_success()),
            Err(e) if e.is_connect() => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn meta(&self) -> Result<EngineMeta, EngineClientError> {
        let response = self
            .http
            .get(self.url("/engine/meta"))
            .header(AUTHORIZATION, self.auth_header())
            .timeout(NON_STREAMING_REQUEST_TIMEOUT)
            .send()
            .await?;
        let response = ensure_success(response, "/engine/meta").await?;
        Ok(response.json::<EngineMeta>().await?)
    }

    pub async fn status(&self) -> Result<EngineStatus, EngineClientError> {
        let response = self
            .http
            .get(self.url("/engine/status"))
            .header(AUTHORIZATION, self.auth_header())
            .timeout(NON_STREAMING_REQUEST_TIMEOUT)
            .send()
            .await?;
        let response = ensure_success(response, "/engine/status").await?;
        Ok(response.json::<EngineStatus>().await?)
    }

    pub async fn ensure_started(
        &self,
        mode: StartupMode,
        startup_policy: StartupPolicy,
    ) -> Result<EngineStatus, EngineClientError> {
        let response = self
            .http
            .post(self.url("/engine/ensure-started"))
            .header(AUTHORIZATION, self.auth_header())
            .header(CONTENT_TYPE, "application/json")
            .body(
                serde_json::json!({
                    "mode": mode,
                    "startup_policy": startup_policy,
                })
                .to_string(),
            )
            .timeout(NON_STREAMING_REQUEST_TIMEOUT)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let parsed_payload = serde_json::from_str::<EngineStatus>(&body).ok();

        if status.is_success() {
            return parsed_payload.ok_or_else(|| {
                EngineClientError::Message(
                    "/engine/ensure-started returned success but payload was invalid".to_string(),
                )
            });
        }

        if let Some(payload) = parsed_payload {
            let code = payload
                .error_code
                .clone()
                .unwrap_or_else(|| "ENGINE_STARTUP_FAILED".to_string());
            let message = payload.error_message.clone().unwrap_or_else(|| {
                format!(
                    "/engine/ensure-started failed with HTTP {}",
                    status.as_u16()
                )
            });
            return Err(EngineClientError::Message(format!(
                "/engine/ensure-started failed with HTTP {} [{}]: {}",
                status.as_u16(),
                code,
                message
            )));
        }

        let detail = if body.trim().is_empty() {
            None
        } else if let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) {
            parse_error_message(&value).or_else(|| Some(value.to_string()))
        } else {
            Some(body)
        };

        let message = match detail {
            Some(detail) => format!(
                "/engine/ensure-started failed with HTTP {}: {}",
                status.as_u16(),
                detail
            ),
            None => format!(
                "/engine/ensure-started failed with HTTP {}",
                status.as_u16()
            ),
        };

        Err(EngineClientError::Message(message))
    }

    pub async fn wait_ready(
        &self,
        options: WaitReadyOptions,
    ) -> Result<EngineStatus, EngineClientError> {
        let timeout = options.timeout.max(Duration::from_millis(1));
        let poll_interval = options
            .poll_interval
            .max(Duration::from_millis(50))
            .min(Duration::from_secs(5));
        let started = Instant::now();

        loop {
            let status = self.status().await?;

            if status.is_ready() {
                return Ok(status);
            }

            if status.is_failed() {
                let message = status
                    .failure_message()
                    .unwrap_or_else(|| "Engine startup failed".to_string());
                return Err(EngineClientError::Message(message));
            }

            if started.elapsed() >= timeout {
                let state = status
                    .state
                    .clone()
                    .or(status.startup_phase.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                let detail = status
                    .failure_message()
                    .unwrap_or_else(|| "No additional error details".to_string());
                return Err(EngineClientError::Message(format!(
                    "Timed out waiting for engine readiness after {}s (state={}): {}",
                    timeout.as_secs_f32(),
                    state,
                    detail
                )));
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    pub async fn load_model(&self, model_id: &str) -> Result<(), EngineClientError> {
        let response = self
            .http
            .post(self.url("/engine/load"))
            .header(AUTHORIZATION, self.auth_header())
            .header(CONTENT_TYPE, "application/json")
            .body(serde_json::json!({"model_id": model_id}).to_string())
            .timeout(LOAD_REQUEST_TIMEOUT)
            .send()
            .await?;
        let _ = ensure_success(response, "/engine/load").await?;
        Ok(())
    }

    pub async fn unload_model(&self, force: bool) -> Result<(), EngineClientError> {
        let response = self
            .http
            .post(self.url("/engine/unload"))
            .header(AUTHORIZATION, self.auth_header())
            .header(CONTENT_TYPE, "application/json")
            .body(serde_json::json!({"force": force}).to_string())
            .timeout(LOAD_REQUEST_TIMEOUT)
            .send()
            .await?;
        let _ = ensure_success(response, "/engine/unload").await?;
        Ok(())
    }

    pub async fn cancel(&self) -> Result<(), EngineClientError> {
        let response = self
            .http
            .post(self.url("/engine/cancel"))
            .header(AUTHORIZATION, self.auth_header())
            .timeout(NON_STREAMING_REQUEST_TIMEOUT)
            .send()
            .await?;
        let _ = ensure_success(response, "/engine/cancel").await?;
        Ok(())
    }

    /// Request graceful engine shutdown. No client-side timeout — caller owns the deadline.
    pub async fn shutdown(&self) -> Result<(), EngineClientError> {
        let response = self
            .http
            .post(self.url("/engine/shutdown"))
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await?;
        ensure_success(response, "/engine/shutdown").await?;
        Ok(())
    }

    /// Returns the full lane-based readiness response for a model.
    pub async fn check_model_readiness(
        &self,
        model_id: &str,
    ) -> Result<CheckModelResponse, EngineClientError> {
        let response = self
            .http
            .post(self.url("/engine/check-model"))
            .header(AUTHORIZATION, self.auth_header())
            .header(CONTENT_TYPE, "application/json")
            .body(serde_json::json!({"model_id": model_id}).to_string())
            .timeout(NON_STREAMING_REQUEST_TIMEOUT)
            .send()
            .await?;
        let response = ensure_success(response, "/engine/check-model").await?;
        Ok(response.json::<CheckModelResponse>().await?)
    }

    /// Compatibility shim for older callers.
    ///
    /// Prefer `check_model_readiness()` for new code.
    pub async fn check_model_exists(&self, model_id: &str) -> Result<bool, EngineClientError> {
        Ok(self.check_model_readiness(model_id).await?.any_ready())
    }

    pub async fn list_models(&self) -> Result<Vec<ModelDefinition>, EngineClientError> {
        let response = self
            .http
            .get(self.url("/v1/models"))
            .header(AUTHORIZATION, self.auth_header())
            .timeout(NON_STREAMING_REQUEST_TIMEOUT)
            .send()
            .await?;
        let response = ensure_success(response, "/v1/models").await?;
        let value = response.json::<serde_json::Value>().await?;
        parse_models_response(&value)
    }

    pub async fn generate_stream<F>(
        &self,
        prompt: &str,
        config: Option<GenerationConfig>,
        on_token: F,
    ) -> Result<GenerationMetrics, EngineClientError>
    where
        F: FnMut(String),
    {
        let body = completion_body(&prompt_as_messages(prompt), true, config);
        let response = self
            .http
            .post(self.url("/v1/chat/completions"))
            .header(AUTHORIZATION, self.auth_header())
            .header(CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send()
            .await
            .map_err(classify_connection_error)?;
        let response = ensure_success(response, "/v1/chat/completions").await?;
        consume_sse_stream(response, on_token).await
    }

    pub async fn generate_stream_messages<F>(
        &self,
        messages: &[EngineChatMessage],
        config: Option<GenerationConfig>,
        on_token: F,
    ) -> Result<GenerationMetrics, EngineClientError>
    where
        F: FnMut(String),
    {
        let body = completion_body(messages, true, config);
        let response = self
            .http
            .post(self.url("/v1/chat/completions"))
            .header(AUTHORIZATION, self.auth_header())
            .header(CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send()
            .await
            .map_err(classify_connection_error)?;
        let response = ensure_success(response, "/v1/chat/completions").await?;
        consume_sse_stream(response, on_token).await
    }
}

pub(crate) async fn consume_sse_stream<F>(
    response: reqwest::Response,
    mut on_token: F,
) -> Result<GenerationMetrics, EngineClientError>
where
    F: FnMut(String),
{
    let started = Instant::now();
    let mut emitted_chunks = 0usize;
    let mut first_chunk_at = None;
    let mut host_metrics: Option<GenerationMetrics> = None;

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(classify_connection_error)?;
        let text = String::from_utf8_lossy(&bytes);
        buffer.push_str(&text);

        let mut consumed_until = 0usize;
        while let Some(rel_newline) = buffer[consumed_until..].find('\n') {
            let newline = consumed_until + rel_newline;
            let line = buffer[consumed_until..newline].trim();
            consumed_until = newline + 1;
            if !line.starts_with("data:") {
                continue;
            }
            let data = line[5..].trim();
            if data == "[DONE]" {
                return Ok(host_metrics.unwrap_or_else(|| {
                    fallback_stream_metrics(started, emitted_chunks, first_chunk_at)
                }));
            }
            if data.is_empty() {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(error_message) = parse_error_message(&value) {
                    return Err(EngineClientError::Message(error_message));
                }

                if let Some(metrics_value) = value.get("smolpc_metrics") {
                    host_metrics = Some(serde_json::from_value(metrics_value.clone())?);
                    continue;
                }

                if value.get("object").and_then(|object| object.as_str())
                    == Some("chat.completion.metrics")
                {
                    return Err(EngineClientError::Message(
                        "Engine stream metrics event is missing required smolpc_metrics payload"
                            .to_string(),
                    ));
                }

                if let Some(content) = value
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|first| first.get("delta"))
                    .and_then(|d| d.get("content"))
                    .and_then(|c| c.as_str())
                {
                    emitted_chunks += 1;
                    if first_chunk_at.is_none() {
                        first_chunk_at = Some(started.elapsed().as_millis() as u64);
                    }
                    on_token(content.to_string());
                }
            }
        }

        if consumed_until > 0 {
            buffer.drain(..consumed_until);
        }
    }

    Ok(host_metrics
        .unwrap_or_else(|| fallback_stream_metrics(started, emitted_chunks, first_chunk_at)))
}

pub(crate) async fn ensure_success(
    response: reqwest::Response,
    context: &str,
) -> Result<reqwest::Response, EngineClientError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let body = response.text().await.unwrap_or_default();
    let detail = if body.trim().is_empty() {
        None
    } else if let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) {
        parse_error_message(&value).or_else(|| Some(value.to_string()))
    } else {
        Some(body)
    };

    let message = match detail {
        Some(detail) => format!("{context} failed with HTTP {}: {}", status.as_u16(), detail),
        None => format!("{context} failed with HTTP {}", status.as_u16()),
    };

    Err(EngineClientError::Message(message))
}

pub(crate) fn completion_body(
    messages: &[EngineChatMessage],
    stream: bool,
    config: Option<GenerationConfig>,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": "smolpc-engine",
        "stream": stream,
        "messages": messages,
    });

    if let Some(config) = config {
        body["max_tokens"] = serde_json::json!(config.max_length);
        body["temperature"] = serde_json::json!(config.temperature);
        body["top_p"] = serde_json::json!(config.top_p);
        body["top_k"] = serde_json::json!(config.top_k);
        body["repetition_penalty"] = serde_json::json!(config.repetition_penalty);
        body["repetition_penalty_last_n"] = serde_json::json!(config.repetition_penalty_last_n);
    }

    body
}

pub(crate) fn prompt_as_messages(prompt: &str) -> Vec<EngineChatMessage> {
    vec![EngineChatMessage {
        role: "user".to_string(),
        content: prompt.to_string(),
    }]
}

pub(crate) fn fallback_stream_metrics(
    started: Instant,
    emitted_chunks: usize,
    first_chunk_at: Option<u64>,
) -> GenerationMetrics {
    let total_time_ms = started.elapsed().as_millis() as u64;
    let tokens_per_second = if emitted_chunks > 0 && total_time_ms > 0 {
        emitted_chunks as f64 / (total_time_ms as f64 / 1_000.0)
    } else {
        0.0
    };

    GenerationMetrics {
        total_tokens: emitted_chunks,
        time_to_first_token_ms: first_chunk_at,
        tokens_per_second,
        total_time_ms,
        truncated: false,
        truncation_reason: None,
    }
}

pub(crate) fn parse_error_message(value: &serde_json::Value) -> Option<String> {
    let error = value.get("error")?;
    if let Some(message) = error.get("message").and_then(|m| m.as_str()) {
        return Some(message.to_string());
    }
    if let Some(message) = error.as_str() {
        return Some(message.to_string());
    }
    Some(error.to_string())
}

pub(crate) fn parse_models_response(
    value: &serde_json::Value,
) -> Result<Vec<ModelDefinition>, EngineClientError> {
    let data = value
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| {
            EngineClientError::Message(
                "Invalid /v1/models response: expected top-level 'data' array".to_string(),
            )
        })?;

    let mut out = Vec::with_capacity(data.len());
    for item in data {
        let Some(id) = item.get("id").and_then(|s| s.as_str()) else {
            return Err(EngineClientError::Message(
                "Invalid /v1/models response: every model entry must contain string 'id'"
                    .to_string(),
            ));
        };

        if let Some(model) = smolpc_engine_core::models::registry::ModelRegistry::get_model(id) {
            out.push(model);
        } else {
            log::warn!("Host reported unknown model id '{id}' in /v1/models; ignoring entry");
        }
    }

    if out.is_empty() && !data.is_empty() {
        return Err(EngineClientError::Message(format!(
            "Host returned {} models but none matched local registry IDs",
            data.len()
        )));
    }

    Ok(out)
}

pub(crate) fn classify_connection_error(e: reqwest::Error) -> EngineClientError {
    if e.is_connect() || e.is_request() {
        EngineClientError::EngineCrashed(e.to_string())
    } else {
        EngineClientError::Http(e)
    }
}
