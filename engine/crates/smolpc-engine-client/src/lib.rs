mod client;
mod spawn;
mod token;
mod version;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

pub use client::{EngineChatMessage, EngineClient};
pub use spawn::{
    check_running_policy, connect_or_spawn, decide_running_host_policy, kill_stale_processes,
    shutdown_and_wait, spawn_engine, wait_for_healthy, with_spawn_lock, RunningHostPolicyDecision,
};
pub use token::load_or_create_token;
pub use version::{engine_api_major_compatible, expected_engine_api_major, version_major};

use smolpc_engine_core::inference::backend::BackendStatus;
use std::path::PathBuf;
use std::time::Duration;

const ENGINE_PROTOCOL_VERSION: &str = "1.0.0";
const ENGINE_API_VERSION: &str = "1.0.0";
const ENGINE_HOST_BASENAME: &str = "smolpc-engine-host";
const SPAWN_LOCK_FILENAME: &str = "engine-spawn.lock";
const SPAWN_LOG_FILENAME: &str = "engine-spawn.log";
const SPAWN_LOCK_WAIT: Duration = Duration::from_secs(10);
const SPAWN_LOCK_STALE_AGE: Duration = Duration::from_secs(30);
pub(crate) const FORCE_EP_ENV: &str = "SMOLPC_FORCE_EP";
pub(crate) const DML_DEVICE_ENV: &str = "SMOLPC_DML_DEVICE_ID";
const SHARED_MODELS_VENDOR_DIR: &str = "SmolPC 2.0";
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
    pub(crate) fn as_force_override(self) -> Option<&'static str> {
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

fn default_engine_api_version() -> String {
    ENGINE_API_VERSION.to_string()
}

fn default_attempt_id() -> String {
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{fallback_stream_metrics, parse_error_message, parse_models_response};
    use crate::test_utils::with_runtime_env;
    use crate::token::load_or_create_token;
    use crate::{decide_running_host_policy, RunningHostPolicyDecision};
    use std::fs;
    use std::time::Instant;

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
