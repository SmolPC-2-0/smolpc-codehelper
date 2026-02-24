/// Tauri commands for ONNX Runtime inference
///
/// Provides IPC interface between frontend and inference engine.
use crate::commands::hardware::HardwareCache;
use crate::hardware::types::HardwareInfo;
use crate::inference::backend::{
    BackendBenchmark, BackendBenchmarkComparison, BackendDecision, BackendDecisionKey, BackendStatus,
    DecisionReason, DirectMLFailureStage, InferenceBackend,
    BENCHMARK_SELECTION_BUDGET_MS, DIRECTML_MIN_DECODE_SPEEDUP_RATIO,
    DIRECTML_MAX_TTFT_REGRESSION_RATIO, ORT_CRATE_VERSION,
};
use crate::inference::backend_store::{backend_store_path, BackendDecisionRecord, BackendStore};
use crate::inference::types::{GenerationConfig, GenerationMetrics, GenerationResult};
use crate::inference::{Generator, InferenceSession, TokenizerWrapper};
use crate::models::{ModelLoader, ModelRegistry, ModelRuntimeSpec};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::ipc::Channel;
use tauri::State;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

const ERR_GENERATION_IN_PROGRESS: &str = "Generation already in progress";
const ERR_GENERATION_CANCELLED: &str = "Generation cancelled";
const ERR_CODE_GENERATION_CANCELLED: &str = "INFERENCE_GENERATION_CANCELLED";
const ERR_MODEL_CHANGE_DURING_GENERATION: &str =
    "Cannot load or unload model while generation is in progress";
const BENCHMARK_PROMPT: &str = "Write a short Rust function that adds two integers.";
const BENCHMARK_MAX_NEW_TOKENS: usize = 8;

fn generation_cancelled_error() -> String {
    format!("{ERR_CODE_GENERATION_CANCELLED}: {ERR_GENERATION_CANCELLED}")
}

fn lock_active_cancel_recover<'a>(
    active_cancel: &'a StdMutex<Option<Arc<AtomicBool>>>,
    context: &str,
) -> std::sync::MutexGuard<'a, Option<Arc<AtomicBool>>> {
    match active_cancel.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::warn!(
                "Recovering from poisoned active cancellation mutex in {context}; continuing with recovered state"
            );
            poisoned.into_inner()
        }
    }
}

/// Global inference state (managed by Tauri)
pub struct InferenceState {
    /// Current generator instance (None if no model loaded)
    generator: Arc<Mutex<Option<Generator>>>,

    /// Currently loaded model ID
    current_model: Arc<Mutex<Option<String>>>,

    /// Cancellation token for the currently active generation (if any)
    active_cancel: Arc<StdMutex<Option<Arc<AtomicBool>>>>,

    /// Whether generation is currently in progress (explicit flag, no TOCTOU race)
    generating: Arc<AtomicBool>,

    /// Backend used by currently loaded model (if any)
    active_backend: Arc<Mutex<Option<InferenceBackend>>>,

    /// Backend diagnostics and persisted decision metadata for current model key.
    backend_status: Arc<Mutex<BackendStatus>>,
}

impl Default for InferenceState {
    fn default() -> Self {
        Self {
            generator: Arc::new(Mutex::new(None)),
            current_model: Arc::new(Mutex::new(None)),
            active_cancel: Arc::new(StdMutex::new(None)),
            generating: Arc::new(AtomicBool::new(false)),
            active_backend: Arc::new(Mutex::new(None)),
            backend_status: Arc::new(Mutex::new(BackendStatus::default())),
        }
    }
}

/// RAII guard for a single active generation.
///
/// When dropped, this guard always clears generation state and active cancellation token.
struct GenerationPermit {
    generating: Arc<AtomicBool>,
    active_cancel: Arc<StdMutex<Option<Arc<AtomicBool>>>>,
}

impl Drop for GenerationPermit {
    fn drop(&mut self) {
        self.generating.store(false, Ordering::SeqCst);
        let mut active_cancel =
            lock_active_cancel_recover(&self.active_cancel, "GenerationPermit::drop");
        *active_cancel = None;
    }
}

impl InferenceState {
    fn try_begin_generation(&self) -> Result<(GenerationPermit, Arc<AtomicBool>), String> {
        if self
            .generating
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(ERR_GENERATION_IN_PROGRESS.to_string());
        }

