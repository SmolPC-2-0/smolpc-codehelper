use crate::inference::types::{GenerationConfig, GenerationMetrics, GenerationResult};
/// Tauri commands for ONNX Runtime inference
///
/// Provides IPC interface between frontend and inference engine.
use crate::inference::{Generator, InferenceBackend, InferenceSession, TokenizerWrapper};
use crate::models::{ModelLoader, ModelRegistry};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::ipc::Channel;
use tauri::State;
use tokio::sync::Mutex;

const ERR_GENERATION_IN_PROGRESS: &str = "Generation already in progress";
const ERR_GENERATION_CANCELLED: &str = "Generation cancelled";
const ERR_CODE_GENERATION_CANCELLED: &str = "INFERENCE_GENERATION_CANCELLED";
const ERR_MODEL_CHANGE_DURING_GENERATION: &str =
    "Cannot load or unload model while generation is in progress";

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
}

impl Default for InferenceState {
    fn default() -> Self {
        Self {
            generator: Arc::new(Mutex::new(None)),
            current_model: Arc::new(Mutex::new(None)),
            active_cancel: Arc::new(StdMutex::new(None)),
            generating: Arc::new(AtomicBool::new(false)),
            active_backend: Arc::new(Mutex::new(None)),
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

    // Session fallback plumbing is wired here; backend selection logic will be layered above this flow.
    let (session, active_backend, fallback_reason) =
        load_session_with_fallback(&model_path, InferenceBackend::Cpu)?;
    let session_info = session.info();

    if let Some(reason) = fallback_reason {
        log::warn!("Model loaded with CPU fallback after DirectML failure: {reason}");
    }

    log::info!("Session loaded - Inputs: {:?}", session_info.inputs);
    log::info!("Session loaded - Outputs: {:?}", session_info.outputs);
    log::info!("Session backend active: {}", active_backend.as_str());

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

    let gen_state = state.generator.lock().await;

    let generator = gen_state
        .as_ref()
        .ok_or("No model loaded. Call load_model first.")?;

    log::info!(
        "Starting generation (prompt length: {} chars)",
        prompt.len()
    );

    let mut generated_text = String::new();
    let metrics = generator
        .generate_stream(&prompt, None, Arc::clone(&cancelled), |token| {
            generated_text.push_str(&token);
        })
        .await?;

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

    let gen_state = state.generator.lock().await;
    let generator = match gen_state.as_ref() {
        Some(g) => g,
        None => return Err("No model loaded. Call load_model first.".to_string()),
    };

    log::info!(
        "Starting streaming generation (prompt length: {} chars)",
        prompt.len()
    );

    // Clone channel for use in closure (Channel is Clone + Send)
    let token_channel = on_token.clone();

    // Generate with streaming callback — tokens sent via Channel
    let result = generator
        .generate_stream(&prompt, config, Arc::clone(&cancelled), move |token| {
            if let Err(e) = token_channel.send(token) {
                log::warn!("Failed to send token via channel: {}", e);
            }
        })
        .await;

    match result {
        Ok(metrics) => {
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
}
