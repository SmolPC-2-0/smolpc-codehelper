use futures_util::StreamExt;
use rand::distributions::{Alphanumeric, DistString};
use rand::rngs::OsRng;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use smolpc_engine_core::inference::backend::{BackendStatus, CheckModelResponse};
use smolpc_engine_core::models::registry::ModelDefinition;
use smolpc_engine_core::{GenerationConfig, GenerationMetrics};
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

const ENGINE_PROTOCOL_VERSION: &str = "1.0.0";
const ENGINE_API_VERSION: &str = "1.0.0";
const ENGINE_HOST_BASENAME: &str = "smolpc-engine-host";
const SPAWN_LOCK_FILENAME: &str = "engine-spawn.lock";
const SPAWN_LOG_FILENAME: &str = "engine-spawn.log";
const SPAWN_LOCK_WAIT: Duration = Duration::from_secs(10);
const SPAWN_LOCK_STALE_AGE: Duration = Duration::from_secs(30);
pub(crate) const FORCE_EP_ENV: &str = "SMOLPC_FORCE_EP";
pub(crate) const DML_DEVICE_ENV: &str = "SMOLPC_DML_DEVICE_ID";
const SHARED_MODELS_VENDOR_DIR: &str = "SmolPC";
const SHARED_MODELS_DIR: &str = "models";
const DEFAULT_WAIT_READY_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_WAIT_READY_POLL_INTERVAL: Duration = Duration::from_millis(250);
const NON_STREAMING_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const LOAD_REQUEST_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeModePreference {
    #[default]
    Auto,
    Cpu,
    Dml,
    Npu,
}

