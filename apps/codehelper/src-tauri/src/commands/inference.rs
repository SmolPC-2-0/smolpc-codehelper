use crate::app_paths::{
    bundled_resource_dir_path, default_dev_bundled_resource_dir,
    select_bundled_resource_dir_resolution,
};
use smolpc_engine_client::{
    connect_or_spawn, read_runtime_env_overrides, EngineChatMessage, EngineClient,
    EngineConnectOptions, RuntimeModePreference,
};
use smolpc_engine_core::inference::backend::{BackendStatus, CheckModelResponse};
use smolpc_engine_core::models::registry::{ModelDefinition, ModelRegistry};
use smolpc_engine_core::{GenerationConfig, GenerationMetrics};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use sysinfo::System;
use tauri::ipc::Channel;
use tauri::Manager;
use tokio::sync::Mutex;

const DEFAULT_ENGINE_PORT: u16 = 19432;
const SHARED_MODELS_VENDOR_DIR: &str = "SmolPC";
const SHARED_MODELS_DIR: &str = "models";
const MEMORY_WARNING_THRESHOLD_GB: f64 = 1.0;
const MEMORY_CRITICAL_THRESHOLD_GB: f64 = 0.6;
const HEAVY_MODE_ADVISORY_THRESHOLD_GB: f64 = 2.0;

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
    desired_model: Arc<Mutex<Option<String>>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChatMessageInput {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct MemoryPressureRequest {
    pub active_mode: Option<String>,
    pub app_minimized: bool,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryPressureLevel {
    Normal,
    Warning,
    Critical,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct MemoryPressureStatus {
    pub total_gb: f64,
    pub available_gb: f64,
    pub level: MemoryPressureLevel,
    pub threshold_warning_gb: f64,
    pub threshold_critical_gb: f64,
    pub current_model_id: Option<String>,
    pub current_model_estimated_ram_gb: Option<f32>,
    pub recommended_model_id: Option<String>,
    pub model_switch_recommended: bool,
    pub heavy_mode_active: bool,
    pub auto_unloaded: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AvailableModelDto {
    #[serde(flatten)]
    pub model: ModelDefinition,
    /// Legacy alias retained for compatibility with older frontend payload contracts.
    pub path: String,
}

impl From<ModelDefinition> for AvailableModelDto {
    fn from(model: ModelDefinition) -> Self {
        Self {
            path: model.directory.clone(),
            model,
        }
    }
}

impl Default for InferenceState {
    fn default() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            connect_lock: Arc::new(Mutex::new(())),
            runtime_config: Arc::new(Mutex::new(RuntimeClientConfig::default())),
            desired_model: Arc::new(Mutex::new(None)),
        }
    }
}

impl InferenceState {
    /// Attempt graceful engine shutdown via the cached client.
    pub(crate) async fn shutdown_engine(&self) -> Result<(), String> {
        let guard = self.client.lock().await;
        if let Some(client) = guard.as_ref() {
            client.shutdown().await.map_err(|e| e.to_string())
        } else {
            Ok(()) // no client — engine wasn't started by us
        }
    }
}

fn parse_runtime_mode(mode: &str) -> Result<RuntimeModePreference, String> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(RuntimeModePreference::Auto),
        "cpu" => Ok(RuntimeModePreference::Cpu),
        "dml" | "directml" => Ok(RuntimeModePreference::Dml),
        "npu" | "openvino" | "openvino_npu" => Ok(RuntimeModePreference::Npu),
        _ => Err(format!(
            "Unsupported runtime mode '{mode}'. Use one of: auto, cpu, dml, npu"
        )),
    }
}