        let cancel_token = Arc::new(AtomicBool::new(false));
        let mut active_cancel =
            lock_active_cancel_recover(&self.active_cancel, "InferenceState::try_begin_generation");
        *active_cancel = Some(Arc::clone(&cancel_token));

        Ok((
            GenerationPermit {
                generating: Arc::clone(&self.generating),
                active_cancel: Arc::clone(&self.active_cancel),
            },
            cancel_token,
        ))
    }
}

fn load_session_with_fallback(
    model_path: &Path,
    preferred_backend: InferenceBackend,
) -> Result<(InferenceSession, InferenceBackend, Option<String>), String> {
    match preferred_backend {
        InferenceBackend::Cpu => InferenceSession::new_with_backend(model_path, InferenceBackend::Cpu)
            .map(|session| (session, InferenceBackend::Cpu, None)),
        InferenceBackend::DirectML => {
            match InferenceSession::new_with_backend(model_path, InferenceBackend::DirectML) {
                Ok(session) => Ok((session, InferenceBackend::DirectML, None)),
                Err(dml_error) => {
                    log::warn!(
                        "DirectML session initialization failed (falling back to CPU): {}",
                        dml_error
                    );
                    let cpu_session = InferenceSession::new_with_backend(model_path, InferenceBackend::Cpu)?;
                    Ok((cpu_session, InferenceBackend::Cpu, Some(dml_error)))
                }
            }
        }
    }
}

fn parse_force_backend_override() -> Option<InferenceBackend> {
    let raw = std::env::var("SMOLPC_FORCE_EP").ok()?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "cpu" => Some(InferenceBackend::Cpu),
        "dml" | "directml" => Some(InferenceBackend::DirectML),
        invalid => {
            log::warn!(
                "Ignoring invalid SMOLPC_FORCE_EP value '{}'. Supported values: cpu|dml",
                invalid
            );
            None
        }
    }
}

fn ort_version_key() -> String {
    format!(
        "ort-crate:{}|onnxruntime-1.{}.x",
        ORT_CRATE_VERSION,
        ort::MINOR_VERSION
    )
}

fn pick_adapter_identity(hardware: Option<&HardwareInfo>) -> (String, String, bool) {
    if let Some(hw) = hardware {
        if let Some(dml_gpu) = hw
            .gpus
            .iter()
            .find(|gpu| gpu.backend.eq_ignore_ascii_case("DirectX 12"))
        {
            let adapter = format!(
                "{:?}:{}:{}",
                dml_gpu.vendor,
                dml_gpu.name,
                dml_gpu
                    .pci_device_id
                    .clone()
                    .unwrap_or_else(|| "unknown-pci".to_string())
            );
            let driver_version = dml_gpu
                .driver_version
                .clone()
                .unwrap_or_else(|| "unknown-driver".to_string());
            return (adapter, driver_version, true);
        }
        if let Some(first_gpu) = hw.gpus.first() {
            let adapter = format!(
                "{:?}:{}:{}",
                first_gpu.vendor,
                first_gpu.name,
                first_gpu
                    .pci_device_id
                    .clone()
                    .unwrap_or_else(|| "unknown-pci".to_string())
            );
            let driver_version = first_gpu
                .driver_version
                .clone()
                .unwrap_or_else(|| "unknown-driver".to_string());
            return (adapter, driver_version, false);
        }
    }
    (
        "unknown-adapter".to_string(),
        "unknown-driver".to_string(),
        false,
    )
}

fn make_decision_key(
    model_id: &str,
    app_handle: &tauri::AppHandle,
    hardware: Option<&HardwareInfo>,
) -> BackendDecisionKey {
    let (adapter_identity, driver_version, _) = pick_adapter_identity(hardware);
    BackendDecisionKey {
        model_id: model_id.to_string(),
        adapter_identity,
        driver_version,
        app_version: app_handle.package_info().version.to_string(),
        ort_version: ort_version_key(),
    }
}

