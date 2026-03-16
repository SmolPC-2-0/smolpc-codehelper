mod openvino;
mod runtime_bundles;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use smolpc_engine_core::inference::backend::{
    BackendDecision, BackendDecisionKey, BackendOpenVinoTuningStatus, BackendRuntimeBundleStatus,
    BackendSelectedDevice, BackendSelectionState, BackendStatus, CheckModelResponse,
    DecisionPersistenceState, DecisionReason, DirectMLFailureStage, FailureCounters,
    InferenceBackend, LanePreflightState, LaneStartupProbeState, ModelLaneReadiness,
    ModelLaneReadinessByBackend,
};
use smolpc_engine_core::inference::backend_store::{
    backend_store_path, BackendDecisionRecord, BackendStore,
};
#[cfg(target_os = "windows")]
use smolpc_engine_core::inference::genai::GenAiDirectMlGenerator;
use smolpc_engine_core::inference::types::InferenceChatMessage;
use smolpc_engine_core::inference::{
    InferenceRuntimeAdapter, OpenVinoPipelineConfig, OpenVinoRuntimeBundle, OrtRuntimeBundle,
    OrtRuntimeLoader, RuntimeVersionMetadata,
};
use smolpc_engine_core::models::{ModelArtifactBackend, ModelLoader, ModelRegistry};
use smolpc_engine_core::{GenerationConfig, GenerationMetrics, GenerationResult};
use std::cmp::Ordering as CmpOrdering;
use std::collections::hash_map::DefaultHasher;
use std::convert::Infallible;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex, Notify, Semaphore};
use tokio::time::{sleep, timeout};

use crate::openvino::{
    openvino_generation_controls_for_model,
    inspect_openvino_artifact, is_blocking_openvino_probe_failure, probe_openvino_startup,
    resolve_openvino_npu_tuning, run_openvino_preflight, OpenVinoPreflightResult,
    OpenVinoStartupProbeResult,
};
use crate::runtime_bundles::{resolve_runtime_bundles, ResolvedRuntimeBundles};
#[cfg(test)]
use crate::runtime_bundles::{resolve_runtime_bundles_for_mode, RuntimeLoadMode};

type ApiError = (StatusCode, Json<ErrorResponse>);

const ENGINE_PROTOCOL_VERSION: &str = "1.0.0";
const ENGINE_API_VERSION: &str = "1.0.0";
const ENGINE_DEFAULT_MODEL_ENV: &str = "SMOLPC_ENGINE_DEFAULT_MODEL_ID";
const LEGACY_DEFAULT_MODEL_ENV: &str = "SMOLPC_DEFAULT_MODEL_ID";

#[derive(Debug, serde::Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, serde::Deserialize)]
struct LoadRequest {
    model_id: String,
}

