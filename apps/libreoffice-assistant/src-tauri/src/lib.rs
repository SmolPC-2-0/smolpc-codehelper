use smolpc_engine_client::{
    connect_or_spawn, engine_api_major_compatible, expected_engine_api_major,
    read_runtime_env_overrides, version_major, EngineClient, EngineConnectOptions, StartupMode,
    StartupPolicy,
};
use smolpc_engine_core::inference::backend::{BackendStatus, CheckModelResponse};
use smolpc_engine_core::models::registry::ModelDefinition;
use smolpc_engine_core::{GenerationConfig, GenerationMetrics, GenerationResult};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tauri::ipc::Channel;
use tauri::Manager;
use tokio::sync::Mutex;

const DEFAULT_ENGINE_PORT: u16 = 19432;
const SHARED_RUNTIME_VENDOR_DIR: &str = "SmolPC";
const SHARED_RUNTIME_DIR: &str = "engine-runtime";
const HOST_DATA_DIR: &str = "host-data";
const SHARED_MODELS_VENDOR_DIR: &str = "SmolPC";
const SHARED_MODELS_DIR: &str = "models";

#[derive(Debug, Clone, serde::Serialize)]
struct BootstrapEngineSnapshot {
    healthy: bool,
    protocol_version: Option<String>,
    engine_api_version: Option<String>,
    state: Option<String>,
    active_backend: Option<String>,
    active_model_id: Option<String>,
    runtime_engine: Option<String>,
    selection_reason: Option<String>,
    error: Option<String>,
}

