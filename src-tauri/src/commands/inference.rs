/// Tauri commands for ONNX Runtime inference
///
/// Provides IPC interface between frontend and inference engine.

use crate::inference::{Generator, InferenceSession, TokenizerWrapper};
use crate::inference::types::{GenerationConfig, GenerationResult};
use crate::models::{ModelLoader, ModelRegistry};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

/// Global inference state (managed by Tauri)
pub struct InferenceState {
    /// Current generator instance (None if no model loaded)
    generator: Arc<Mutex<Option<Generator>>>,

    /// Currently loaded model ID
    current_model: Arc<Mutex<Option<String>>>,

    /// Cancellation flag for generation
    cancelled: Arc<AtomicBool>,
}

impl Default for InferenceState {
    fn default() -> Self {
        Self {
            generator: Arc::new(Mutex::new(None)),
            current_model: Arc::new(Mutex::new(None)),
            cancelled: Arc::new(AtomicBool::new(false)),
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
    log::info!("Loading model: {}", model_id);

    // Validate model exists in registry
    let model_def = ModelRegistry::get_model(&model_id)
        .ok_or_else(|| format!("Unknown model ID: {}", model_id))?;

    log::info!("Model definition: {} ({})", model_def.name, model_def.size);

    // Validate model files exist
    ModelLoader::validate_model(&model_def.directory)?;

    // Get file paths
    let model_path = ModelLoader::model_file(&model_def.directory);
    let tokenizer_path = ModelLoader::tokenizer_file(&model_def.directory);

    log::info!("Model path: {}", model_path.display());
    log::info!("Tokenizer path: {}", tokenizer_path.display());

    // Load ONNX session
    let session = InferenceSession::new(&model_path)?;
    let session_info = session.info();

    log::info!("Session loaded - Inputs: {:?}", session_info.inputs);
    log::info!("Session loaded - Outputs: {:?}", session_info.outputs);

    // Load tokenizer
    let tokenizer = TokenizerWrapper::from_file(&tokenizer_path)?;
    log::info!("Tokenizer loaded - Vocab size: {}", tokenizer.vocab_size());

    // Create generator
    let generator = Generator::new(session, tokenizer);

    // Store in state
    let mut gen_state = state.generator.lock().await;
    *gen_state = Some(generator);

    let mut current_model = state.current_model.lock().await;
    *current_model = Some(model_id.clone());

    log::info!("Model loaded successfully: {}", model_id);

    Ok(format!(
        "Model loaded: {} ({} parameters)",
        model_def.name, model_def.size
    ))
}

/// Unload the current model and free memory
#[tauri::command]
pub async fn unload_model(state: State<'_, InferenceState>) -> Result<String, String> {
    let mut gen_state = state.generator.lock().await;
    *gen_state = None;

    let mut current_model = state.current_model.lock().await;
    *current_model = None;

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
    let gen_state = state.generator.lock().await;

    let generator = gen_state
        .as_ref()
        .ok_or("No model loaded. Call load_model first.")?;

    log::info!("Starting generation (prompt length: {} chars)", prompt.len());

    let result = generator.generate(&prompt).await?;

    log::info!(
        "Generation complete: {} tokens, {:.2} tok/s",
        result.metrics.total_tokens,
        result.metrics.tokens_per_second
    );

    Ok(result)
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

/// Generate text with streaming output via Tauri events (ONNX Runtime)
///
/// Emits events:
/// - `inference_token` (String) - Each generated token
/// - `inference_done` (GenerationMetrics) - Generation complete with metrics
/// - `inference_error` (String) - On error
/// - `inference_cancelled` () - When cancelled
///
/// # Arguments
/// * `prompt` - Input text prompt
/// * `config` - Optional generation configuration (temperature, top_k, etc.)
#[tauri::command]
pub async fn inference_generate(
    app_handle: AppHandle,
    prompt: String,
    config: Option<GenerationConfig>,
    state: State<'_, InferenceState>,
) -> Result<(), String> {
    // Reset cancellation flag
    state.cancelled.store(false, Ordering::SeqCst);

    let gen_state = state.generator.lock().await;
    let generator = gen_state
        .as_ref()
        .ok_or("No model loaded. Call load_model first.")?;

    log::info!(
        "Starting streaming generation (prompt length: {} chars)",
        prompt.len()
    );

    // Get the cancellation flag for the generator
    let cancelled = Arc::clone(&state.cancelled);

    // Generate with streaming callback
    let result = generator
        .generate_stream(
            &prompt,
            config,
            cancelled,
            |token| {
                // Emit each token to frontend
                if let Err(e) = app_handle.emit("inference_token", &token) {
                    log::warn!("Failed to emit token: {}", e);
                }
            },
        )
        .await;

    match result {
        Ok(metrics) => {
            // Check if we were cancelled
            if state.cancelled.load(Ordering::SeqCst) {
                log::info!("Generation was cancelled");
                let _ = app_handle.emit("inference_cancelled", ());
            } else {
                log::info!(
                    "Streaming generation complete: {} tokens, {:.2} tok/s",
                    metrics.total_tokens,
                    metrics.tokens_per_second
                );
                let _ = app_handle.emit("inference_done", &metrics);
            }
        }
        Err(e) => {
            log::error!("Generation error: {}", e);
            let _ = app_handle.emit("inference_error", &e);
            return Err(e);
        }
    }

    Ok(())
}

/// Cancel the current ONNX generation
#[tauri::command]
pub async fn inference_cancel(state: State<'_, InferenceState>) -> Result<(), String> {
    state.cancelled.store(true, Ordering::SeqCst);
    log::info!("Generation cancellation requested");
    Ok(())
}

/// Check if generation is currently in progress
#[tauri::command]
pub async fn is_generating(state: State<'_, InferenceState>) -> Result<bool, String> {
    // If cancelled is false and generator is locked, we're generating
    // This is a simple heuristic - for more accurate tracking we'd need a separate flag
    let gen_state = state.generator.try_lock();
    Ok(gen_state.is_err()) // If we can't lock, generation is in progress
}