async fn run_backend_benchmark(
    model_path: &Path,
    tokenizer_path: &Path,
    runtime_spec: ModelRuntimeSpec,
    backend: InferenceBackend,
) -> Result<BackendBenchmark, String> {
    let session = InferenceSession::new_with_backend(model_path, backend)?;
    let tokenizer =
        TokenizerWrapper::from_file_with_stop_tokens(tokenizer_path, runtime_spec.stop_token_ids)?;
    let generator = Generator::new(session, tokenizer, runtime_spec)?;
    let config = GenerationConfig {
        max_length: BENCHMARK_MAX_NEW_TOKENS,
        temperature: 0.0,
        top_k: Some(1),
        top_p: None,
        ..Default::default()
    };
    let cancelled = Arc::new(AtomicBool::new(false));
    let metrics = generator
        .generate_stream(BENCHMARK_PROMPT, Some(config), cancelled, |_token| {})
        .await?;

    let ttft_ms = metrics
        .time_to_first_token_ms
        .unwrap_or(metrics.total_time_ms);
    let decode_tokens_per_second = if metrics.total_tokens <= 1 || metrics.total_time_ms <= ttft_ms
    {
        0.0
    } else {
        (metrics.total_tokens - 1) as f64
            / ((metrics.total_time_ms - ttft_ms) as f64 / 1_000.0)
    };

    Ok(BackendBenchmark {
        backend,
        sample_tokens: metrics.total_tokens,
        total_time_ms: metrics.total_time_ms,
        time_to_first_token_ms: ttft_ms,
        overall_tokens_per_second: metrics.tokens_per_second,
        decode_tokens_per_second,
    })
}

async fn run_selection_benchmark(
    model_path: &Path,
    tokenizer_path: &Path,
    runtime_spec: ModelRuntimeSpec,
) -> Result<BackendBenchmarkComparison, String> {
    let started = std::time::Instant::now();
    let cpu = run_backend_benchmark(model_path, tokenizer_path, runtime_spec, InferenceBackend::Cpu)
        .await?;
    let directml = run_backend_benchmark(
        model_path,
        tokenizer_path,
        runtime_spec,
        InferenceBackend::DirectML,
    )
    .await?;

    Ok(BackendBenchmarkComparison {
        cpu,
        directml,
        elapsed_ms: started.elapsed().as_millis() as u64,
        budget_ms: BENCHMARK_SELECTION_BUDGET_MS,
    })
}

fn select_backend(
    force_override: Option<InferenceBackend>,
    persisted_record: Option<&BackendDecisionRecord>,
    directml_candidate_available: bool,
    benchmark: Option<&BackendBenchmarkComparison>,
) -> (InferenceBackend, DecisionReason) {
    if let Some(forced) = force_override {
        return (forced, DecisionReason::ForcedOverride);
    }

    if let Some(record) = persisted_record {
        if record.decision.backend == InferenceBackend::DirectML
            && record.failure_counters.should_demote_directml()
        {
            return (InferenceBackend::Cpu, DecisionReason::DemotedAfterFailures);
        }
        return (record.decision.backend, DecisionReason::PersistedDecision);
    }

    if !directml_candidate_available {
        return (InferenceBackend::Cpu, DecisionReason::NoDirectMLCandidate);
    }

    if let Some(comparison) = benchmark {
        if comparison.directml_passes_gate() {
            return (InferenceBackend::DirectML, DecisionReason::BenchmarkPassed);
        }
        if comparison.directml_decode_speedup_ratio() < DIRECTML_MIN_DECODE_SPEEDUP_RATIO {
            return (InferenceBackend::Cpu, DecisionReason::BenchmarkDecodeTooSlow);
        }
        if comparison.directml_ttft_ratio() > DIRECTML_MAX_TTFT_REGRESSION_RATIO {
            return (InferenceBackend::Cpu, DecisionReason::BenchmarkTtftTooHigh);
        }
    }

    (InferenceBackend::Cpu, DecisionReason::DefaultCpu)
}

async fn persist_backend_status_snapshot(state: &InferenceState) -> Result<(), String> {
    let snapshot = {
        let status = state.backend_status.lock().await;
        (
            status.store_path.clone(),
            status.decision_key.clone(),
            status.last_decision.clone(),
            status.failure_counters.clone(),
        )
    };

    let (store_path, key, decision, counters) = match snapshot {
        (Some(path), Some(key), Some(decision), counters) => (path, key, decision, counters),
        _ => return Ok(()),
    };

    let mut store = BackendStore::load(PathBuf::from(store_path))?;
    store.upsert(BackendDecisionRecord {
        key,
        decision,
        failure_counters: counters,
        updated_at: Utc::now().to_rfc3339(),
    });
    store.persist()
}