#[derive(Debug, serde::Deserialize)]
struct UnloadRequest {
    force: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
struct CheckModelRequest {
    model_id: String,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum StartupMode {
    #[default]
    Auto,
    DirectmlRequired,
}

impl StartupMode {
    fn requires_directml(self) -> bool {
        matches!(self, Self::DirectmlRequired)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
struct StartupPolicy {
    default_model_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct EnsureStartedRequest {
    #[serde(default)]
    mode: StartupMode,
    #[serde(default)]
    startup_policy: StartupPolicy,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum ReadinessState {
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
    fn ordinal(self) -> u8 {
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

    fn is_starting(self) -> bool {
        matches!(
            self,
            Self::Starting | Self::ResolvingAssets | Self::Probing | Self::LoadingModel
        )
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct LastStartupError {
    attempt_id: String,
    phase: ReadinessState,
    code: String,
    message: String,
    retryable: bool,
    at: String,
}

#[derive(Debug, Clone)]
struct StartupReadiness {
    attempt_id: String,
    state: ReadinessState,
    state_since: String,
    effective_mode: StartupMode,
    effective_startup_policy: StartupPolicy,
    error_code: Option<String>,
    error_message: Option<String>,
    retryable: Option<bool>,
    last_error: Option<LastStartupError>,
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
    fn transition(&mut self, next: ReadinessState) {
        if next.ordinal() >= self.state.ordinal() || matches!(self.state, ReadinessState::Ready) {
            self.state = next;
            self.state_since = Utc::now().to_rfc3339();
        }
    }

    fn begin_attempt(
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

    fn mark_failed(&mut self, phase: ReadinessState, code: &str, message: String, retryable: bool) {
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

    fn mark_ready(&mut self) {
        self.state = ReadinessState::Ready;
        self.state_since = Utc::now().to_rfc3339();
        self.error_code = None;
        self.error_message = None;
        self.retryable = None;
        self.last_error = None;
    }

    fn mark_idle(&mut self) {
        self.state = ReadinessState::Idle;
        self.state_since = Utc::now().to_rfc3339();
        self.error_code = None;
        self.error_message = None;
        self.retryable = None;
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct ReadinessPayload {
    ok: bool,
    ready: bool,
    attempt_id: String,
    state: ReadinessState,
    startup_phase: ReadinessState,
    state_since: String,
    active_backend: Option<InferenceBackend>,
    active_model_id: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    retryable: Option<bool>,
    last_error: Option<LastStartupError>,
    engine_version: &'static str,
    engine_api_version: &'static str,
    effective_mode: StartupMode,
    effective_startup_policy: StartupPolicy,
    current_model: Option<String>,
    generating: bool,
    backend_status: BackendStatus,
}

#[derive(Debug, Clone)]
struct StartupError {
    phase: ReadinessState,
    code: &'static str,
    message: String,
    retryable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnsureStartedOutcome {
    Ready,
    Failed,
    Conflict,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ChatCompletionMessage {
    role: String,
    content: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ChatCompletionRequest {
    model: Option<String>,
    messages: Vec<ChatCompletionMessage>,
    stream: Option<bool>,
    max_tokens: Option<usize>,
    temperature: Option<f32>,
    top_k: Option<usize>,
    top_p: Option<f32>,
    repetition_penalty: Option<f32>,
    repetition_penalty_last_n: Option<usize>,
}

#[derive(Debug)]
struct ParsedArgs {
    port: u16,
    data_dir: PathBuf,
    resource_dir: Option<PathBuf>,
    app_version: String,
    queue_size: usize,
    queue_timeout: Duration,
    model_idle_unload: Option<Duration>,
    process_idle_exit: Option<Duration>,
}

const STARTUP_PROBE_WAIT_MS: u64 = 1_500;
/// Extended probe budget for DirectML startup.
/// Worst-case total probe wait: STARTUP_PROBE_WAIT_MS + STARTUP_PROBE_RECOVERY_WAIT_MS.
const STARTUP_PROBE_RECOVERY_WAIT_MS: u64 = 8_000;
const OPENVINO_STARTUP_PROBE_WAIT: Duration = Duration::from_secs(30);
const OPENVINO_PREFLIGHT_BUDGET: Duration = Duration::from_secs(300);
const OPENVINO_SELECTION_PROFILE: &str = "openvino_native_v1";
const OPENVINO_CHAT_MODE_STRUCTURED: &str = "structured_messages";
const OPENVINO_CHAT_MODE_LEGACY_PROMPT: &str = "legacy_prompt";
const OPENVINO_MAX_TOKENS_HARD_CAP_ENV: &str = "SMOLPC_OPENVINO_MAX_TOKENS_HARD_CAP";
const OPENVINO_MAX_TOKENS_HARD_CAP_DEFAULT: usize = 8192;

#[derive(Debug, Clone)]
struct DirectMlCandidate {
    device_id: i32,
    device_name: String,
    adapter_identity: String,
    driver_version: String,
}

#[derive(Debug, Clone)]
struct BackendProbeResult {
    available_backends: Vec<InferenceBackend>,
    directml_device_count: usize,
    directml_candidate: Option<DirectMlCandidate>,
    npu_hardware_detected: bool,
}

impl Default for BackendProbeResult {
    fn default() -> Self {
        Self {
            available_backends: vec![InferenceBackend::Cpu],
            directml_device_count: 0,
            directml_candidate: None,
            npu_hardware_detected: false,
        }
    }
}

fn epoch_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as u64,
        Err(_) => 0,
    }
}

fn default_data_dir() -> PathBuf {
    if let Some(path) = dirs::data_local_dir() {
        return path.join("SmolPC").join("engine");
    }
    PathBuf::from(".smolpc-engine")
}

fn normalize_non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|candidate| {
        let trimmed = candidate.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn env_default_model_id() -> Option<String> {
    normalize_non_empty(std::env::var(ENGINE_DEFAULT_MODEL_ENV).ok())
        .or_else(|| normalize_non_empty(std::env::var(LEGACY_DEFAULT_MODEL_ENV).ok()))
}

fn built_in_default_model_id() -> Option<String> {
    ModelRegistry::available_models()
        .into_iter()
        .next()
        .map(|m| m.id)
}

fn resolve_default_model_id_with_sources(
    request_model_id: Option<String>,
    config_model_id: Option<String>,
    built_in_model_id: Option<String>,
) -> Result<String, StartupError> {
    if let Some(request_model) = normalize_non_empty(request_model_id) {
        return Ok(request_model);
    }
    if let Some(config_model) = normalize_non_empty(config_model_id) {
        return Ok(config_model);
    }
    if let Some(built_in_model) = normalize_non_empty(built_in_model_id) {
        return Ok(built_in_model);
    }
    Err(StartupError {
        phase: ReadinessState::ResolvingAssets,
        code: "STARTUP_DEFAULT_MODEL_INVALID",
        message: "No default model is configured or registered".to_string(),
        retryable: false,
    })
}

fn resolve_default_model_id(startup_policy: &StartupPolicy) -> Result<String, StartupError> {
    resolve_default_model_id_with_sources(
        startup_policy.default_model_id.clone(),
        env_default_model_id(),
        built_in_default_model_id(),
    )
}

fn classify_startup_model_error(error: &str) -> StartupError {
    let lowered = error.to_ascii_lowercase();
    if lowered.contains("unknown model id") {
        return StartupError {
            phase: ReadinessState::LoadingModel,
            code: "STARTUP_DEFAULT_MODEL_INVALID",
            message: error.to_string(),
            retryable: false,
        };
    }
    if lowered.contains("not found")
        || lowered.contains("missing")
        || lowered.contains("artifact is incomplete")
    {
        return StartupError {
            phase: ReadinessState::LoadingModel,
            code: "STARTUP_MODEL_ASSET_MISSING",
            message: error.to_string(),
            retryable: false,
        };
    }
    if lowered.contains("requires directml backend") || lowered.contains("directml") {
        return StartupError {
            phase: ReadinessState::LoadingModel,
            code: "STARTUP_DML_REQUIRED_UNAVAILABLE",
            message: error.to_string(),
            retryable: false,
        };
    }
    StartupError {
        phase: ReadinessState::LoadingModel,
        code: "STARTUP_MODEL_LOAD_FAILED",
        message: error.to_string(),
        retryable: true,
    }
}

fn parse_args() -> ParsedArgs {
    let mut port = 19432u16;
    let mut data_dir = default_data_dir();
    let mut resource_dir = None;
    let mut app_version = "dev".to_string();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => {
                if let Some(v) = args.next() {
                    if let Ok(p) = v.parse::<u16>() {
                        port = p;
                    }
                }
            }
            "--data-dir" => {
                if let Some(v) = args.next() {
                    data_dir = PathBuf::from(v);
                }
            }
            "--resource-dir" => {
                if let Some(v) = args.next() {
                    resource_dir = Some(PathBuf::from(v));
                }
            }
            "--app-version" => {
                if let Some(v) = args.next() {
                    app_version = v;
                }
            }
            _ => {}
        }
    }

    let queue_size = std::env::var("SMOLPC_ENGINE_QUEUE_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(3)
        .max(1);
    let queue_timeout = Duration::from_secs(
        std::env::var("SMOLPC_ENGINE_QUEUE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60)
            .max(1),
    );
    let model_idle_unload =
        parse_idle_timeout_secs("SMOLPC_ENGINE_MODEL_IDLE_UNLOAD_SECS", Some(0), 30);
    let process_idle_exit =
        parse_idle_timeout_secs("SMOLPC_ENGINE_PROCESS_IDLE_EXIT_SECS", None, 60);

    ParsedArgs {
        port,
        data_dir,
        resource_dir,
        app_version,
        queue_size,
        queue_timeout,
        model_idle_unload,
        process_idle_exit,
    }
}

fn parse_idle_timeout_secs(
    key: &str,
    default_secs: Option<u64>,
    min_secs: u64,
) -> Option<Duration> {
    match std::env::var(key) {
        Ok(raw) => match raw.parse::<u64>() {
            Ok(0) => None,
            Ok(secs) => Some(Duration::from_secs(secs.max(min_secs))),
            Err(_) => default_secs.map(|secs| Duration::from_secs(secs.max(min_secs))),
        },
        Err(_) => default_secs.map(|secs| Duration::from_secs(secs.max(min_secs))),
    }
}

fn parse_force_override() -> Option<InferenceBackend> {
    let value = std::env::var("SMOLPC_FORCE_EP").ok()?;
    match value.trim().to_ascii_lowercase().as_str() {
        "cpu" => Some(InferenceBackend::Cpu),
        "dml" | "directml" => Some(InferenceBackend::DirectML),
        "openvino" | "openvino_npu" => Some(InferenceBackend::OpenVinoNpu),
        _ => None,
    }
}

fn parse_dml_device_id_env() -> Option<i32> {
    std::env::var("SMOLPC_DML_DEVICE_ID")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
}

fn current_openvino_tuning_status() -> Option<BackendOpenVinoTuningStatus> {
    resolve_openvino_npu_tuning()
        .ok()
        .map(|tuning| BackendOpenVinoTuningStatus {
            max_prompt_len: Some(tuning.max_prompt_len),
            min_response_len: Some(tuning.min_response_len),
        })
}

fn model_requires_directml(model_id: &str) -> bool {
    let _ = model_id;
    false
}

fn model_requires_openvino(model_id: &str) -> bool {
    let _ = model_id;
    false
}

fn directml_required_error(model_id: &str, reason: &str) -> String {
    format!("Model '{model_id}' currently requires DirectML backend in shared engine: {reason}")
}

fn openvino_required_error(model_id: &str, reason: &str) -> String {
    format!("Model '{model_id}' currently requires OpenVINO NPU backend in shared engine: {reason}")
}

fn directml_unavailable_reason(
    directml_detected: bool,
    directml_artifact_available: bool,
    runtime_bundles: &ResolvedRuntimeBundles,
) -> String {
    if !directml_detected {
        "no DirectML-capable adapter was detected".to_string()
    } else if !directml_artifact_available {
        "the DirectML model artifact is missing".to_string()
    } else if let Some(code) = runtime_bundles.ort.directml_failure_code() {
        format!("the DirectML runtime bundle is unavailable ({code})")
    } else {
        "the DirectML runtime bundle is unavailable".to_string()
    }
}

fn decision_reason_code(reason: &DecisionReason) -> &'static str {
    match reason {
        DecisionReason::DefaultCpu => "default_cpu",
        DecisionReason::DefaultOpenVinoCandidate => "default_openvino_candidate",
        DecisionReason::DefaultDirectMLCandidate => "default_directml_candidate",
        DecisionReason::ForcedOverride => "forced_override",
        DecisionReason::PersistedDecision => "persisted_decision",
        DecisionReason::BenchmarkPassed => "benchmark_passed",
        DecisionReason::BenchmarkDecodeTooSlow => "benchmark_directml_decode_too_slow",
        DecisionReason::BenchmarkTtftTooHigh => "benchmark_ttft_too_high",
        DecisionReason::BenchmarkBudgetExceeded => "benchmark_budget_exceeded",
        DecisionReason::NoDirectMLCandidate => "no_directml_candidate",
        DecisionReason::DirectMLInitializationFailed => "directml_initialization_failed",
        DecisionReason::DirectMLPreflightFailed => "directml_preflight_failed",
        DecisionReason::NoOpenVinoCandidate => "no_openvino_candidate",
        DecisionReason::OpenVinoStartupProbePending => "openvino_startup_probe_pending",
        DecisionReason::OpenVinoPreflightFailed => "openvino_preflight_failed",
        DecisionReason::OpenVinoPreflightTimeout => "openvino_preflight_timeout",
        DecisionReason::OpenVinoRuntimeUnavailable => "runtime_unavailable",
        DecisionReason::RuntimeFailureFallback => "runtime_failure_fallback",
        DecisionReason::DemotedAfterFailures => "demoted_after_failures",
    }
}

#[cfg(target_os = "windows")]
fn gpu_rank_key(gpu: &hardware_query::GPUInfo) -> (bool, u64, String) {
    let device_type = gpu.gpu_type().to_string().to_ascii_lowercase();
    let is_discrete = device_type.contains("discrete");
    let vram_mb = gpu.memory_mb();
    let name = gpu.model_name().to_ascii_lowercase();
    (is_discrete, vram_mb, name)
}

#[cfg(target_os = "windows")]
fn pick_best_dml_candidate(gpus: &[hardware_query::GPUInfo]) -> Option<DirectMlCandidate> {
    let mut candidates = gpus
        .iter()
        .enumerate()
        .filter(|(_, gpu)| gpu.supports_directml())
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| {
        let ka = gpu_rank_key(a.1);
        let kb = gpu_rank_key(b.1);
        match kb.0.cmp(&ka.0) {
            CmpOrdering::Equal => match kb.1.cmp(&ka.1) {
                CmpOrdering::Equal => ka.2.cmp(&kb.2),
                other => other,
            },
            other => other,
        }
    });

    let (device_index, gpu) = candidates.first()?;
    let vendor = gpu.vendor().to_string().to_ascii_lowercase();
    let model = gpu.model_name().trim().to_ascii_lowercase();
    let pci = gpu
        .pci_device_id
        .as_deref()
        .unwrap_or("unknown")
        .trim()
        .to_ascii_lowercase();
    let driver = gpu
        .driver_version
        .as_deref()
        .unwrap_or("unknown")
        .trim()
        .to_string();
    Some(DirectMlCandidate {
        device_id: *device_index as i32,
        device_name: gpu.model_name().to_string(),
        adapter_identity: format!("{vendor}:{model}:{pci}"),
        driver_version: driver,
    })
}

#[cfg(target_os = "windows")]
fn probe_backend_capabilities() -> BackendProbeResult {
    let queried = hardware_query::HardwareInfo::query();
    let Ok(info) = queried else {
        return BackendProbeResult::default();
    };
    let directml_device_count = info
        .gpus()
        .iter()
        .filter(|gpu| gpu.supports_directml())
        .count();
    let mut result = BackendProbeResult {
        npu_hardware_detected: !info.npus().is_empty(),
        directml_device_count,
        ..Default::default()
    };
    if let Some(candidate) = pick_best_dml_candidate(info.gpus()) {
        result.available_backends.push(InferenceBackend::DirectML);
        result.directml_candidate = Some(candidate);
    }
    result
}

#[cfg(not(target_os = "windows"))]
fn probe_backend_capabilities() -> BackendProbeResult {
    BackendProbeResult::default()
}

#[derive(Debug, Clone, Default)]
struct ModelLaneArtifacts {
    cpu_ready: bool,
    directml_ready: bool,
    openvino_artifact: Option<crate::openvino::OpenVinoReadyArtifact>,
    openvino_reason: Option<String>,
    openvino_message: Option<String>,
    fingerprint: Option<String>,
}

impl ModelLaneArtifacts {
    fn openvino_npu_ready(&self) -> bool {
        self.openvino_artifact.is_some()
    }
}

fn compute_artifact_fingerprint(paths: &[PathBuf]) -> Option<String> {
    let existing = paths
        .iter()
        .filter(|path| path.exists())
        .cloned()
        .collect::<Vec<_>>();
    if existing.is_empty() {
        return None;
    }

    let mut sorted = existing;
    sorted.sort_by(|a, b| {
        a.display()
            .to_string()
            .to_ascii_lowercase()
            .cmp(&b.display().to_string().to_ascii_lowercase())
    });

    let mut hasher = DefaultHasher::new();
    for path in &sorted {
        path.display()
            .to_string()
            .to_ascii_lowercase()
            .hash(&mut hasher);
        if let Ok(metadata) = std::fs::metadata(path) {
            metadata.len().hash(&mut hasher);
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                    duration.as_secs().hash(&mut hasher);
                    duration.subsec_nanos().hash(&mut hasher);
                }
            }
        }
    }

    Some(format!("{:016x}", hasher.finish()))
}

fn sanitize_cache_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

fn resolve_model_lane_artifacts(model_dir: &str) -> ModelLaneArtifacts {
    let (directml_model_exists, directml_tokenizer_exists) =
        ModelLoader::check_model_files_for_backend(model_dir, ModelArtifactBackend::DirectML);
    let openvino_manifest = ModelLoader::openvino_manifest_file(model_dir);
    let openvino_artifact = inspect_openvino_artifact(&openvino_manifest);

    let directml_root =
        ModelLoader::model_path(model_dir).join(ModelArtifactBackend::DirectML.as_dir());
    let mut artifact_paths = vec![
        directml_root.join("model.onnx"),
        directml_root.join("genai_config.json"),
        directml_root.join("tokenizer.json"),
    ];
    artifact_paths.extend(openvino_artifact.fingerprint_paths());

    ModelLaneArtifacts {
        cpu_ready: openvino_artifact.ready_artifact().is_some(),
        directml_ready: directml_model_exists && directml_tokenizer_exists,
        openvino_artifact: openvino_artifact.ready_artifact().cloned(),
        openvino_reason: Some(openvino_artifact.reason_code().to_string()),
        openvino_message: openvino_artifact.message(),
        fingerprint: compute_artifact_fingerprint(&artifact_paths),
    }
}

fn runtime_version_value(
    version_metadata: &[RuntimeVersionMetadata],
    component: &str,
) -> Option<String> {
    version_metadata
        .iter()
        .find(|entry| entry.component == component)
        .map(|entry| entry.version.clone())
}

fn runtime_version_summary(version_metadata: &[RuntimeVersionMetadata]) -> Option<String> {
    (!version_metadata.is_empty()).then(|| {
        version_metadata
            .iter()
            .map(|entry| format!("{}={}", entry.component, entry.version))
            .collect::<Vec<_>>()
            .join("; ")
    })
}

fn apply_model_lane_artifacts(status: &mut BackendStatus, artifacts: &ModelLaneArtifacts) {
    status.lanes.cpu.artifact_ready = artifacts.cpu_ready;
    status.lanes.directml.artifact_ready = artifacts.directml_ready;
    status.lanes.openvino_npu.artifact_ready = artifacts.openvino_npu_ready();
}

fn apply_persisted_eligibility(
    status: &mut BackendStatus,
    persisted_decision: Option<&BackendDecision>,
) {
    status.lanes.cpu.persisted_eligibility = false;
    status.lanes.directml.persisted_eligibility = false;
    status.lanes.openvino_npu.persisted_eligibility = false;

    if let Some(decision) = persisted_decision {
        match decision.backend {
            InferenceBackend::Cpu => status.lanes.cpu.persisted_eligibility = true,
            InferenceBackend::DirectML => status.lanes.directml.persisted_eligibility = true,
            InferenceBackend::OpenVinoNpu => status.lanes.openvino_npu.persisted_eligibility = true,
        }
    }
}

fn apply_directml_device(
    status: &mut BackendStatus,
    device_id: Option<i32>,
    device_name: Option<String>,
) {
    status.lanes.directml.device_id = device_id;
    status.lanes.directml.device_name = device_name.clone();
    status.selected_device = if device_id.is_some() || device_name.is_some() {
        Some(BackendSelectedDevice {
            backend: InferenceBackend::DirectML,
            device_id,
            device_name,
        })
    } else {
        None
    };
}

fn rebuild_available_backends(status: &mut BackendStatus) {
    status.available_backends = vec![InferenceBackend::Cpu];
    if status.lanes.directml.detected {
        status.available_backends.push(InferenceBackend::DirectML);
    }
    if status.lanes.openvino_npu.startup_probe_state == LaneStartupProbeState::Ready {
        status
            .available_backends
            .push(InferenceBackend::OpenVinoNpu);
    }
}

fn apply_directml_startup_probe_status(status: &mut BackendStatus, probe: &BackendProbeResult) {
    status.lanes.directml.detected = probe.directml_candidate.is_some();
    if let Some(candidate) = probe.directml_candidate.as_ref() {
        status.lanes.directml.startup_probe_state = LaneStartupProbeState::Ready;
        status.lanes.directml.driver_version = Some(candidate.driver_version.clone());
        status.lanes.directml.last_failure_class = None;
        status.lanes.directml.last_failure_message = None;
        apply_directml_device(
            status,
            Some(candidate.device_id),
            Some(candidate.device_name.clone()),
        );
    } else {
        status.lanes.directml.startup_probe_state = LaneStartupProbeState::Error;
        status.lanes.directml.last_failure_class = Some("directml_candidate_missing".to_string());
        status.lanes.directml.last_failure_message =
            Some("No DirectML-capable adapter detected".to_string());
        apply_directml_device(status, None, None);
    }
    rebuild_available_backends(status);
}

fn apply_openvino_startup_probe_status(
    status: &mut BackendStatus,
    probe: Option<&OpenVinoStartupProbeResult>,
) {
    let Some(probe) = probe else {
        rebuild_available_backends(status);
        return;
    };

    status.lanes.openvino_npu.detected = probe.hardware_detected;
    status.lanes.openvino_npu.device_name = probe.device_name.clone();
    status.lanes.openvino_npu.driver_version = probe.driver_version.clone();
    status.lanes.openvino_npu.device_id = None;
    status.lanes.openvino_npu.startup_probe_state = if probe.startup_ready {
        LaneStartupProbeState::Ready
    } else {
        LaneStartupProbeState::Error
    };
    status.lanes.openvino_npu.last_failure_class = probe.failure_class.clone();
    status.lanes.openvino_npu.last_failure_message = probe.failure_message.clone();
    rebuild_available_backends(status);
}

fn apply_runtime_bundle_status(
    runtime_bundles: &ResolvedRuntimeBundles,
    status: &mut BackendStatus,
) {
    status.runtime_bundles.load_mode = Some(runtime_bundles.mode.as_str().to_string());
    status.runtime_bundles.ort = BackendRuntimeBundleStatus {
        root: Some(runtime_bundles.ort.display_root().display().to_string()),
        fingerprint: Some(runtime_bundles.ort.fingerprint.value.clone()),
        validated: runtime_bundles.ort.ort_validated(),
        failure: runtime_bundles
            .ort
            .ort_failure_code()
            .map(ToString::to_string),
    };
    status.runtime_bundles.directml = BackendRuntimeBundleStatus {
        root: Some(runtime_bundles.ort.display_root().display().to_string()),
        fingerprint: Some(runtime_bundles.ort.fingerprint.value.clone()),
        validated: runtime_bundles.ort.directml_validated(),
        failure: runtime_bundles
            .ort
            .directml_failure_code()
            .map(ToString::to_string),
    };
    status.runtime_bundles.openvino = BackendRuntimeBundleStatus {
        root: Some(
            runtime_bundles
                .openvino
                .display_root()
                .display()
                .to_string(),
        ),
        fingerprint: Some(runtime_bundles.openvino.fingerprint.value.clone()),
        validated: runtime_bundles.openvino.cpu_validated(),
        failure: runtime_bundles
            .openvino
            .failure_code()
            .map(ToString::to_string),
    };

    status.lanes.cpu.detected = true;
    status.lanes.cpu.bundle_ready = runtime_bundles.openvino.cpu_validated();
    status.lanes.cpu.runtime_version =
        runtime_version_summary(&runtime_bundles.openvino.version_metadata);
    status.lanes.cpu.startup_probe_state = LaneStartupProbeState::Ready;
    if !status.lanes.cpu.bundle_ready && status.lanes.cpu.last_failure_class.is_none() {
        status.lanes.cpu.last_failure_class = runtime_bundles
            .openvino
            .cpu_failure_code()
            .map(ToString::to_string);
    }

    status.lanes.directml.bundle_ready = runtime_bundles.ort.directml_validated();
    status.lanes.directml.runtime_version =
        runtime_version_summary(&runtime_bundles.ort.version_metadata);
    if !status.lanes.directml.bundle_ready && status.lanes.directml.last_failure_class.is_none() {
        status.lanes.directml.last_failure_class = runtime_bundles
            .ort
            .directml_failure_code()
            .map(ToString::to_string);
    }

    status.lanes.openvino_npu.bundle_ready = runtime_bundles.openvino.npu_validated();
    status.lanes.openvino_npu.runtime_version =
        runtime_version_summary(&runtime_bundles.openvino.version_metadata);
    if !status.lanes.openvino_npu.bundle_ready
        && status.lanes.openvino_npu.last_failure_class.is_none()
    {
        status.lanes.openvino_npu.last_failure_class = runtime_bundles
            .openvino
            .npu_failure_code()
            .map(ToString::to_string);
    }
}

fn bundle_reason(code: Option<&str>) -> String {
    code.unwrap_or("bundle_missing").to_string()
}

fn build_check_model_response(
    model_id: &str,
    runtime_bundles: &ResolvedRuntimeBundles,
    startup_probe: Option<&BackendProbeResult>,
    openvino_probe: Option<&OpenVinoStartupProbeResult>,
) -> CheckModelResponse {
    let Some(model_def) = ModelRegistry::get_model(model_id) else {
        let unknown = ModelLaneReadiness {
            artifact_ready: false,
            bundle_ready: false,
            ready: false,
            reason: "unknown_model".to_string(),
        };
        return CheckModelResponse {
            model_id: model_id.to_string(),
            lanes: ModelLaneReadinessByBackend {
                openvino_npu: unknown.clone(),
                directml: unknown.clone(),
                cpu: unknown,
            },
        };
    };

    let artifacts = resolve_model_lane_artifacts(&model_def.directory);
    let cpu_bundle_ready = runtime_bundles.openvino.cpu_validated();
    let directml_bundle_ready = runtime_bundles.ort.directml_validated();
    let openvino_bundle_ready = runtime_bundles.openvino.npu_validated();
    let directml_detected = startup_probe
        .and_then(|probe| probe.directml_candidate.as_ref())
        .is_some();
    let openvino_probe_ready = openvino_probe.is_some_and(|probe| probe.startup_ready);
    let openvino_probe_failure = openvino_probe.and_then(|probe| {
        probe
            .failure_class
            .as_deref()
            .filter(|class| is_blocking_openvino_probe_failure(class))
            .map(ToString::to_string)
    });

    let cpu = ModelLaneReadiness {
        artifact_ready: artifacts.cpu_ready,
        bundle_ready: cpu_bundle_ready,
        ready: artifacts.cpu_ready && cpu_bundle_ready,
        reason: if artifacts.cpu_ready && cpu_bundle_ready {
            "ready".to_string()
        } else if !artifacts.cpu_ready {
            "artifact_missing".to_string()
        } else {
            bundle_reason(runtime_bundles.openvino.cpu_failure_code())
        },
    };
    let directml = ModelLaneReadiness {
        artifact_ready: artifacts.directml_ready,
        bundle_ready: directml_bundle_ready,
        ready: artifacts.directml_ready && directml_bundle_ready && directml_detected,
        reason: if !artifacts.directml_ready {
            "artifact_missing".to_string()
        } else if !directml_bundle_ready {
            bundle_reason(runtime_bundles.ort.directml_failure_code())
        } else if startup_probe.is_none() {
            "startup_probe_pending".to_string()
        } else if !directml_detected {
            "directml_candidate_missing".to_string()
        } else {
            "ready".to_string()
        },
    };
    let openvino_npu = ModelLaneReadiness {
        artifact_ready: artifacts.openvino_npu_ready(),
        bundle_ready: openvino_bundle_ready,
        ready: artifacts.openvino_npu_ready()
            && openvino_bundle_ready
            && openvino_probe_ready
            && openvino_probe_failure.is_none(),
        reason: if !artifacts.openvino_npu_ready() {
            artifacts
                .openvino_reason
                .clone()
                .unwrap_or_else(|| "artifact_missing".to_string())
        } else if !openvino_bundle_ready {
            bundle_reason(runtime_bundles.openvino.npu_failure_code())
        } else if openvino_probe.is_none() {
            "startup_probe_pending".to_string()
        } else if let Some(failure) = openvino_probe_failure {
            failure
        } else if !openvino_probe_ready {
            "startup_probe_failed".to_string()
        } else {
            "ready".to_string()
        },
    };

    CheckModelResponse {
        model_id: model_id.to_string(),
        lanes: ModelLaneReadinessByBackend {
            openvino_npu,
            directml,
            cpu,
        },
    }
}

struct EngineState {
    runtime_adapter: Arc<Mutex<Option<InferenceRuntimeAdapter>>>,
    current_model: Arc<Mutex<Option<String>>>,
    backend_status: Arc<Mutex<BackendStatus>>,
    runtime_bundles: ResolvedRuntimeBundles,
    data_dir: PathBuf,
    active_cancel: Arc<StdMutex<Option<Arc<AtomicBool>>>>,
    generating: Arc<AtomicBool>,
    app_version: String,
    store_path: Option<PathBuf>,
    backend_store: Arc<Mutex<Option<BackendStore>>>,
    startup_probe: Arc<Mutex<Option<BackendProbeResult>>>,
    startup_probe_ready: Arc<Notify>,
    readiness: Arc<Mutex<StartupReadiness>>,
    startup_terminal: Arc<Notify>,
    startup_attempt_seq: AtomicU64,
    openvino_startup_probe: Arc<Mutex<Option<OpenVinoStartupProbeResult>>>,
    openvino_startup_probe_ready: Arc<Notify>,
}

impl EngineState {
    fn next_attempt_id(&self) -> String {
        let attempt = self.startup_attempt_seq.fetch_add(1, Ordering::SeqCst) + 1;
        format!("startup-{}-{attempt}", epoch_ms())
    }

    fn new(args: &ParsedArgs) -> Self {
        let runtime_bundles = resolve_runtime_bundles(args.resource_dir.as_deref());
        Self::new_with_runtime_bundles(args, runtime_bundles)
    }

    fn new_with_runtime_bundles(
        args: &ParsedArgs,
        runtime_bundles: ResolvedRuntimeBundles,
    ) -> Self {
        let store_path = match backend_store_path(&args.data_dir) {
            Ok(path) => Some(path),
            Err(error) => {
                log::warn!("Failed to resolve backend decision store path: {error}");
                None
            }
        };

        let backend_store = match store_path.as_ref() {
            Some(path) => match BackendStore::load(path) {
                Ok(store) => Some(store),
                Err(error) => {
                    log::warn!(
                        "Failed to load backend decision store {}: {error}",
                        path.display()
                    );
                    None
                }
            },
            None => None,
        };

        let mut status = BackendStatus {
            available_backends: vec![InferenceBackend::Cpu],
            selection_state: Some(BackendSelectionState::Pending),
            selection_reason: Some("startup_probe_pending".to_string()),
            openvino_message_mode: Some(OPENVINO_CHAT_MODE_STRUCTURED.to_string()),
            openvino_tuning: current_openvino_tuning_status(),
            store_path: store_path.as_ref().map(|path| path.display().to_string()),
            ..Default::default()
        };
        apply_runtime_bundle_status(&runtime_bundles, &mut status);

        Self {
            runtime_adapter: Arc::new(Mutex::new(None)),
            current_model: Arc::new(Mutex::new(None)),
            backend_status: Arc::new(Mutex::new(status)),
            runtime_bundles,
            data_dir: args.data_dir.clone(),
            active_cancel: Arc::new(StdMutex::new(None)),
            generating: Arc::new(AtomicBool::new(false)),
            app_version: args.app_version.clone(),
            store_path,
            backend_store: Arc::new(Mutex::new(backend_store)),
            startup_probe: Arc::new(Mutex::new(None)),
            startup_probe_ready: Arc::new(Notify::new()),
            readiness: Arc::new(Mutex::new(StartupReadiness::default())),
            startup_terminal: Arc::new(Notify::new()),
            startup_attempt_seq: AtomicU64::new(0),
            openvino_startup_probe: Arc::new(Mutex::new(None)),
            openvino_startup_probe_ready: Arc::new(Notify::new()),
        }
    }

    fn runtime_bundles(&self) -> &ResolvedRuntimeBundles {
        &self.runtime_bundles
    }

    async fn active_backend(&self) -> Option<InferenceBackend> {
        self.backend_status.lock().await.active_backend
    }

    fn openvino_cache_dir(&self, model_id: &str, artifacts: &ModelLaneArtifacts) -> PathBuf {
        let model_key = sanitize_cache_component(model_id);
        let artifact_key = artifacts
            .fingerprint
            .as_deref()
            .map(sanitize_cache_component)
            .unwrap_or_else(|| "artifact-unknown".to_string());

        self.data_dir
            .join("inference")
            .join("openvino-cache")
            .join(model_key)
            .join(artifact_key)
    }

    async fn run_openvino_preflight_with_timeout(
        &self,
        model_id: &str,
        artifacts: &ModelLaneArtifacts,
        probe: &OpenVinoStartupProbeResult,
    ) -> OpenVinoPreflightResult {
        let Some(artifact) = artifacts.openvino_artifact.clone() else {
            return OpenVinoPreflightResult::Failed {
                class: "openvino_npu_artifact_missing".to_string(),
                message: "OpenVINO lane artifact is missing".to_string(),
            };
        };

        let bundle = self.runtime_bundles().openvino.clone();
        let cache_dir = self.openvino_cache_dir(model_id, artifacts);
        let probe = probe.clone();
        let model_id = model_id.to_string();
        let task = tokio::task::spawn_blocking(move || {
            run_openvino_preflight(&bundle, &model_id, &artifact, &probe, &cache_dir)
        });

        match timeout(OPENVINO_PREFLIGHT_BUDGET, task).await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => OpenVinoPreflightResult::Failed {
                class: "openvino_npu_preflight_join_failed".to_string(),
                message: format!("OpenVINO preflight task failed: {error}"),
            },
            Err(_) => OpenVinoPreflightResult::Timeout,
        }
    }

    fn build_decision_key(
        &self,
        model_id: &str,
        artifacts: &ModelLaneArtifacts,
        probe: &BackendProbeResult,
        openvino_probe: Option<&OpenVinoStartupProbeResult>,
        selected_device_id: Option<i32>,
    ) -> BackendDecisionKey {
        let directml_candidate = probe.directml_candidate.as_ref();
        let openvino_probe = openvino_probe.filter(|probe| probe.device_visible);
        let openvino_tuning = resolve_openvino_npu_tuning().ok();
        BackendDecisionKey {
            model_id: model_id.to_string(),
            model_artifact_fingerprint: artifacts.fingerprint.clone(),
            app_version: self.app_version.clone(),
            selector_engine_id: "engine_host".to_string(),
            ort_runtime_version: runtime_version_value(
                &self.runtime_bundles().ort.version_metadata,
                "onnxruntime",
            ),
            ort_bundle_fingerprint: Some(self.runtime_bundles().ort.fingerprint.value.clone()),
            openvino_runtime_version: runtime_version_value(
                &self.runtime_bundles().openvino.version_metadata,
                "openvino-runtime",
            ),
            openvino_genai_version: runtime_version_value(
                &self.runtime_bundles().openvino.version_metadata,
                "openvino-genai",
            ),
            openvino_tokenizers_version: runtime_version_value(
                &self.runtime_bundles().openvino.version_metadata,
                "openvino-tokenizers",
            ),
            openvino_bundle_fingerprint: Some(
                self.runtime_bundles().openvino.fingerprint.value.clone(),
            ),
            gpu_adapter_identity: directml_candidate
                .map(|candidate| candidate.adapter_identity.clone()),
            gpu_driver_version: directml_candidate
                .map(|candidate| candidate.driver_version.clone()),
            gpu_device_id: selected_device_id,
            npu_adapter_identity: openvino_probe.and_then(|probe| probe.adapter_identity.clone()),
            npu_driver_version: openvino_probe.and_then(|probe| probe.driver_version.clone()),
            openvino_npu_max_prompt_len: openvino_tuning
                .as_ref()
                .map(|tuning| tuning.max_prompt_len),
            openvino_npu_min_response_len: openvino_tuning
                .as_ref()
                .map(|tuning| tuning.min_response_len),
            openvino_message_mode: Some(OPENVINO_CHAT_MODE_STRUCTURED.to_string()),
            selection_profile: Some(OPENVINO_SELECTION_PROFILE.to_string()),
        }
    }

    fn launch_startup_probe(self: &Arc<Self>) {
        let engine = Arc::clone(self);
        tokio::spawn(async move {
            let probed = tokio::task::spawn_blocking(probe_backend_capabilities)
                .await
                .unwrap_or_else(|error| {
                    log::warn!("Backend startup probe task failed: {error}");
                    BackendProbeResult::default()
                });

            {
                let mut probe_guard = engine.startup_probe.lock().await;
                *probe_guard = Some(probed.clone());
            }

            {
                let mut status = engine.backend_status.lock().await;
                apply_runtime_bundle_status(engine.runtime_bundles(), &mut status);
                apply_directml_startup_probe_status(&mut status, &probed);
                if status.selection_state == Some(BackendSelectionState::Pending) {
                    status.selection_state = Some(BackendSelectionState::Ready);
                    status.selection_reason = Some(if probed.directml_candidate.is_some() {
                        "startup_probe_ready".to_string()
                    } else {
                        "startup_probe_cpu_only".to_string()
                    });
                }
            }

            engine.startup_probe_ready.notify_waiters();

            let openvino_bundle = engine.runtime_bundles().openvino.clone();
            let hardware_detected = probed.npu_hardware_detected;
            let openvino_probe = tokio::task::spawn_blocking(move || {
                probe_openvino_startup(&openvino_bundle, hardware_detected)
            })
            .await
            .unwrap_or_else(|error| {
                log::warn!("OpenVINO startup probe task failed: {error}");
                OpenVinoStartupProbeResult {
                    hardware_detected,
                    failure_class: Some("openvino_npu_plugin_unavailable".to_string()),
                    failure_message: Some(format!("OpenVINO startup probe task failed: {error}")),
                    ..Default::default()
                }
            });

            {
                let mut probe_guard = engine.openvino_startup_probe.lock().await;
                *probe_guard = Some(openvino_probe.clone());
            }

            {
                let mut status = engine.backend_status.lock().await;
                apply_runtime_bundle_status(engine.runtime_bundles(), &mut status);
                apply_directml_startup_probe_status(&mut status, &probed);
                apply_openvino_startup_probe_status(&mut status, Some(&openvino_probe));
            }

            engine.openvino_startup_probe_ready.notify_waiters();
        });
    }

    async fn wait_for_startup_probe(&self, budget: Duration) -> BackendProbeResult {
        if let Some(existing) = self.startup_probe.lock().await.clone() {
            return existing;
        }

        let wait = self.startup_probe_ready.notified();
        let _ = timeout(budget, wait).await;
        self.startup_probe.lock().await.clone().unwrap_or_default()
    }

    async fn wait_for_startup_probe_with_recovery(
        &self,
        require_directml: bool,
    ) -> BackendProbeResult {
        let mut probe = self
            .wait_for_startup_probe(Duration::from_millis(STARTUP_PROBE_WAIT_MS))
            .await;
        if require_directml
            && !probe
                .available_backends
                .contains(&InferenceBackend::DirectML)
        {
            // DirectML enumeration may still be in flight after the initial budget.
            // If the probe has already settled, this returns immediately.
            probe = self
                .wait_for_startup_probe(Duration::from_millis(STARTUP_PROBE_RECOVERY_WAIT_MS))
                .await;
        }
        probe
    }

    async fn wait_for_openvino_startup_probe(
        &self,
        budget: Duration,
    ) -> Option<OpenVinoStartupProbeResult> {
        if let Some(existing) = self.openvino_startup_probe.lock().await.clone() {
            return Some(existing);
        }

        let wait = self.openvino_startup_probe_ready.notified();
        let _ = timeout(budget, wait).await;
        self.openvino_startup_probe.lock().await.clone()
    }

    async fn lookup_backend_record(
        &self,
        key: &BackendDecisionKey,
    ) -> Option<BackendDecisionRecord> {
        let store_guard = self.backend_store.lock().await;
        let store = store_guard.as_ref()?;
        store.get(key).cloned()
    }

    async fn persist_backend_record(
        &self,
        key: BackendDecisionKey,
        persisted_decision: Option<BackendDecision>,
        failure_counters: FailureCounters,
    ) {
        let mut store_guard = self.backend_store.lock().await;
        let Some(store) = store_guard.as_mut() else {
            return;
        };

        let record = BackendDecisionRecord {
            key,
            persisted_decision,
            failure_counters,
            updated_at: Utc::now().to_rfc3339(),
        };

        store.upsert(record);
        if let Err(error) = store.persist() {
            log::warn!("Failed to persist backend decision store: {error}");
        }
    }

    async fn transition_readiness(&self, next: ReadinessState) {
        let mut readiness = self.readiness.lock().await;
        readiness.transition(next);
    }

    async fn mark_readiness_failed(&self, error: StartupError) {
        let mut readiness = self.readiness.lock().await;
        readiness.mark_failed(error.phase, error.code, error.message, error.retryable);
    }

    async fn mark_readiness_ready(&self) {
        let mut readiness = self.readiness.lock().await;
        readiness.mark_ready();
    }

    async fn mark_readiness_idle_after_unload(&self) {
        let mut readiness = self.readiness.lock().await;
        if readiness.state.is_starting() {
            return;
        }
        readiness.mark_idle();
    }

    async fn mark_ready_after_external_load(&self, model_id: String) {
        let mut readiness = self.readiness.lock().await;
        if readiness.state.is_starting() {
            return;
        }
        if readiness.state != ReadinessState::Ready {
            readiness.begin_attempt(
                self.next_attempt_id(),
                StartupMode::Auto,
                StartupPolicy {
                    default_model_id: Some(model_id.clone()),
                },
            );
        } else {
            readiness.effective_mode = StartupMode::Auto;
            readiness.effective_startup_policy = StartupPolicy {
                default_model_id: Some(model_id.clone()),
            };
        }
        readiness.mark_ready();
    }

    async fn current_readiness_payload(
        &self,
        ok: bool,
        override_error: Option<StartupError>,
    ) -> ReadinessPayload {
        let readiness = self.readiness.lock().await.clone();
        let current_model = self.current_model.lock().await.clone();
        let backend_status = self.backend_status.lock().await.clone();
        let active_backend = backend_status.active_backend;
        let active_model_id = current_model.clone();
        let ready = readiness.state == ReadinessState::Ready
            && active_backend.is_some()
            && active_model_id.is_some();

        let (error_code, error_message, retryable, last_error) = match override_error {
            Some(error) => {
                let stamped = LastStartupError {
                    attempt_id: readiness.attempt_id.clone(),
                    phase: error.phase,
                    code: error.code.to_string(),
                    message: error.message.clone(),
                    retryable: error.retryable,
                    at: Utc::now().to_rfc3339(),
                };
                (
                    Some(error.code.to_string()),
                    Some(error.message),
                    Some(error.retryable),
                    Some(stamped),
                )
            }
            None => (
                readiness.error_code.clone(),
                readiness.error_message.clone(),
                readiness.retryable,
                readiness.last_error.clone(),
            ),
        };

        ReadinessPayload {
            ok,
            ready,
            attempt_id: readiness.attempt_id,
            state: readiness.state,
            startup_phase: readiness.state,
            state_since: readiness.state_since,
            active_backend,
            active_model_id,
            error_code,
            error_message,
            retryable,
            last_error,
            engine_version: env!("CARGO_PKG_VERSION"),
            engine_api_version: ENGINE_API_VERSION,
            effective_mode: readiness.effective_mode,
            effective_startup_policy: readiness.effective_startup_policy,
            current_model,
            generating: self.generating.load(Ordering::SeqCst),
            backend_status,
        }
    }

    async fn run_startup_attempt(
        &self,
        mode: StartupMode,
        default_model_id: String,
    ) -> Result<(), StartupError> {
        self.transition_readiness(ReadinessState::ResolvingAssets)
            .await;
        if ModelRegistry::get_model(&default_model_id).is_none() {
            return Err(StartupError {
                phase: ReadinessState::LoadingModel,
                code: "STARTUP_DEFAULT_MODEL_INVALID",
                message: format!("Unknown default model id '{default_model_id}'"),
                retryable: false,
            });
        }

        self.transition_readiness(ReadinessState::Probing).await;
        let probe = self
            .wait_for_startup_probe_with_recovery(mode.requires_directml())
            .await;
        let has_directml = probe
            .available_backends
            .contains(&InferenceBackend::DirectML);
        if mode.requires_directml() && !has_directml {
            return Err(StartupError {
                phase: ReadinessState::Probing,
                code: "STARTUP_DML_REQUIRED_UNAVAILABLE",
                message: "DirectML adapter is required but unavailable.".to_string(),
                retryable: false,
            });
        }

        self.transition_readiness(ReadinessState::LoadingModel)
            .await;
        self.load_model(default_model_id, mode)
            .await
            .map_err(|error| classify_startup_model_error(&error))?;
        self.mark_readiness_ready().await;
        Ok(())
    }

    async fn ensure_started(
        &self,
        mode: StartupMode,
        startup_policy: StartupPolicy,
    ) -> EnsureStartedOutcome {
        let default_model_id = match resolve_default_model_id(&startup_policy) {
            Ok(model_id) => model_id,
            Err(error) => {
                let mut readiness = self.readiness.lock().await;
                readiness.begin_attempt(self.next_attempt_id(), mode, startup_policy);
                readiness.mark_failed(error.phase, error.code, error.message, error.retryable);
                drop(readiness);
                self.startup_terminal.notify_waiters();
                return EnsureStartedOutcome::Failed;
            }
        };
        let effective_startup_policy = StartupPolicy {
            default_model_id: Some(default_model_id.clone()),
        };

        let mut readiness = self.readiness.lock().await;
        match readiness.state {
            ReadinessState::Ready => {
                let matches_policy = readiness.effective_mode == mode
                    && readiness.effective_startup_policy == effective_startup_policy;
                return if matches_policy {
                    EnsureStartedOutcome::Ready
                } else {
                    EnsureStartedOutcome::Conflict
                };
            }
            state if state.is_starting() => {
                let joined = self.startup_terminal.notified();
                drop(readiness);
                joined.await;
                let terminal = self.readiness.lock().await.state;
                return if terminal == ReadinessState::Ready {
                    EnsureStartedOutcome::Ready
                } else {
                    EnsureStartedOutcome::Failed
                };
            }
            ReadinessState::Idle | ReadinessState::Failed => {
                readiness.begin_attempt(
                    self.next_attempt_id(),
                    mode,
                    effective_startup_policy.clone(),
                );
            }
            ReadinessState::Starting
            | ReadinessState::ResolvingAssets
            | ReadinessState::Probing
            | ReadinessState::LoadingModel => {}
        }
        drop(readiness);

        let outcome = match self.run_startup_attempt(mode, default_model_id).await {
            Ok(()) => EnsureStartedOutcome::Ready,
            Err(error) => {
                self.mark_readiness_failed(error).await;
                EnsureStartedOutcome::Failed
            }
        };
        self.startup_terminal.notify_waiters();
        outcome
    }

    fn begin_generation(&self) -> Result<(GenerationPermit, Arc<AtomicBool>), String> {
        if self
            .generating
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("Generation already in progress".to_string());
        }
        let cancel_token = Arc::new(AtomicBool::new(false));
        let mut active = lock_cancel(&self.active_cancel);
        *active = Some(cancel_token.clone());
        Ok((
            GenerationPermit {
                generating: self.generating.clone(),
                active_cancel: self.active_cancel.clone(),
            },
            cancel_token,
        ))
    }

    async fn load_model(&self, model_id: String, startup_mode: StartupMode) -> Result<(), String> {
        if self.generating.load(Ordering::SeqCst) {
            return Err("Cannot load or unload model while generation is in progress".to_string());
        }
        let directml_required =
            model_requires_directml(&model_id) || startup_mode.requires_directml();
        let openvino_required = model_requires_openvino(&model_id);
        let model_def = ModelRegistry::get_model(&model_id)
            .ok_or_else(|| format!("Unknown model ID: {model_id}"))?;
        let cpu_model_dir = ModelLoader::openvino_dir(&model_def.directory);
        let artifacts = resolve_model_lane_artifacts(&model_def.directory);
        let dml_model_path = ModelLoader::resolve_model_file_for_backend(
            &model_def.directory,
            ModelArtifactBackend::DirectML,
        );

        let force_override = parse_force_override();
        let forced_device_id = parse_dml_device_id_env();
        let probe = self
            .wait_for_startup_probe_with_recovery(directml_required)
            .await;
        let openvino_probe_budget =
            if openvino_required || force_override == Some(InferenceBackend::OpenVinoNpu) {
                OPENVINO_STARTUP_PROBE_WAIT
            } else {
                Duration::from_millis(STARTUP_PROBE_WAIT_MS)
            };
        let openvino_probe = self
            .wait_for_openvino_startup_probe(openvino_probe_budget)
            .await;

        let directml_detected = probe.directml_candidate.is_some();
        let directml_artifact_available = artifacts.directml_ready && dml_model_path.is_some();
        let has_dml_candidate = directml_detected
            && directml_artifact_available
            && self.runtime_bundles().ort.directml_validated();
        let openvino_bundle_ready = self.runtime_bundles().openvino.npu_validated();

        if directml_required && force_override == Some(InferenceBackend::Cpu) {
            return Err(directml_required_error(
                &model_id,
                "forced CPU mode is not supported for this model",
            ));
        }
        if openvino_required && force_override == Some(InferenceBackend::Cpu) {
            return Err(openvino_required_error(
                &model_id,
                "forced CPU mode is not supported for this model",
            ));
        }
        if openvino_required && force_override == Some(InferenceBackend::DirectML) {
            return Err(openvino_required_error(
                &model_id,
                "forced DirectML mode is not supported for this model",
            ));
        }

        let probe_device_id = probe
            .directml_candidate
            .as_ref()
            .map(|candidate| candidate.device_id);
        let mut selected_device_id = forced_device_id.or(probe_device_id);
        let mut selected_device_name = if selected_device_id == probe_device_id {
            probe
                .directml_candidate
                .as_ref()
                .map(|candidate| candidate.device_name.clone())
        } else {
            None
        };

        let make_status = |selection_state: BackendSelectionState,
                           selection_reason: String,
                           device_id: Option<i32>,
                           device_name: Option<String>| {
            let mut status = BackendStatus {
                selection_state: Some(selection_state),
                selection_reason: Some(selection_reason),
                openvino_message_mode: Some(OPENVINO_CHAT_MODE_STRUCTURED.to_string()),
                openvino_tuning: current_openvino_tuning_status(),
                force_override,
                store_path: self
                    .store_path
                    .as_ref()
                    .map(|path| path.display().to_string()),
                ..Default::default()
            };
            apply_runtime_bundle_status(self.runtime_bundles(), &mut status);
            apply_directml_startup_probe_status(&mut status, &probe);
            apply_openvino_startup_probe_status(&mut status, openvino_probe.as_ref());
            apply_model_lane_artifacts(&mut status, &artifacts);
            apply_directml_device(&mut status, device_id, device_name);
            status
        };

        if let Some(forced_id) = forced_device_id {
            let forced_out_of_range = forced_id < 0
                || (probe.directml_device_count > 0
                    && forced_id as usize >= probe.directml_device_count);
            if forced_out_of_range {
                let error = format!(
                    "Invalid SMOLPC_DML_DEVICE_ID={forced_id}; detected DirectML device count={}",
                    probe.directml_device_count
                );
                if force_override == Some(InferenceBackend::DirectML) {
                    let mut status = make_status(
                        BackendSelectionState::Error,
                        "invalid_directml_device_id".to_string(),
                        Some(forced_id),
                        selected_device_name.clone(),
                    );
                    status.lanes.directml.last_failure_class =
                        Some("invalid_directml_device_id".to_string());
                    status.lanes.directml.last_failure_message = Some(error.clone());
                    *self.backend_status.lock().await = status;
                    return Err(error);
                }
                selected_device_id = probe_device_id;
                selected_device_name = probe
                    .directml_candidate
                    .as_ref()
                    .map(|candidate| candidate.device_name.clone());
            }
        }

        let decision_key = self.build_decision_key(
            &model_id,
            &artifacts,
            &probe,
            openvino_probe.as_ref(),
            selected_device_id,
        );
        let stored = self.lookup_backend_record(&decision_key).await;
        let mut failure_counters = stored
            .as_ref()
            .map(|record| record.failure_counters.clone())
            .unwrap_or_default();
        let persisted_backend = stored
            .as_ref()
            .and_then(|record| record.persisted_decision.as_ref())
            .map(|decision| decision.backend);
        let stored_openvino = persisted_backend == Some(InferenceBackend::OpenVinoNpu);
        let should_attempt_openvino = match force_override {
            Some(InferenceBackend::OpenVinoNpu) => true,
            Some(_) => false,
            None => persisted_backend.is_none() || stored_openvino,
        };
        let mut persisted_record_decision = stored
            .as_ref()
            .and_then(|record| record.persisted_decision.clone());
        let mut directml_preflight_state = LanePreflightState::NotStarted;
        let mut openvino_preflight_state = LanePreflightState::NotStarted;
        let mut directml_failure_class = None;
        let mut directml_failure_message = None;
        let mut openvino_failure_class = openvino_probe
            .as_ref()
            .and_then(|probe| probe.failure_class.clone());
        let mut openvino_failure_message = openvino_probe
            .as_ref()
            .and_then(|probe| probe.failure_message.clone());
        let mut suppress_store_update = false;
        let mut decision_persistence_state = if force_override.is_some() {
            DecisionPersistenceState::None
        } else {
            DecisionPersistenceState::Persisted
        };
        let mut selection_state = BackendSelectionState::Ready;
        let mut openvino_reason_override = None;
        let mut openvino_ready = None;

        if should_attempt_openvino {
            if !artifacts.openvino_npu_ready() {
                openvino_failure_class = artifacts.openvino_reason.clone();
                openvino_failure_message = artifacts.openvino_message.clone();
                openvino_reason_override = Some(DecisionReason::NoOpenVinoCandidate);
            } else if !openvino_bundle_ready {
                openvino_failure_class = Some(bundle_reason(
                    self.runtime_bundles().openvino.failure_code(),
                ));
                openvino_failure_message = Some(format!(
                    "OpenVINO runtime bundle is unavailable at {}",
                    self.runtime_bundles().openvino.display_root().display()
                ));
                openvino_reason_override = Some(DecisionReason::NoOpenVinoCandidate);
            } else if openvino_probe.is_none() {
                openvino_failure_class = Some("openvino_startup_probe_pending".to_string());
                openvino_failure_message =
                    Some("OpenVINO startup probe is still running".to_string());
                suppress_store_update = true;
                decision_persistence_state = DecisionPersistenceState::TemporaryFallback;
                selection_state = BackendSelectionState::Fallback;
                openvino_reason_override = Some(DecisionReason::OpenVinoStartupProbePending);
            } else if let Some(class) = openvino_probe
                .as_ref()
                .and_then(|probe| probe.failure_class.as_deref())
                .filter(|class| is_blocking_openvino_probe_failure(class))
            {
                openvino_failure_class = Some(class.to_string());
                openvino_failure_message = openvino_probe
                    .as_ref()
                    .and_then(|probe| probe.failure_message.clone());
                openvino_reason_override = Some(DecisionReason::NoOpenVinoCandidate);
            } else {
                let probe = openvino_probe
                    .as_ref()
                    .expect("OpenVINO startup probe should exist when artifact is ready");
                match self
                    .run_openvino_preflight_with_timeout(&model_id, &artifacts, probe)
                    .await
                {
                    OpenVinoPreflightResult::Ready(ready) => {
                        openvino_preflight_state = LanePreflightState::Ready;
                        openvino_ready = Some(ready);
                    }
                    OpenVinoPreflightResult::Timeout => {
                        openvino_preflight_state = LanePreflightState::Timeout;
                        openvino_failure_class = Some("openvino_npu_preflight_timeout".to_string());
                        openvino_failure_message = Some(format!(
                            "OpenVINO preflight exceeded the {} second budget",
                            OPENVINO_PREFLIGHT_BUDGET.as_secs()
                        ));
                        suppress_store_update = true;
                        decision_persistence_state = DecisionPersistenceState::TemporaryFallback;
                        selection_state = BackendSelectionState::Fallback;
                        openvino_reason_override = Some(DecisionReason::OpenVinoPreflightTimeout);
                    }
                    OpenVinoPreflightResult::Failed { class, message } => {
                        openvino_preflight_state = LanePreflightState::Error;
                        openvino_failure_class = Some(class);
                        openvino_failure_message = Some(message);
                        selection_state = BackendSelectionState::Fallback;
                        openvino_reason_override = Some(DecisionReason::OpenVinoPreflightFailed);
                    }
                }
            }
        }

        if (force_override == Some(InferenceBackend::OpenVinoNpu) || openvino_required)
            && openvino_ready.is_none()
        {
            let selection_reason = openvino_reason_override
                .as_ref()
                .map(decision_reason_code)
                .unwrap_or_else(|| {
                    openvino_failure_class
                        .as_deref()
                        .unwrap_or("openvino_npu_forced_activation_failed")
                })
                .to_string();
            let detail = openvino_failure_message
                .clone()
                .or_else(|| artifacts.openvino_message.clone())
                .unwrap_or_else(|| format!("OpenVINO lane is unavailable: {selection_reason}"));
            let error =
                if openvino_required && force_override != Some(InferenceBackend::OpenVinoNpu) {
                    openvino_required_error(&model_id, &detail)
                } else {
                    detail
                };

            let mut status = make_status(
                BackendSelectionState::Error,
                selection_reason,
                selected_device_id,
                selected_device_name.clone(),
            );
            status.decision_key = Some(decision_key.clone());
            status.selection_fingerprint = Some(decision_key.fingerprint());
            status.last_decision = None;
            status.lanes.openvino_npu.preflight_state = openvino_preflight_state;
            status.lanes.openvino_npu.last_failure_class = openvino_failure_class.clone();
            status.lanes.openvino_npu.last_failure_message = openvino_failure_message.clone();
            *self.backend_status.lock().await = status;
            return Err(error);
        }

        let has_openvino_candidate = openvino_ready.is_some();
        let (preferred_backend, decision_reason) = choose_preferred_backend(
            force_override,
            &failure_counters,
            stored.as_ref(),
            has_dml_candidate,
            has_openvino_candidate,
        );

        let mut active_backend = preferred_backend;
        let mut active_reason = decision_reason.clone();
        let mut runtime_engine = "ov_genai_cpu".to_string();
        if let Some(reason) = openvino_reason_override {
            active_reason = reason;
            selection_state = BackendSelectionState::Fallback;
        }
        let active_model_path: String;

        let adapter = if preferred_backend == InferenceBackend::OpenVinoNpu {
            let ready = openvino_ready.take().ok_or_else(|| {
                "OpenVINO selection expected a retained preflight generator".to_string()
            })?;
            if !suppress_store_update
                && force_override.is_none()
                && active_reason != DecisionReason::PersistedDecision
            {
                persisted_record_decision = Some(BackendDecision::new(
                    InferenceBackend::OpenVinoNpu,
                    active_reason.clone(),
                    None,
                ));
            }
            runtime_engine = "ov_genai_npu".to_string();
            active_model_path = artifacts
                .openvino_artifact
                .as_ref()
                .and_then(|artifact| artifact.manifest_path.parent())
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| {
                    ModelLoader::openvino_dir(&model_def.directory)
                        .display()
                        .to_string()
                });
            InferenceRuntimeAdapter::openvino_genai(ready.generator)
        } else if preferred_backend == InferenceBackend::DirectML {
            match dml_model_path.as_deref() {
                Some(dml_path) => {
                    match build_directml_runtime_adapter(
                        &self.runtime_bundles().ort,
                        dml_path,
                        selected_device_id,
                    ) {
                        Ok(adapter) => {
                            failure_counters.record_directml_success();
                            if !suppress_store_update
                                && force_override.is_none()
                                && active_reason != DecisionReason::PersistedDecision
                            {
                                persisted_record_decision = Some(BackendDecision::new(
                                    InferenceBackend::DirectML,
                                    active_reason.clone(),
                                    None,
                                ));
                            }
                            runtime_engine = "genai_dml".to_string();
                            directml_preflight_state = LanePreflightState::Ready;
                            active_model_path = dml_path.display().to_string();
                            adapter
                        }
                        Err(error) => {
                            if force_override == Some(InferenceBackend::DirectML)
                                || directml_required
                            {
                                failure_counters.record_directml_failure(
                                    DirectMLFailureStage::Init,
                                    error.clone(),
                                );
                                let mut status = make_status(
                                    BackendSelectionState::Error,
                                    decision_reason_code(
                                        &DecisionReason::DirectMLInitializationFailed,
                                    )
                                    .to_string(),
                                    selected_device_id,
                                    selected_device_name.clone(),
                                );
                                status.failure_counters = failure_counters.clone();
                                status.decision_key = Some(decision_key.clone());
                                status.selection_fingerprint = Some(decision_key.fingerprint());
                                status.lanes.directml.preflight_state = LanePreflightState::Error;
                                status.lanes.directml.last_failure_class =
                                    Some("directml_initialization_failed".to_string());
                                status.lanes.directml.last_failure_message = Some(error.clone());
                                *self.backend_status.lock().await = status;
                                return Err(error);
                            }

                            failure_counters
                                .record_directml_failure(DirectMLFailureStage::Init, error.clone());
                            selection_state = BackendSelectionState::Fallback;
                            active_backend = InferenceBackend::Cpu;
                            if !matches!(
                                active_reason,
                                DecisionReason::OpenVinoStartupProbePending
                                    | DecisionReason::OpenVinoPreflightTimeout
                                    | DecisionReason::OpenVinoRuntimeUnavailable
                            ) {
                                active_reason = DecisionReason::DirectMLInitializationFailed;
                            }
                            decision_persistence_state =
                                DecisionPersistenceState::TemporaryFallback;
                            directml_preflight_state = LanePreflightState::Error;
                            directml_failure_class =
                                Some("directml_initialization_failed".to_string());
                            directml_failure_message = Some(error.clone());

                            if failure_counters.should_demote_directml() {
                                failure_counters.mark_demotion();
                                active_reason = DecisionReason::DemotedAfterFailures;
                                decision_persistence_state = DecisionPersistenceState::Persisted;
                                persisted_record_decision = Some(BackendDecision::new(
                                    InferenceBackend::Cpu,
                                    DecisionReason::DemotedAfterFailures,
                                    None,
                                ));
                            }

                            let adapter = build_openvino_cpu_runtime_adapter(
                                &self.runtime_bundles().openvino,
                                &model_id,
                                &cpu_model_dir,
                            )?;
                            active_model_path = cpu_model_dir.display().to_string();
                            adapter
                        }
                    }
                }
                None => {
                    let error =
                        "DirectML model artifact missing (expected models/<model>/dml/model.onnx)"
                            .to_string();
                    if force_override == Some(InferenceBackend::DirectML) || directml_required {
                        failure_counters
                            .record_directml_failure(DirectMLFailureStage::Init, error.clone());
                        let mut status = make_status(
                            BackendSelectionState::Error,
                            "directml_artifact_missing".to_string(),
                            selected_device_id,
                            selected_device_name.clone(),
                        );
                        status.failure_counters = failure_counters.clone();
                        status.decision_key = Some(decision_key.clone());
                        status.selection_fingerprint = Some(decision_key.fingerprint());
                        status.lanes.directml.last_failure_class =
                            Some("directml_artifact_missing".to_string());
                        status.lanes.directml.last_failure_message = Some(error.clone());
                        *self.backend_status.lock().await = status;
                        return Err(error);
                    }
                    let adapter = build_openvino_cpu_runtime_adapter(
                        &self.runtime_bundles().openvino,
                        &model_id,
                        &cpu_model_dir,
                    )?;
                    active_backend = InferenceBackend::Cpu;
                    if !matches!(
                        active_reason,
                        DecisionReason::OpenVinoStartupProbePending
                            | DecisionReason::OpenVinoPreflightFailed
                            | DecisionReason::OpenVinoPreflightTimeout
                            | DecisionReason::OpenVinoRuntimeUnavailable
                            | DecisionReason::NoOpenVinoCandidate
                    ) {
                        active_reason = DecisionReason::NoDirectMLCandidate;
                    }
                    selection_state = BackendSelectionState::Fallback;
                    decision_persistence_state =
                        if suppress_store_update || force_override.is_some() {
                            DecisionPersistenceState::TemporaryFallback
                        } else {
                            DecisionPersistenceState::Persisted
                        };
                    if !suppress_store_update && force_override.is_none() {
                        persisted_record_decision = Some(BackendDecision::new(
                            InferenceBackend::Cpu,
                            active_reason.clone(),
                            None,
                        ));
                    }
                    directml_failure_class = Some("directml_artifact_missing".to_string());
                    directml_failure_message = Some(error.clone());
                    active_model_path = cpu_model_dir.display().to_string();
                    adapter
                }
            }
        } else {
            if directml_required {
                let reason = directml_unavailable_reason(
                    directml_detected,
                    directml_artifact_available,
                    self.runtime_bundles(),
                );
                return Err(directml_required_error(&model_id, &reason));
            }
            let adapter = build_openvino_cpu_runtime_adapter(
                &self.runtime_bundles().openvino,
                &model_id,
                &cpu_model_dir,
            )?;
            if !suppress_store_update
                && force_override.is_none()
                && active_reason != DecisionReason::PersistedDecision
            {
                persisted_record_decision = Some(BackendDecision::new(
                    InferenceBackend::Cpu,
                    active_reason.clone(),
                    None,
                ));
            }
            if should_attempt_openvino && active_reason != DecisionReason::PersistedDecision {
                selection_state = BackendSelectionState::Fallback;
            }
            active_model_path = cpu_model_dir.display().to_string();
            adapter
        };

        *self.runtime_adapter.lock().await = Some(adapter);
        *self.current_model.lock().await = Some(model_id.clone());
        let mut status = BackendStatus {
            active_backend: Some(active_backend),
            active_model_path: Some(active_model_path),
            active_artifact_backend: Some(active_backend),
            runtime_engine: Some(runtime_engine),
            selection_state: Some(selection_state),
            selection_reason: Some(decision_reason_code(&active_reason).to_string()),
            decision_persistence_state,
            selection_fingerprint: Some(decision_key.fingerprint()),
            decision_key: Some(decision_key.clone()),
            last_decision: Some(BackendDecision::new(active_backend, active_reason, None)),
            openvino_message_mode: Some(OPENVINO_CHAT_MODE_STRUCTURED.to_string()),
            openvino_tuning: current_openvino_tuning_status(),
            failure_counters: failure_counters.clone(),
            force_override,
            store_path: self
                .store_path
                .as_ref()
                .map(|path| path.display().to_string()),
            ..Default::default()
        };
        apply_runtime_bundle_status(self.runtime_bundles(), &mut status);
        apply_directml_startup_probe_status(&mut status, &probe);
        apply_openvino_startup_probe_status(&mut status, openvino_probe.as_ref());
        apply_model_lane_artifacts(&mut status, &artifacts);
        apply_directml_device(&mut status, selected_device_id, selected_device_name);
        apply_persisted_eligibility(&mut status, persisted_record_decision.as_ref());
        status.lanes.directml.preflight_state = directml_preflight_state;
        status.lanes.openvino_npu.preflight_state = openvino_preflight_state;
        status.lanes.openvino_npu.last_failure_class = openvino_failure_class;
        status.lanes.openvino_npu.last_failure_message = openvino_failure_message;
        if active_backend == InferenceBackend::DirectML {
            status.lanes.directml.last_failure_class = None;
            status.lanes.directml.last_failure_message = None;
        } else if let Some(class) = directml_failure_class {
            status.lanes.directml.last_failure_class = Some(class);
            status.lanes.directml.last_failure_message = directml_failure_message;
        } else if !directml_detected {
            status.lanes.directml.last_failure_class =
                Some("directml_candidate_missing".to_string());
            status.lanes.directml.last_failure_message =
                Some("No DirectML-capable adapter detected".to_string());
        }
        *self.backend_status.lock().await = status;

        if force_override.is_none() && !suppress_store_update {
            self.persist_backend_record(decision_key, persisted_record_decision, failure_counters)
                .await;
        }

        Ok(())
    }

    async fn unload_model(&self, force: bool) -> Result<(), String> {
        if self.generating.load(Ordering::SeqCst) {
            if force {
                self.cancel();
            } else {
                return Err("Cannot unload model while generation is in progress".to_string());
            }
        }
        *self.runtime_adapter.lock().await = None;
        *self.current_model.lock().await = None;
        let mut status = self.backend_status.lock().await;
        status.active_backend = None;
        status.active_model_path = None;
        status.active_artifact_backend = None;
        status.runtime_engine = None;
        status.selection_state = Some(BackendSelectionState::Ready);
        status.selection_reason = Some("model_unloaded".to_string());
        status.decision_persistence_state = DecisionPersistenceState::None;
        status.selection_fingerprint = None;
        status.decision_key = None;
        status.last_decision = None;
        status.lanes.cpu.artifact_ready = false;
        status.lanes.directml.artifact_ready = false;
        status.lanes.openvino_npu.artifact_ready = false;
        status.lanes.cpu.persisted_eligibility = false;
        status.lanes.directml.persisted_eligibility = false;
        status.lanes.openvino_npu.persisted_eligibility = false;
        status.lanes.directml.preflight_state = LanePreflightState::NotStarted;
        status.lanes.openvino_npu.preflight_state = LanePreflightState::NotStarted;
        apply_runtime_bundle_status(self.runtime_bundles(), &mut status);
        drop(status);
        self.mark_readiness_idle_after_unload().await;
        Ok(())
    }

    async fn try_runtime_fallback_after_directml_failure(&self, error: &str) {
        if error.contains("INFERENCE_GENERATION_CANCELLED") {
            return;
        }
        if parse_force_override() == Some(InferenceBackend::DirectML) {
            return;
        }

        let status_snapshot = self.backend_status.lock().await.clone();
        if status_snapshot.active_backend != Some(InferenceBackend::DirectML) {
            return;
        }

        let Some(model_id) = self.current_model.lock().await.clone() else {
            return;
        };
        let Some(model_def) = ModelRegistry::get_model(&model_id) else {
            return;
        };
        let model_artifacts = resolve_model_lane_artifacts(&model_def.directory);
        let cpu_model_dir = ModelLoader::openvino_dir(&model_def.directory);
        let Ok(cpu_adapter) = build_openvino_cpu_runtime_adapter(
            &self.runtime_bundles().openvino,
            &model_id,
            &cpu_model_dir,
        ) else {
            return;
        };

        *self.runtime_adapter.lock().await = Some(cpu_adapter);
        let mut counters = status_snapshot.failure_counters.clone();
        counters.record_directml_failure(DirectMLFailureStage::Runtime, error.to_string());
        let mut persisted_record_decision =
            if let Some(decision_key) = status_snapshot.decision_key.as_ref() {
                self.lookup_backend_record(decision_key)
                    .await
                    .and_then(|record| record.persisted_decision)
            } else {
                None
            };
        let mut decision_reason = DecisionReason::RuntimeFailureFallback;
        let mut decision_persistence_state = DecisionPersistenceState::TemporaryFallback;
        if counters.should_demote_directml() {
            counters.mark_demotion();
            decision_reason = DecisionReason::DemotedAfterFailures;
            decision_persistence_state = DecisionPersistenceState::Persisted;
            persisted_record_decision = Some(BackendDecision::new(
                InferenceBackend::Cpu,
                DecisionReason::DemotedAfterFailures,
                None,
            ));
        }

        let mut updated = status_snapshot.clone();
        updated.active_backend = Some(InferenceBackend::Cpu);
        updated.active_artifact_backend = Some(InferenceBackend::Cpu);
        updated.runtime_engine = Some("ov_genai_cpu".to_string());
        updated.active_model_path = Some(cpu_model_dir.display().to_string());
        updated.selection_state = Some(BackendSelectionState::Fallback);
        updated.selection_reason = Some(decision_reason_code(&decision_reason).to_string());
        updated.decision_persistence_state = decision_persistence_state;
        updated.last_decision = Some(BackendDecision::new(
            InferenceBackend::Cpu,
            decision_reason,
            None,
        ));
        updated.failure_counters = counters.clone();
        updated.force_override = parse_force_override();
        apply_runtime_bundle_status(self.runtime_bundles(), &mut updated);
        apply_model_lane_artifacts(&mut updated, &model_artifacts);
        apply_persisted_eligibility(&mut updated, persisted_record_decision.as_ref());
        updated.lanes.directml.preflight_state = LanePreflightState::Ready;
        updated.lanes.directml.last_failure_class = Some("directml_runtime_failed".to_string());
        updated.lanes.directml.last_failure_message = Some(error.to_string());
        *self.backend_status.lock().await = updated;

        if let Some(decision_key) = status_snapshot.decision_key {
            self.persist_backend_record(decision_key, persisted_record_decision, counters)
                .await;
        }
    }

    async fn generate_text(
        &self,
        prompt: &str,
        config: Option<GenerationConfig>,
    ) -> Result<GenerationResult, String> {
        let (_permit, cancelled) = self.begin_generation()?;
        let mut text = String::new();
        let result = {
            let adapter_guard = self.runtime_adapter.lock().await;
            let adapter = adapter_guard
                .as_ref()
                .ok_or_else(|| "No model loaded. Call /engine/load first.".to_string())?;
            adapter
                .generate_stream(prompt, config, cancelled.clone(), |token| {
                    text.push_str(&token)
                })
                .await
        };
        let metrics = match result {
            Ok(metrics) => metrics,
            Err(error) => {
                self.try_runtime_fallback_after_directml_failure(&error)
                    .await;
                return Err(error);
            }
        };
        if cancelled.load(Ordering::SeqCst) {
            return Err("INFERENCE_GENERATION_CANCELLED: Generation cancelled".to_string());
        }
        Ok(GenerationResult { text, metrics })
    }

    async fn generate_stream<F>(
        &self,
        prompt: &str,
        config: Option<GenerationConfig>,
        on_token: F,
    ) -> Result<GenerationMetrics, String>
    where
        F: FnMut(String),
    {
        let (_permit, cancelled) = self.begin_generation()?;
        let result = {
            let adapter_guard = self.runtime_adapter.lock().await;
            let adapter = adapter_guard
                .as_ref()
                .ok_or_else(|| "No model loaded. Call /engine/load first.".to_string())?;
            adapter
                .generate_stream(prompt, config, cancelled.clone(), on_token)
                .await
        };
        let metrics = match result {
            Ok(metrics) => metrics,
            Err(error) => {
                self.try_runtime_fallback_after_directml_failure(&error)
                    .await;
                return Err(error);
            }
        };
        if cancelled.load(Ordering::SeqCst) {
            return Err("INFERENCE_GENERATION_CANCELLED: Generation cancelled".to_string());
        }
        Ok(metrics)
    }

    async fn generate_text_messages(
        &self,
        messages: &[InferenceChatMessage],
        config: Option<GenerationConfig>,
    ) -> Result<GenerationResult, String> {
        let (_permit, cancelled) = self.begin_generation()?;
        let mut text = String::new();
        let result = {
            let adapter_guard = self.runtime_adapter.lock().await;
            let adapter = adapter_guard
                .as_ref()
                .ok_or_else(|| "No model loaded. Call /engine/load first.".to_string())?;
            adapter
                .generate_stream_messages(messages, config, cancelled.clone(), |token| {
                    text.push_str(&token)
                })
                .await
        };
        let metrics = match result {
            Ok(metrics) => metrics,
            Err(error) => {
                self.try_runtime_fallback_after_directml_failure(&error)
                    .await;
                return Err(error);
            }
        };
        if cancelled.load(Ordering::SeqCst) {
            return Err("INFERENCE_GENERATION_CANCELLED: Generation cancelled".to_string());
        }
        Ok(GenerationResult { text, metrics })
    }

    async fn generate_stream_messages<F>(
        &self,
        messages: &[InferenceChatMessage],
        config: Option<GenerationConfig>,
        on_token: F,
    ) -> Result<GenerationMetrics, String>
    where
        F: FnMut(String),
    {
        let (_permit, cancelled) = self.begin_generation()?;
        let result = {
            let adapter_guard = self.runtime_adapter.lock().await;
            let adapter = adapter_guard
                .as_ref()
                .ok_or_else(|| "No model loaded. Call /engine/load first.".to_string())?;
            adapter
                .generate_stream_messages(messages, config, cancelled.clone(), on_token)
                .await
        };
        let metrics = match result {
            Ok(metrics) => metrics,
            Err(error) => {
                self.try_runtime_fallback_after_directml_failure(&error)
                    .await;
                return Err(error);
            }
        };
        if cancelled.load(Ordering::SeqCst) {
            return Err("INFERENCE_GENERATION_CANCELLED: Generation cancelled".to_string());
        }
        Ok(metrics)
    }

    fn cancel(&self) {
        if let Some(token) = lock_cancel(&self.active_cancel).clone() {
            token.store(true, Ordering::SeqCst);
        }
    }
}

fn choose_preferred_backend(
    force_override: Option<InferenceBackend>,
    failure_counters: &FailureCounters,
    stored: Option<&BackendDecisionRecord>,
    has_dml_candidate: bool,
    has_openvino_candidate: bool,
) -> (InferenceBackend, DecisionReason) {
    if let Some(override_backend) = force_override {
        return (override_backend, DecisionReason::ForcedOverride);
    }
    if let Some(record) = stored {
        if let Some(decision) = record.persisted_decision.as_ref() {
            match decision.backend {
                InferenceBackend::OpenVinoNpu => {
                    if has_openvino_candidate {
                        return (
                            InferenceBackend::OpenVinoNpu,
                            DecisionReason::PersistedDecision,
                        );
                    }
                }
                InferenceBackend::DirectML => {
                    if has_dml_candidate && !failure_counters.should_demote_directml() {
                        return (
                            InferenceBackend::DirectML,
                            DecisionReason::PersistedDecision,
                        );
                    }
                }
                InferenceBackend::Cpu => {
                    return (InferenceBackend::Cpu, DecisionReason::PersistedDecision);
                }
            }
        }
    }
    if has_openvino_candidate {
        return (
            InferenceBackend::OpenVinoNpu,
            DecisionReason::DefaultOpenVinoCandidate,
        );
    }
    if failure_counters.should_demote_directml() {
        return (InferenceBackend::Cpu, DecisionReason::DemotedAfterFailures);
    }
    if has_dml_candidate {
        return (
            InferenceBackend::DirectML,
            DecisionReason::DefaultDirectMLCandidate,
        );
    }
    (InferenceBackend::Cpu, DecisionReason::NoDirectMLCandidate)
}

struct GenerationPermit {
    generating: Arc<AtomicBool>,
    active_cancel: Arc<StdMutex<Option<Arc<AtomicBool>>>>,
}

impl Drop for GenerationPermit {
    fn drop(&mut self) {
        self.generating.store(false, Ordering::SeqCst);
        *lock_cancel(&self.active_cancel) = None;
    }
}

fn lock_cancel<'a>(
    active_cancel: &'a StdMutex<Option<Arc<AtomicBool>>>,
) -> std::sync::MutexGuard<'a, Option<Arc<AtomicBool>>> {
    match active_cancel.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn build_openvino_cpu_runtime_adapter(
    bundle: &OpenVinoRuntimeBundle,
    model_id: &str,
    model_dir: &Path,
) -> Result<InferenceRuntimeAdapter, String> {
    let pipeline_config = OpenVinoPipelineConfig::cpu()
        .with_generation_controls(openvino_generation_controls_for_model(model_id))
        .with_disable_thinking(true);
    let generator = smolpc_engine_core::inference::OpenVinoGenAiGenerator::new(
        bundle,
        model_dir,
        &pipeline_config,
    )?;
    generator.run_preflight("Warmup preflight")?;
    Ok(InferenceRuntimeAdapter::openvino_genai(generator))
}

#[cfg(target_os = "windows")]
fn build_directml_runtime_adapter(
    ort_bundle: &OrtRuntimeBundle,
    dml_model_path: &Path,
    directml_device_id: Option<i32>,
) -> Result<InferenceRuntimeAdapter, String> {
    let model_dir = dml_model_path
        .parent()
        .ok_or_else(|| format!("Invalid DirectML model path: {}", dml_model_path.display()))?;
    OrtRuntimeLoader::ensure_initialized(ort_bundle)?;
    let generator = GenAiDirectMlGenerator::new(ort_bundle, model_dir, directml_device_id)?;
    generator.run_preflight("Warmup preflight")?;
    Ok(InferenceRuntimeAdapter::genai_directml(generator))
}

#[cfg(not(target_os = "windows"))]
fn build_directml_runtime_adapter(
    _ort_bundle: &OrtRuntimeBundle,
    _dml_model_path: &Path,
    _directml_device_id: Option<i32>,
) -> Result<InferenceRuntimeAdapter, String> {
    Err("DirectML is only supported on Windows".to_string())
}

#[derive(Clone)]
struct AppState {
    token: Arc<String>,
    engine: Arc<EngineState>,
    generation_semaphore: Arc<Semaphore>,
    queue_semaphore: Arc<Semaphore>,
    queue_timeout: Duration,
    shutdown: Arc<Notify>,
    last_activity_ms: Arc<AtomicU64>,
}

enum StreamMessage {
    Token(String),
    Metrics(GenerationMetrics),
    Done,
    Error { message: String, code: &'static str },
}

enum CompletionInput {
    Prompt(String),
    Messages(Vec<InferenceChatMessage>),
}

struct CancelOnDrop {
    engine: Arc<EngineState>,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.engine.cancel();
    }
}

fn auth(headers: &HeaderMap, token: &str) -> Result<(), ApiError> {
    let Some(value) = headers.get("authorization") else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        ));
    };
    let Ok(value) = value.to_str() else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        ));
    };
    let expected = format!("Bearer {token}");
    if !constant_time_eq(value.as_bytes(), expected.as_bytes()) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        ));
    }
    Ok(())
}

