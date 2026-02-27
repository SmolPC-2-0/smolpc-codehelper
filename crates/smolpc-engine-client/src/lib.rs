
use futures_util::StreamExt;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use smolpc_engine_core::inference::backend::BackendStatus;
use smolpc_engine_core::models::registry::ModelDefinition;
use smolpc_engine_core::{GenerationConfig, GenerationMetrics, GenerationResult};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum EngineClientError {
    #[error("{0}")]
    Message(String),
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
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EngineMeta {
    pub ok: bool,
    pub protocol_version: String,
    pub engine_version: String,
    pub pid: u32,
    pub busy: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EngineStatus {
    pub ok: bool,
    pub current_model: Option<String>,
    pub generating: bool,
    pub backend_status: BackendStatus,
}

#[derive(Debug, Clone)]
pub struct EngineClient {
    base_url: String,
    token: String,
    http: Client,
}

impl EngineClient {
    fn new(base_url: String, token: String) -> Self {
        Self {
            base_url,
            token,
            http: Client::builder()
                .connect_timeout(Duration::from_secs(2))
                .timeout(Duration::from_secs(120))
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
            .send()
            .await?;
        Ok(response.error_for_status()?.json::<EngineMeta>().await?)
    }

    pub async fn status(&self) -> Result<EngineStatus, EngineClientError> {
        let response = self
            .http
            .get(self.url("/engine/status"))
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await?;
        Ok(response.error_for_status()?.json::<EngineStatus>().await?)
    }

    pub async fn load_model(&self, model_id: &str) -> Result<(), EngineClientError> {
        self.http
            .post(self.url("/engine/load"))
            .header(AUTHORIZATION, self.auth_header())
            .header(CONTENT_TYPE, "application/json")
            .body(serde_json::json!({"model_id": model_id}).to_string())
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn unload_model(&self, force: bool) -> Result<(), EngineClientError> {
        self.http
            .post(self.url("/engine/unload"))
            .header(AUTHORIZATION, self.auth_header())
            .header(CONTENT_TYPE, "application/json")
            .body(serde_json::json!({"force": force}).to_string())
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn cancel(&self) -> Result<(), EngineClientError> {
        self.http
            .post(self.url("/engine/cancel"))
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn check_model_exists(&self, model_id: &str) -> Result<bool, EngineClientError> {
        let value = self
            .http
            .post(self.url("/engine/check-model"))
            .header(AUTHORIZATION, self.auth_header())
            .header(CONTENT_TYPE, "application/json")
            .body(serde_json::json!({"model_id": model_id}).to_string())
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        Ok(value
            .get("exists")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    pub async fn list_models(&self) -> Result<Vec<ModelDefinition>, EngineClientError> {
        let value = self
            .http
            .get(self.url("/v1/models"))
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        let mut out = Vec::new();
        if let Some(data) = value.get("data").and_then(|d| d.as_array()) {
            for item in data {
                if let Some(id) = item.get("id").and_then(|s| s.as_str()) {
                    if let Some(model) =
                        smolpc_engine_core::models::registry::ModelRegistry::get_model(id)
                    {
                        out.push(model);
                    }
                }
            }
        }
        if out.is_empty() {
            out = smolpc_engine_core::models::registry::ModelRegistry::available_models();
        }
        Ok(out)
    }

    pub async fn generate_text(&self, prompt: &str, config: Option<GenerationConfig>) -> Result<GenerationResult, EngineClientError> {
        let body = completion_body(prompt, false, config);
        let value = self
            .http
            .post(self.url("/v1/chat/completions"))
            .header(AUTHORIZATION, self.auth_header())
            .header(CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        let text = value
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|first| first.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .to_string();

        Ok(GenerationResult {
            text,
            metrics: GenerationMetrics {
                total_tokens: 0,
                time_to_first_token_ms: None,
                tokens_per_second: 0.0,
                total_time_ms: 0,
            },
        })
    }

    pub async fn generate_stream<F>(
        &self,
        prompt: &str,
        config: Option<GenerationConfig>,
        mut on_token: F,
    ) -> Result<GenerationMetrics, EngineClientError>
    where
        F: FnMut(String),
    {
        let body = completion_body(prompt, true, config);
        let response = self
            .http
            .post(self.url("/v1/chat/completions"))
            .header(AUTHORIZATION, self.auth_header())
            .header(CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send()
            .await?
            .error_for_status()?;

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            let text = String::from_utf8_lossy(&bytes);
            buffer.push_str(&text);

            while let Some(newline) = buffer.find('\n') {
                let line = buffer[..newline].trim().to_string();
                buffer = buffer[newline + 1..].to_string();
                if !line.starts_with("data:") {
                    continue;
                }
                let data = line[5..].trim();
                if data == "[DONE]" {
                    return Ok(GenerationMetrics {
                        total_tokens: 0,
                        time_to_first_token_ms: None,
                        tokens_per_second: 0.0,
                        total_time_ms: 0,
                    });
                }
                if data.is_empty() {
                    continue;
                }
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(content) = value
                        .get("choices")
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|first| first.get("delta"))
                        .and_then(|d| d.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        on_token(content.to_string());
                    }
                }
            }
        }

        Ok(GenerationMetrics {
            total_tokens: 0,
            time_to_first_token_ms: None,
            tokens_per_second: 0.0,
            total_time_ms: 0,
        })
    }
}

pub async fn connect_or_spawn(options: EngineConnectOptions) -> Result<EngineClient, EngineClientError> {
    std::fs::create_dir_all(&options.shared_runtime_dir)?;
    std::fs::create_dir_all(&options.data_dir)?;

    let token_path = options.shared_runtime_dir.join("engine-token.txt");
    let token = load_or_create_token(&token_path)?;
    let base_url = format!("http://127.0.0.1:{}", options.port);
    let client = EngineClient::new(base_url, token.clone());

    if client.health().await.unwrap_or(false) {
        let meta = client.meta().await?;
        if protocol_major_matches(&meta.protocol_version, "1.0.0") {
            return Ok(client);
        }

        let status = client.status().await?;
        if !status.generating {
            let _ = client
                .http
                .post(client.url("/engine/shutdown"))
                .header(AUTHORIZATION, client.auth_header())
                .send()
                .await;
            tokio::time::sleep(Duration::from_millis(300)).await;
        } else {
            return Err(EngineClientError::Message(
                "Running engine protocol is incompatible and daemon is busy".to_string(),
            ));
        }
    }

    spawn_host(&options, &token)?;

    let started = std::time::Instant::now();
    loop {
        if client.health().await.unwrap_or(false) {
            break;
        }
        if started.elapsed() > Duration::from_secs(30) {
            return Err(EngineClientError::Message(
                "Engine failed to become healthy within 30s".to_string(),
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let meta = client.meta().await?;
    if !protocol_major_matches(&meta.protocol_version, "1.0.0") {
        return Err(EngineClientError::Message(format!(
            "Engine protocol mismatch: {}",
            meta.protocol_version
        )));
    }

    Ok(client)
}

fn completion_body(prompt: &str, stream: bool, config: Option<GenerationConfig>) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": "smolpc-engine",
        "stream": stream,
        "messages": [{"role": "user", "content": prompt}],
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

fn load_or_create_token(path: &Path) -> Result<String, EngineClientError> {
    if let Ok(token) = std::fs::read_to_string(path) {
        let trimmed = token.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }

    let token: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .map(char::from)
        .take(48)
        .collect();
    std::fs::write(path, &token)?;
    Ok(token)
}

fn spawn_host(options: &EngineConnectOptions, token: &str) -> Result<(), EngineClientError> {
    let host_bin = resolve_host_binary(options)?;
    let mut cmd = Command::new(host_bin);
    cmd.arg("--port")
        .arg(options.port.to_string())
        .arg("--data-dir")
        .arg(&options.data_dir)
        .arg("--app-version")
        .arg(&options.app_version)
        .env("SMOLPC_ENGINE_TOKEN", token)
        .env("SMOLPC_ENGINE_PORT", options.port.to_string());

    if let Some(resource_dir) = &options.resource_dir {
        cmd.arg("--resource-dir").arg(resource_dir);
    }
    if let Some(models_dir) = &options.models_dir {
        cmd.env("SMOLPC_MODELS_DIR", models_dir);
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    cmd.spawn()?;
    Ok(())
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

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            let sidecar = dir.join(format!(
                "smolpc-engine-host{}",
                std::env::consts::EXE_SUFFIX
            ));
            if sidecar.exists() {
                return Ok(sidecar);
            }
        }
    }

    let fallback = PathBuf::from("target")
        .join("debug")
        .join(format!("smolpc-engine-host{}", std::env::consts::EXE_SUFFIX));
    if fallback.exists() {
        return Ok(fallback);
    }

    Err(EngineClientError::Message(
        "Unable to locate smolpc-engine-host binary".to_string(),
    ))
}