async fn reset_directml_failures_on_success(state: &InferenceState) {
    let active_backend = *state.active_backend.lock().await;
    if active_backend != Some(InferenceBackend::DirectML) {
        return;
    }

    let changed = {
        let mut status = state.backend_status.lock().await;
        let before = status.failure_counters.directml_consecutive_failures;
        status.failure_counters.record_directml_success();
        before != status.failure_counters.directml_consecutive_failures
    };

    if changed {
        if let Err(e) = persist_backend_status_snapshot(state).await {
            log::warn!("Failed to persist DirectML success reset: {e}");
        }
    }
}

async fn reload_loaded_model_on_cpu(state: &InferenceState) -> Result<(), String> {
    let model_id = state
        .current_model
        .lock()
        .await
        .clone()
        .ok_or_else(|| "No current model to reload for CPU demotion".to_string())?;

    let model_def = ModelRegistry::get_model(&model_id)
        .ok_or_else(|| format!("Unknown model ID during CPU demotion reload: {}", model_id))?;
    let runtime_spec = ModelRegistry::runtime_spec(&model_id)
        .ok_or_else(|| format!("Runtime spec missing during CPU demotion reload: {}", model_id))?;
    runtime_spec.validate()?;

    let model_path = ModelLoader::model_file(&model_def.directory);
    let tokenizer_path = ModelLoader::tokenizer_file(&model_def.directory);
    let session = InferenceSession::new_with_backend(&model_path, InferenceBackend::Cpu)?;
    let tokenizer =
        TokenizerWrapper::from_file_with_stop_tokens(&tokenizer_path, runtime_spec.stop_token_ids)?;
    let generator = Generator::new(session, tokenizer, runtime_spec)?;

    *state.generator.lock().await = Some(generator);
    *state.active_backend.lock().await = Some(InferenceBackend::Cpu);
    {
        let mut status = state.backend_status.lock().await;
        status.active_backend = Some(InferenceBackend::Cpu);
    }

    log::warn!("Inference backend demoted to CPU for model '{}'", model_id);
    Ok(())
}

async fn record_directml_runtime_failure(state: &InferenceState, error_message: &str) {
    let active_backend = *state.active_backend.lock().await;
    if active_backend != Some(InferenceBackend::DirectML) {
        return;
    }

    let mut should_demote = false;
    {
        let mut status = state.backend_status.lock().await;
        status
            .failure_counters
            .record_directml_failure(DirectMLFailureStage::Runtime, error_message);
        log::warn!(
            "DirectML runtime failure recorded: consecutive_failures={} reason={}",
            status.failure_counters.directml_consecutive_failures,
            error_message
        );

        if status.failure_counters.should_demote_directml() {
            status.failure_counters.mark_demotion();
            let benchmark = status
                .last_decision
                .as_ref()
                .and_then(|d| d.benchmark.clone());
            status.last_decision = Some(BackendDecision::new(
                InferenceBackend::Cpu,
                DecisionReason::DemotedAfterFailures,
                benchmark,
            ));
            should_demote = true;
        }
    }

    if let Err(e) = persist_backend_status_snapshot(state).await {
        log::warn!("Failed to persist backend status after runtime failure: {e}");
    }

    if should_demote {
        if let Err(e) = reload_loaded_model_on_cpu(state).await {
            log::error!("Failed to reload model on CPU after DirectML demotion: {e}");
        }
    }
}

