use smolpc_engine_client::{connect_or_spawn, EngineClient, EngineConnectOptions};
use smolpc_engine_core::inference::backend::BackendStatus;
use smolpc_engine_core::models::registry::ModelDefinition;
use smolpc_engine_core::{GenerationConfig, GenerationMetrics, GenerationResult};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tauri::ipc::Channel;
use tauri::Manager;
use tokio::sync::Mutex;

const DEFAULT_ENGINE_PORT: u16 = 19432;

pub struct InferenceState {
    client: Arc<Mutex<Option<EngineClient>>>,
    connect_lock: Arc<Mutex<()>>,
}

impl Default for InferenceState {
    fn default() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            connect_lock: Arc::new(Mutex::new(())),
        }
    }
}

async fn resolve_client(
    app_handle: &tauri::AppHandle,
    state: &InferenceState,
) -> Result<EngineClient, String> {
    if let Some(client) = state.client.lock().await.clone() {
        if client.health().await.unwrap_or(false) {
            return Ok(client);
        }
        log::warn!("Cached shared engine client is unhealthy; reconnecting");
        *state.client.lock().await = None;
    }

    let _guard = state.connect_lock.lock().await;

    if let Some(client) = state.client.lock().await.clone() {
        if client.health().await.unwrap_or(false) {
            return Ok(client);
        }
        log::warn!("Cached shared engine client is unhealthy after lock; reconnecting");
        *state.client.lock().await = None;
    }

    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data directory: {e}"))?;
    let shared_runtime_dir = if let Some(base) = dirs::data_local_dir() {
        base.join("SmolPC").join("engine-runtime")
    } else {
        app_data_dir.join("engine-runtime")
    };
    let data_dir = shared_runtime_dir.join("host-data");

    let resource_dir = app_handle
        .path()
        .resource_dir()
        .ok()
        .or_else(|| Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))));
    let models_dir = resolve_models_dir(resource_dir.as_ref());
    let host_binary = resolve_host_binary_path();
    log_host_binary_resolution(host_binary.as_ref());

    let port = std::env::var("SMOLPC_ENGINE_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(DEFAULT_ENGINE_PORT);

    let options = EngineConnectOptions {
        port,
        app_version: app_handle.package_info().version.to_string(),
        shared_runtime_dir,
        data_dir,
        resource_dir,
        models_dir,
        host_binary,
    };

    let client = connect_or_spawn(options)
        .await
        .map_err(|e| format!("Failed to connect or spawn engine host: {e}"))?;

    *state.client.lock().await = Some(client.clone());
    Ok(client)
}

fn resolve_models_dir(resource_dir: Option<&PathBuf>) -> Option<PathBuf> {
    if let Ok(override_dir) = std::env::var("SMOLPC_MODELS_DIR") {
        let path = PathBuf::from(override_dir);
        if path.exists() {
            return Some(path);
        }
    }

    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models");
    if dev_path.exists() {
        return Some(dev_path);
    }

    if let Some(res_dir) = resource_dir {
        let bundled = res_dir.join("models");
        if bundled.exists() {
            return Some(bundled);
        }
    }

    None
}

fn resolve_host_binary_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("SMOLPC_ENGINE_HOST_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    let workspace_target = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("target")
        .join(if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        })
        .join(format!(
            "smolpc-engine-host{}",
            std::env::consts::EXE_SUFFIX
        ));
    if workspace_target.exists() {
        return Some(workspace_target);
    }

    None
}

fn log_host_binary_resolution(host_binary: Option<&PathBuf>) {
    let Some(path) = host_binary else {
        log::info!("Shared engine host binary will be resolved via runtime discovery");
        return;
    };

    match std::fs::metadata(path) {
        Ok(metadata) => {
            let modified_unix = metadata
                .modified()
                .ok()
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            log::info!(
                "Resolved shared engine host binary: path={} size_bytes={} modified_unix={}",
                path.display(),
                metadata.len(),
                modified_unix
            );
        }
        Err(error) => {
            log::warn!(
                "Resolved shared engine host binary path exists check failed: path={} error={}",
                path.display(),
                error
            );
        }
    }
}

#[tauri::command]
pub async fn load_model(
    model_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<String, String> {
    let client = resolve_client(&app_handle, &state).await?;
    client
        .load_model(&model_id)
        .await
        .map_err(|e| format!("Failed to load model: {e}"))?;
    Ok(format!("Model loaded: {model_id}"))
}

#[tauri::command]
pub async fn unload_model(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<String, String> {
    let client = resolve_client(&app_handle, &state).await?;
    client
        .unload_model(false)
        .await
        .map_err(|e| format!("Failed to unload model: {e}"))?;
    Ok("Model unloaded successfully".to_string())
}

#[tauri::command]
pub async fn generate_text(
    prompt: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<GenerationResult, String> {
    let client = resolve_client(&app_handle, &state).await?;
    client
        .generate_text(&prompt, None)
        .await
        .map_err(|e| format!("Generation failed: {e}"))
}

#[tauri::command]
pub async fn list_models(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<Vec<ModelDefinition>, String> {
    let client = resolve_client(&app_handle, &state).await?;
    client
        .list_models()
        .await
        .map_err(|e| format!("Failed to list models: {e}"))
}

#[tauri::command]
pub async fn get_current_model(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<Option<String>, String> {
    let client = resolve_client(&app_handle, &state).await?;
    let status = client
        .status()
        .await
        .map_err(|e| format!("Failed to get engine status: {e}"))?;
    Ok(status.current_model)
}

#[tauri::command]
pub async fn get_inference_backend_status(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<BackendStatus, String> {
    let client = resolve_client(&app_handle, &state).await?;
    let status = client
        .status()
        .await
        .map_err(|e| format!("Failed to get backend status: {e}"))?;
    Ok(status.backend_status)
}

#[tauri::command]
pub async fn check_model_exists(
    model_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<bool, String> {
    let client = resolve_client(&app_handle, &state).await?;
    client
        .check_model_exists(&model_id)
        .await
        .map_err(|e| format!("Failed to check model availability: {e}"))
}

#[tauri::command]
pub async fn inference_generate(
    prompt: String,
    config: Option<GenerationConfig>,
    on_token: Channel<String>,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<GenerationMetrics, String> {
    let client = resolve_client(&app_handle, &state).await?;
    client
        .generate_stream(&prompt, config, |token| {
            if let Err(e) = on_token.send(token) {
                log::warn!("Failed to send token via channel: {e}");
            }
        })
        .await
        .map_err(|e| format!("Streaming generation failed: {e}"))
}

#[tauri::command]
pub async fn inference_cancel(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<(), String> {
    let client = resolve_client(&app_handle, &state).await?;
    client
        .cancel()
        .await
        .map_err(|e| format!("Failed to cancel generation: {e}"))
}

#[tauri::command]
pub async fn is_generating(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<bool, String> {
    let client = resolve_client(&app_handle, &state).await?;
    let status = client
        .status()
        .await
        .map_err(|e| format!("Failed to query generation state: {e}"))?;
    Ok(status.generating)
}