impl RuntimeModePreference {
    fn as_force_override(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::Cpu => Some("cpu"),
            Self::Dml => Some("dml"),
            Self::Npu => Some("openvino_npu"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeEnvOverrides {
    pub runtime_mode: RuntimeModePreference,
    pub dml_device_id: Option<i32>,
}

impl Default for RuntimeEnvOverrides {
    fn default() -> Self {
        Self {
            runtime_mode: RuntimeModePreference::Auto,
            dml_device_id: None,
        }
    }
}

fn parse_runtime_mode_override(value: &str) -> Option<RuntimeModePreference> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cpu" => Some(RuntimeModePreference::Cpu),
        "dml" | "directml" => Some(RuntimeModePreference::Dml),
        "npu" | "openvino" | "openvino_npu" => Some(RuntimeModePreference::Npu),
        _ => {
            log::warn!(
                "Ignoring unsupported {FORCE_EP_ENV} value '{value}'; expected one of: cpu, dml, directml, npu, openvino"
            );
            None
        }
    }
}

pub fn read_runtime_env_overrides() -> RuntimeEnvOverrides {
    let runtime_mode = std::env::var(FORCE_EP_ENV)
        .ok()
        .and_then(|value| parse_runtime_mode_override(&value))
        .unwrap_or(RuntimeModePreference::Auto);
    let dml_device_id = match std::env::var(DML_DEVICE_ENV) {
        Ok(value) => match value.parse::<i32>() {
            Ok(parsed) => Some(parsed),
            Err(_) => {
                log::warn!(
                    "Ignoring invalid {DML_DEVICE_ENV} value '{value}'; expected a signed integer"
                );
                None
            }
        },
        Err(_) => None,
    };
    RuntimeEnvOverrides {
        runtime_mode,
        dml_device_id,
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StartupMode {
    #[default]
    Auto,
    DirectmlRequired,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct StartupPolicy {
    pub default_model_id: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LastStartupError {
    pub attempt_id: String,
    pub phase: String,
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub at: String,
}

#[derive(Debug, Clone, Copy)]
pub struct WaitReadyOptions {
    pub timeout: Duration,
    pub poll_interval: Duration,
}

impl Default for WaitReadyOptions {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_WAIT_READY_TIMEOUT,
            poll_interval: DEFAULT_WAIT_READY_POLL_INTERVAL,
        }
    }
}

struct SpawnLockGuard {
    path: PathBuf,
}

impl Drop for SpawnLockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EngineClientError {
    #[error("{0}")]
    Message(String),
    #[error("Engine process crashed or is unreachable: {0}")]
    EngineCrashed(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct EngineConnectOptions {
    pub port: u16,
    pub app_version: String,
    pub shared_runtime_dir: PathBuf,
    pub data_dir: PathBuf,
    pub resource_dir: Option<PathBuf>,
    pub models_dir: Option<PathBuf>,
    pub host_binary: Option<PathBuf>,
    pub runtime_mode: RuntimeModePreference,
    pub dml_device_id: Option<i32>,
    pub force_respawn: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EngineMeta {
    pub ok: bool,
    pub protocol_version: String,
    #[serde(default = "default_engine_api_version")]
    pub engine_api_version: String,
    pub engine_version: String,
    pub pid: u32,
    pub busy: bool,
}

impl EngineMeta {
    pub fn effective_engine_api_version(&self) -> &str {
        &self.engine_api_version
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EngineStatus {
    pub ok: bool,
    #[serde(default)]
    pub ready: bool,
    #[serde(default = "default_attempt_id")]
    pub attempt_id: String,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub startup_phase: Option<String>,
    #[serde(default)]
    pub state_since: Option<String>,
    #[serde(default)]
    pub active_backend: Option<String>,
    #[serde(default)]
    pub active_model_id: Option<String>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub retryable: Option<bool>,
    #[serde(default)]
    pub last_error: Option<LastStartupError>,
    #[serde(default = "default_engine_api_version")]
    pub engine_api_version: String,
    #[serde(default)]
    pub effective_mode: Option<String>,
    #[serde(default)]
    pub effective_startup_policy: Option<StartupPolicy>,
    pub current_model: Option<String>,
    pub generating: bool,
    pub backend_status: BackendStatus,
}

impl EngineStatus {
    pub fn is_ready(&self) -> bool {
        if self.ready {
            return true;
        }

        if self
            .state
            .as_deref()
            .is_some_and(|state| state.eq_ignore_ascii_case("ready"))
        {
            return true;
        }

        self.current_model.is_some()
    }

    pub fn is_failed(&self) -> bool {
        if self
            .state
            .as_deref()
            .is_some_and(|state| state.eq_ignore_ascii_case("failed"))
        {
            return true;
        }

        self.error_code.is_some() || self.error_message.is_some() || self.last_error.is_some()
    }

    pub fn failure_message(&self) -> Option<String> {
        if let Some(last_error) = self.last_error.as_ref() {
            return Some(format!("{}: {}", last_error.code, last_error.message));
        }

        if let Some(code) = self.error_code.as_deref() {
            let message = self
                .error_message
                .as_deref()
                .unwrap_or("Engine startup failed");
            return Some(format!("{code}: {message}"));
        }

        self.error_message.clone()
    }
}

#[derive(Debug, Clone)]
pub struct EngineClient {
    base_url: String,
    token: String,
    http: Client,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EngineChatMessage {
    pub role: String,
    pub content: String,
}

impl EngineClient {
    fn new(base_url: String, token: String) -> Self {
        Self {
            base_url,
            token,
            http: Client::builder()
                .connect_timeout(Duration::from_secs(2))
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.token)
    }

    fn url(&self, path: &str) -> String {
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

async fn consume_sse_stream<F>(
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

fn default_engine_api_version() -> String {
    ENGINE_API_VERSION.to_string()
}

fn default_attempt_id() -> String {
    "unknown".to_string()
}

async fn ensure_success(
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

#[derive(Debug, PartialEq, Eq)]
enum RunningHostPolicyDecision {
    Reuse,
    Restart,
    Reject(String),
}

fn decide_running_host_policy(
    protocol_matches: bool,
    generating: bool,
    force_override: Option<&str>,
    force_respawn: bool,
) -> RunningHostPolicyDecision {
    if !protocol_matches {
        if generating {
            return RunningHostPolicyDecision::Reject(
                "Running engine protocol is incompatible and daemon is busy".to_string(),
            );
        }
        return RunningHostPolicyDecision::Restart;
    }

    if force_override.is_some() || force_respawn {
        if generating {
            let policy = force_override
                .map(|value| format!("{FORCE_EP_ENV}={value}"))
                .unwrap_or_else(|| "forced respawn policy".to_string());
            return RunningHostPolicyDecision::Reject(format!(
                "Engine is busy and cannot apply {policy}. Cancel generation and retry."
            ));
        }
        return RunningHostPolicyDecision::Restart;
    }

    RunningHostPolicyDecision::Reuse
}

pub async fn connect_or_spawn(
    options: EngineConnectOptions,
) -> Result<EngineClient, EngineClientError> {
    std::fs::create_dir_all(&options.shared_runtime_dir)?;
    std::fs::create_dir_all(&options.data_dir)?;

    let token_path = options.shared_runtime_dir.join("engine-token.txt");
    let token = load_or_create_token(&token_path)?;
    let base_url = format!("http://127.0.0.1:{}", options.port);
    let client = EngineClient::new(base_url, token.clone());
    let force_override = options.runtime_mode.as_force_override();
    let force_respawn = options.force_respawn;

    if enforce_running_host_policy(&client, force_override, force_respawn).await? {
        return Ok(client);
    }

    let _spawn_lock = acquire_spawn_lock(&options.shared_runtime_dir).await?;
    if enforce_running_host_policy(&client, force_override, force_respawn).await? {
        return Ok(client);
    }

    if !client.health().await.unwrap_or(false) {
        // Kill any stale engine on our port before spawning a fresh one.
        kill_stale_engine_on_port(options.port);

        // Regenerate the token so client and freshly-spawned host share the
        // same secret — stale tokens from a dead host are the #1 cause of
        // "failed to become healthy" on clean installs.
        let _ = std::fs::remove_file(&token_path);
        let fresh_token = load_or_create_token(&token_path)?;
        let client = EngineClient::new(
            format!("http://127.0.0.1:{}", options.port),
            fresh_token.clone(),
        );

        spawn_host(&options, &fresh_token)?;

        let spawn_log = options.shared_runtime_dir.join(SPAWN_LOG_FILENAME);
        let started = std::time::Instant::now();
        loop {
            if client.health().await.unwrap_or(false) {
                return finish_connect(client).await;
            }
            if started.elapsed() > Duration::from_secs(30) {
                let log_hint = if spawn_log.exists() {
                    format!(" Check spawn log: {}", spawn_log.display())
                } else {
                    String::new()
                };
                return Err(EngineClientError::Message(format!(
                    "Engine failed to become healthy within 30s.{log_hint}"
                )));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    finish_connect(client).await
}

async fn finish_connect(client: EngineClient) -> Result<EngineClient, EngineClientError> {
    let meta = client.meta().await?;
    if !protocol_major_matches(&meta.protocol_version, ENGINE_PROTOCOL_VERSION) {
        return Err(EngineClientError::Message(format!(
            "Engine protocol mismatch: {}",
            meta.protocol_version
        )));
    }
    Ok(client)
}

async fn enforce_running_host_policy(
    client: &EngineClient,
    force_override: Option<&str>,
    force_respawn: bool,
) -> Result<bool, EngineClientError> {
    if !client.health().await.unwrap_or(false) {
        return Ok(false);
    }

    let meta = client.meta().await?;
    let protocol_matches = protocol_major_matches(&meta.protocol_version, ENGINE_PROTOCOL_VERSION);
    let needs_status_probe = !protocol_matches || force_override.is_some() || force_respawn;
    if !needs_status_probe {
        return Ok(true);
    }

    let status = client.status().await?;
    match decide_running_host_policy(
        protocol_matches,
        status.generating,
        force_override,
        force_respawn,
    ) {
        RunningHostPolicyDecision::Reuse => Ok(true),
        RunningHostPolicyDecision::Restart => {
            request_engine_shutdown(client).await?;
            wait_for_engine_down(client, Duration::from_secs(5)).await?;
            Ok(false)
        }
        RunningHostPolicyDecision::Reject(message) => Err(EngineClientError::Message(message)),
    }
}

async fn request_engine_shutdown(client: &EngineClient) -> Result<(), EngineClientError> {
    let response = client
        .http
        .post(client.url("/engine/shutdown"))
        .header(AUTHORIZATION, client.auth_header())
        .timeout(NON_STREAMING_REQUEST_TIMEOUT)
        .send()
        .await;

    match response {
        Ok(r) => {
            r.error_for_status()?;
            Ok(())
        }
        Err(e) if e.is_connect() => Ok(()),
        Err(e) => Err(e.into()),
    }
}

async fn wait_for_engine_down(
    client: &EngineClient,
    timeout: Duration,
) -> Result<(), EngineClientError> {
    let started = Instant::now();
    while client.health().await.unwrap_or(false) {
        if started.elapsed() > timeout {
            return Err(EngineClientError::Message(
                "Engine shutdown timed out while applying runtime policy".to_string(),
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Ok(())
}

fn prompt_as_messages(prompt: &str) -> Vec<EngineChatMessage> {
    vec![EngineChatMessage {
        role: "user".to_string(),
        content: prompt.to_string(),
    }]
}

fn completion_body(
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

fn protocol_major_matches(actual: &str, expected: &str) -> bool {
    let a = actual.split('.').next().unwrap_or(actual);
    let e = expected.split('.').next().unwrap_or(expected);
    a == e
}

pub fn version_major(version: &str) -> Option<u64> {
    version
        .trim()
        .split('.')
        .next()
        .and_then(|major| major.parse::<u64>().ok())
}

pub fn engine_api_major_compatible(actual_version: &str, required_major: u64) -> bool {
    version_major(actual_version).is_some_and(|major| major >= required_major)
}

pub fn expected_engine_api_major() -> Option<u64> {
    version_major(ENGINE_API_VERSION)
}

fn classify_connection_error(e: reqwest::Error) -> EngineClientError {
    if e.is_connect() || e.is_request() {
        EngineClientError::EngineCrashed(e.to_string())
    } else {
        EngineClientError::Http(e)
    }
}

fn parse_error_message(value: &serde_json::Value) -> Option<String> {
    let error = value.get("error")?;
    if let Some(message) = error.get("message").and_then(|m| m.as_str()) {
        return Some(message.to_string());
    }
    if let Some(message) = error.as_str() {
        return Some(message.to_string());
    }
    Some(error.to_string())
}

fn parse_models_response(
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

fn fallback_stream_metrics(
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
    }
}

fn load_or_create_token(path: &Path) -> Result<String, EngineClientError> {
    if let Some(token) = read_non_empty_token(path)? {
        return Ok(token);
    }

    for _ in 0..3 {
        let token = Alphanumeric.sample_string(&mut OsRng, 48);

        match create_new_token_file(path, &token) {
            Ok(()) => return Ok(token),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                if let Some(existing) = read_non_empty_token(path)? {
                    return Ok(existing);
                }
                let _ = std::fs::remove_file(path);
            }
            Err(error) => return Err(error.into()),
        }
    }

    Err(EngineClientError::Message(
        "Failed to initialize engine token file after retrying".to_string(),
    ))
}

fn read_non_empty_token(path: &Path) -> Result<Option<String>, EngineClientError> {
    match std::fs::read_to_string(path) {
        Ok(token) => {
            let trimmed = token.trim().to_string();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed))
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn create_new_token_file(path: &Path, token: &str) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(token.as_bytes())?;
        return Ok(());
    }

    #[cfg(not(unix))]
    {
        // TODO: Harden Windows ACLs for engine-token.txt so only the current user can read it.
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(token.as_bytes())?;
        Ok(())
    }
}

fn spawn_host(options: &EngineConnectOptions, token: &str) -> Result<(), EngineClientError> {
    let host_bin = resolve_host_binary(options)?;

    // Write spawn diagnostics before attempting launch.
    let spawn_log_path = options.shared_runtime_dir.join(SPAWN_LOG_FILENAME);
    let spawn_log = write_spawn_diagnostics(&spawn_log_path, &host_bin, options, token);

    let mut cmd = Command::new(&host_bin);
    cmd.arg("--port")
        .arg(options.port.to_string())
        .arg("--data-dir")
        .arg(&options.data_dir)
        .arg("--app-version")
        .arg(&options.app_version)
        .env("SMOLPC_ENGINE_TOKEN", token)
        .env("SMOLPC_ENGINE_PORT", options.port.to_string())
        .env("RUST_LOG", "info");

    if let Some(force_ep) = options.runtime_mode.as_force_override() {
        cmd.env(FORCE_EP_ENV, force_ep);
    } else {
        cmd.env_remove(FORCE_EP_ENV);
    }

    if let Some(device_id) = options.dml_device_id {
        cmd.env(DML_DEVICE_ENV, device_id.to_string());
    } else {
        cmd.env_remove(DML_DEVICE_ENV);
    }

    if let Some(resource_dir) = &options.resource_dir {
        cmd.arg("--resource-dir").arg(resource_dir);
    }
    if let Some(models_dir) = options
        .models_dir
        .as_ref()
        .cloned()
        .or_else(default_shared_models_dir)
    {
        cmd.env("SMOLPC_MODELS_DIR", &models_dir);
    }

    // Redirect engine stderr to the spawn log so crash output is captured.
    let stderr_target = spawn_log
        .and_then(|path| std::fs::File::options().append(true).open(path).ok())
        .map(std::process::Stdio::from)
        .unwrap_or_else(std::process::Stdio::null);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(stderr_target);
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(stderr_target);
    }

    cmd.spawn()?;
    Ok(())
}

/// Write pre-spawn diagnostics to a log file for post-mortem debugging.
fn write_spawn_diagnostics(
    log_path: &Path,
    host_bin: &Path,
    options: &EngineConnectOptions,
    _token: &str,
) -> Option<PathBuf> {
    use std::fmt::Write as _;
    let mut buf = String::new();
    let _ = writeln!(buf, "--- spawn diagnostics {} ---", chrono_stamp());
    let _ = writeln!(buf, "engine_binary: {}", host_bin.display());
    let _ = writeln!(buf, "binary_exists: {}", host_bin.exists());
    let _ = writeln!(buf, "port: {}", options.port);
    let _ = writeln!(
        buf,
        "resource_dir: {}",
        options
            .resource_dir
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".into())
    );
    let _ = writeln!(buf, "data_dir: {}", options.data_dir.display());
    let _ = writeln!(
        buf,
        "shared_runtime_dir: {}",
        options.shared_runtime_dir.display()
    );
    let models_dir = options
        .models_dir
        .as_ref()
        .cloned()
        .or_else(default_shared_models_dir);
    let _ = writeln!(
        buf,
        "models_dir: {}",
        models_dir
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".into())
    );
    let token_path = options.shared_runtime_dir.join("engine-token.txt");
    let _ = writeln!(buf, "token_path: {}", token_path.display());
    let _ = writeln!(buf, "token_exists: {}", token_path.exists());

    // Check libs directory reachable from resource_dir
    if let Some(rd) = &options.resource_dir {
        let libs = rd.join("libs");
        let ov_libs = rd.join("libs").join("openvino");
        let _ = writeln!(buf, "libs_dir_exists: {}", libs.exists());
        let _ = writeln!(buf, "openvino_libs_dir_exists: {}", ov_libs.exists());
    }

    let _ = writeln!(buf, "---");
    match std::fs::write(log_path, &buf) {
        Ok(()) => Some(log_path.to_path_buf()),
        Err(_) => None,
    }
}

fn chrono_stamp() -> String {
    use std::time::SystemTime;
    let d = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s", d.as_secs())
}

/// Best-effort kill of any stale smolpc-engine-host process bound to our port.
fn kill_stale_engine_on_port(_port: u16) {
    // On Windows, find and kill any lingering engine-host processes.
    // This is a blunt instrument but prevents stale-port deadlocks.
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "smolpc-engine-host.exe"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        // Brief sleep to let the port release.
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn default_shared_models_dir() -> Option<PathBuf> {
    let base = dirs::data_local_dir()?;
    let path = base.join(SHARED_MODELS_VENDOR_DIR).join(SHARED_MODELS_DIR);
    path.exists().then_some(path)
}

async fn acquire_spawn_lock(
    shared_runtime_dir: &Path,
) -> Result<SpawnLockGuard, EngineClientError> {
    let lock_path = shared_runtime_dir.join(SPAWN_LOCK_FILENAME);
    let started = Instant::now();

    loop {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                let _ = writeln!(file, "pid={}", std::process::id());
                return Ok(SpawnLockGuard { path: lock_path });
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                // Check if lock holder PID is still alive.
                if is_lock_holder_dead(&lock_path) {
                    let _ = std::fs::remove_file(&lock_path);
                    continue;
                }

                let stale = std::fs::metadata(&lock_path)
                    .and_then(|meta| meta.modified())
                    .ok()
                    .and_then(|modified| modified.elapsed().ok())
                    .is_some_and(|age| age > SPAWN_LOCK_STALE_AGE);
                if stale {
                    let _ = std::fs::remove_file(&lock_path);
                    continue;
                }

                if started.elapsed() > SPAWN_LOCK_WAIT {
                    // Force-remove lock after timeout as last resort.
                    let _ = std::fs::remove_file(&lock_path);
                    continue;
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

/// Check if the PID recorded in the lock file is still running.
fn is_lock_holder_dead(lock_path: &Path) -> bool {
    let Ok(contents) = std::fs::read_to_string(lock_path) else {
        return true; // Can't read -> treat as dead.
    };
    let Some(pid_str) = contents
        .lines()
        .find_map(|line| line.strip_prefix("pid="))
    else {
        return true; // No PID recorded -> treat as dead.
    };
    let Ok(pid) = pid_str.trim().parse::<u32>() else {
        return true;
    };

    #[cfg(target_os = "windows")]
    {
        // OpenProcess with SYNCHRONIZE (0x00100000) returns null if process doesn't exist.
        extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
            fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        }
        const SYNCHRONIZE: u32 = 0x0010_0000;
        let handle = unsafe { OpenProcess(SYNCHRONIZE, 0, pid) };
        if handle.is_null() {
            return true; // Process doesn't exist.
        }
        unsafe { CloseHandle(handle) };
        false // Process is alive.
    }

    #[cfg(unix)]
    {
        // kill(pid, 0) checks if process exists without sending a signal.
        unsafe { libc::kill(pid as i32, 0) != 0 }
    }
}

fn host_binary_candidates() -> Vec<String> {
    let mut candidates = vec![format!(
        "{}{}",
        ENGINE_HOST_BASENAME,
        std::env::consts::EXE_SUFFIX
    )];
    if let Ok(target_triple) = std::env::var("TAURI_ENV_TARGET_TRIPLE") {
        candidates.push(format!(
            "{}-{}{}",
            ENGINE_HOST_BASENAME,
            target_triple,
            std::env::consts::EXE_SUFFIX
        ));
    }
    candidates
}

fn find_host_binary_in_dir(dir: &Path) -> Option<PathBuf> {
    for candidate in host_binary_candidates() {
        let full_path = dir.join(&candidate);
        if full_path.exists() {
            return Some(full_path);
        }
    }

    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        if file_name.starts_with(ENGINE_HOST_BASENAME) {
            return Some(path);
        }
    }

    None
}

fn resolve_host_binary(options: &EngineConnectOptions) -> Result<PathBuf, EngineClientError> {
    if let Some(path) = &options.host_binary {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    if let Ok(path) = std::env::var("SMOLPC_ENGINE_HOST_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }

    if let Some(resource_dir) = &options.resource_dir {
        if let Some(path) = find_host_binary_in_dir(resource_dir) {
            return Ok(path);
        }
        let binaries_dir = resource_dir.join("binaries");
        if let Some(path) = find_host_binary_in_dir(&binaries_dir) {
            return Ok(path);
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            if let Some(path) = find_host_binary_in_dir(dir) {
                return Ok(path);
            }

            let resources_dir = dir.join("resources");
            if let Some(path) = find_host_binary_in_dir(&resources_dir) {
                return Ok(path);
            }
        }
    }

    #[cfg(debug_assertions)]
    {
        let fallback = PathBuf::from("target").join("debug").join(format!(
            "{}{}",
            ENGINE_HOST_BASENAME,
            std::env::consts::EXE_SUFFIX
        ));
        if fallback.exists() {
            return Ok(fallback);
        }

        let fallback_release = PathBuf::from("target").join("release").join(format!(
            "{}{}",
            ENGINE_HOST_BASENAME,
            std::env::consts::EXE_SUFFIX
        ));
        if fallback_release.exists() {
            return Ok(fallback_release);
        }
    }

    Err(EngineClientError::Message(
        "Unable to locate smolpc-engine-host binary".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::with_runtime_env;
    use std::fs;

    #[test]
    fn runtime_env_overrides_default_when_unset() {
        with_runtime_env(None, None, || {
            assert_eq!(read_runtime_env_overrides(), RuntimeEnvOverrides::default());
        });
    }

    #[test]
    fn runtime_env_overrides_parse_force_ep_tokens() {
        with_runtime_env(Some(" cpu "), None, || {
            let overrides = read_runtime_env_overrides();
            assert_eq!(overrides.runtime_mode, RuntimeModePreference::Cpu);
        });

        with_runtime_env(Some("DIRECTML"), None, || {
            let overrides = read_runtime_env_overrides();
            assert_eq!(overrides.runtime_mode, RuntimeModePreference::Dml);
        });

        with_runtime_env(Some("dml"), None, || {
            let overrides = read_runtime_env_overrides();
            assert_eq!(overrides.runtime_mode, RuntimeModePreference::Dml);
        });

        with_runtime_env(Some("unknown"), None, || {
            let overrides = read_runtime_env_overrides();
            assert_eq!(overrides.runtime_mode, RuntimeModePreference::Auto);
        });
    }

    #[test]
    fn runtime_env_overrides_parse_dml_device_id() {
        with_runtime_env(None, Some("1"), || {
            let overrides = read_runtime_env_overrides();
            assert_eq!(overrides.dml_device_id, Some(1));
        });

        with_runtime_env(None, Some("abc"), || {
            let overrides = read_runtime_env_overrides();
            assert_eq!(overrides.dml_device_id, None);
        });
    }

    #[test]
    fn parse_error_message_extracts_nested_message() {
        let value = serde_json::json!({
            "error": {
                "message": "stream failed"
            }
        });

        assert_eq!(
            parse_error_message(&value),
            Some("stream failed".to_string())
        );
    }

    #[test]
    fn fallback_stream_metrics_reflects_emitted_chunks() {
        let metrics = fallback_stream_metrics(Instant::now(), 3, Some(10));
        assert_eq!(metrics.total_tokens, 3);
        assert_eq!(metrics.time_to_first_token_ms, Some(10));
    }

    #[test]
    fn parse_models_response_rejects_missing_data_array() {
        let payload = serde_json::json!({"object": "list"});
        let error = parse_models_response(&payload).expect_err("missing data should fail");
        assert!(error
            .to_string()
            .contains("expected top-level 'data' array"));
    }

    #[test]
    fn parse_models_response_rejects_unknown_only_models() {
        let payload = serde_json::json!({
            "object": "list",
            "data": [{"id": "unknown-model", "object": "model"}]
        });
        let error = parse_models_response(&payload).expect_err("unknown-only should fail");
        assert!(error
            .to_string()
            .contains("none matched local registry IDs"));
    }

    #[test]
    fn parse_models_response_accepts_known_model() {
        let payload = serde_json::json!({
            "object": "list",
            "data": [{"id": "qwen2.5-1.5b-instruct", "object": "model"}]
        });
        let models = parse_models_response(&payload).expect("known model should parse");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "qwen2.5-1.5b-instruct");
    }

    #[test]
    fn running_host_policy_restarts_when_protocol_is_incompatible_and_idle() {
        let decision = decide_running_host_policy(false, false, None, false);
        assert_eq!(decision, RunningHostPolicyDecision::Restart);
    }

    #[test]
    fn running_host_policy_rejects_forced_override_when_busy() {
        let decision = decide_running_host_policy(true, true, Some("cpu"), false);
        let RunningHostPolicyDecision::Reject(message) = decision else {
            panic!("busy forced override should reject");
        };
        assert!(message.contains("SMOLPC_FORCE_EP=cpu"));
        assert!(message.contains("busy"));
    }

    #[test]
    fn version_major_extracts_major_component() {
        assert_eq!(version_major("2.3.4"), Some(2));
        assert_eq!(version_major("10"), Some(10));
        assert_eq!(version_major(""), None);
        assert_eq!(version_major("beta"), None);
    }

    #[test]
    fn engine_api_major_compatible_requires_equal_or_higher_major() {
        assert!(engine_api_major_compatible("2.0.0", 2));
        assert!(engine_api_major_compatible("3.1.9", 2));
        assert!(!engine_api_major_compatible("1.9.9", 2));
        assert!(!engine_api_major_compatible("unknown", 2));
    }

    #[test]
    fn load_or_create_token_creates_private_file() {
        let unique = format!(
            "smolpc-engine-client-token-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("engine-token.txt");

        let created = load_or_create_token(&path).expect("create token");
        assert!(!created.is_empty());
        assert_eq!(created.len(), 48);
        assert!(
            created
                .chars()
                .all(|character| character.is_ascii_alphanumeric()),
            "token should remain alphanumeric"
        );
        let loaded_again = load_or_create_token(&path).expect("load token");
        assert_eq!(created, loaded_again);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path)
                .expect("token metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(
                mode & 0o077,
                0,
                "token file must not be group/other-readable"
            );
        }

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn startup_mode_serializes_as_contract_value() {
        let serialized =
            serde_json::to_string(&StartupMode::DirectmlRequired).expect("serialize startup mode");
        assert_eq!(serialized, "\"directml_required\"");
    }

    #[test]
    fn engine_status_parses_canonical_readiness_fields() {
        let payload = serde_json::json!({
            "ok": true,
            "ready": true,
            "attempt_id": "startup-1-1",
            "state": "ready",
            "startup_phase": "ready",
            "state_since": "2026-03-05T18:45:00Z",
            "active_backend": "cpu",
            "active_model_id": "qwen2.5-1.5b-instruct",
            "error_code": null,
            "error_message": null,
            "retryable": null,
            "last_error": null,
            "engine_api_version": "1.0.0",
            "current_model": "qwen2.5-1.5b-instruct",
            "generating": false,
            "backend_status": {}
        });

        let status: EngineStatus =
            serde_json::from_value(payload).expect("status payload should deserialize");
        assert!(status.ready);
        assert_eq!(status.attempt_id, "startup-1-1");
        assert_eq!(status.state.as_deref(), Some("ready"));
        assert_eq!(
            status.active_model_id.as_deref(),
            Some("qwen2.5-1.5b-instruct")
        );
        assert_eq!(status.engine_api_version, "1.0.0");
    }

    #[test]
    fn engine_status_keeps_legacy_payload_compatible() {
        let payload = serde_json::json!({
            "ok": true,
            "current_model": null,
            "generating": false,
            "backend_status": {}
        });
        let status: EngineStatus =
            serde_json::from_value(payload).expect("legacy payload should deserialize");
        assert!(!status.ready);
        assert_eq!(status.attempt_id, "unknown");
        assert_eq!(status.engine_api_version, ENGINE_API_VERSION);
        assert!(status.state.is_none());
    }

    #[test]
    fn engine_status_readiness_prefers_ready_flag_and_state() {
        let payload = serde_json::json!({
            "ok": true,
            "ready": true,
            "attempt_id": "attempt-1",
            "state": "ready",
            "current_model": null,
            "generating": false,
            "backend_status": {}
        });
        let status: EngineStatus =
            serde_json::from_value(payload).expect("status payload should deserialize");
        assert!(status.is_ready());
    }

    #[test]
    fn engine_status_failure_message_prefers_last_startup_error() {
        let payload = serde_json::json!({
            "ok": true,
            "ready": false,
            "attempt_id": "attempt-2",
            "state": "failed",
            "last_error": {
                "attempt_id": "attempt-2",
                "phase": "loading_model",
                "code": "MODEL_MISSING",
                "message": "Default model file missing",
                "retryable": false,
                "at": "2026-03-05T18:00:00Z"
            },
            "current_model": null,
            "generating": false,
            "backend_status": {}
        });
        let status: EngineStatus =
            serde_json::from_value(payload).expect("status payload should deserialize");
        assert!(status.is_failed());
        assert_eq!(
            status.failure_message().as_deref(),
            Some("MODEL_MISSING: Default model file missing")
        );
    }
}