/// Load a model and initialize the inference engine
///
/// # Arguments
/// * `model_id` - Model ID from registry (e.g., "qwen2.5-coder-1.5b")
///
/// # Returns
/// Success message with model info
#[tauri::command]
pub async fn load_model(
    model_id: String,
    app_handle: tauri::AppHandle,
    hardware_cache: State<'_, HardwareCache>,
    state: State<'_, InferenceState>,
) -> Result<String, String> {
    if state.generating.load(Ordering::SeqCst) {
        return Err(ERR_MODEL_CHANGE_DURING_GENERATION.to_string());
    }

    log::info!("Loading model: {}", model_id);

    // Validate model exists in registry
    let model_def = ModelRegistry::get_model(&model_id)
        .ok_or_else(|| format!("Unknown model ID: {}", model_id))?;
    let runtime_spec = ModelRegistry::runtime_spec(&model_id)
        .ok_or_else(|| format!("Runtime spec not implemented for model ID: {}", model_id))?;
    runtime_spec
        .validate()
        .map_err(|e| format!("Invalid runtime spec for '{}': {}", model_id, e))?;

    log::info!("Model definition: {} ({})", model_def.name, model_def.size);

    // Validate model files exist
    ModelLoader::validate_model(&model_def.directory)?;

    // Get file paths
    let model_path = ModelLoader::model_file(&model_def.directory);
    let tokenizer_path = ModelLoader::tokenizer_file(&model_def.directory);

    log::info!("Model path: {}", model_path.display());
    log::info!("Tokenizer path: {}", tokenizer_path.display());

    let hardware_info = match hardware_cache.get_or_detect().await {
        Ok(info) => Some(info),
        Err(e) => {
            log::warn!("Hardware detection unavailable for backend selection: {e}");
            None
        }
    };
    let (_, _, directml_candidate_available) = pick_adapter_identity(hardware_info.as_deref());
    let force_override = parse_force_backend_override();
    let decision_key = make_decision_key(&model_id, &app_handle, hardware_info.as_deref());
    let store_path = backend_store_path(&app_handle)?;
    let mut store = BackendStore::load(&store_path)?;

    let invalidated = store.remove_stale_for_model(&decision_key);
    if invalidated > 0 {
        log::info!(
            "Invalidated {} stale backend decision(s) for model '{}' due to decision key change",
            invalidated,
            model_id
        );
        store.persist()?;
    }

    let persisted_record = store.get(&decision_key).cloned();
    let mut failure_counters = persisted_record
        .as_ref()
        .map(|record| record.failure_counters.clone())
        .unwrap_or_default();

    let (benchmark_result, benchmark_timed_out) = if force_override.is_none()
        && persisted_record.is_none()
        && directml_candidate_available
        && cfg!(target_os = "windows")
    {
        match timeout(
            Duration::from_millis(BENCHMARK_SELECTION_BUDGET_MS),
            run_selection_benchmark(&model_path, &tokenizer_path, runtime_spec),
        )
        .await
        {
            Ok(Ok(comparison)) => {
                log::info!(
                    "Backend benchmark result: cpu_decode_tps={:.3}, dml_decode_tps={:.3}, speedup_ratio={:.3}, cpu_ttft_ms={}, dml_ttft_ms={}, ttft_ratio={:.3}, elapsed_ms={}",
                    comparison.cpu.decode_tokens_per_second,
                    comparison.directml.decode_tokens_per_second,
                    comparison.directml_decode_speedup_ratio(),
                    comparison.cpu.time_to_first_token_ms,
                    comparison.directml.time_to_first_token_ms,
                    comparison.directml_ttft_ratio(),
                    comparison.elapsed_ms
                );
                (Some(comparison), false)
            }
            Ok(Err(e)) => {
                log::warn!("Backend benchmark failed; defaulting to CPU: {e}");
                (None, false)
            }
            Err(_) => {
                log::warn!(
                    "Backend benchmark exceeded {}ms budget; defaulting to CPU",
                    BENCHMARK_SELECTION_BUDGET_MS
                );
                (None, true)
            }
        }
    } else {
        (None, false)
    };

    let (preferred_backend, base_reason) = if benchmark_timed_out {
        (InferenceBackend::Cpu, DecisionReason::BenchmarkBudgetExceeded)
    } else {
        select_backend(
            force_override,
            persisted_record.as_ref(),
            directml_candidate_available && cfg!(target_os = "windows"),
            benchmark_result.as_ref(),
        )
    };

    log::info!(
        "Backend selector candidate ranking: model={}, adapter={}, force_override={:?}, persisted={}, directml_candidate={}, selected_preferred={:?}, reason={:?}",
        model_id,
        decision_key.adapter_identity,
        force_override,
        persisted_record.is_some(),
        directml_candidate_available,
        preferred_backend,
        base_reason
    );

    let initial_decision =
        BackendDecision::new(preferred_backend, base_reason, benchmark_result.clone());
    let (session, active_backend, fallback_reason) =
        load_session_with_fallback(&model_path, preferred_backend)?;
    let session_info = session.info();

    let final_reason = if preferred_backend == InferenceBackend::DirectML
        && active_backend == InferenceBackend::Cpu
    {
        let failure_reason = fallback_reason
            .clone()
            .unwrap_or_else(|| "Unknown DirectML initialization error".to_string());
        failure_counters.record_directml_failure(DirectMLFailureStage::Init, failure_reason);
        if failure_counters.should_demote_directml() {
            failure_counters.mark_demotion();
            DecisionReason::DemotedAfterFailures
        } else {
            DecisionReason::DirectMLInitializationFailed
        }
    } else {
        if active_backend == InferenceBackend::DirectML {
            failure_counters.record_directml_success();
        }
        initial_decision.reason.clone()
    };

    let final_decision = BackendDecision::new(active_backend, final_reason, benchmark_result);

    if let Some(reason) = fallback_reason {
        log::warn!(
            "Backend fallback triggered in load flow: preferred={}, active={}, reason={}",
            preferred_backend.as_str(),
            active_backend.as_str(),
            reason
        );
    }

    store.upsert(BackendDecisionRecord {
        key: decision_key.clone(),
        decision: final_decision.clone(),
        failure_counters: failure_counters.clone(),
        updated_at: Utc::now().to_rfc3339(),
    });
    store.persist()?;

    log::info!("Session loaded - Inputs: {:?}", session_info.inputs);
    log::info!("Session loaded - Outputs: {:?}", session_info.outputs);
    log::info!(
        "Session backend active: {} (reason: {:?})",
        active_backend.as_str(),
        final_decision.reason
    );

    // Load tokenizer
    let tokenizer =
        TokenizerWrapper::from_file_with_stop_tokens(&tokenizer_path, runtime_spec.stop_token_ids)?;
    log::info!("Tokenizer loaded - Vocab size: {}", tokenizer.vocab_size());

    // Create generator
    let generator = Generator::new(session, tokenizer, runtime_spec)?;

    // Store in state
    let mut gen_state = state.generator.lock().await;
    *gen_state = Some(generator);

    let mut current_model = state.current_model.lock().await;
    *current_model = Some(model_id.clone());

    let mut backend_state = state.active_backend.lock().await;
    *backend_state = Some(active_backend);

    let mut backend_status = state.backend_status.lock().await;
    *backend_status = BackendStatus {
        active_backend: Some(active_backend),
        decision_key: Some(decision_key),
        last_decision: Some(final_decision),
        failure_counters,
        force_override,
        store_path: Some(store_path.display().to_string()),
    };

    log::info!("Model loaded successfully: {}", model_id);

    Ok(format!(
        "Model loaded: {} ({} parameters)",
        model_def.name, model_def.size
    ))
}