pub(super) fn runtime_mode_label(mode: RuntimeModePreference) -> &'static str {
    match mode {
        RuntimeModePreference::Auto => "auto",
        RuntimeModePreference::Cpu => "cpu",
        RuntimeModePreference::Dml => "dml",
        RuntimeModePreference::Npu => "npu",
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

fn sysinfo_memory_values_are_bytes(total_raw: u64) -> bool {
    // sysinfo may expose bytes (newer releases) or KiB (older releases).
    // Cargo.toml pins sysinfo to 0.32.1; this check guards us if that contract
    // changes during dependency updates.
    // This heuristic is intentionally scoped to supported SmolPC targets where
    // model minimum RAM starts at 8 GB. In that range, KiB totals remain far
    // below 1e9 while byte totals are above 1e9.
    // Sub-1 GB machines are unsupported and may be misclassified.
    total_raw > 1_000_000_000
}

fn raw_memory_to_gb(raw: u64, values_are_bytes: bool) -> f64 {
    if values_are_bytes {
        raw as f64 / (1024.0 * 1024.0 * 1024.0)
    } else {
        raw as f64 / (1024.0 * 1024.0)
    }
}

fn sample_system_memory_gb() -> (f64, f64) {
    let mut system = System::new();
    system.refresh_memory();
    let total_raw = system.total_memory();
    let available_raw = system.available_memory();
    let values_are_bytes = sysinfo_memory_values_are_bytes(total_raw);
    (
        raw_memory_to_gb(total_raw, values_are_bytes),
        raw_memory_to_gb(available_raw, values_are_bytes),
    )
}

fn classify_memory_level(available_gb: f64) -> MemoryPressureLevel {
    if available_gb < MEMORY_CRITICAL_THRESHOLD_GB {
        MemoryPressureLevel::Critical
    } else if available_gb < MEMORY_WARNING_THRESHOLD_GB {
        MemoryPressureLevel::Warning
    } else {
        MemoryPressureLevel::Normal
    }
}

fn normalize_mode_id(mode: Option<&str>) -> Option<String> {
    mode.and_then(|value| {
        let trimmed = value.trim().to_ascii_lowercase();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn is_heavy_host_mode(mode: Option<&str>) -> bool {
    // Keep this list aligned with host-tool mode registration in
    // apps/codehelper/src-tauri/src/modes/config.rs.
    matches!(normalize_mode_id(mode).as_deref(), Some("gimp" | "blender"))
}

fn smallest_model_id() -> Option<String> {
    // Recommendations are constrained to the static registry IDs.
    ModelRegistry::available_models()
        .into_iter()
        .min_by(|a, b| {
            a.estimated_runtime_ram_gb
                .partial_cmp(&b.estimated_runtime_ram_gb)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|model| model.id)
}

fn current_model_estimated_ram_gb(current_model_id: Option<&str>) -> Option<f32> {
    current_model_id
        .and_then(ModelRegistry::get_model)
        .map(|model| model.estimated_runtime_ram_gb)
}

fn should_recommend_model_switch(
    level: MemoryPressureLevel,
    available_gb: f64,
    current_model_id: Option<&str>,
    recommended_model_id: Option<&str>,
    heavy_mode_active: bool,
) -> bool {
    let Some(recommended_model_id) = recommended_model_id else {
        return false;
    };
    let Some(current_model_id) = current_model_id else {
        return false;
    };
    if current_model_id == recommended_model_id {
        return false;
    }

    !matches!(level, MemoryPressureLevel::Normal)
        || (heavy_mode_active && available_gb < HEAVY_MODE_ADVISORY_THRESHOLD_GB)
}

fn build_memory_pressure_message(
    available_gb: f64,
    level: MemoryPressureLevel,
    recommended_model_id: Option<&str>,
    model_switch_recommended: bool,
    heavy_mode_active: bool,
    auto_unloaded: bool,
) -> Option<String> {
    let recommendation = recommended_model_id
        .map(|model_id| format!("Switch to '{model_id}' for lower memory usage."))
        .unwrap_or_else(|| "Close other heavy apps and retry.".to_string());

    if auto_unloaded {
        return Some(format!(
            "Available RAM is {:.1} GB and the app was minimized, so the model was unloaded to avoid instability. {}",
            available_gb, recommendation
        ));
    }

    match level {
        MemoryPressureLevel::Critical => Some(format!(
            "Available RAM is critically low ({:.1} GB). {}",
            available_gb, recommendation
        )),
        MemoryPressureLevel::Warning => {
            if model_switch_recommended {
                Some(format!(
                    "Available RAM is low ({:.1} GB). {}",
                    available_gb, recommendation
                ))
            } else {
                Some(format!(
                    "Available RAM is low ({:.1} GB). Close heavy apps to avoid generation failures.",
                    available_gb
                ))
            }
        }
        MemoryPressureLevel::Normal => {
            if heavy_mode_active && model_switch_recommended {
                Some(format!(
                    "Blender/GIMP mode is active with {:.1} GB free RAM. {}",
                    available_gb, recommendation
                ))
            } else {
                None
            }
        }
    }
}

fn is_generation_in_progress_unload_error(error: &str) -> bool {
    error.contains("Cannot unload model while generation is in progress")
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
    let bundled_resource_dir = select_bundled_resource_dir_resolution(
        app_handle
            .path()
            .resource_dir()
            .map_err(|error| error.to_string()),
        cfg!(debug_assertions),
        Some(default_dev_bundled_resource_dir()),
    )
    .map(|resolution| bundled_resource_dir_path(&resolution).to_path_buf());
    let models_dir = resolve_models_dir(bundled_resource_dir.as_deref());
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

fn resolve_models_dir(resource_dir: Option<&Path>) -> Option<PathBuf> {
    let override_dir = std::env::var("SMOLPC_MODELS_DIR").ok().map(PathBuf::from);
    let shared_dir = dirs::data_local_dir()
        .map(|base| base.join(SHARED_MODELS_VENDOR_DIR).join(SHARED_MODELS_DIR));

    select_models_dir(override_dir, shared_dir, resource_dir)
}

fn select_models_dir(
    override_dir: Option<PathBuf>,
    shared_dir: Option<PathBuf>,
    resource_dir: Option<&Path>,
) -> Option<PathBuf> {
    override_dir
        .filter(|path| path.exists())
        .or_else(|| shared_dir.filter(|path| path.exists()))
        .or_else(|| {
            resource_dir
                .map(|res_dir| res_dir.join("models"))
                .filter(|path| path.exists())
        })
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

fn desired_model_to_restore<'a>(
    desired_model: Option<&'a str>,
    current_model: Option<&str>,
) -> Option<&'a str> {
    match desired_model {
        Some(model_id) if current_model != Some(model_id) => Some(model_id),
        _ => None,
    }
}

async fn ensure_desired_model_loaded(
    client: &EngineClient,
    state: &InferenceState,
) -> Result<(), String> {
    let desired_model = state.desired_model.lock().await.clone();
    let Some(desired_model) = desired_model else {
        return Ok(());
    };

    let status = client
        .status()
        .await
        .map_err(|e| format!("Failed to query engine status before generation: {e}"))?;
    let Some(model_to_restore) =
        desired_model_to_restore(Some(&desired_model), status.current_model.as_deref())
    else {
        return Ok(());
    };

    log::info!("Restoring desired model '{model_to_restore}' into shared engine before generation");
    client
        .load_model(model_to_restore)
        .await
        .map_err(|e| format!("Failed to restore model '{model_to_restore}': {e}"))
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
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<String, String> {
    log::info!("Loading model via supervisor: {model_id}");
    let client = supervisor
        .get_client(std::time::Duration::from_secs(60))
        .await?;
    client.load_model(&model_id).await.map_err(|e| {
        log::error!("Model load failed for {model_id}: {e}");
        format!("Failed to load model: {e}")
    })?;
    supervisor.set_desired_model(Some(model_id.clone())).await;
    supervisor.refresh_status().await;
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
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<String, String> {
    let client = supervisor
        .get_client(std::time::Duration::from_secs(60))
        .await?;
    client
        .unload_model(false)
        .await
        .map_err(|e| format!("Failed to unload model: {e}"))?;
    supervisor.set_desired_model(None).await;
    supervisor.refresh_status().await;
    Ok("Model unloaded successfully".to_string())
}

#[tauri::command]
pub async fn list_models(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<Vec<AvailableModelDto>, String> {
    let client = resolve_client(&app_handle, &state, false).await?;
    client
        .list_models()
        .await
        .map(|models| models.into_iter().map(AvailableModelDto::from).collect())
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
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<BackendStatus, String> {
    let requested_mode = parse_runtime_mode(&mode)?;
    log::info!(
        "Applying inference runtime mode via supervisor: requested={}",
        runtime_mode_label(requested_mode)
    );

    supervisor.set_runtime_mode(requested_mode).await?;

    let client = supervisor
        .get_client(std::time::Duration::from_secs(60))
        .await?;

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
            supervisor.set_desired_model(Some(model_id.to_string())).await;
            supervisor.refresh_status().await;
        }
    }

    let status = client
        .status()
        .await
        .map_err(|e| format!("Failed to query engine status after mode switch: {e}"))?;
    Ok(status.backend_status)
}

#[tauri::command]
pub async fn check_model_readiness(
    model_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<CheckModelResponse, String> {
    let client = resolve_client(&app_handle, &state, false).await?;
    client
        .check_model_readiness(&model_id)
        .await
        .map_err(|e| format!("Failed to check model readiness: {e}"))
}

/// Compatibility shim for older callers.
///
/// Prefer `check_model_readiness` for new code so lane detail is not lost.
#[tauri::command]
pub async fn check_model_exists(
    model_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<bool, String> {
    Ok(check_model_readiness(model_id, app_handle, state)
        .await?
        .any_ready())
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
    ensure_desired_model_loaded(&client, &state).await?;
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
pub async fn inference_generate_messages(
    messages: Vec<ChatMessageInput>,
    config: Option<GenerationConfig>,
    on_token: Channel<String>,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<GenerationMetrics, String> {
    let client = resolve_client(&app_handle, &state, false).await?;
    ensure_desired_model_loaded(&client, &state).await?;
    let messages = messages
        .into_iter()
        .map(|message| EngineChatMessage {
            role: message.role,
            content: message.content,
        })
        .collect::<Vec<_>>();
    client
        .generate_stream_messages(&messages, config, |token| {
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

#[tauri::command]
pub async fn evaluate_memory_pressure(
    request: MemoryPressureRequest,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<MemoryPressureStatus, String> {
    let (total_gb, available_gb) = sample_system_memory_gb();
    let level = classify_memory_level(available_gb);
    let heavy_mode_active = is_heavy_host_mode(request.active_mode.as_deref());

    let client = resolve_client(&app_handle, &state, false).await.ok();
    let mut current_model_id = None;
    if let Some(client) = client.as_ref() {
        if let Ok(status) = client.status().await {
            current_model_id = status.current_model;
        }
    }

    let recommended_model_id = smallest_model_id();
    let model_switch_recommended = should_recommend_model_switch(
        level,
        available_gb,
        current_model_id.as_deref(),
        recommended_model_id.as_deref(),
        heavy_mode_active,
    );

    let mut auto_unloaded = false;
    if request.app_minimized && level == MemoryPressureLevel::Critical && current_model_id.is_some()
    {
        if let Some(client) = client.as_ref() {
            match client.unload_model(false).await {
                Ok(()) => {
                    *state.desired_model.lock().await = None;
                    current_model_id = None;
                    auto_unloaded = true;
                }
                Err(error) => {
                    let error_text = error.to_string();
                    if is_generation_in_progress_unload_error(&error_text) {
                        log::debug!(
                            "Skipped auto-unload during critical memory pressure because generation was in progress"
                        );
                    } else {
                        log::warn!(
                            "Failed to auto-unload model during critical memory pressure: {error_text}"
                        );
                    }
                }
            }
        }
    }

    let current_model_estimated_ram_gb =
        current_model_estimated_ram_gb(current_model_id.as_deref());
    let message = build_memory_pressure_message(
        available_gb,
        level,
        recommended_model_id.as_deref(),
        model_switch_recommended,
        heavy_mode_active,
        auto_unloaded,
    );

    Ok(MemoryPressureStatus {
        total_gb,
        available_gb,
        level,
        threshold_warning_gb: MEMORY_WARNING_THRESHOLD_GB,
        threshold_critical_gb: MEMORY_CRITICAL_THRESHOLD_GB,
        current_model_id,
        current_model_estimated_ram_gb,
        recommended_model_id,
        model_switch_recommended,
        heavy_mode_active,
        auto_unloaded,
        message,
    })
}

/// Resolve a client suitable for generation, ensuring the engine is started.
/// Used by addon mode providers (GIMP, Blender, LibreOffice).
pub(crate) async fn resolve_generation_client(
    app_handle: &tauri::AppHandle,
    state: &InferenceState,
) -> Result<EngineClient, String> {
    resolve_client(app_handle, state, false).await
}

/// Return the cached engine client (if any) without reconnection.
/// Used by assistant_cancel to send cancel to an in-flight request.
pub(crate) async fn cached_generation_client(state: &InferenceState) -> Option<EngineClient> {
    state.client.lock().await.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use smolpc_engine_client::test_utils::with_runtime_env;
    use tempfile::TempDir;

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

    #[test]
    fn desired_model_to_restore_requests_reload_after_host_restart() {
        assert_eq!(
            desired_model_to_restore(Some("qwen2.5-1.5b-instruct"), None),
            Some("qwen2.5-1.5b-instruct")
        );
    }

    #[test]
    fn desired_model_to_restore_skips_reload_when_model_is_already_loaded() {
        assert_eq!(
            desired_model_to_restore(Some("qwen2.5-1.5b-instruct"), Some("qwen2.5-1.5b-instruct")),
            None
        );
    }

    #[test]
    fn sysinfo_memory_unit_heuristic_targets_supported_ram_range() {
        assert!(!sysinfo_memory_values_are_bytes(8 * 1024 * 1024));
        assert!(sysinfo_memory_values_are_bytes(8 * 1024 * 1024 * 1024));
        assert!(!sysinfo_memory_values_are_bytes(999_999_488));
    }

    #[test]
    fn sysinfo_032_contract_matches_ci_host_expectation() {
        let mut system = System::new();
        system.refresh_memory();
        let total_raw = system.total_memory();

        assert!(
            !sysinfo_memory_values_are_bytes(total_raw),
            "Pinned sysinfo=0.32.1 is expected to report KiB totals on CI/dev hosts (<1 TiB RAM). Got raw total {total_raw}; revisit memory unit conversion on dependency upgrade."
        );
    }

    #[test]
    fn unload_in_progress_error_detection_matches_engine_message() {
        assert!(is_generation_in_progress_unload_error(
            "/engine/unload failed with HTTP 409: Cannot unload model while generation is in progress"
        ));
        assert!(!is_generation_in_progress_unload_error(
            "Failed to auto-unload model during critical memory pressure: network timeout"
        ));
    }

    #[test]
    fn classify_memory_level_uses_warning_and_critical_thresholds() {
        assert_eq!(
            classify_memory_level(MEMORY_WARNING_THRESHOLD_GB + 0.01),
            MemoryPressureLevel::Normal
        );
        assert_eq!(
            classify_memory_level(MEMORY_WARNING_THRESHOLD_GB - 0.01),
            MemoryPressureLevel::Warning
        );
        assert_eq!(
            classify_memory_level(MEMORY_CRITICAL_THRESHOLD_GB - 0.01),
            MemoryPressureLevel::Critical
        );
    }

    #[test]
    fn recommend_switch_returns_false_when_current_model_is_already_recommended() {
        assert!(!should_recommend_model_switch(
            MemoryPressureLevel::Warning,
            0.8,
            Some("qwen2.5-1.5b-instruct"),
            Some("qwen2.5-1.5b-instruct"),
            false,
        ));
    }

    #[test]
    fn recommend_switch_triggers_for_heavy_mode_even_when_level_is_normal() {
        assert!(should_recommend_model_switch(
            MemoryPressureLevel::Normal,
            HEAVY_MODE_ADVISORY_THRESHOLD_GB - 0.1,
            Some("qwen3-4b"),
            Some("qwen2.5-1.5b-instruct"),
            true,
        ));
        assert!(!should_recommend_model_switch(
            MemoryPressureLevel::Normal,
            HEAVY_MODE_ADVISORY_THRESHOLD_GB + 0.1,
            Some("qwen3-4b"),
            Some("qwen2.5-1.5b-instruct"),
            true,
        ));
    }

    #[test]
    fn heavy_mode_detection_flags_blender_and_gimp() {
        assert!(is_heavy_host_mode(Some("blender")));
        assert!(is_heavy_host_mode(Some("  GIMP ")));
        assert!(!is_heavy_host_mode(Some("code")));
    }

    #[test]
    fn memory_message_reports_auto_unload_when_triggered() {
        let message = build_memory_pressure_message(
            0.4,
            MemoryPressureLevel::Critical,
            Some("qwen2.5-1.5b-instruct"),
            true,
            false,
            true,
        )
        .expect("auto-unload message");
        assert!(message.contains("was minimized"));
        assert!(message.contains("qwen2.5-1.5b-instruct"));
    }

    #[test]
    fn select_models_dir_prefers_override_over_shared_and_bundled() {
        let override_temp = TempDir::new().expect("override temp");
        let shared_temp = TempDir::new().expect("shared temp");
        let resource_temp = TempDir::new().expect("resource temp");
        std::fs::create_dir_all(override_temp.path()).expect("override dir");
        std::fs::create_dir_all(shared_temp.path()).expect("shared dir");
        std::fs::create_dir_all(resource_temp.path().join("models")).expect("bundled models");

        let selected = select_models_dir(
            Some(override_temp.path().to_path_buf()),
            Some(shared_temp.path().to_path_buf()),
            Some(resource_temp.path()),
        )
        .expect("selected models dir");

        assert_eq!(selected, override_temp.path());
    }

    #[test]
    fn select_models_dir_uses_bundled_models_from_normalized_resource_root() {
        let resource_temp = TempDir::new().expect("resource temp");
        let bundled_models = resource_temp.path().join("models");
        std::fs::create_dir_all(&bundled_models).expect("bundled models");

        let selected = select_models_dir(None, None, Some(resource_temp.path()))
            .expect("selected bundled models");

        assert_eq!(selected, bundled_models);
    }
}
