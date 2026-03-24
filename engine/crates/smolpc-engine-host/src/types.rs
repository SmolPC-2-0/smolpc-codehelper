use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use smolpc_engine_core::inference::backend::{BackendStatus, InferenceBackend};
use smolpc_engine_core::inference::types::InferenceChatMessage;
use smolpc_engine_core::GenerationMetrics;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

pub(crate) type ApiError = (StatusCode, Json<ErrorResponse>);

pub(crate) const ENGINE_PROTOCOL_VERSION: &str = "1.0.0";
pub(crate) const ENGINE_API_VERSION: &str = "1.0.0";
pub(crate) const ENGINE_DEFAULT_MODEL_ENV: &str = "SMOLPC_ENGINE_DEFAULT_MODEL_ID";
pub(crate) const LEGACY_DEFAULT_MODEL_ENV: &str = "SMOLPC_DEFAULT_MODEL_ID";
pub(crate) const STARTUP_DEFAULT_MODEL_INVALID: &str = "STARTUP_DEFAULT_MODEL_INVALID";
pub(crate) const STARTUP_MEMORY_PRESSURE: &str = "STARTUP_MEMORY_PRESSURE";
pub(crate) const STARTUP_MODEL_ASSET_MISSING: &str = "STARTUP_MODEL_ASSET_MISSING";
pub(crate) const STARTUP_DML_REQUIRED_UNAVAILABLE: &str = "STARTUP_DML_REQUIRED_UNAVAILABLE";
pub(crate) const STARTUP_MODEL_LOAD_FAILED: &str = "STARTUP_MODEL_LOAD_FAILED";

pub(crate) const STARTUP_PROBE_WAIT_MS: u64 = 1_500;
/// Extended probe budget for DirectML startup.
/// Worst-case total probe wait: STARTUP_PROBE_WAIT_MS + STARTUP_PROBE_RECOVERY_WAIT_MS.
pub(crate) const STARTUP_PROBE_RECOVERY_WAIT_MS: u64 = 8_000;
pub(crate) const STARTUP_PROBE_TOTAL_WAIT_MS: u64 =
    STARTUP_PROBE_WAIT_MS + STARTUP_PROBE_RECOVERY_WAIT_MS;
pub(crate) const OPENVINO_STARTUP_PROBE_WAIT: Duration = Duration::from_secs(30);
pub(crate) const OPENVINO_PREFLIGHT_BUDGET: Duration = Duration::from_secs(300);
pub(crate) const CPU_PREFLIGHT_BUDGET: Duration = Duration::from_secs(30);
pub(crate) const DIRECTML_PREFLIGHT_BUDGET: Duration = Duration::from_secs(60);
pub(crate) const OPENVINO_SELECTION_PROFILE: &str = "openvino_native_v1";
pub(crate) const OPENVINO_CHAT_MODE_STRUCTURED: &str = "structured_messages";
pub(crate) const OPENVINO_CHAT_MODE_LEGACY_PROMPT: &str = "legacy_prompt";
pub(crate) const OPENVINO_MAX_TOKENS_HARD_CAP_ENV: &str = "SMOLPC_OPENVINO_MAX_TOKENS_HARD_CAP";
pub(crate) const OPENVINO_MAX_TOKENS_HARD_CAP_DEFAULT: usize = 8192;