/// Unload the current model and free memory
#[tauri::command]
pub async fn unload_model(state: State<'_, InferenceState>) -> Result<String, String> {
    if state.generating.load(Ordering::SeqCst) {
        return Err(ERR_MODEL_CHANGE_DURING_GENERATION.to_string());
    }

    let mut gen_state = state.generator.lock().await;
    *gen_state = None;

    let mut current_model = state.current_model.lock().await;
    *current_model = None;

    let mut backend_state = state.active_backend.lock().await;
    *backend_state = None;

    let mut backend_status = state.backend_status.lock().await;
    backend_status.active_backend = None;

    log::info!("Model unloaded");
    Ok("Model unloaded successfully".to_string())
}

/// Generate text from a prompt
///
/// # Phase 0
/// - Non-streaming: Returns full result when complete
/// - Greedy sampling only
///
/// # Phase 1
/// - Will add streaming via Tauri events
/// - Will add cancellation support
///
/// # Arguments
/// * `prompt` - Input text prompt
///
/// # Returns
/// Generated text and performance metrics
#[tauri::command]
pub async fn generate_text(
    prompt: String,
    state: State<'_, InferenceState>,
) -> Result<GenerationResult, String> {
    let (_permit, cancelled) = state.try_begin_generation()?;

    log::info!(
        "Starting generation (prompt length: {} chars)",
        prompt.len()
    );

    let mut generated_text = String::new();
    let metrics_result = {
        let gen_state = state.generator.lock().await;
        let generator = gen_state
            .as_ref()
            .ok_or("No model loaded. Call load_model first.")?;
        generator
            .generate_stream(&prompt, None, Arc::clone(&cancelled), |token| {
                generated_text.push_str(&token);
            })
            .await
    };

    let metrics = match metrics_result {
        Ok(metrics) => {
            reset_directml_failures_on_success(&state).await;
            metrics
        }
        Err(e) => {
            record_directml_runtime_failure(&state, &e).await;
            return Err(e);
        }
    };

    if cancelled.load(Ordering::SeqCst) {
        log::info!("Generation was cancelled");
        return Err(generation_cancelled_error());
    }

    log::info!(
        "Generation complete: {} tokens, {:.2} tok/s",
        metrics.total_tokens,
        metrics.tokens_per_second
    );

    Ok(GenerationResult {
        text: generated_text,
        metrics,
    })
}

