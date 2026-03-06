use smolpc_engine_client::{
    connect_or_spawn, read_runtime_env_overrides, EngineClient, EngineConnectOptions,
    RuntimeModePreference,
};
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
const SHARED_MODELS_VENDOR_DIR: &str = "SmolPC";
const SHARED_MODELS_DIR: &str = "models";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeClientConfig {
    runtime_mode: RuntimeModePreference,
    dml_device_id: Option<i32>,
}

impl Default for RuntimeClientConfig {
    fn default() -> Self {
        let runtime_overrides = read_runtime_env_overrides();
        Self {
            runtime_mode: runtime_overrides.runtime_mode,
            dml_device_id: runtime_overrides.dml_device_id,
        }
    }
}

pub struct InferenceState {
    client: Arc<Mutex<Option<EngineClient>>>,
    connect_lock: Arc<Mutex<()>>,
    runtime_config: Arc<Mutex<RuntimeClientConfig>>,
}

impl Default for InferenceState {
    fn default() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            connect_lock: Arc::new(Mutex::new(())),
            runtime_config: Arc::new(Mutex::new(RuntimeClientConfig::default())),
        }
    }
}

fn parse_runtime_mode(mode: &str) -> Result<RuntimeModePreference, String> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(RuntimeModePreference::Auto),
        "cpu" => Ok(RuntimeModePreference::Cpu),
        "dml" | "directml" => Ok(RuntimeModePreference::Dml),
        _ => Err(format!(
            "Unsupported runtime mode '{mode}'. Use one of: auto, cpu, dml"
        )),
    }
}

pub(super) fn runtime_mode_label(mode: RuntimeModePreference) -> &'static str {
    match mode {
        RuntimeModePreference::Auto => "auto",
        RuntimeModePreference::Cpu => "cpu",
        RuntimeModePreference::Dml => "dml",
    }
}

pub(super) async fn apply_runtime_mode_preference(
    state: &InferenceState,
    runtime_mode: RuntimeModePreference,
) -> bool {
    let mut runtime_config = state.runtime_config.lock().await;
    if runtime_config.runtime_mode == runtime_mode {
        return false;
    }

    runtime_config.runtime_mode = runtime_mode;
    drop(runtime_config);

    *state.client.lock().await = None;
    true
}

fn apply_runtime_mode_rollback<T>(
    runtime_config: &mut RuntimeClientConfig,
    client_slot: &mut Option<T>,
    previous_config: RuntimeClientConfig,
) {
    *runtime_config = previous_config;
    *client_slot = None;
}

fn format_runtime_mode_switch_failure(
    switch_error: &str,
    previous_mode: RuntimeModePreference,
    rollback_error: Option<&str>,
) -> String {
    match rollback_error {
        Some(rollback_error) => format!(
            "Runtime mode switch failed: {switch_error}. Rollback to '{}' also failed: {rollback_error}",
            runtime_mode_label(previous_mode)
        ),
        None => format!("Runtime mode switch failed: {switch_error}"),
    }
}

