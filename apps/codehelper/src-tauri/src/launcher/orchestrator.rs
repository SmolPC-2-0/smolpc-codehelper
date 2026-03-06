use super::catalog;
use super::types::{EngineApiGateInfo, LaunchAction, LauncherLaunchResult, LauncherManifestApp};
use smolpc_engine_client::{
    connect_or_spawn, engine_api_major_compatible, read_runtime_env_overrides, version_major,
    EngineClient, EngineConnectOptions, EngineMeta, EngineStatus, StartupMode, StartupPolicy,
    WaitReadyOptions,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use sysinfo::System;
use tauri::Manager;
use tokio::sync::Mutex;

const DEFAULT_ENGINE_PORT: u16 = 19432;
const SHARED_RUNTIME_VENDOR_DIR: &str = "SmolPC";
const SHARED_RUNTIME_DIR: &str = "engine-runtime";
const HOST_DATA_DIR: &str = "host-data";

pub struct LauncherState {
    client: Arc<Mutex<Option<EngineClient>>>,
    connect_lock: Arc<Mutex<()>>,
    launch_lock: Arc<Mutex<()>>,
}

impl Default for LauncherState {
    fn default() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            connect_lock: Arc::new(Mutex::new(())),
            launch_lock: Arc::new(Mutex::new(())),
        }
    }
}

pub async fn launch_or_focus(
    app_id: &str,
    app_handle: &tauri::AppHandle,
    state: &LauncherState,
) -> Result<LauncherLaunchResult, String> {
    let _launch_guard = state.launch_lock.lock().await;
    let manifest = catalog::load_manifest(app_handle)?;
    let app = catalog::find_app(&manifest, app_id)?;
    let client = resolve_engine_client(app_handle, state).await?;

    client
        .ensure_started(StartupMode::Auto, StartupPolicy::default())
        .await
        .map_err(|error| format!("Engine ensure_started failed: {error}"))?;

    let status_after_ensure = client
        .status()
        .await
        .map_err(|error| format!("Engine status failed after ensure_started: {error}"))?;
    let ready_status = if status_after_ensure.is_ready() {
        status_after_ensure
    } else {
        client
            .wait_ready(WaitReadyOptions::default())
            .await
            .map_err(|error| format!("Engine wait_ready failed: {error}"))?
    };

    let meta = client
        .meta()
        .await
        .map_err(|error| format!("Failed to read engine metadata for API gate: {error}"))?;
    let gate = evaluate_engine_api_gate(&app, &meta, &ready_status)?;

    let executable = app.executable_path();
    let action = if is_app_running(&executable) {
        focus_existing_app(&app)?;
        LaunchAction::Focused
    } else {
        launch_app_process(&app)?;
        LaunchAction::Launched
    };

    Ok(LauncherLaunchResult {
        app_id: app.app_id,
        action,
        readiness_state: ready_status.state.or(ready_status.startup_phase),
        readiness_attempt_id: Some(ready_status.attempt_id),
        engine_api_gate: gate,
    })
}

async fn resolve_engine_client(
    app_handle: &tauri::AppHandle,
    state: &LauncherState,
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
        resource_dir,
        models_dir: None,
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

fn evaluate_engine_api_gate(
    app: &LauncherManifestApp,
    meta: &EngineMeta,
    status: &EngineStatus,
) -> Result<EngineApiGateInfo, String> {
    let (actual_version, source) = if !status.engine_api_version.trim().is_empty() {
        (
            status.engine_api_version.clone(),
            "status.engine_api_version".to_string(),
        )
    } else if !meta.engine_api_version.trim().is_empty() {
        (
            meta.engine_api_version.clone(),
            "meta.engine_api_version".to_string(),
        )
    } else {
        (
            meta.protocol_version.clone(),
            "meta.protocol_version_fallback".to_string(),
        )
    };
    let actual_major = version_major(&actual_version);

    if let Some(required_major) = app.min_engine_api_major {
        if !engine_api_major_compatible(&actual_version, required_major) {
            let actual_major_label = actual_major
                .map(|major| major.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            return Err(format!(
                "App '{}' requires engine API major {} but engine reports '{}' (major={}, source={})",
                app.app_id, required_major, actual_version, actual_major_label, source
            ));
        }
    }

    Ok(EngineApiGateInfo {
        required_major: app.min_engine_api_major,
        actual_version,
        actual_major,
        source,
    })
}

fn is_app_running(executable: &Path) -> bool {
    let mut system = System::new_all();
    system.refresh_all();

    let target_file = executable
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase());
    let target_stem = executable
        .file_stem()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase());
    let target_stem_exe = target_stem.as_ref().map(|stem| format!("{stem}.exe"));

    system.processes().values().any(|process| {
        let process_name = process.name().to_string_lossy().to_ascii_lowercase();
        target_file.as_deref() == Some(process_name.as_str())
            || target_stem.as_deref() == Some(process_name.as_str())
            || target_stem_exe.as_deref() == Some(process_name.as_str())
    })
}