/// Get list of available models
#[tauri::command]
pub fn list_models() -> Vec<crate::models::registry::ModelDefinition> {
    ModelRegistry::available_models()
}

/// Get currently loaded model ID
#[tauri::command]
pub async fn get_current_model(state: State<'_, InferenceState>) -> Result<Option<String>, String> {
    let current_model = state.current_model.lock().await;
    Ok(current_model.clone())
}

/// Get current backend diagnostics and selection metadata.
#[tauri::command]
pub async fn get_inference_backend_status(
    state: State<'_, InferenceState>,
) -> Result<BackendStatus, String> {
    let status = state.backend_status.lock().await;
    Ok(status.clone())
}

/// Check if model files exist locally
#[tauri::command]
pub fn check_model_exists(model_id: String) -> Result<bool, String> {
    let model_def = ModelRegistry::get_model(&model_id)
        .ok_or_else(|| format!("Unknown model ID: {}", model_id))?;

    let (model_exists, tokenizer_exists) = ModelLoader::check_model_files(&model_def.directory);

    Ok(model_exists && tokenizer_exists)
}

/// Generate text with streaming output via Tauri Channel
///
/// Tokens are streamed to the frontend via the `on_token` Channel.
/// The command returns `GenerationMetrics` directly when generation completes.
///
/// # Arguments
/// * `prompt` - Input text prompt
/// * `config` - Optional generation configuration (temperature, top_k, etc.)
/// * `on_token` - Channel for streaming tokens to frontend
#[tauri::command]
pub async fn inference_generate(
    prompt: String,
    config: Option<GenerationConfig>,
    on_token: Channel<String>,
    state: State<'_, InferenceState>,
) -> Result<GenerationMetrics, String> {
    let (_permit, cancelled) = state.try_begin_generation()?;

    log::info!(
        "Starting streaming generation (prompt length: {} chars)",
        prompt.len()
    );

    // Clone channel for use in closure (Channel is Clone + Send)
    let token_channel = on_token.clone();

    // Generate with streaming callback — tokens sent via Channel
    let result = {
        let gen_state = state.generator.lock().await;
        let generator = match gen_state.as_ref() {
            Some(g) => g,
            None => return Err("No model loaded. Call load_model first.".to_string()),
        };
        generator
            .generate_stream(&prompt, config, Arc::clone(&cancelled), move |token| {
                if let Err(e) = token_channel.send(token) {
                    log::warn!("Failed to send token via channel: {}", e);
                }
            })
            .await
    };

    match result {
        Ok(metrics) => {
            reset_directml_failures_on_success(&state).await;
            if cancelled.load(Ordering::SeqCst) {
                log::info!("Generation was cancelled");
                Err(generation_cancelled_error())
            } else {
                log::info!(
                    "Streaming generation complete: {} tokens, {:.2} tok/s",
                    metrics.total_tokens,
                    metrics.tokens_per_second
                );
                Ok(metrics)
            }
        }
        Err(e) => {
            record_directml_runtime_failure(&state, &e).await;
            log::error!("Generation error: {}", e);
            Err(e)
        }
    }
}

/// Cancel the current ONNX generation
#[tauri::command]
pub async fn inference_cancel(state: State<'_, InferenceState>) -> Result<(), String> {
    let active_cancel =
        lock_active_cancel_recover(&state.active_cancel, "inference_cancel").clone();

    if let Some(cancel_token) = active_cancel {
        cancel_token.store(true, Ordering::SeqCst);
        log::info!("Generation cancellation requested");
    } else {
        // No active generation: no-op success by design.
        log::debug!("Cancellation requested with no active generation");
    }

    Ok(())
}