pub(super) async fn resolve_client(
    app_handle: &tauri::AppHandle,
    state: &InferenceState,
    force_respawn: bool,
) -> Result<EngineClient, String> {
    if !force_respawn {
        if let Some(client) = state.client.lock().await.clone() {
            if client.health().await.unwrap_or(false) {
                return Ok(client);
            }
            log::warn!("Cached shared engine client is unhealthy; reconnecting");
            *state.client.lock().await = None;
        }
    }

    let _guard = state.connect_lock.lock().await;

    if !force_respawn {
        if let Some(client) = state.client.lock().await.clone() {
            if client.health().await.unwrap_or(false) {
                return Ok(client);
            }
            log::warn!("Cached shared engine client is unhealthy after lock; reconnecting");
            *state.client.lock().await = None;
        }
    } else {
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

    let runtime_config = *state.runtime_config.lock().await;
    let options = EngineConnectOptions {
        port,
        app_version: app_handle.package_info().version.to_string(),
        shared_runtime_dir,
        data_dir,
        resource_dir,
        models_dir,
        host_binary,
        runtime_mode: runtime_config.runtime_mode,
        dml_device_id: runtime_config.dml_device_id,
        force_respawn,
    };
    log::info!(
        "Resolving shared engine client: port={} runtime_mode={} dml_device_id={:?} force_respawn={}",
        options.port,
        runtime_mode_label(options.runtime_mode),
        options.dml_device_id,
        options.force_respawn
    );

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

    if let Some(base) = dirs::data_local_dir() {
        let shared = base.join(SHARED_MODELS_VENDOR_DIR).join(SHARED_MODELS_DIR);
        if shared.exists() {
            return Some(shared);
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
        .join("..")
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
    log::info!("Loading model via shared engine: {}", model_id);
    let client = resolve_client(&app_handle, &state, false).await?;
    client.load_model(&model_id).await.map_err(|e| {
        log::error!("Model load failed for {}: {}", model_id, e);
        format!("Failed to load model: {e}")
    })?;
    if let Ok(status) = client.status().await {
        log::info!(
            "Model loaded: model={} backend={:?} runtime_engine={:?} selection_reason={:?}",
            model_id,
            status.backend_status.active_backend,
            status.backend_status.runtime_engine,
            status.backend_status.selection_reason
        );
    }
    Ok(format!("Model loaded: {model_id}"))
}

#[tauri::command]
pub async fn unload_model(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<String, String> {
    let client = resolve_client(&app_handle, &state, false).await?;
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
    let client = resolve_client(&app_handle, &state, false).await?;
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
    let client = resolve_client(&app_handle, &state, false).await?;
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
    let client = resolve_client(&app_handle, &state, false).await?;
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
    let client = resolve_client(&app_handle, &state, false).await?;
    let status = client
        .status()
        .await
        .map_err(|e| format!("Failed to get backend status: {e}"))?;
    Ok(status.backend_status)
}

#[tauri::command]
pub async fn set_inference_runtime_mode(
    mode: String,
    model_id: Option<String>,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<BackendStatus, String> {
    let requested_mode = parse_runtime_mode(&mode)?;
    let previous_config = *state.runtime_config.lock().await;
    log::info!(
        "Applying inference runtime mode: previous={} requested={}",
        runtime_mode_label(previous_config.runtime_mode),
        runtime_mode_label(requested_mode)
    );

    state.runtime_config.lock().await.runtime_mode = requested_mode;
    *state.client.lock().await = None;

    let result = async {
        let client = resolve_client(&app_handle, &state, true).await?;

        if let Some(model_id) = model_id.as_ref().map(|value| value.trim()) {
            if !model_id.is_empty() {
                log::info!(
                    "Reloading model '{}' after runtime mode switch to {}",
                    model_id,
                    runtime_mode_label(requested_mode)
                );
                client
                    .load_model(model_id)
                    .await
                    .map_err(|e| format!("Failed to load model after mode switch: {e}"))?;
            }
        }

        let status = client
            .status()
            .await
            .map_err(|e| format!("Failed to query engine status after mode switch: {e}"))?;
        Ok::<BackendStatus, String>(status.backend_status)
    }
    .await;

    if let Err(error) = &result {
        log::warn!(
            "Runtime mode switch '{}' failed: {}",
            runtime_mode_label(requested_mode),
            error
        );
        {
            let mut runtime_config = state.runtime_config.lock().await;
            let mut client = state.client.lock().await;
            apply_runtime_mode_rollback(&mut runtime_config, &mut *client, previous_config);
        }

        if let Err(rollback_error) = resolve_client(&app_handle, &state, true).await {
            return Err(format_runtime_mode_switch_failure(
                error,
                previous_config.runtime_mode,
                Some(&rollback_error),
            ));
        }
    }

    result
}

#[tauri::command]
pub async fn check_model_exists(
    model_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<bool, String> {
    let client = resolve_client(&app_handle, &state, false).await?;
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
    let client = resolve_client(&app_handle, &state, false).await?;
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
    let client = resolve_client(&app_handle, &state, false).await?;
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
    let client = resolve_client(&app_handle, &state, false).await?;
    let status = client
        .status()
        .await
        .map_err(|e| format!("Failed to query generation state: {e}"))?;
    Ok(status.generating)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::test_env::with_runtime_env;

    #[test]
    fn apply_runtime_mode_rollback_restores_previous_config_and_clears_client() {
        let previous_config = RuntimeClientConfig {
            runtime_mode: RuntimeModePreference::Auto,
            dml_device_id: Some(3),
        };
        let mut runtime_config = RuntimeClientConfig {
            runtime_mode: RuntimeModePreference::Cpu,
            dml_device_id: None,
        };
        let mut client_slot = Some(42usize);

        apply_runtime_mode_rollback(&mut runtime_config, &mut client_slot, previous_config);

        assert_eq!(runtime_config, previous_config);
        assert!(client_slot.is_none());
    }

    #[test]
    fn format_runtime_mode_switch_failure_includes_rollback_context() {
        let message = format_runtime_mode_switch_failure(
            "switch connect failed",
            RuntimeModePreference::Dml,
            Some("reconnect failed"),
        );

        assert!(message.contains("Runtime mode switch failed: switch connect failed."));
        assert!(message.contains("Rollback to 'dml' also failed: reconnect failed"));
    }

    #[test]
    fn runtime_client_config_default_reads_env_overrides() {
        with_runtime_env(Some("directml"), Some("1"), || {
            let config = RuntimeClientConfig::default();
            assert_eq!(config.runtime_mode, RuntimeModePreference::Dml);
            assert_eq!(config.dml_device_id, Some(1));
        });
    }

    #[test]
    fn runtime_client_config_default_falls_back_for_invalid_env_values() {
        with_runtime_env(Some("unknown"), Some("abc"), || {
            let config = RuntimeClientConfig::default();
            assert_eq!(config.runtime_mode, RuntimeModePreference::Auto);
            assert_eq!(config.dml_device_id, None);
        });
    }
}