#[derive(Debug, serde::Serialize)]
pub(crate) struct ErrorResponse {
    pub(crate) error: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct LoadRequest {
    pub(crate) model_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct UnloadRequest {
    pub(crate) force: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CheckModelRequest {
    pub(crate) model_id: String,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StartupMode {
    #[default]
    Auto,
    DirectmlRequired,
}

impl StartupMode {
    pub(crate) fn requires_directml(self) -> bool {
        matches!(self, Self::DirectmlRequired)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub(crate) struct StartupPolicy {
    pub(crate) default_model_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct EnsureStartedRequest {
    #[serde(default)]
    pub(crate) mode: StartupMode,
    #[serde(default)]
    pub(crate) startup_policy: StartupPolicy,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReadinessState {
    #[default]
    Idle,
    Starting,
    ResolvingAssets,
    Probing,
    LoadingModel,
    Ready,
    Failed,
}

impl ReadinessState {
    pub(crate) fn ordinal(self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Starting => 1,
            Self::ResolvingAssets => 2,
            Self::Probing => 3,
            Self::LoadingModel => 4,
            Self::Ready => 5,
            Self::Failed => 6,
        }
    }

    pub(crate) fn is_starting(self) -> bool {
        matches!(
            self,
            Self::Starting | Self::ResolvingAssets | Self::Probing | Self::LoadingModel
        )
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct LastStartupError {
    pub(crate) attempt_id: String,
    pub(crate) phase: ReadinessState,
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) retryable: bool,
    pub(crate) at: String,
}

#[derive(Debug, Clone)]
pub(crate) struct StartupReadiness {
    pub(crate) attempt_id: String,
    pub(crate) state: ReadinessState,
    pub(crate) state_since: String,
    pub(crate) effective_mode: StartupMode,
    pub(crate) effective_startup_policy: StartupPolicy,
    pub(crate) error_code: Option<String>,
    pub(crate) error_message: Option<String>,
    pub(crate) retryable: Option<bool>,
    pub(crate) last_error: Option<LastStartupError>,
}

impl Default for StartupReadiness {
    fn default() -> Self {
        Self {
            attempt_id: "idle".to_string(),
            state: ReadinessState::Idle,
            state_since: Utc::now().to_rfc3339(),
            effective_mode: StartupMode::Auto,
            effective_startup_policy: StartupPolicy::default(),
            error_code: None,
            error_message: None,
            retryable: None,
            last_error: None,
        }
    }
}

impl StartupReadiness {
    pub(crate) fn transition(&mut self, next: ReadinessState) {
        if next.ordinal() >= self.state.ordinal() || matches!(self.state, ReadinessState::Ready) {
            self.state = next;
            self.state_since = Utc::now().to_rfc3339();
        }
    }

    pub(crate) fn begin_attempt(
        &mut self,
        attempt_id: String,
        mode: StartupMode,
        startup_policy: StartupPolicy,
    ) {
        self.attempt_id = attempt_id;
        self.state = ReadinessState::Starting;
        self.state_since = Utc::now().to_rfc3339();
        self.effective_mode = mode;
        self.effective_startup_policy = startup_policy;
        self.error_code = None;
        self.error_message = None;
        self.retryable = None;
    }

    pub(crate) fn mark_failed(
        &mut self,
        phase: ReadinessState,
        code: &str,
        message: String,
        retryable: bool,
    ) {
        self.state = ReadinessState::Failed;
        self.state_since = Utc::now().to_rfc3339();
        self.error_code = Some(code.to_string());
        self.error_message = Some(message.clone());
        self.retryable = Some(retryable);
        self.last_error = Some(LastStartupError {
            attempt_id: self.attempt_id.clone(),
            phase,
            code: code.to_string(),
            message,
            retryable,
            at: Utc::now().to_rfc3339(),
        });
    }

    pub(crate) fn mark_ready(&mut self) {
        self.state = ReadinessState::Ready;
        self.state_since = Utc::now().to_rfc3339();
        self.error_code = None;
        self.error_message = None;
        self.retryable = None;
        self.last_error = None;
    }

    pub(crate) fn mark_idle(&mut self) {
        self.state = ReadinessState::Idle;
        self.state_since = Utc::now().to_rfc3339();
        self.error_code = None;
        self.error_message = None;
        self.retryable = None;
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ReadinessPayload {
    pub(crate) ok: bool,
    pub(crate) ready: bool,
    pub(crate) attempt_id: String,
    pub(crate) state: ReadinessState,
    pub(crate) startup_phase: ReadinessState,
    pub(crate) state_since: String,
    pub(crate) active_backend: Option<InferenceBackend>,
    pub(crate) active_model_id: Option<String>,
    pub(crate) error_code: Option<String>,
    pub(crate) error_message: Option<String>,
    pub(crate) retryable: Option<bool>,
    pub(crate) last_error: Option<LastStartupError>,
    pub(crate) engine_version: &'static str,
    pub(crate) engine_api_version: &'static str,
    pub(crate) effective_mode: StartupMode,
    pub(crate) effective_startup_policy: StartupPolicy,
    pub(crate) current_model: Option<String>,
    pub(crate) generating: bool,
    pub(crate) backend_status: BackendStatus,
}

#[derive(Debug, Clone)]
pub(crate) struct StartupError {
    pub(crate) phase: ReadinessState,
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) retryable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnsureStartedOutcome {
    Ready,
    Failed,
    Conflict,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ChatCompletionMessage {
    pub(crate) role: String,
    pub(crate) content: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ChatCompletionRequest {
    pub(crate) model: Option<String>,
    pub(crate) messages: Vec<ChatCompletionMessage>,
    pub(crate) stream: Option<bool>,
    pub(crate) max_tokens: Option<usize>,
    pub(crate) temperature: Option<f32>,
    pub(crate) top_k: Option<usize>,
    pub(crate) top_p: Option<f32>,
    pub(crate) repetition_penalty: Option<f32>,
    pub(crate) repetition_penalty_last_n: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct ParsedArgs {
    pub(crate) port: u16,
    pub(crate) data_dir: PathBuf,
    pub(crate) resource_dir: Option<PathBuf>,
    pub(crate) app_version: String,
    pub(crate) queue_size: usize,
    pub(crate) queue_timeout: Duration,
    pub(crate) model_idle_unload: Option<Duration>,
    pub(crate) process_idle_exit: Option<Duration>,
}

pub(crate) enum StreamMessage {
    Token(String),
    Metrics(GenerationMetrics),
    Done,
    Error { message: String, code: &'static str },
}

pub(crate) enum CompletionInput {
    Prompt(String),
    Messages(Vec<InferenceChatMessage>),
}

pub(crate) struct CancelOnDrop {
    pub(crate) engine: Arc<EngineState>,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.engine.cancel();
    }
}

pub(crate) struct GenerationPermit {
    pub(crate) generating: Arc<AtomicBool>,
    pub(crate) active_cancel: Arc<StdMutex<Option<Arc<AtomicBool>>>>,
}

impl Drop for GenerationPermit {
    fn drop(&mut self) {
        self.generating.store(false, Ordering::SeqCst);
        *lock_cancel(&self.active_cancel) = None;
    }
}

pub(crate) struct TransitionGuard(pub(crate) Arc<AtomicBool>);
impl Drop for TransitionGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

pub(crate) fn lock_cancel<'a>(
    active_cancel: &'a StdMutex<Option<Arc<AtomicBool>>>,
) -> std::sync::MutexGuard<'a, Option<Arc<AtomicBool>>> {
    match active_cancel.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

// Forward reference -- EngineState is defined in state.rs but CancelOnDrop needs it.
// During incremental extraction, EngineState still lives in main.rs, so this import
// comes from the crate root via `use super::*` or `use crate::*`.
// We use a qualified path here to avoid a circular dependency during the extraction.
use crate::EngineState;

// ── Voice I/O types ──────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct AudioSpeechRequest {
    pub(crate) text: String,
    #[serde(default = "default_tts_voice")]
    pub(crate) voice: String,
    #[serde(default = "default_tts_speed")]
    pub(crate) speed: f32,
}

fn default_tts_voice() -> String {
    "Bella".to_string()
}

fn default_tts_speed() -> f32 {
    1.0
}
