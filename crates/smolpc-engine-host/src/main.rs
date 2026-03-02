use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use smolpc_engine_core::inference::backend::{
    BackendDecision, BackendDecisionKey, BackendStatus, DecisionReason, DirectMLFailureStage,
    FailureCounters, InferenceBackend, ORT_CRATE_VERSION,
};
use smolpc_engine_core::inference::backend_store::{
    backend_store_path, BackendDecisionRecord, BackendStore,
};
#[cfg(target_os = "windows")]
use smolpc_engine_core::inference::genai::GenAiDirectMlGenerator;
use smolpc_engine_core::inference::session::SessionBackendOptions;
use smolpc_engine_core::inference::{
    init_onnx_runtime, Generator, InferenceRuntimeAdapter, InferenceSession, TokenizerWrapper,
};
use smolpc_engine_core::models::{
    ModelArtifactBackend, ModelLoader, ModelRegistry, ModelRuntimeSpec, RuntimeBackendTarget,
};
use smolpc_engine_core::{GenerationConfig, GenerationMetrics, GenerationResult};
use std::cmp::Ordering as CmpOrdering;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex, Notify, Semaphore};
use tokio::time::{sleep, timeout};

type ApiError = (StatusCode, Json<ErrorResponse>);

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
    model_idle_unload: Duration,
    process_idle_exit: Duration,
}

const STARTUP_PROBE_WAIT_MS: u64 = 1_500;

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
}