fn focus_existing_app(app: &LauncherManifestApp) -> Result<(), String> {
    let Some(command_tokens) = app.focus_command.as_ref() else {
        return Err(format!(
            "App '{}' is already running but no focus_command is configured",
            app.app_id
        ));
    };

    if command_tokens.is_empty() || command_tokens[0].trim().is_empty() {
        return Err(format!(
            "App '{}' focus_command must include an executable token",
            app.app_id
        ));
    }

    let mut command = Command::new(&command_tokens[0]);
    if command_tokens.len() > 1 {
        command.args(&command_tokens[1..]);
    }

    let status = command.status().map_err(|error| {
        format!(
            "Failed to execute focus_command for app '{}': {error}",
            app.app_id
        )
    })?;

    if !status.success() {
        return Err(format!(
            "focus_command for app '{}' exited with status {}",
            app.app_id, status
        ));
    }

    Ok(())
}

fn launch_app_process(app: &LauncherManifestApp) -> Result<(), String> {
    let executable = app.executable_path();
    if !executable.exists() {
        return Err(format!(
            "App '{}' executable path does not exist: {}",
            app.app_id,
            executable.display()
        ));
    }

    let mut command = Command::new(&executable);
    command.args(&app.args);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }

    command.spawn().map_err(|error| {
        format!(
            "Failed to launch app '{}' ({}): {error}",
            app.app_id,
            executable.display()
        )
    })?;

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_required_major(required_major: u64) -> LauncherManifestApp {
        LauncherManifestApp {
            app_id: "codehelper".to_string(),
            display_name: "Code Helper".to_string(),
            exe_path: "C:\\Program Files\\SmolPC\\Code Helper\\SmolPC Code Helper.exe".to_string(),
            args: vec![],
            focus_command: None,
            min_engine_api_major: Some(required_major),
        }
    }

    fn ready_status(engine_api_version: &str) -> EngineStatus {
        EngineStatus {
            ok: true,
            ready: true,
            attempt_id: "attempt-1".to_string(),
            state: Some("ready".to_string()),
            startup_phase: Some("ready".to_string()),
            state_since: None,
            active_backend: Some("directml".to_string()),
            active_model_id: Some("qwen3-4b-instruct-2507".to_string()),
            error_code: None,
            error_message: None,
            retryable: Some(true),
            last_error: None,
            engine_api_version: engine_api_version.to_string(),
            effective_mode: Some("auto".to_string()),
            effective_startup_policy: Some(StartupPolicy::default()),
            current_model: Some("qwen3-4b-instruct-2507".to_string()),
            generating: false,
            backend_status: Default::default(),
        }
    }

    fn meta_with_versions(engine_api_version: &str, protocol_version: &str) -> EngineMeta {
        EngineMeta {
            ok: true,
            protocol_version: protocol_version.to_string(),
            engine_api_version: engine_api_version.to_string(),
            engine_version: "0.1.0".to_string(),
            pid: 1234,
            busy: false,
        }
    }

    #[test]
    fn evaluate_engine_api_gate_accepts_matching_major() {
        let app = app_with_required_major(1);
        let meta = meta_with_versions("1.2.0", "1.0.0");
        let status = ready_status("1.1.0");

        let gate = evaluate_engine_api_gate(&app, &meta, &status).expect("compatible versions");
        assert_eq!(gate.required_major, Some(1));
        assert_eq!(gate.actual_major, Some(1));
        assert_eq!(gate.source, "status.engine_api_version");
    }

    #[test]
    fn evaluate_engine_api_gate_rejects_incompatible_major() {
        let app = app_with_required_major(2);
        let meta = meta_with_versions("1.9.0", "1.0.0");
        let status = ready_status("1.9.0");

        let error =
            evaluate_engine_api_gate(&app, &meta, &status).expect_err("lower major must fail");
        assert!(error.contains("requires engine API major 2"));
    }

    #[test]
    fn evaluate_engine_api_gate_uses_protocol_fallback_when_api_version_missing() {
        let app = app_with_required_major(1);
        let meta = meta_with_versions("", "1.0.0");
        let status = ready_status("");

        let gate = evaluate_engine_api_gate(&app, &meta, &status)
            .expect("protocol fallback should still gate by major");
        assert_eq!(gate.source, "meta.protocol_version_fallback");
        assert_eq!(gate.actual_version, "1.0.0");
    }
}