impl Default for BootstrapEngineSnapshot {
    fn default() -> Self {
        Self {
            healthy: false,
            protocol_version: None,
            engine_api_version: None,
            state: None,
            active_backend: None,
            active_model_id: None,
            runtime_engine: None,
            selection_reason: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct BootstrapStatus {
    stage: String,
    notes: Vec<String>,
    engine: BootstrapEngineSnapshot,
}

#[derive(Debug, Clone, serde::Serialize)]
struct EngineMetaSnapshot {
    protocol_version: String,
    engine_api_version: String,
    engine_version: String,
    pid: u32,
    busy: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct EngineStatusSnapshot {
    attempt_id: String,
    state: Option<String>,
    active_backend: Option<String>,
    active_model_id: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    runtime_engine: Option<String>,
    selection_reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct RuntimeOverrideSnapshot {
    smolpc_force_ep: Option<String>,
    smolpc_dml_device_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct IntegrationIssueReport {
    app_name: String,
    app_version: String,
    os: String,
    arch: String,
    hardware_summary: serde_json::Value,
    request_payload: Option<serde_json::Value>,
    http_status: Option<u16>,
    response_body: Option<String>,
    engine_status: EngineStatusSnapshot,
    engine_meta: EngineMetaSnapshot,
    runtime_overrides: RuntimeOverrideSnapshot,
}

#[derive(Debug, Clone, serde::Serialize)]
struct VerificationCheck {
    id: String,
    ok: bool,
    detail: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct RuntimeVerificationReport {
    generated_at_unix: u64,
    model_id: String,
    checks: Vec<VerificationCheck>,
    all_passed: bool,
}

pub struct EngineBridgeState {
    client: Arc<Mutex<Option<EngineClient>>>,
    connect_lock: Arc<Mutex<()>>,
    desired_model: Arc<Mutex<Option<String>>>,
}

impl Default for EngineBridgeState {
    fn default() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            connect_lock: Arc::new(Mutex::new(())),
            desired_model: Arc::new(Mutex::new(None)),
        }
    }
}

fn phase_one_notes() -> Vec<String> {
    vec![
        "Frontend shell created under apps/libreoffice-assistant/src".to_string(),
        "Tauri backend shell created under apps/libreoffice-assistant/src-tauri".to_string(),
        "MCP resource path reserved at src-tauri/resources/mcp_server".to_string(),
        "Shared engine bootstrap/status bridge wired through smolpc-engine-client".to_string(),
    ]
}

fn append_error(existing: Option<String>, next: impl Into<String>) -> Option<String> {
    let next = next.into();
    match existing {
        Some(current) => Some(format!("{current}; {next}")),
        None => Some(next),
    }
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

async fn resolve_client(
    app_handle: &tauri::AppHandle,
    state: &EngineBridgeState,
) -> Result<EngineClient, String> {
    if let Some(client) = state.client.lock().await.clone() {
        if client.health().await.unwrap_or(false) {
            return Ok(client);
        }
        *state.client.lock().await = None;
    }

    let _guard = state.connect_lock.lock().await;
    if let Some(client) = state.client.lock().await.clone() {
        if client.health().await.unwrap_or(false) {
            return Ok(client);
        }
        *state.client.lock().await = None;
    }

    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|error| format!("Failed to resolve app data directory: {error}"))?;
    let shared_runtime_dir = dirs::data_local_dir()
        .map(|base| {
            base.join(SHARED_RUNTIME_VENDOR_DIR)
                .join(SHARED_RUNTIME_DIR)
        })
        .unwrap_or_else(|| app_data_dir.join(SHARED_RUNTIME_DIR));
    let data_dir = shared_runtime_dir.join(HOST_DATA_DIR);

    let resource_dir = app_handle
        .path()
        .resource_dir()
        .ok()
        .or_else(|| Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))));
    let runtime_overrides = read_runtime_env_overrides();
    let options = EngineConnectOptions {
        port: std::env::var("SMOLPC_ENGINE_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(DEFAULT_ENGINE_PORT),
        app_version: app_handle.package_info().version.to_string(),
        shared_runtime_dir,
        data_dir,
        resource_dir: resource_dir.clone(),
        models_dir: resolve_models_dir(resource_dir.as_ref()),
        host_binary: resolve_host_binary_path(),
        runtime_mode: runtime_overrides.runtime_mode,
        dml_device_id: runtime_overrides.dml_device_id,
        force_respawn: false,
    };

    let client = connect_or_spawn(options)
        .await
        .map_err(|error| format!("Failed to connect or spawn engine host: {error}"))?;
    *state.client.lock().await = Some(client.clone());
    Ok(client)
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
    state: &EngineBridgeState,
) -> Result<(), String> {
    let desired_model = state.desired_model.lock().await.clone();
    let Some(desired_model) = desired_model else {
        return Ok(());
    };

    let status = client
        .status()
        .await
        .map_err(|error| format!("Failed to query engine status before generation: {error}"))?;
    let Some(model_to_restore) =
        desired_model_to_restore(Some(&desired_model), status.current_model.as_deref())
    else {
        return Ok(());
    };

    client
        .load_model(model_to_restore)
        .await
        .map_err(|error| format!("Failed to restore model '{model_to_restore}': {error}"))
}

fn apply_engine_status(
    snapshot: &mut BootstrapEngineSnapshot,
    status: &smolpc_engine_client::EngineStatus,
) {
    snapshot.state = status.state.clone().or(status.startup_phase.clone());
    snapshot.active_backend = status.active_backend.clone().or_else(|| {
        status
            .backend_status
            .active_backend
            .map(|backend| backend.as_str().to_string())
    });
    snapshot.active_model_id = status
        .active_model_id
        .clone()
        .or(status.current_model.clone());
    snapshot.runtime_engine = status.backend_status.runtime_engine.clone();
    snapshot.selection_reason = status.backend_status.selection_reason.clone();

    let status_error = status.error_message.clone().or_else(|| {
        status
            .last_error
            .as_ref()
            .map(|error| format!("{}: {}", error.code, error.message))
    });
    if let Some(status_error) = status_error {
        snapshot.error = append_error(snapshot.error.clone(), status_error);
    }
}

fn now_unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn map_engine_meta_snapshot(meta: &smolpc_engine_client::EngineMeta) -> EngineMetaSnapshot {
    EngineMetaSnapshot {
        protocol_version: meta.protocol_version.clone(),
        engine_api_version: meta.engine_api_version.clone(),
        engine_version: meta.engine_version.clone(),
        pid: meta.pid,
        busy: meta.busy,
    }
}

fn map_engine_status_snapshot(status: &smolpc_engine_client::EngineStatus) -> EngineStatusSnapshot {
    EngineStatusSnapshot {
        attempt_id: status.attempt_id.clone(),
        state: status.state.clone().or(status.startup_phase.clone()),
        active_backend: status.active_backend.clone().or_else(|| {
            status
                .backend_status
                .active_backend
                .map(|backend| backend.as_str().to_string())
        }),
        active_model_id: status
            .active_model_id
            .clone()
            .or(status.current_model.clone()),
        error_code: status.error_code.clone(),
        error_message: status.error_message.clone().or_else(|| {
            status
                .last_error
                .as_ref()
                .map(|error| format!("{}: {}", error.code, error.message))
        }),
        runtime_engine: status.backend_status.runtime_engine.clone(),
        selection_reason: status.backend_status.selection_reason.clone(),
    }
}

fn hardware_summary(status: &smolpc_engine_client::EngineStatus) -> serde_json::Value {
    let decision_key = status.backend_status.decision_key.as_ref();
    serde_json::json!({
        "selected_device": status.backend_status.selected_device,
        "gpu_adapter_identity": decision_key.and_then(|key| key.gpu_adapter_identity.clone()),
        "gpu_driver_version": decision_key.and_then(|key| key.gpu_driver_version.clone()),
        "gpu_device_id": decision_key.and_then(|key| key.gpu_device_id),
        "npu_adapter_identity": decision_key.and_then(|key| key.npu_adapter_identity.clone()),
        "npu_driver_version": decision_key.and_then(|key| key.npu_driver_version.clone()),
        "directml_lane": status.backend_status.lanes.directml,
        "openvino_npu_lane": status.backend_status.lanes.openvino_npu
    })
}

fn runtime_override_snapshot() -> RuntimeOverrideSnapshot {
    RuntimeOverrideSnapshot {
        smolpc_force_ep: std::env::var("SMOLPC_FORCE_EP").ok(),
        smolpc_dml_device_id: std::env::var("SMOLPC_DML_DEVICE_ID").ok(),
    }
}

fn build_runtime_verification_checks(
    model_id: &str,
    health_ok: bool,
    meta: &smolpc_engine_client::EngineMeta,
    status: &smolpc_engine_client::EngineStatus,
    readiness: &CheckModelResponse,
) -> Vec<VerificationCheck> {
    let protocol_major = version_major(&meta.protocol_version);
    let expected_api_major = expected_engine_api_major().unwrap_or(1);
    let api_major_ok = engine_api_major_compatible(&meta.engine_api_version, expected_api_major);
    let directml_lane = &readiness.lanes.directml;
    let has_backend_diagnostics = status.backend_status.runtime_engine.is_some()
        && status.backend_status.selection_reason.is_some()
        && status.backend_status.selection_state.is_some();

    vec![
        VerificationCheck {
            id: "connectivity_health".to_string(),
            ok: health_ok,
            detail: if health_ok {
                "GET /engine/health returned ok".to_string()
            } else {
                "Engine health check failed".to_string()
            },
        },
        VerificationCheck {
            id: "protocol_major_v1".to_string(),
            ok: protocol_major == Some(1),
            detail: format!(
                "meta.protocol_version={} (major={})",
                meta.protocol_version,
                protocol_major
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            ),
        },
        VerificationCheck {
            id: "engine_api_major_compatible".to_string(),
            ok: api_major_ok,
            detail: format!(
                "meta.engine_api_version={} expected_major>={}",
                meta.engine_api_version, expected_api_major
            ),
        },
        VerificationCheck {
            id: "model_readiness_any_lane".to_string(),
            ok: readiness.any_ready(),
            detail: format!(
                "model={} lanes=openvino_npu:{} directml:{} cpu:{}",
                model_id,
                readiness.lanes.openvino_npu.reason,
                readiness.lanes.directml.reason,
                readiness.lanes.cpu.reason
            ),
        },
        VerificationCheck {
            id: "backend_diagnostics_present".to_string(),
            ok: has_backend_diagnostics,
            detail: format!(
                "runtime_engine={:?} selection_reason={:?} selection_state={:?}",
                status.backend_status.runtime_engine,
                status.backend_status.selection_reason,
                status.backend_status.selection_state
            ),
        },
        VerificationCheck {
            id: "directml_lane_observed".to_string(),
            ok: directml_lane.ready || !directml_lane.reason.trim().is_empty(),
            detail: format!(
                "directml.ready={} reason={} bundle_ready={} artifact_ready={}",
                directml_lane.ready,
                directml_lane.reason,
                directml_lane.bundle_ready,
                directml_lane.artifact_ready
            ),
        },
    ]
}

async fn collect_bootstrap_status(
    app_handle: &tauri::AppHandle,
    state: &EngineBridgeState,
    ensure_started: bool,
) -> BootstrapStatus {
    let mut notes = phase_one_notes();
    let mut engine = BootstrapEngineSnapshot::default();

    let client = match resolve_client(app_handle, state).await {
        Ok(client) => client,
        Err(error) => {
            engine.error = Some(error);
            notes.push("Engine client could not be resolved from this app shell.".to_string());
            return BootstrapStatus {
                stage: "phase_1_scaffold".to_string(),
                notes,
                engine,
            };
        }
    };

    if ensure_started {
        if let Err(error) = client
            .ensure_started(StartupMode::Auto, StartupPolicy::default())
            .await
        {
            engine.error = append_error(engine.error, format!("ensure_started failed: {error}"));
        }
    }

    match client.health().await {
        Ok(healthy) => engine.healthy = healthy,
        Err(error) => {
            engine.error = append_error(engine.error, format!("health check failed: {error}"));
        }
    }

    if !engine.healthy {
        notes.push("Shared engine health check did not pass at probe time.".to_string());
        return BootstrapStatus {
            stage: "phase_1_scaffold".to_string(),
            notes,
            engine,
        };
    }

    match client.meta().await {
        Ok(meta) => {
            engine.protocol_version = Some(meta.protocol_version);
            engine.engine_api_version = Some(meta.engine_api_version);
        }
        Err(error) => {
            engine.error = append_error(engine.error, format!("meta query failed: {error}"));
        }
    }

    match client.status().await {
        Ok(status) => apply_engine_status(&mut engine, &status),
        Err(error) => {
            engine.error = append_error(engine.error, format!("status query failed: {error}"));
        }
    }

    if engine.error.is_some() {
        notes
            .push("Bootstrap probe returned partial status due to one or more errors.".to_string());
    }

    let stage = if engine
        .state
        .as_deref()
        .is_some_and(|state| state.eq_ignore_ascii_case("ready"))
    {
        "phase_1_engine_ready"
    } else {
        "phase_1_scaffold"
    };

    BootstrapStatus {
        stage: stage.to_string(),
        notes,
        engine,
    }
}

#[tauri::command]
async fn get_bootstrap_status(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineBridgeState>,
) -> Result<BootstrapStatus, String> {
    Ok(collect_bootstrap_status(&app_handle, &state, false).await)
}

#[tauri::command]
async fn ensure_engine_started(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineBridgeState>,
) -> Result<BootstrapStatus, String> {
    Ok(collect_bootstrap_status(&app_handle, &state, true).await)
}

#[tauri::command]
async fn list_models(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineBridgeState>,
) -> Result<Vec<ModelDefinition>, String> {
    let client = resolve_client(&app_handle, &state).await?;
    client
        .list_models()
        .await
        .map_err(|error| format!("Failed to list models: {error}"))
}

#[tauri::command]
async fn load_model(
    model_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineBridgeState>,
) -> Result<String, String> {
    let client = resolve_client(&app_handle, &state).await?;
    client
        .ensure_started(StartupMode::Auto, StartupPolicy::default())
        .await
        .map_err(|error| format!("Engine ensure_started failed: {error}"))?;
    client
        .load_model(&model_id)
        .await
        .map_err(|error| format!("Failed to load model '{model_id}': {error}"))?;
    *state.desired_model.lock().await = Some(model_id.clone());
    Ok(format!("Model loaded: {model_id}"))
}

#[tauri::command]
async fn unload_model(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineBridgeState>,
) -> Result<String, String> {
    let client = resolve_client(&app_handle, &state).await?;
    client
        .unload_model(false)
        .await
        .map_err(|error| format!("Failed to unload model: {error}"))?;
    *state.desired_model.lock().await = None;
    Ok("Model unloaded successfully".to_string())
}

#[tauri::command]
async fn generate_text(
    prompt: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineBridgeState>,
) -> Result<GenerationResult, String> {
    let client = resolve_client(&app_handle, &state).await?;
    ensure_desired_model_loaded(&client, &state).await?;
    client
        .generate_text(&prompt, None)
        .await
        .map_err(|error| format!("Generation failed: {error}"))
}

#[tauri::command]
async fn inference_generate(
    prompt: String,
    config: Option<GenerationConfig>,
    on_token: Channel<String>,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineBridgeState>,
) -> Result<GenerationMetrics, String> {
    let client = resolve_client(&app_handle, &state).await?;
    ensure_desired_model_loaded(&client, &state).await?;
    client
        .generate_stream(&prompt, config, |token| {
            if let Err(error) = on_token.send(token) {
                log::warn!("Failed to send token via channel: {error}");
            }
        })
        .await
        .map_err(|error| format!("Streaming generation failed: {error}"))
}

#[tauri::command]
async fn inference_cancel(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineBridgeState>,
) -> Result<(), String> {
    let client = resolve_client(&app_handle, &state).await?;
    client
        .cancel()
        .await
        .map_err(|error| format!("Failed to cancel generation: {error}"))
}

#[tauri::command]
async fn is_generating(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineBridgeState>,
) -> Result<bool, String> {
    let client = resolve_client(&app_handle, &state).await?;
    let status = client
        .status()
        .await
        .map_err(|error| format!("Failed to query generation state: {error}"))?;
    Ok(status.generating)
}

#[tauri::command]
async fn get_current_model(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineBridgeState>,
) -> Result<Option<String>, String> {
    let client = resolve_client(&app_handle, &state).await?;
    let status = client
        .status()
        .await
        .map_err(|error| format!("Failed to query current model: {error}"))?;
    Ok(status.current_model)
}

#[tauri::command]
async fn get_inference_backend_status(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineBridgeState>,
) -> Result<BackendStatus, String> {
    let client = resolve_client(&app_handle, &state).await?;
    let status = client
        .status()
        .await
        .map_err(|error| format!("Failed to query backend status: {error}"))?;
    Ok(status.backend_status)
}

#[tauri::command]
async fn check_model_readiness(
    model_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineBridgeState>,
) -> Result<CheckModelResponse, String> {
    let client = resolve_client(&app_handle, &state).await?;
    client
        .check_model_readiness(&model_id)
        .await
        .map_err(|error| format!("Failed to check model readiness for '{model_id}': {error}"))
}

#[tauri::command]
async fn check_model_exists(
    model_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineBridgeState>,
) -> Result<bool, String> {
    Ok(check_model_readiness(model_id, app_handle, state)
        .await?
        .any_ready())
}

#[tauri::command]
async fn create_integration_issue_report(
    request_payload: Option<serde_json::Value>,
    http_status: Option<u16>,
    response_body: Option<String>,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineBridgeState>,
) -> Result<IntegrationIssueReport, String> {
    let client = resolve_client(&app_handle, &state).await?;
    let meta = client
        .meta()
        .await
        .map_err(|error| format!("Failed to query engine meta for issue report: {error}"))?;
    let status = client
        .status()
        .await
        .map_err(|error| format!("Failed to query engine status for issue report: {error}"))?;

    Ok(IntegrationIssueReport {
        app_name: "SmolPC LibreOffice Assistant".to_string(),
        app_version: app_handle.package_info().version.to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        hardware_summary: hardware_summary(&status),
        request_payload,
        http_status,
        response_body,
        engine_status: map_engine_status_snapshot(&status),
        engine_meta: map_engine_meta_snapshot(&meta),
        runtime_overrides: runtime_override_snapshot(),
    })
}

#[tauri::command]
async fn run_runtime_verification_checklist(
    model_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, EngineBridgeState>,
) -> Result<RuntimeVerificationReport, String> {
    let client = resolve_client(&app_handle, &state).await?;
    let health_ok = client
        .health()
        .await
        .map_err(|error| format!("Health check failed during runtime verification: {error}"))?;
    let meta = client
        .meta()
        .await
        .map_err(|error| format!("Meta query failed during runtime verification: {error}"))?;
    let status = client
        .status()
        .await
        .map_err(|error| format!("Status query failed during runtime verification: {error}"))?;
    let readiness = client
        .check_model_readiness(&model_id)
        .await
        .map_err(|error| format!("Readiness query failed during runtime verification: {error}"))?;

    let checks =
        build_runtime_verification_checks(&model_id, health_ok, &meta, &status, &readiness);
    let all_passed = checks.iter().all(|check| check.ok);

    Ok(RuntimeVerificationReport {
        generated_at_unix: now_unix_seconds(),
        model_id,
        checks,
        all_passed,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Debug)
                        .build(),
                )?;
            }

            log::info!("LibreOffice Assistant Phase 1 bootstrap initialized");
            Ok(())
        })
        .manage(EngineBridgeState::default())
        .invoke_handler(tauri::generate_handler![
            get_bootstrap_status,
            ensure_engine_started,
            list_models,
            load_model,
            unload_model,
            generate_text,
            inference_generate,
            inference_cancel,
            is_generating,
            get_current_model,
            get_inference_backend_status,
            check_model_readiness,
            check_model_exists,
            create_integration_issue_report,
            run_runtime_verification_checklist
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{build_runtime_verification_checks, desired_model_to_restore};
    use smolpc_engine_client::{EngineMeta, EngineStatus};
    use smolpc_engine_core::inference::backend::{
        BackendSelectionState, CheckModelResponse, ModelLaneReadiness, ModelLaneReadinessByBackend,
    };

    fn sample_meta() -> EngineMeta {
        EngineMeta {
            ok: true,
            protocol_version: "1.0.0".to_string(),
            engine_api_version: "1.2.0".to_string(),
            engine_version: "0.1.0".to_string(),
            pid: 42,
            busy: false,
        }
    }

    fn sample_status() -> EngineStatus {
        let mut backend_status = smolpc_engine_core::inference::backend::BackendStatus::default();
        backend_status.runtime_engine = Some("genai_dml".to_string());
        backend_status.selection_reason = Some("default_directml_candidate".to_string());
        backend_status.selection_state = Some(BackendSelectionState::Ready);

        EngineStatus {
            ok: true,
            ready: true,
            attempt_id: "attempt-1".to_string(),
            state: Some("ready".to_string()),
            startup_phase: Some("ready".to_string()),
            state_since: Some("2026-03-01T00:00:00Z".to_string()),
            active_backend: Some("directml".to_string()),
            active_model_id: Some("qwen3-4b-instruct-2507".to_string()),
            error_code: None,
            error_message: None,
            retryable: Some(false),
            last_error: None,
            engine_api_version: "1.2.0".to_string(),
            effective_mode: Some("auto".to_string()),
            effective_startup_policy: None,
            current_model: Some("qwen3-4b-instruct-2507".to_string()),
            generating: false,
            backend_status,
        }
    }

    fn sample_readiness() -> CheckModelResponse {
        CheckModelResponse {
            model_id: "qwen3-4b-instruct-2507".to_string(),
            lanes: ModelLaneReadinessByBackend {
                openvino_npu: ModelLaneReadiness {
                    artifact_ready: false,
                    bundle_ready: true,
                    ready: false,
                    reason: "runtime_unavailable".to_string(),
                },
                directml: ModelLaneReadiness {
                    artifact_ready: true,
                    bundle_ready: true,
                    ready: true,
                    reason: "ready".to_string(),
                },
                cpu: ModelLaneReadiness {
                    artifact_ready: true,
                    bundle_ready: true,
                    ready: true,
                    reason: "ready".to_string(),
                },
            },
        }
    }

    #[test]
    fn desired_model_to_restore_requests_reload_after_restart() {
        assert_eq!(
            desired_model_to_restore(Some("qwen3-4b-instruct-2507"), None),
            Some("qwen3-4b-instruct-2507")
        );
    }

    #[test]
    fn desired_model_to_restore_skips_reload_when_model_matches() {
        assert_eq!(
            desired_model_to_restore(
                Some("qwen3-4b-instruct-2507"),
                Some("qwen3-4b-instruct-2507")
            ),
            None
        );
    }

    #[test]
    fn runtime_verification_checks_pass_with_ready_status_and_v1_protocol() {
        let checks = build_runtime_verification_checks(
            "qwen3-4b-instruct-2507",
            true,
            &sample_meta(),
            &sample_status(),
            &sample_readiness(),
        );

        assert!(checks.iter().all(|check| check.ok));
    }

    #[test]
    fn runtime_verification_checks_fail_when_protocol_major_is_not_v1() {
        let mut meta = sample_meta();
        meta.protocol_version = "2.0.0".to_string();
        let checks = build_runtime_verification_checks(
            "qwen3-4b-instruct-2507",
            true,
            &meta,
            &sample_status(),
            &sample_readiness(),
        );

        let protocol_check = checks
            .iter()
            .find(|check| check.id == "protocol_major_v1")
            .expect("protocol check should exist");
        assert!(!protocol_check.ok);
    }
}