impl Default for BackendProbeResult {
    fn default() -> Self {
        Self {
            available_backends: vec![InferenceBackend::Cpu],
            directml_device_count: 0,
            directml_candidate: None,
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
    let model_idle_unload = Duration::from_secs(
        std::env::var("SMOLPC_ENGINE_MODEL_IDLE_UNLOAD_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(300)
            .max(30),
    );
    let process_idle_exit = Duration::from_secs(
        std::env::var("SMOLPC_ENGINE_PROCESS_IDLE_EXIT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1800)
            .max(60),
    );

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

fn parse_force_override() -> Option<InferenceBackend> {
    let value = std::env::var("SMOLPC_FORCE_EP").ok()?;
    match value.trim().to_ascii_lowercase().as_str() {
        "cpu" => Some(InferenceBackend::Cpu),
        "dml" | "directml" => Some(InferenceBackend::DirectML),
        _ => None,
    }
}

fn parse_dml_device_id_env() -> Option<i32> {
    std::env::var("SMOLPC_DML_DEVICE_ID")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
}

fn decision_reason_code(reason: &DecisionReason) -> &'static str {
    match reason {
        DecisionReason::DefaultCpu => "default_cpu",
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
    let mut result = BackendProbeResult::default();
    let directml_device_count = info
        .gpus()
        .iter()
        .filter(|gpu| gpu.supports_directml())
        .count();
    result.directml_device_count = directml_device_count;
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

struct EngineState {
    runtime_adapter: Arc<Mutex<Option<InferenceRuntimeAdapter>>>,
    current_model: Arc<Mutex<Option<String>>>,
    backend_status: Arc<Mutex<BackendStatus>>,
    active_cancel: Arc<StdMutex<Option<Arc<AtomicBool>>>>,
    generating: Arc<AtomicBool>,
    app_version: String,
    store_path: Option<PathBuf>,
    backend_store: Arc<Mutex<Option<BackendStore>>>,
    startup_probe: Arc<Mutex<Option<BackendProbeResult>>>,
    startup_probe_ready: Arc<Notify>,
}

impl EngineState {
    fn new(args: &ParsedArgs) -> Self {
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

        let status = BackendStatus {
            available_backends: vec![InferenceBackend::Cpu],
            selection_state: Some("pending".to_string()),
            selection_reason: Some("startup_probe_pending".to_string()),
            store_path: store_path.as_ref().map(|path| path.display().to_string()),
            ..Default::default()
        };

        Self {
            runtime_adapter: Arc::new(Mutex::new(None)),
            current_model: Arc::new(Mutex::new(None)),
            backend_status: Arc::new(Mutex::new(status)),
            active_cancel: Arc::new(StdMutex::new(None)),
            generating: Arc::new(AtomicBool::new(false)),
            app_version: args.app_version.clone(),
            store_path,
            backend_store: Arc::new(Mutex::new(backend_store)),
            startup_probe: Arc::new(Mutex::new(None)),
            startup_probe_ready: Arc::new(Notify::new()),
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

            let now = Utc::now().to_rfc3339();
            {
                let mut probe_guard = engine.startup_probe.lock().await;
                *probe_guard = Some(probed.clone());
            }

            {
                let mut status = engine.backend_status.lock().await;
                status.available_backends = probed.available_backends.clone();
                status.directml_probe_passed = Some(probed.directml_candidate.is_some());
                status.directml_probe_error = if probed.directml_candidate.is_some() {
                    None
                } else {
                    Some("No DirectML-capable adapter detected".to_string())
                };
                status.directml_probe_at = Some(now);
                status.selected_device_id = probed.directml_candidate.as_ref().map(|c| c.device_id);
                status.selected_device_name = probed
                    .directml_candidate
                    .as_ref()
                    .map(|c| c.device_name.clone());
                if status.selection_state.as_deref() == Some("pending") {
                    status.selection_state = Some("ready".to_string());
                    status.selection_reason = Some(if probed.directml_candidate.is_some() {
                        "startup_probe_ready".to_string()
                    } else {
                        "startup_probe_cpu_only".to_string()
                    });
                }
            }

            engine.startup_probe_ready.notify_waiters();
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
        decision: BackendDecision,
        failure_counters: FailureCounters,
    ) {
        let mut store_guard = self.backend_store.lock().await;
        let Some(store) = store_guard.as_mut() else {
            return;
        };

        let record = BackendDecisionRecord {
            key: key.clone(),
            decision,
            failure_counters,
            updated_at: Utc::now().to_rfc3339(),
        };

        let _ = store.remove_stale_for_model(&key);
        store.upsert(record);
        if let Err(error) = store.persist() {
            log::warn!("Failed to persist backend decision store: {error}");
        }
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

    async fn load_model(&self, model_id: String) -> Result<(), String> {
        if self.generating.load(Ordering::SeqCst) {
            return Err("Cannot load or unload model while generation is in progress".to_string());
        }
        let model_def = ModelRegistry::get_model(&model_id)
            .ok_or_else(|| format!("Unknown model ID: {}", model_id))?;
        let cpu_spec =
            ModelRegistry::runtime_spec_for_backend(&model_id, RuntimeBackendTarget::Cpu)
                .ok_or_else(|| format!("Missing CPU runtime spec for model ID: {}", model_id))?;
        cpu_spec.validate()?;

        let cpu_model_path = ModelLoader::validate_model_for_backend(
            &model_def.directory,
            ModelArtifactBackend::Cpu,
        )?;
        let dml_model_path = ModelLoader::resolve_model_file_for_backend(
            &model_def.directory,
            ModelArtifactBackend::DirectML,
        );
        let tokenizer_path = ModelLoader::tokenizer_file(&model_def.directory);

        let force_override = parse_force_override();
        let forced_device_id = parse_dml_device_id_env();
        let probe = self
            .wait_for_startup_probe(Duration::from_millis(STARTUP_PROBE_WAIT_MS))
            .await;

        let mut available_backends = probe.available_backends.clone();
        if !available_backends.contains(&InferenceBackend::Cpu) {
            available_backends.push(InferenceBackend::Cpu);
        }
        let directml_detected = probe
            .available_backends
            .contains(&InferenceBackend::DirectML)
            || force_override == Some(InferenceBackend::DirectML);
        let directml_artifact_available = dml_model_path.is_some();
        let has_dml_candidate = directml_detected && directml_artifact_available;

        let probe_device_id = probe
            .directml_candidate
            .as_ref()
            .map(|candidate| candidate.device_id);
        let mut selected_device_id = forced_device_id.or(probe_device_id);
        let selected_device_name = probe
            .directml_candidate
            .as_ref()
            .map(|candidate| candidate.device_name.clone());

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
                    *self.backend_status.lock().await = BackendStatus {
                        available_backends: available_backends.clone(),
                        selection_state: Some("error".to_string()),
                        selection_reason: Some("invalid_directml_device_id".to_string()),
                        selected_device_id: Some(forced_id),
                        selected_device_name: selected_device_name.clone(),
                        dml_gate_state: Some("error".to_string()),
                        dml_gate_reason: Some("invalid_directml_device_id".to_string()),
                        directml_probe_passed: Some(directml_detected),
                        directml_probe_error: Some(error.clone()),
                        directml_probe_at: Some(Utc::now().to_rfc3339()),
                        force_override,
                        store_path: self
                            .store_path
                            .as_ref()
                            .map(|path| path.display().to_string()),
                        ..Default::default()
                    };
                    return Err(error);
                }
                selected_device_id = probe_device_id;
            }
        }

        let adapter_identity = probe
            .directml_candidate
            .as_ref()
            .map(|candidate| candidate.adapter_identity.clone())
            .unwrap_or_else(|| "cpu-only".to_string());
        let driver_version = probe
            .directml_candidate
            .as_ref()
            .map(|candidate| candidate.driver_version.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let decision_key = BackendDecisionKey {
            model_id: model_id.clone(),
            adapter_identity,
            driver_version,
            app_version: self.app_version.clone(),
            ort_version: ORT_CRATE_VERSION.to_string(),
            directml_device_id: selected_device_id,
        };
        let stored = self.lookup_backend_record(&decision_key).await;
        let mut failure_counters = stored
            .as_ref()
            .map(|record| record.failure_counters.clone())
            .unwrap_or_default();

        let mut preferred_backend = InferenceBackend::Cpu;
        let mut decision_reason = DecisionReason::NoDirectMLCandidate;
        if let Some(override_backend) = force_override {
            preferred_backend = override_backend;
            decision_reason = DecisionReason::ForcedOverride;
        } else if failure_counters.should_demote_directml() {
            preferred_backend = InferenceBackend::Cpu;
            decision_reason = DecisionReason::DemotedAfterFailures;
        } else if let Some(record) = stored.as_ref() {
            if record.decision.backend == InferenceBackend::DirectML && has_dml_candidate {
                preferred_backend = InferenceBackend::DirectML;
                decision_reason = DecisionReason::PersistedDecision;
            } else if has_dml_candidate {
                preferred_backend = InferenceBackend::DirectML;
                decision_reason = DecisionReason::DefaultDirectMLCandidate;
            } else {
                preferred_backend = InferenceBackend::Cpu;
                decision_reason = DecisionReason::PersistedDecision;
            }
        } else if has_dml_candidate {
            preferred_backend = InferenceBackend::DirectML;
            decision_reason = DecisionReason::DefaultDirectMLCandidate;
        }

        let mut persisted_backend = preferred_backend;
        let mut persisted_reason = decision_reason.clone();
        let mut active_backend = preferred_backend;
        let mut active_reason = decision_reason.clone();
        let mut runtime_engine = "ort_cpu".to_string();
        let mut selection_state = "ready".to_string();
        let mut selection_reason = decision_reason_code(&active_reason).to_string();

        let adapter = if preferred_backend == InferenceBackend::DirectML {
            match dml_model_path.as_deref() {
                Some(dml_path) => {
                    match build_directml_runtime_adapter(dml_path, selected_device_id) {
                        Ok(adapter) => {
                            failure_counters.record_directml_success();
                            runtime_engine = "genai_dml".to_string();
                            adapter
                        }
                        Err(error) => {
                            if force_override == Some(InferenceBackend::DirectML) {
                                failure_counters.record_directml_failure(
                                    DirectMLFailureStage::Init,
                                    error.clone(),
                                );
                                *self.backend_status.lock().await = BackendStatus {
                                    available_backends: available_backends.clone(),
                                    selection_state: Some("error".to_string()),
                                    selection_reason: Some(
                                        decision_reason_code(
                                            &DecisionReason::DirectMLInitializationFailed,
                                        )
                                        .to_string(),
                                    ),
                                    selected_device_id,
                                    selected_device_name: selected_device_name.clone(),
                                    dml_gate_state: Some("error".to_string()),
                                    dml_gate_reason: Some(
                                        decision_reason_code(
                                            &DecisionReason::DirectMLInitializationFailed,
                                        )
                                        .to_string(),
                                    ),
                                    directml_probe_passed: Some(directml_detected),
                                    directml_probe_error: Some(error.clone()),
                                    directml_probe_at: Some(Utc::now().to_rfc3339()),
                                    failure_counters: failure_counters.clone(),
                                    force_override,
                                    store_path: self
                                        .store_path
                                        .as_ref()
                                        .map(|path| path.display().to_string()),
                                    ..Default::default()
                                };
                                return Err(error);
                            }

                            failure_counters
                                .record_directml_failure(DirectMLFailureStage::Init, error.clone());
                            selection_state = "fallback".to_string();
                            active_backend = InferenceBackend::Cpu;
                            active_reason = DecisionReason::DirectMLInitializationFailed;
                            persisted_backend = InferenceBackend::DirectML;
                            persisted_reason = DecisionReason::DirectMLInitializationFailed;

                            if failure_counters.should_demote_directml() {
                                failure_counters.mark_demotion();
                                persisted_backend = InferenceBackend::Cpu;
                                persisted_reason = DecisionReason::DemotedAfterFailures;
                                active_reason = DecisionReason::DemotedAfterFailures;
                            }
                            selection_reason = decision_reason_code(&active_reason).to_string();

                            let (adapter, _) = build_cpu_runtime_adapter(
                                &cpu_model_path,
                                &tokenizer_path,
                                cpu_spec,
                            )?;
                            adapter
                        }
                    }
                }
                None => {
                    let error =
                        "DirectML model artifact missing (expected models/<model>/dml/model.onnx)"
                            .to_string();
                    if force_override == Some(InferenceBackend::DirectML) {
                        failure_counters
                            .record_directml_failure(DirectMLFailureStage::Init, error.clone());
                        *self.backend_status.lock().await = BackendStatus {
                            available_backends: available_backends.clone(),
                            selection_state: Some("error".to_string()),
                            selection_reason: Some("directml_artifact_missing".to_string()),
                            selected_device_id,
                            selected_device_name: selected_device_name.clone(),
                            dml_gate_state: Some("artifact_missing".to_string()),
                            dml_gate_reason: Some("directml_artifact_missing".to_string()),
                            directml_probe_passed: Some(directml_detected),
                            directml_probe_error: if directml_detected {
                                None
                            } else {
                                Some("No DirectML-capable adapter detected".to_string())
                            },
                            directml_probe_at: Some(Utc::now().to_rfc3339()),
                            failure_counters: failure_counters.clone(),
                            force_override,
                            store_path: self
                                .store_path
                                .as_ref()
                                .map(|path| path.display().to_string()),
                            ..Default::default()
                        };
                        return Err(error);
                    }
                    let (adapter, _) =
                        build_cpu_runtime_adapter(&cpu_model_path, &tokenizer_path, cpu_spec)?;
                    active_backend = InferenceBackend::Cpu;
                    active_reason = DecisionReason::NoDirectMLCandidate;
                    persisted_backend = InferenceBackend::Cpu;
                    persisted_reason = DecisionReason::NoDirectMLCandidate;
                    selection_state = "fallback".to_string();
                    selection_reason = decision_reason_code(&active_reason).to_string();
                    adapter
                }
            }
        } else {
            let (adapter, _) =
                build_cpu_runtime_adapter(&cpu_model_path, &tokenizer_path, cpu_spec)?;
            adapter
        };

        let active_model_path = if active_backend == InferenceBackend::DirectML {
            dml_model_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| cpu_model_path.display().to_string())
        } else {
            cpu_model_path.display().to_string()
        };

        *self.runtime_adapter.lock().await = Some(adapter);
        *self.current_model.lock().await = Some(model_id.clone());
        *self.backend_status.lock().await = BackendStatus {
            active_backend: Some(active_backend),
            active_model_path: Some(active_model_path),
            active_artifact_backend: Some(active_backend),
            runtime_engine: Some(runtime_engine),
            available_backends,
            selection_state: Some(selection_state),
            selection_reason: Some(selection_reason),
            selected_device_id,
            selected_device_name,
            dml_gate_state: Some(if active_backend == InferenceBackend::DirectML {
                "selected".to_string()
            } else if directml_detected && !directml_artifact_available {
                "artifact_missing".to_string()
            } else if has_dml_candidate {
                "fallback_cpu".to_string()
            } else {
                "cpu_only".to_string()
            }),
            dml_gate_reason: Some(decision_reason_code(&persisted_reason).to_string()),
            decision_key: Some(decision_key.clone()),
            last_decision: Some(BackendDecision::new(active_backend, active_reason, None)),
            directml_probe_passed: Some(directml_detected),
            directml_probe_error: if directml_detected {
                None
            } else {
                Some("No DirectML-capable adapter detected".to_string())
            },
            directml_probe_at: Some(Utc::now().to_rfc3339()),
            failure_counters: failure_counters.clone(),
            force_override,
            store_path: self
                .store_path
                .as_ref()
                .map(|path| path.display().to_string()),
            ..Default::default()
        };

        if force_override.is_none() {
            self.persist_backend_record(
                decision_key,
                BackendDecision::new(persisted_backend, persisted_reason, None),
                failure_counters,
            )
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
        status.selection_state = Some("ready".to_string());
        status.selection_reason = Some("model_unloaded".to_string());
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
        let Some(cpu_spec) =
            ModelRegistry::runtime_spec_for_backend(&model_id, RuntimeBackendTarget::Cpu)
        else {
            return;
        };
        if cpu_spec.validate().is_err() {
            return;
        }

        let cpu_model_path = match ModelLoader::validate_model_for_backend(
            &model_def.directory,
            ModelArtifactBackend::Cpu,
        ) {
            Ok(path) => path,
            Err(_) => return,
        };
        let tokenizer_path = ModelLoader::tokenizer_file(&model_def.directory);
        let Ok((cpu_adapter, _)) =
            build_cpu_runtime_adapter(&cpu_model_path, &tokenizer_path, cpu_spec)
        else {
            return;
        };

        *self.runtime_adapter.lock().await = Some(cpu_adapter);
        let mut counters = status_snapshot.failure_counters.clone();
        counters.record_directml_failure(DirectMLFailureStage::Runtime, error.to_string());
        let mut persisted_backend = InferenceBackend::DirectML;
        let mut persisted_reason = DecisionReason::RuntimeFailureFallback;
        let mut decision_reason = DecisionReason::RuntimeFailureFallback;
        if counters.should_demote_directml() {
            counters.mark_demotion();
            persisted_backend = InferenceBackend::Cpu;
            persisted_reason = DecisionReason::DemotedAfterFailures;
            decision_reason = DecisionReason::DemotedAfterFailures;
        }

        let mut updated = status_snapshot.clone();
        updated.active_backend = Some(InferenceBackend::Cpu);
        updated.active_artifact_backend = Some(InferenceBackend::Cpu);
        updated.runtime_engine = Some("ort_cpu".to_string());
        updated.active_model_path = Some(cpu_model_path.display().to_string());
        updated.selection_state = Some("fallback".to_string());
        updated.selection_reason = Some(decision_reason_code(&decision_reason).to_string());
        updated.dml_gate_state = Some("fallback_cpu".to_string());
        updated.dml_gate_reason = Some(decision_reason_code(&persisted_reason).to_string());
        updated.last_decision = Some(BackendDecision::new(
            InferenceBackend::Cpu,
            decision_reason,
            None,
        ));
        updated.failure_counters = counters.clone();
        updated.force_override = parse_force_override();
        *self.backend_status.lock().await = updated;

        if let Some(decision_key) = status_snapshot.decision_key {
            self.persist_backend_record(
                decision_key,
                BackendDecision::new(persisted_backend, persisted_reason, None),
                counters,
            )
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

    fn cancel(&self) {
        if let Some(token) = lock_cancel(&self.active_cancel).clone() {
            token.store(true, Ordering::SeqCst);
        }
    }
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

fn build_cpu_runtime_adapter(
    model_path: &Path,
    tokenizer_path: &Path,
    runtime_spec: ModelRuntimeSpec,
) -> Result<
    (
        InferenceRuntimeAdapter,
        smolpc_engine_core::inference::types::ModelInfo,
    ),
    String,
> {
    let session = InferenceSession::new_with_backend_options(
        model_path,
        InferenceBackend::Cpu,
        SessionBackendOptions::default(),
    )?;
    let session_info = session.info();
    let tokenizer =
        TokenizerWrapper::from_file_with_stop_tokens(tokenizer_path, runtime_spec.stop_token_ids)?;
    let generator = Generator::new(session, tokenizer, runtime_spec)?;
    Ok((InferenceRuntimeAdapter::ort(generator), session_info))
}

#[cfg(target_os = "windows")]
fn build_directml_runtime_adapter(
    dml_model_path: &Path,
    directml_device_id: Option<i32>,
) -> Result<InferenceRuntimeAdapter, String> {
    let model_dir = dml_model_path
        .parent()
        .ok_or_else(|| format!("Invalid DirectML model path: {}", dml_model_path.display()))?;
    let generator = GenAiDirectMlGenerator::new(model_dir, directml_device_id)?;
    generator.run_preflight("Warmup preflight")?;
    Ok(InferenceRuntimeAdapter::genai_directml(generator))
}

#[cfg(not(target_os = "windows"))]
fn build_directml_runtime_adapter(
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
    if value != format!("Bearer {}", token) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        ));
    }
    Ok(())
}

fn request_to_prompt(messages: &[ChatCompletionMessage]) -> Result<String, String> {
    if messages.is_empty() {
        return Err("messages cannot be empty".to_string());
    }
    let mut prompt = String::new();
    for m in messages {
        let content = m.content.clone().unwrap_or_default();
        if !content.is_empty() {
            prompt.push_str(&m.role);
            prompt.push_str(": ");
            prompt.push_str(&content);
            prompt.push('\n');
        }
    }
    prompt.push_str("assistant: ");
    Ok(prompt)
}

fn request_to_config(request: &ChatCompletionRequest) -> Option<GenerationConfig> {
    let mut c = GenerationConfig::default();
    let mut changed = false;
    if let Some(v) = request.max_tokens {
        c.max_length = v;
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
        Some(c)
    } else {
        None
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
        "protocol_version": "1.0.0",
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
    let current_model = state.engine.current_model.lock().await.clone();
    let backend_status = state.engine.backend_status.lock().await.clone();
    Ok(Json(serde_json::json!({
        "ok": true,
        "current_model": current_model,
        "generating": state.engine.generating.load(Ordering::SeqCst),
        "backend_status": backend_status,
    })))
}

async fn load(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<LoadRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    state.engine.load_model(req.model_id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e }),
        )
    })?;
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
    let exists = ModelRegistry::get_model(&req.model_id)
        .map(|m| {
            let (me, te) = ModelLoader::check_model_files(&m.directory);
            me && te
        })
        .unwrap_or(false);
    Ok(Json(serde_json::json!({"exists": exists})))
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

    let prompt = request_to_prompt(&req.messages)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;
    let config = request_to_config(&req);
    let model_name = req.model.unwrap_or_else(|| "smolpc-engine".to_string());
    let request_id = format!("chatcmpl-{}", Utc::now().timestamp_millis());
    let created = Utc::now().timestamp();

    if req.stream.unwrap_or(false) {
        let (tx, mut rx) = mpsc::unbounded_channel::<StreamMessage>();
        let engine = state.engine.clone();
        let activity = state.last_activity_ms.clone();
        tokio::spawn(async move {
            let _permit = gen_permit;
            let result = engine
                .generate_stream(&prompt, config, |t| {
                    let _ = tx.send(StreamMessage::Token(t));
                })
                .await;
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

    let result = state
        .engine
        .generate_text(&prompt, config)
        .await
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
    init_onnx_runtime(args.resource_dir.as_deref())
        .map_err(|e| format!("ONNX Runtime init failed: {e}"))?;

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
            if idle_ms >= args.model_idle_unload.as_millis() as u64
                && !idle_state.engine.generating.load(Ordering::SeqCst)
                && idle_state.engine.current_model.lock().await.is_some()
            {
                let _ = idle_state.engine.unload_model(false).await;
            }
            if idle_ms >= args.process_idle_exit.as_millis() as u64
                && !idle_state.engine.generating.load(Ordering::SeqCst)
            {
                idle_state.shutdown.notify_waiters();
                break;
            }
        }
    });

    let app = Router::new()
        .route("/engine/health", get(health))
        .route("/engine/meta", get(meta))
        .route("/engine/status", get(status))
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