/// Check if generation is currently in progress
#[tauri::command]
pub async fn is_generating(state: State<'_, InferenceState>) -> Result<bool, String> {
    Ok(state.generating.load(Ordering::SeqCst))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::backend::{BackendBenchmark, FailureCounters};

    fn benchmark(
        backend: InferenceBackend,
        decode_tps: f64,
        ttft_ms: u64,
    ) -> BackendBenchmark {
        BackendBenchmark {
            backend,
            sample_tokens: 8,
            total_time_ms: 900,
            time_to_first_token_ms: ttft_ms,
            overall_tokens_per_second: decode_tps,
            decode_tokens_per_second: decode_tps,
        }
    }

    #[test]
    fn single_flight_rejects_second_generation() {
        let state = InferenceState::default();
        let _first = state
            .try_begin_generation()
            .expect("first generation should start");

        let second = state.try_begin_generation();
        assert!(second.is_err());
        let err = second.err().expect("second generation must be rejected");
        assert_eq!(err, ERR_GENERATION_IN_PROGRESS);
    }

    #[test]
    fn permit_drop_clears_generation_state() {
        let state = InferenceState::default();
        {
            let (_permit, _token) = state
                .try_begin_generation()
                .expect("generation should start");
            assert!(state.generating.load(Ordering::SeqCst));
            let active = state
                .active_cancel
                .lock()
                .expect("active cancel mutex should not be poisoned");
            assert!(active.is_some());
        }

        assert!(!state.generating.load(Ordering::SeqCst));
        let active = state
            .active_cancel
            .lock()
            .expect("active cancel mutex should not be poisoned");
        assert!(active.is_none());
    }

    #[test]
    fn cancellation_scopes_to_active_generation() {
        let state = InferenceState::default();
        let (_permit, cancel_token) = state
            .try_begin_generation()
            .expect("generation should start");
        assert!(!cancel_token.load(Ordering::SeqCst));

        let active_cancel = state
            .active_cancel
            .lock()
            .expect("active cancel mutex should not be poisoned")
            .clone()
            .expect("active cancel token should be set");
        active_cancel.store(true, Ordering::SeqCst);

        assert!(cancel_token.load(Ordering::SeqCst));
    }

    #[test]
    fn no_active_generation_has_no_cancel_token() {
        let state = InferenceState::default();
        let active = state
            .active_cancel
            .lock()
            .expect("active cancel mutex should not be poisoned");
        assert!(active.is_none());
    }

    #[test]
    fn cancellation_error_has_stable_code_and_message() {
        let err = generation_cancelled_error();
        assert!(err.contains(ERR_CODE_GENERATION_CANCELLED));
        assert!(err.contains(ERR_GENERATION_CANCELLED));
    }

    #[test]
    fn select_backend_honors_force_override() {
        let (backend, reason) =
            select_backend(Some(InferenceBackend::Cpu), None, true, None);
        assert_eq!(backend, InferenceBackend::Cpu);
        assert_eq!(reason, DecisionReason::ForcedOverride);
    }

    #[test]
    fn select_backend_prefers_persisted_decision_without_override() {
        let key = BackendDecisionKey {
            model_id: "qwen2.5-coder-1.5b".to_string(),
            adapter_identity: "intel:arc".to_string(),
            driver_version: "31.0.101.5522".to_string(),
            app_version: "2.2.0".to_string(),
            ort_version: "1.23".to_string(),
        };
        let record = BackendDecisionRecord {
            key,
            decision: BackendDecision::new(
                InferenceBackend::DirectML,
                DecisionReason::BenchmarkPassed,
                None,
            ),
            failure_counters: FailureCounters::default(),
            updated_at: Utc::now().to_rfc3339(),
        };

        let (backend, reason) = select_backend(None, Some(&record), true, None);
        assert_eq!(backend, InferenceBackend::DirectML);
        assert_eq!(reason, DecisionReason::PersistedDecision);
    }

    #[test]
    fn select_backend_uses_benchmark_gate() {
        let comparison = BackendBenchmarkComparison {
            cpu: benchmark(InferenceBackend::Cpu, 10.0, 100),
            directml: benchmark(InferenceBackend::DirectML, 13.5, 114),
            elapsed_ms: 1_000,
            budget_ms: BENCHMARK_SELECTION_BUDGET_MS,
        };
        let (backend, reason) = select_backend(None, None, true, Some(&comparison));
        assert_eq!(backend, InferenceBackend::DirectML);
        assert_eq!(reason, DecisionReason::BenchmarkPassed);
    }
}
