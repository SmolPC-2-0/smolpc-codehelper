use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use serde_json::{json, Value};
use smolpc_engine_client::{connect_or_spawn, EngineClient, EngineConnectOptions, RuntimeModePreference};

const DEFAULT_ENGINE_PORT: u16 = 19432;

pub struct EngineState {
    client: Arc<Mutex<Option<EngineClient>>>,
    connect_lock: Arc<Mutex<()>>,
}

impl Default for EngineState {
    fn default() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            connect_lock: Arc::new(Mutex::new(())),
        }
    }
}

/// Lazily connect to (or spawn) the engine, caching the client.
pub async fn resolve_client(
    app_handle: &tauri::AppHandle,
    state: &EngineState,
) -> Result<EngineClient, String> {
    // Fast path: cached client that is still healthy
    {
        let guard = state.client.lock().await;
        if let Some(ref c) = *guard {
            if c.health().await.unwrap_or(false) {
                return Ok(c.clone());
            }
        }
    }

    // Acquire spawn lock (prevents concurrent connect_or_spawn)
    let _lock = state.connect_lock.lock().await;

    // Double-check after acquiring lock
    {
        let guard = state.client.lock().await;
        if let Some(ref c) = *guard {
            if c.health().await.unwrap_or(false) {
                return Ok(c.clone());
            }
        }
    }

    let port = std::env::var("SMOLPC_ENGINE_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_ENGINE_PORT);

    let app_version = app_handle
        .package_info()
        .version
        .to_string();

    let shared_runtime_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("SmolPC")
        .join("engine-runtime");

    let data_dir = shared_runtime_dir.join("host-data");

    let resource_dir = app_handle.path().resource_dir().ok();
    let models_dir = resolve_models_dir(resource_dir.as_deref());
    let host_binary = resolve_host_binary_path();

    let options = EngineConnectOptions {
        port,
        app_version,
        shared_runtime_dir,
        data_dir,
        resource_dir,
        models_dir,
        host_binary,
        runtime_mode: RuntimeModePreference::Auto,
        dml_device_id: None,
        force_respawn: false,
    };

    log::info!("[gimp-engine] connecting to engine on port {port}...");
    let client = connect_or_spawn(options)
        .await
        .map_err(|e| format!("Engine connection failed: {e}"))?;
    log::info!("[gimp-engine] engine connected");

    *state.client.lock().await = Some(client.clone());
    Ok(client)
}

fn resolve_models_dir(resource_dir: Option<&std::path::Path>) -> Option<PathBuf> {
    // 1. Environment override
    if let Ok(p) = std::env::var("SMOLPC_MODELS_DIR") {
        let path = PathBuf::from(&p);
        if path.exists() {
            return Some(path);
        }
    }
    // 2. Shared system location
    if let Some(local) = dirs::data_local_dir() {
        let path = local.join("SmolPC").join("models");
        if path.exists() {
            return Some(path);
        }
    }
    // 3. Dev-time path relative to manifest
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let path = PathBuf::from(manifest).join("models");
        if path.exists() {
            return Some(path);
        }
    }
    // 4. Bundled resources
    if let Some(res) = resource_dir {
        let path = res.join("models");
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn resolve_host_binary_path() -> Option<PathBuf> {
    // 1. Environment override
    if let Ok(p) = std::env::var("SMOLPC_ENGINE_HOST_BIN") {
        let path = PathBuf::from(&p);
        if path.exists() {
            return Some(path);
        }
    }
    // 2. Workspace target directory (dev builds)
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let workspace_root = PathBuf::from(manifest)
            .join("..")
            .join("..")
            .join("..");
        let suffix = if cfg!(windows) { ".exe" } else { "" };
        for profile in ["debug", "release"] {
            let bin = workspace_root
                .join("target")
                .join(profile)
                .join(format!("smolpc-engine-host{suffix}"));
            if bin.exists() {
                return Some(bin);
            }
        }
    }
    None
}

// ── Tauri commands ──

#[tauri::command]
pub async fn engine_health(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
) -> Result<bool, String> {
    match resolve_client(&app_handle, &state).await {
        Ok(client) => Ok(client.health().await.unwrap_or(false)),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
pub async fn engine_cancel(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    let client = resolve_client(&app_handle, &state).await?;
    client.cancel().await.map_err(|e| format!("Cancel failed: {e}"))
}

#[tauri::command]
pub async fn engine_status(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
) -> Result<Value, String> {
    match resolve_client(&app_handle, &state).await {
        Ok(client) => {
            let status = client.status().await.map_err(|e| format!("{e}"))?;
            Ok(json!({
                "ready": status.is_ready(),
                "model": status.active_model_id,
                "generating": status.generating,
            }))
        }
        Err(_) => Ok(json!({
            "ready": false,
            "model": null,
            "generating": false,
        })),
    }
}