fn constant_time_eq(lhs: &[u8], rhs: &[u8]) -> bool {
    if lhs.len() != rhs.len() {
        return false;
    }

    let mut diff = 0u8;
    for (a, b) in lhs.iter().zip(rhs.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

fn looks_like_chatml_prompt(content: &str) -> bool {
    content.contains("<|im_start|>") && content.contains("<|im_end|>")
}

fn is_preformatted_chatml_single_user_message(messages: &[ChatCompletionMessage]) -> bool {
    if messages.len() != 1 {
        return false;
    }
    let only = &messages[0];
    if !only.role.trim().eq_ignore_ascii_case("user") {
        return false;
    }
    let content = only.content.clone().unwrap_or_default();
    !content.trim().is_empty() && looks_like_chatml_prompt(&content)
}

fn request_to_prompt(messages: &[ChatCompletionMessage]) -> Result<String, String> {
    if messages.is_empty() {
        return Err("messages cannot be empty".to_string());
    }

    // Compatibility mode: older clients may already send a full ChatML prompt
    // as a single user message. Preserve that payload as-is.
    if messages.len() == 1 {
        let only = &messages[0];
        if only.role.trim().eq_ignore_ascii_case("user") {
            let content = only.content.clone().unwrap_or_default();
            if !content.trim().is_empty() && looks_like_chatml_prompt(&content) {
                return Ok(content);
            }
        }
    }

    let mut prompt = String::new();
    for m in messages {
        let content = m.content.clone().unwrap_or_default();
        if !content.is_empty() {
            let role = match m.role.trim().to_ascii_lowercase().as_str() {
                "system" => "system",
                "user" => "user",
                "assistant" => "assistant",
                other => return Err(format!("unsupported message role: {other}")),
            };
            prompt.push_str("<|im_start|>");
            prompt.push_str(role);
            prompt.push('\n');
            prompt.push_str(&content);
            prompt.push_str("<|im_end|>\n");
        }
    }

    if prompt.is_empty() {
        return Err("messages must contain at least one non-empty content item".to_string());
    }

    prompt.push_str("<|im_start|>assistant\n");
    Ok(prompt)
}

fn request_to_structured_messages(
    messages: &[ChatCompletionMessage],
) -> Result<Vec<InferenceChatMessage>, String> {
    if messages.is_empty() {
        return Err("messages cannot be empty".to_string());
    }

    let mut out = Vec::new();
    for message in messages {
        let content = message.content.clone().unwrap_or_default();
        if content.is_empty() {
            continue;
        }

        let role = match message.role.trim().to_ascii_lowercase().as_str() {
            "system" => "system",
            "user" => "user",
            "assistant" => "assistant",
            other => return Err(format!("unsupported message role: {other}")),
        };
        out.push(InferenceChatMessage {
            role: role.to_string(),
            content,
        });
    }

    if out.is_empty() {
        return Err("messages must contain at least one non-empty content item".to_string());
    }
    Ok(out)
}

fn max_tokens_hard_cap() -> usize {
    std::env::var(OPENVINO_MAX_TOKENS_HARD_CAP_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(OPENVINO_MAX_TOKENS_HARD_CAP_DEFAULT)
}

fn request_to_config(request: &ChatCompletionRequest) -> Result<Option<GenerationConfig>, String> {
    let mut c = GenerationConfig::default();
    let mut changed = false;
    if let Some(v) = request.max_tokens {
        if v == 0 {
            return Err("max_tokens must be greater than zero".to_string());
        }
        let hard_cap = max_tokens_hard_cap();
        c.max_length = v.min(hard_cap);
        if v > hard_cap {
            log::info!("Capping max_tokens from {v} to backend hard cap {hard_cap}");
        }
        changed = true;
    }
    if let Some(v) = request.temperature {
        c.temperature = v;
        changed = true;
    }
    if let Some(v) = request.top_k {
        c.top_k = Some(v);
        changed = true;
    }
    if let Some(v) = request.top_p {
        c.top_p = Some(v);
        changed = true;
    }
    if let Some(v) = request.repetition_penalty {
        c.repetition_penalty = v;
        changed = true;
    }
    if let Some(v) = request.repetition_penalty_last_n {
        c.repetition_penalty_last_n = v;
        changed = true;
    }
    if changed {
        Ok(Some(c))
    } else {
        Ok(None)
    }
}

fn stream_error_code(error: &str) -> &'static str {
    if error.contains("INFERENCE_GENERATION_CANCELLED") {
        "INFERENCE_GENERATION_CANCELLED"
    } else {
        "ENGINE_STREAM_ERROR"
    }
}

async fn health(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn meta(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    Ok(Json(serde_json::json!({
        "ok": true,
        "protocol_version": ENGINE_PROTOCOL_VERSION,
        "engine_api_version": ENGINE_API_VERSION,
        "engine_version": env!("CARGO_PKG_VERSION"),
        "pid": std::process::id(),
        "busy": state.engine.generating.load(Ordering::SeqCst),
    })))
}

async fn status(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    let payload = state.engine.current_readiness_payload(true, None).await;
    Ok(Json(serde_json::json!(payload)))
}

async fn ensure_started(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<EnsureStartedRequest>,
) -> Result<impl IntoResponse, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);

    let outcome = state
        .engine
        .ensure_started(req.mode, req.startup_policy.clone())
        .await;
    let (http_status, ok, override_error) = match outcome {
        EnsureStartedOutcome::Ready => (StatusCode::OK, true, None),
        EnsureStartedOutcome::Failed => (StatusCode::SERVICE_UNAVAILABLE, false, None),
        EnsureStartedOutcome::Conflict => (
            StatusCode::CONFLICT,
            false,
            Some(StartupError {
                phase: ReadinessState::Ready,
                code: "STARTUP_POLICY_CONFLICT",
                message: "Engine is already ready under a different startup mode/policy. Perform explicit shutdown and restart.".to_string(),
                retryable: false,
            }),
        ),
    };
    let payload = state
        .engine
        .current_readiness_payload(ok, override_error)
        .await;
    Ok((http_status, Json(payload)).into_response())
}

async fn load(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<LoadRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    state
        .engine
        .load_model(req.model_id.clone(), StartupMode::Auto)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: e }),
            )
        })?;
    state
        .engine
        .mark_ready_after_external_load(req.model_id)
        .await;
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn unload(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<UnloadRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    state
        .engine
        .unload_model(req.force.unwrap_or(false))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: e }),
            )
        })?;
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn cancel(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    state.engine.cancel();
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn shutdown(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    state.shutdown.notify_waiters();
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn check_model(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<CheckModelRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    let startup_probe = state.engine.startup_probe.lock().await.clone();
    let openvino_probe = state.engine.openvino_startup_probe.lock().await.clone();
    let readiness = build_check_model_response(
        &req.model_id,
        state.engine.runtime_bundles(),
        startup_probe.as_ref(),
        openvino_probe.as_ref(),
    );
    Ok(Json(
        serde_json::to_value(readiness).expect("check-model response should serialize"),
    ))
}

async fn v1_models(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    let data = ModelRegistry::available_models()
        .into_iter()
        .map(|m| serde_json::json!({"id": m.id, "object": "model", "owned_by": "smolpc"}))
        .collect::<Vec<_>>();
    Ok(Json(serde_json::json!({"object": "list", "data": data})))
}

async fn v1_chat_completions(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);

    let queue_permit = state
        .queue_semaphore
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse {
                    error: "Engine queue is full".to_string(),
                }),
            )
        })?;

    let gen_permit = timeout(
        state.queue_timeout,
        state.generation_semaphore.clone().acquire_owned(),
    )
    .await
    .map_err(|_| {
        (
            StatusCode::GATEWAY_TIMEOUT,
            Json(ErrorResponse {
                error: "Queued request timed out".to_string(),
            }),
        )
    })
    .and_then(|r| {
        r.map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Generation semaphore closed".to_string(),
                }),
            )
        })
    })?;

    drop(queue_permit);

    let config = request_to_config(&req)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;
    let openvino_active =
        state.engine.active_backend().await == Some(InferenceBackend::OpenVinoNpu);
    let use_legacy_prompt = is_preformatted_chatml_single_user_message(&req.messages);
    let completion_input = if openvino_active && !use_legacy_prompt {
        let messages = request_to_structured_messages(&req.messages)
            .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;
        (
            OPENVINO_CHAT_MODE_STRUCTURED,
            CompletionInput::Messages(messages),
        )
    } else {
        let prompt = request_to_prompt(&req.messages)
            .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;
        let mode = if use_legacy_prompt {
            OPENVINO_CHAT_MODE_LEGACY_PROMPT
        } else {
            OPENVINO_CHAT_MODE_STRUCTURED
        };
        (mode, CompletionInput::Prompt(prompt))
    };
    if openvino_active {
        let mut backend_status = state.engine.backend_status.lock().await;
        backend_status.openvino_message_mode = Some(completion_input.0.to_string());
    }
    let completion_input = completion_input.1;
    let model_name = req.model.unwrap_or_else(|| "smolpc-engine".to_string());
    let request_id = format!("chatcmpl-{}", Utc::now().timestamp_millis());
    let created = Utc::now().timestamp();

    if req.stream.unwrap_or(false) {
        let (tx, mut rx) = mpsc::unbounded_channel::<StreamMessage>();
        let engine = state.engine.clone();
        let activity = state.last_activity_ms.clone();
        let input = completion_input;
        tokio::spawn(async move {
            let _permit = gen_permit;
            let result = match input {
                CompletionInput::Prompt(prompt) => {
                    engine
                        .generate_stream(&prompt, config, |t| {
                            let _ = tx.send(StreamMessage::Token(t));
                        })
                        .await
                }
                CompletionInput::Messages(messages) => {
                    engine
                        .generate_stream_messages(&messages, config, |t| {
                            let _ = tx.send(StreamMessage::Token(t));
                        })
                        .await
                }
            };
            match result {
                Ok(metrics) => {
                    let _ = tx.send(StreamMessage::Metrics(metrics));
                    let _ = tx.send(StreamMessage::Done);
                }
                Err(e) => {
                    let _ = tx.send(StreamMessage::Error {
                        code: stream_error_code(&e),
                        message: e,
                    });
                    let _ = tx.send(StreamMessage::Done);
                }
            }
            activity.store(epoch_ms(), Ordering::SeqCst);
        });

        let stream = async_stream::stream! {
            let _cancel_guard = CancelOnDrop { engine: state.engine.clone() };
            let start = serde_json::json!({
                "id": request_id,
                "object": "chat.completion.chunk",
                "created": created,
                "model": model_name,
                "choices": [{"index": 0, "delta": {"role": "assistant"}, "finish_reason": serde_json::Value::Null}],
            });
            yield Ok::<Event, Infallible>(Event::default().data(start.to_string()));

            while let Some(item) = rx.recv().await {
                match item {
                    StreamMessage::Token(token) => {
                        let chunk = serde_json::json!({
                            "id": request_id,
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": model_name,
                            "choices": [{"index": 0, "delta": {"content": token}, "finish_reason": serde_json::Value::Null}],
                        });
                        yield Ok(Event::default().data(chunk.to_string()));
                    }
                    StreamMessage::Metrics(metrics) => {
                        let metrics_event = serde_json::json!({
                            "id": request_id,
                            "object": "chat.completion.metrics",
                            "created": created,
                            "model": model_name,
                            "smolpc_metrics": metrics,
                        });
                        yield Ok(Event::default().data(metrics_event.to_string()));
                    }
                    StreamMessage::Error { message, code } => {
                        let error_type = if code == "INFERENCE_GENERATION_CANCELLED" {
                            "cancelled"
                        } else {
                            "runtime_error"
                        };
                        let error_event = serde_json::json!({
                            "error": {
                                "message": message,
                                "code": code,
                                "type": error_type
                            }
                        });
                        yield Ok(Event::default().data(error_event.to_string()));
                    }
                    StreamMessage::Done => {
                        let done = serde_json::json!({
                            "id": request_id,
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": model_name,
                            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                        });
                        yield Ok(Event::default().data(done.to_string()));
                        yield Ok(Event::default().data("[DONE]"));
                        break;
                    }
                }
            }
        };

        return Ok(Sse::new(stream)
            .keep_alive(KeepAlive::default())
            .into_response());
    }

    let result = match completion_input {
        CompletionInput::Prompt(prompt) => state.engine.generate_text(&prompt, config).await,
        CompletionInput::Messages(messages) => {
            state.engine.generate_text_messages(&messages, config).await
        }
    }
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e }),
        )
    })?;
    drop(gen_permit);

    let response = serde_json::json!({
        "id": request_id,
        "object": "chat.completion",
        "created": created,
        "model": model_name,
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": result.text},
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 0,
            "completion_tokens": result.metrics.total_tokens,
            "total_tokens": result.metrics.total_tokens
        },
        "smolpc_metrics": result.metrics
    });

    Ok(Json(response).into_response())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args();
    std::fs::create_dir_all(&args.data_dir)?;

    let token =
        std::env::var("SMOLPC_ENGINE_TOKEN").map_err(|_| "SMOLPC_ENGINE_TOKEN is required")?;

    let state = AppState {
        token: Arc::new(token),
        engine: Arc::new(EngineState::new(&args)),
        generation_semaphore: Arc::new(Semaphore::new(1)),
        queue_semaphore: Arc::new(Semaphore::new(args.queue_size)),
        queue_timeout: args.queue_timeout,
        shutdown: Arc::new(Notify::new()),
        last_activity_ms: Arc::new(AtomicU64::new(epoch_ms())),
    };
    state.engine.launch_startup_probe();

    let idle_state = state.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(30)).await;
            let idle_ms =
                epoch_ms().saturating_sub(idle_state.last_activity_ms.load(Ordering::SeqCst));
            if let Some(model_idle_unload) = args.model_idle_unload {
                if idle_ms >= model_idle_unload.as_millis() as u64
                    && !idle_state.engine.generating.load(Ordering::SeqCst)
                    && idle_state.engine.current_model.lock().await.is_some()
                {
                    let _ = idle_state.engine.unload_model(false).await;
                }
            }
            if let Some(process_idle_exit) = args.process_idle_exit {
                if idle_ms >= process_idle_exit.as_millis() as u64
                    && !idle_state.engine.generating.load(Ordering::SeqCst)
                {
                    idle_state.shutdown.notify_waiters();
                    break;
                }
            }
        }
    });

    let app = Router::new()
        .route("/engine/health", get(health))
        .route("/engine/meta", get(meta))
        .route("/engine/status", get(status))
        .route("/engine/ensure-started", post(ensure_started))
        .route("/engine/load", post(load))
        .route("/engine/unload", post(unload))
        .route("/engine/cancel", post(cancel))
        .route("/engine/shutdown", post(shutdown))
        .route("/engine/check-model", post(check_model))
        .route("/v1/models", get(v1_models))
        .route("/v1/chat/completions", post(v1_chat_completions))
        .with_state(state.clone());

    let listener = TcpListener::bind(("127.0.0.1", args.port)).await?;
    println!(
        "smolpc-engine-host listening on http://127.0.0.1:{}",
        args.port
    );

    let shutdown_signal = async move {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = state.shutdown.notified() => {},
        }
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    #[test]
    fn request_to_prompt_renders_chatml() {
        let messages = vec![
            ChatCompletionMessage {
                role: "system".to_string(),
                content: Some("You are helpful.".to_string()),
            },
            ChatCompletionMessage {
                role: "user".to_string(),
                content: Some("hello".to_string()),
            },
        ];

        let prompt = request_to_prompt(&messages).expect("chatml prompt");
        assert!(prompt.contains("<|im_start|>system\nYou are helpful.<|im_end|>\n"));
        assert!(prompt.contains("<|im_start|>user\nhello<|im_end|>\n"));
        assert!(prompt.ends_with("<|im_start|>assistant\n"));
    }

    #[test]
    fn request_to_prompt_preserves_preformatted_chatml_single_user_message() {
        let preformatted = "<|im_start|>system\ns<|im_end|>\n<|im_start|>user\nu<|im_end|>\n<|im_start|>assistant\n";
        let messages = vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(preformatted.to_string()),
        }];

        let prompt = request_to_prompt(&messages).expect("preformatted chatml");
        assert_eq!(prompt, preformatted);
    }

    #[test]
    fn request_to_config_rejects_zero_max_tokens() {
        let _guard = lock_env();
        std::env::remove_var(OPENVINO_MAX_TOKENS_HARD_CAP_ENV);
        let request = ChatCompletionRequest {
            model: None,
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some("hi".to_string()),
            }],
            stream: None,
            max_tokens: Some(0),
            temperature: None,
            top_k: None,
            top_p: None,
            repetition_penalty: None,
            repetition_penalty_last_n: None,
        };

        let error = request_to_config(&request).expect_err("zero max_tokens should fail");
        assert!(error.contains("max_tokens"));
    }

    #[test]
    fn request_to_config_caps_max_tokens_to_hard_limit() {
        let _guard = lock_env();
        std::env::remove_var(OPENVINO_MAX_TOKENS_HARD_CAP_ENV);
        let request = ChatCompletionRequest {
            model: None,
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some("hi".to_string()),
            }],
            stream: None,
            max_tokens: Some(99_999),
            temperature: None,
            top_k: None,
            top_p: None,
            repetition_penalty: None,
            repetition_penalty_last_n: None,
        };

        let config = request_to_config(&request)
            .expect("config parse")
            .expect("config");
        assert_eq!(config.max_length, OPENVINO_MAX_TOKENS_HARD_CAP_DEFAULT);
    }

    #[test]
    fn auth_compare_is_constant_time_functionally() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }

    #[test]
    fn engine_state_startup_succeeds_with_missing_ort_bundle() {
        let temp = tempdir().expect("temp dir");
        let resource_dir = temp.path().join("resources");
        fs::create_dir_all(&resource_dir).expect("create resource dir");

        let args = test_args(temp.path(), Some(resource_dir.clone()));
        let bundles =
            resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Production);
        let engine = EngineState::new_with_runtime_bundles(&args, bundles);
        let status = engine.backend_status.blocking_lock().clone();

        assert_eq!(status.selection_state, Some(BackendSelectionState::Pending));
        assert!(!status.runtime_bundles.ort.validated);
        assert_eq!(
            status.runtime_bundles.ort.failure.as_deref(),
            Some("missing_root")
        );
        assert!(!status.lanes.cpu.bundle_ready);
    }

    #[test]
    fn engine_state_startup_succeeds_with_missing_openvino_bundle() {
        let temp = tempdir().expect("temp dir");
        let resource_dir = temp.path().join("resources");
        let libs = resource_dir.join("libs");
        create_ort_files(
            &libs,
            &[
                "onnxruntime.dll",
                "onnxruntime_providers_shared.dll",
                "onnxruntime-genai.dll",
                "DirectML.dll",
            ],
        );

        let args = test_args(temp.path(), Some(resource_dir.clone()));
        let bundles =
            resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Production);
        let engine = EngineState::new_with_runtime_bundles(&args, bundles);
        let status = engine.backend_status.blocking_lock().clone();

        assert!(status.runtime_bundles.ort.validated);
        assert!(!status.runtime_bundles.openvino.validated);
        assert_eq!(
            status.runtime_bundles.openvino.failure.as_deref(),
            Some("missing_root")
        );
        assert!(!status.lanes.openvino_npu.bundle_ready);
    }

    #[test]
    fn engine_state_startup_succeeds_with_missing_openvino_plugin() {
        let temp = tempdir().expect("temp dir");
        let resource_dir = temp.path().join("resources");
        let libs = resource_dir.join("libs");
        let openvino_root = libs.join("openvino");
        create_ort_files(
            &libs,
            &[
                "onnxruntime.dll",
                "onnxruntime_providers_shared.dll",
                "onnxruntime-genai.dll",
                "DirectML.dll",
            ],
        );
        create_openvino_files(&openvino_root);
        fs::remove_file(openvino_root.join("openvino_intel_npu_plugin.dll"))
            .expect("remove npu plugin");

        let args = test_args(temp.path(), Some(resource_dir.clone()));
        let bundles =
            resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Production);
        let engine = EngineState::new_with_runtime_bundles(&args, bundles);
        let status = engine.backend_status.blocking_lock().clone();

        assert!(status.runtime_bundles.ort.validated);
        assert!(status.runtime_bundles.openvino.validated);
        assert_eq!(status.runtime_bundles.openvino.failure, None);
        assert!(!status.lanes.openvino_npu.bundle_ready);
        assert_eq!(
            status.lanes.openvino_npu.last_failure_class.as_deref(),
            Some("openvino_npu_plugin_missing")
        );
    }

    #[test]
    fn backend_selection_prefers_openvino_when_candidate_is_ready() {
        let (backend, reason) =
            choose_preferred_backend(None, &FailureCounters::default(), None, true, true);

        assert_eq!(backend, InferenceBackend::OpenVinoNpu);
        assert_eq!(reason, DecisionReason::DefaultOpenVinoCandidate);
    }

    #[test]
    fn backend_selection_prefers_directml_when_openvino_is_unavailable() {
        let (backend, reason) =
            choose_preferred_backend(None, &FailureCounters::default(), None, true, false);

        assert_eq!(backend, InferenceBackend::DirectML);
        assert_eq!(reason, DecisionReason::DefaultDirectMLCandidate);
    }

    #[test]
    fn backend_selection_keeps_persisted_cpu_choice() {
        let record = BackendDecisionRecord {
            key: BackendDecisionKey {
                model_id: "qwen2.5-coder-1.5b".to_string(),
                model_artifact_fingerprint: Some("artifact-v1".to_string()),
                app_version: "test".to_string(),
                selector_engine_id: "engine_host".to_string(),
                ort_runtime_version: Some("2.0.0-rc.11".to_string()),
                ort_bundle_fingerprint: Some("ort-bundle".to_string()),
                openvino_runtime_version: Some("2026.0.0".to_string()),
                openvino_genai_version: Some("2026.0.0".to_string()),
                openvino_tokenizers_version: Some("2026.0.0".to_string()),
                openvino_bundle_fingerprint: Some("openvino-bundle".to_string()),
                gpu_adapter_identity: Some("intel:arc".to_string()),
                gpu_driver_version: Some("31.0.101.5522".to_string()),
                gpu_device_id: Some(0),
                npu_adapter_identity: None,
                npu_driver_version: None,
                openvino_npu_max_prompt_len: Some(512),
                openvino_npu_min_response_len: Some(1024),
                openvino_message_mode: Some(OPENVINO_CHAT_MODE_STRUCTURED.to_string()),
                selection_profile: Some(OPENVINO_SELECTION_PROFILE.to_string()),
            },
            persisted_decision: Some(BackendDecision::new(
                InferenceBackend::Cpu,
                DecisionReason::NoDirectMLCandidate,
                None,
            )),
            failure_counters: FailureCounters::default(),
            updated_at: Utc::now().to_rfc3339(),
        };

        let (backend, reason) = choose_preferred_backend(
            None,
            &FailureCounters::default(),
            Some(&record),
            true,
            false,
        );

        assert_eq!(backend, InferenceBackend::Cpu);
        assert_eq!(reason, DecisionReason::PersistedDecision);
    }

    #[test]
    fn backend_selection_keeps_persisted_openvino_choice_when_candidate_is_ready() {
        let record = BackendDecisionRecord {
            key: BackendDecisionKey {
                model_id: "qwen2.5-coder-1.5b".to_string(),
                model_artifact_fingerprint: Some("artifact-v1".to_string()),
                app_version: "test".to_string(),
                selector_engine_id: "engine_host".to_string(),
                ort_runtime_version: Some("2.0.0-rc.11".to_string()),
                ort_bundle_fingerprint: Some("ort-bundle".to_string()),
                openvino_runtime_version: Some("2026.0.0".to_string()),
                openvino_genai_version: Some("2026.0.0".to_string()),
                openvino_tokenizers_version: Some("2026.0.0".to_string()),
                openvino_bundle_fingerprint: Some("openvino-bundle".to_string()),
                gpu_adapter_identity: Some("intel:arc".to_string()),
                gpu_driver_version: Some("31.0.101.5522".to_string()),
                gpu_device_id: Some(0),
                npu_adapter_identity: Some("openvino:npu:intel_npu".to_string()),
                npu_driver_version: Some("32.0.100.3104".to_string()),
                openvino_npu_max_prompt_len: Some(512),
                openvino_npu_min_response_len: Some(1024),
                openvino_message_mode: Some(OPENVINO_CHAT_MODE_STRUCTURED.to_string()),
                selection_profile: Some(OPENVINO_SELECTION_PROFILE.to_string()),
            },
            persisted_decision: Some(BackendDecision::new(
                InferenceBackend::OpenVinoNpu,
                DecisionReason::PersistedDecision,
                None,
            )),
            failure_counters: FailureCounters::default(),
            updated_at: Utc::now().to_rfc3339(),
        };

        let (backend, reason) =
            choose_preferred_backend(None, &FailureCounters::default(), Some(&record), true, true);

        assert_eq!(backend, InferenceBackend::OpenVinoNpu);
        assert_eq!(reason, DecisionReason::PersistedDecision);
    }

    #[test]
    fn backend_selection_falls_back_when_persisted_openvino_candidate_is_unavailable() {
        let record = BackendDecisionRecord {
            key: BackendDecisionKey {
                model_id: "qwen2.5-coder-1.5b".to_string(),
                model_artifact_fingerprint: Some("artifact-v1".to_string()),
                app_version: "test".to_string(),
                selector_engine_id: "engine_host".to_string(),
                ort_runtime_version: Some("2.0.0-rc.11".to_string()),
                ort_bundle_fingerprint: Some("ort-bundle".to_string()),
                openvino_runtime_version: Some("2026.0.0".to_string()),
                openvino_genai_version: Some("2026.0.0".to_string()),
                openvino_tokenizers_version: Some("2026.0.0".to_string()),
                openvino_bundle_fingerprint: Some("openvino-bundle".to_string()),
                gpu_adapter_identity: Some("intel:arc".to_string()),
                gpu_driver_version: Some("31.0.101.5522".to_string()),
                gpu_device_id: Some(0),
                npu_adapter_identity: Some("openvino:npu:intel_npu".to_string()),
                npu_driver_version: Some("32.0.100.3104".to_string()),
                openvino_npu_max_prompt_len: Some(512),
                openvino_npu_min_response_len: Some(1024),
                openvino_message_mode: Some(OPENVINO_CHAT_MODE_STRUCTURED.to_string()),
                selection_profile: Some(OPENVINO_SELECTION_PROFILE.to_string()),
            },
            persisted_decision: Some(BackendDecision::new(
                InferenceBackend::OpenVinoNpu,
                DecisionReason::PersistedDecision,
                None,
            )),
            failure_counters: FailureCounters::default(),
            updated_at: Utc::now().to_rfc3339(),
        };

        let (backend, reason) = choose_preferred_backend(
            None,
            &FailureCounters::default(),
            Some(&record),
            true,
            false,
        );

        assert_eq!(backend, InferenceBackend::DirectML);
        assert_eq!(reason, DecisionReason::DefaultDirectMLCandidate);
    }

    #[test]
    fn check_model_response_reports_lane_readiness() {
        let _guard = lock_env();
        let temp = tempdir().expect("temp dir");
        let resource_dir = temp.path().join("resources");
        let libs = resource_dir.join("libs");
        let models_dir = temp.path().join("models");
        let model_dir = models_dir.join("qwen2.5-coder-1.5b");
        let dml_dir = model_dir.join("dml");
        let openvino_dir = model_dir.join("openvino");

        create_ort_files(
            &libs,
            &[
                "onnxruntime.dll",
                "onnxruntime_providers_shared.dll",
                "onnxruntime-genai.dll",
                "DirectML.dll",
            ],
        );
        create_openvino_files(&libs.join("openvino"));
        fs::create_dir_all(&dml_dir).expect("create dml dir");
        fs::create_dir_all(&openvino_dir).expect("create openvino dir");
        fs::write(dml_dir.join("model.onnx"), []).expect("write dml model");
        fs::write(dml_dir.join("genai_config.json"), []).expect("write dml config");
        fs::write(dml_dir.join("tokenizer.json"), []).expect("write dml tokenizer");
        fs::write(openvino_dir.join("model.xml"), []).expect("write openvino model");
        fs::write(
            openvino_dir.join("manifest.json"),
            br#"{"required_files":["model.xml"]}"#,
        )
        .expect("write openvino manifest");

        let models_guard = EnvVarGuard::set("SMOLPC_MODELS_DIR", models_dir.as_os_str());
        let bundles =
            resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Production);
        let probe = BackendProbeResult {
            available_backends: vec![InferenceBackend::Cpu, InferenceBackend::DirectML],
            directml_device_count: 1,
            directml_candidate: Some(DirectMlCandidate {
                device_id: 0,
                device_name: "Intel Arc".to_string(),
                adapter_identity: "intel:arc".to_string(),
                driver_version: "31.0.101.5522".to_string(),
            }),
            npu_hardware_detected: false,
        };

        let response =
            build_check_model_response("qwen2.5-coder-1.5b", &bundles, Some(&probe), None);
        drop(models_guard);

        assert!(response.lanes.cpu.ready);
        assert_eq!(response.lanes.cpu.reason, "ready");
        assert!(response.lanes.directml.ready);
        assert_eq!(response.lanes.directml.reason, "ready");
        assert!(!response.lanes.openvino_npu.ready);
        assert_eq!(response.lanes.openvino_npu.reason, "startup_probe_pending");
    }

    #[test]
    fn check_model_response_reports_openvino_lane_ready_when_probe_is_ready() {
        let _guard = lock_env();
        let temp = tempdir().expect("temp dir");
        let resource_dir = temp.path().join("resources");
        let libs = resource_dir.join("libs");
        let models_dir = temp.path().join("models");
        let model_dir = models_dir.join("qwen2.5-coder-1.5b");
        let dml_dir = model_dir.join("dml");
        let openvino_dir = model_dir.join("openvino");

        create_ort_files(
            &libs,
            &[
                "onnxruntime.dll",
                "onnxruntime_providers_shared.dll",
                "onnxruntime-genai.dll",
                "DirectML.dll",
            ],
        );
        create_openvino_files(&libs.join("openvino"));
        fs::create_dir_all(&dml_dir).expect("create dml dir");
        fs::create_dir_all(&openvino_dir).expect("create openvino dir");
        fs::write(dml_dir.join("model.onnx"), []).expect("write dml model");
        fs::write(dml_dir.join("genai_config.json"), []).expect("write dml config");
        fs::write(dml_dir.join("tokenizer.json"), []).expect("write dml tokenizer");
        fs::write(openvino_dir.join("model.xml"), []).expect("write openvino model");
        fs::write(
            openvino_dir.join("manifest.json"),
            br#"{"required_files":["model.xml"]}"#,
        )
        .expect("write openvino manifest");

        let models_guard = EnvVarGuard::set("SMOLPC_MODELS_DIR", models_dir.as_os_str());
        let bundles =
            resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Production);
        let probe = BackendProbeResult {
            available_backends: vec![InferenceBackend::Cpu, InferenceBackend::DirectML],
            directml_device_count: 1,
            directml_candidate: Some(DirectMlCandidate {
                device_id: 0,
                device_name: "Intel Arc".to_string(),
                adapter_identity: "intel:arc".to_string(),
                driver_version: "31.0.101.5522".to_string(),
            }),
            npu_hardware_detected: true,
        };
        let openvino_probe = OpenVinoStartupProbeResult {
            hardware_detected: true,
            startup_ready: true,
            device_visible: true,
            adapter_identity: Some("openvino:npu:intel_npu".to_string()),
            device_name: Some("Intel NPU".to_string()),
            driver_version: Some("32.0.100.3104".to_string()),
            failure_class: None,
            failure_message: None,
        };

        let response = build_check_model_response(
            "qwen2.5-coder-1.5b",
            &bundles,
            Some(&probe),
            Some(&openvino_probe),
        );
        drop(models_guard);

        assert!(response.lanes.openvino_npu.artifact_ready);
        assert!(response.lanes.openvino_npu.bundle_ready);
        assert!(response.lanes.openvino_npu.ready);
        assert_eq!(response.lanes.openvino_npu.reason, "ready");
    }

    #[test]
    fn check_model_response_reports_shared_openvino_artifact_readiness() {
        let _guard = lock_env();
        let temp = tempdir().expect("temp dir");
        let resource_dir = temp.path().join("resources");
        let libs = resource_dir.join("libs");
        let models_dir = temp.path().join("models");
        let model_dir = models_dir.join("qwen3-4b-instruct");
        let openvino_dir = model_dir.join("openvino");

        create_ort_files(
            &libs,
            &[
                "onnxruntime.dll",
                "onnxruntime_providers_shared.dll",
                "onnxruntime-genai.dll",
                "DirectML.dll",
            ],
        );
        create_openvino_files(&libs.join("openvino"));
        fs::create_dir_all(&openvino_dir).expect("create openvino dir");
        fs::write(openvino_dir.join("openvino_model.xml"), []).expect("write openvino model");
        fs::write(openvino_dir.join("openvino_model.bin"), []).expect("write openvino weights");
        fs::write(openvino_dir.join("openvino_tokenizer.xml"), []).expect("write tokenizer xml");
        fs::write(openvino_dir.join("openvino_tokenizer.bin"), []).expect("write tokenizer bin");
        fs::write(openvino_dir.join("openvino_detokenizer.xml"), [])
            .expect("write detokenizer xml");
        fs::write(openvino_dir.join("openvino_detokenizer.bin"), [])
            .expect("write detokenizer bin");
        fs::write(openvino_dir.join("openvino_config.json"), []).expect("write ov config");
        fs::write(openvino_dir.join("generation_config.json"), [])
            .expect("write generation config");
        fs::write(openvino_dir.join("config.json"), []).expect("write config");
        fs::write(openvino_dir.join("tokenizer.json"), []).expect("write tokenizer");
        fs::write(openvino_dir.join("tokenizer_config.json"), []).expect("write tokenizer config");
        fs::write(openvino_dir.join("special_tokens_map.json"), [])
            .expect("write special tokens map");
        fs::write(openvino_dir.join("chat_template.jinja"), []).expect("write chat template");
        fs::write(openvino_dir.join("added_tokens.json"), []).expect("write added tokens");
        fs::write(openvino_dir.join("merges.txt"), []).expect("write merges");
        fs::write(openvino_dir.join("vocab.json"), []).expect("write vocab");
        fs::write(
            openvino_dir.join("manifest.json"),
            br#"{"entrypoint":"openvino_model.xml","required_files":["openvino_model.bin","openvino_tokenizer.xml","openvino_tokenizer.bin","openvino_detokenizer.xml","openvino_detokenizer.bin","openvino_config.json","generation_config.json","config.json","tokenizer.json","tokenizer_config.json","special_tokens_map.json","chat_template.jinja","added_tokens.json","merges.txt","vocab.json"]}"#,
        )
        .expect("write openvino manifest");

        let models_guard = EnvVarGuard::set("SMOLPC_MODELS_DIR", models_dir.as_os_str());
        let bundles =
            resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Production);
        let probe = BackendProbeResult {
            available_backends: vec![InferenceBackend::Cpu, InferenceBackend::DirectML],
            directml_device_count: 1,
            directml_candidate: Some(DirectMlCandidate {
                device_id: 0,
                device_name: "Intel Arc".to_string(),
                adapter_identity: "intel:arc".to_string(),
                driver_version: "31.0.101.5522".to_string(),
            }),
            npu_hardware_detected: true,
        };
        let openvino_probe = OpenVinoStartupProbeResult {
            hardware_detected: true,
            startup_ready: true,
            device_visible: true,
            adapter_identity: Some("openvino:npu:intel_npu".to_string()),
            device_name: Some("Intel NPU".to_string()),
            driver_version: Some("32.0.100.3104".to_string()),
            failure_class: None,
            failure_message: None,
        };

        let response = build_check_model_response(
            "qwen3-4b-instruct",
            &bundles,
            Some(&probe),
            Some(&openvino_probe),
        );
        drop(models_guard);

        assert!(response.lanes.openvino_npu.ready);
        assert_eq!(response.lanes.openvino_npu.reason, "ready");
        assert!(!response.lanes.directml.ready);
        assert_eq!(response.lanes.directml.reason, "artifact_missing");
        assert!(response.lanes.cpu.ready);
        assert_eq!(response.lanes.cpu.reason, "ready");
    }

    #[test]
    fn process_idle_exit_is_disabled_by_default() {
        let _guard = lock_env();
        let env_guard = EnvVarGuard::unset("SMOLPC_ENGINE_PROCESS_IDLE_EXIT_SECS");

        assert_eq!(
            parse_idle_timeout_secs("SMOLPC_ENGINE_PROCESS_IDLE_EXIT_SECS", None, 60),
            None
        );

        drop(env_guard);
    }

    #[test]
    fn idle_timeout_zero_disables_timer() {
        let _guard = lock_env();
        let env_guard = EnvVarGuard::set("SMOLPC_ENGINE_PROCESS_IDLE_EXIT_SECS", "0");

        assert_eq!(
            parse_idle_timeout_secs("SMOLPC_ENGINE_PROCESS_IDLE_EXIT_SECS", Some(1800), 60),
            None
        );

        drop(env_guard);
    }

    #[test]
    fn model_idle_unload_keeps_default_when_unset() {
        let _guard = lock_env();
        let env_guard = EnvVarGuard::unset("SMOLPC_ENGINE_MODEL_IDLE_UNLOAD_SECS");

        assert_eq!(
            parse_idle_timeout_secs("SMOLPC_ENGINE_MODEL_IDLE_UNLOAD_SECS", Some(300), 30),
            Some(Duration::from_secs(300))
        );

        drop(env_guard);
    }

    fn test_args(base: &Path, resource_dir: Option<PathBuf>) -> ParsedArgs {
        ParsedArgs {
            port: 19432,
            data_dir: base.join("data"),
            resource_dir,
            app_version: "test".to_string(),
            queue_size: 1,
            queue_timeout: Duration::from_secs(1),
            model_idle_unload: Some(Duration::from_secs(30)),
            process_idle_exit: Some(Duration::from_secs(60)),
        }
    }

    fn create_ort_files(root: &Path, files: &[&str]) {
        fs::create_dir_all(root).expect("create ort root");
        for file in files {
            fs::write(root.join(file), []).expect("write ort runtime file");
        }
    }

    fn create_openvino_files(root: &Path) {
        fs::create_dir_all(root).expect("create openvino root");
        for file in [
            "openvino.dll",
            "openvino_c.dll",
            "openvino_intel_npu_plugin.dll",
            "openvino_intel_npu_compiler.dll",
            "openvino_intel_cpu_plugin.dll",
            "openvino_ir_frontend.dll",
            "openvino_genai.dll",
            "openvino_genai_c.dll",
            "openvino_tokenizers.dll",
            "tbb12.dll",
            "tbbbind_2_5.dll",
            "tbbmalloc.dll",
            "tbbmalloc_proxy.dll",
            "icudt70.dll",
            "icuuc70.dll",
        ] {
            fs::write(root.join(file), []).expect("write openvino runtime file");
        }
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        match env_lock().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }

        fn unset(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    #[test]
    fn resolve_default_model_id_prefers_request_then_config_then_builtin() {
        let selected = resolve_default_model_id_with_sources(
            Some("request-model".to_string()),
            Some("config-model".to_string()),
            Some("built-in-model".to_string()),
        )
        .expect("request model should win");
        assert_eq!(selected, "request-model");

        let selected = resolve_default_model_id_with_sources(
            None,
            Some("config-model".to_string()),
            Some("built-in-model".to_string()),
        )
        .expect("config model should win when request missing");
        assert_eq!(selected, "config-model");

        let selected =
            resolve_default_model_id_with_sources(None, None, Some("built-in-model".to_string()))
                .expect("built-in model should be used as final fallback");
        assert_eq!(selected, "built-in-model");
    }

    #[test]
    fn classify_startup_model_error_flags_unknown_model_as_non_retryable() {
        let classified = classify_startup_model_error("Unknown model ID: bad-model");
        assert_eq!(classified.code, "STARTUP_DEFAULT_MODEL_INVALID");
        assert!(!classified.retryable);
    }

    #[test]
    fn classify_startup_model_error_flags_missing_assets_as_non_retryable() {
        let classified =
            classify_startup_model_error("Model file for backend 'cpu' not found: C:/models/x");
        assert_eq!(classified.code, "STARTUP_MODEL_ASSET_MISSING");
        assert!(!classified.retryable);
    }

    #[test]
    fn startup_mode_directml_required_sets_directml_gate() {
        assert!(StartupMode::DirectmlRequired.requires_directml());
        assert!(!StartupMode::Auto.requires_directml());
    }
}
