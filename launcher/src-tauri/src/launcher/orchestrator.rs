use super::catalog;
use super::types::{
    EngineApiGateInfo, InstallerKind, LaunchAction, LauncherInstallOutcome, LauncherInstallResult,
    LauncherInstallState, LauncherLaunchResult, LauncherLaunchableApp, LauncherRegistryApp,
};
use sha2::{Digest, Sha256};
use smolpc_engine_client::{
    connect_or_spawn, engine_api_major_compatible, read_runtime_env_overrides, version_major,
    EngineClient, EngineConnectOptions, EngineMeta, EngineStatus, StartupMode, StartupPolicy,
    WaitReadyOptions,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{ProcessesToUpdate, System};
use tauri::Manager;
use tokio::sync::Mutex;

const DEFAULT_ENGINE_PORT: u16 = 19432;
const SHARED_RUNTIME_VENDOR_DIR: &str = "SmolPC";
const SHARED_RUNTIME_DIR: &str = "engine-runtime";
const HOST_DATA_DIR: &str = "host-data";
const INSTALLER_CACHE_DIR: &str = "launcher/installers";

pub struct LauncherState {
    client: Arc<Mutex<Option<EngineClient>>>,
    connect_lock: Arc<Mutex<()>>,
    launch_lock: Arc<Mutex<()>>,
    install_lock: Arc<Mutex<()>>,
    process_system: Arc<Mutex<System>>,
    missing_registration_attempts: Arc<Mutex<HashMap<String, u8>>>,
    manual_registration_required: Arc<Mutex<HashSet<String>>>,
}

impl Default for LauncherState {
    fn default() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            connect_lock: Arc::new(Mutex::new(())),
            launch_lock: Arc::new(Mutex::new(())),
            install_lock: Arc::new(Mutex::new(())),
            process_system: Arc::new(Mutex::new(System::new_all())),
            missing_registration_attempts: Arc::new(Mutex::new(HashMap::new())),
            manual_registration_required: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

impl LauncherState {
    /// Peek at the cached engine client without spawning.
    /// Returns None if no client is cached or it is unreachable.
    pub async fn peek_client(&self) -> Option<EngineClient> {
        let guard = self.client.lock().await;
        guard.clone()
    }

    pub async fn manual_required_apps(&self) -> HashSet<String> {
        self.manual_registration_required.lock().await.clone()
    }

    pub async fn running_process_names(&self) -> HashSet<String> {
        let mut system = self.process_system.lock().await;
        system.refresh_processes(ProcessesToUpdate::All, true);
        process_names_from_system(&system)
    }

    pub async fn is_app_running(&self, executable: &Path) -> bool {
        let mut system = self.process_system.lock().await;
        system.refresh_processes(ProcessesToUpdate::All, true);
        !matching_process_ids_in_system(&system, executable).is_empty()
    }

    async fn record_missing_registration_attempt(&self, app_id: &str) -> u8 {
        let mut attempts = self.missing_registration_attempts.lock().await;
        let value = attempts.entry(app_id.to_string()).or_insert(0);
        *value = value.saturating_add(1);
        *value
    }

    async fn clear_install_tracking(&self, app_id: &str) {
        self.missing_registration_attempts
            .lock()
            .await
            .remove(app_id);
        self.manual_registration_required
            .lock()
            .await
            .remove(app_id);
    }

    async fn mark_manual_registration_required(&self, app_id: &str) {
        self.manual_registration_required
            .lock()
            .await
            .insert(app_id.to_string());
    }
}

pub async fn launch_or_focus(
    app_id: &str,
    app_handle: &tauri::AppHandle,
    state: &LauncherState,
) -> Result<LauncherLaunchResult, String> {
    let _launch_guard = state.launch_lock.lock().await;
    let resolved = catalog::resolve_app(app_handle, app_id)?;
    let app = resolved.launchable()?;
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
    let action = if state.is_app_running(&executable).await {
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

pub async fn install_app(
    app_id: &str,
    app_handle: &tauri::AppHandle,
    state: &LauncherState,
) -> Result<LauncherInstallResult, String> {
    let _install_guard = state.install_lock.lock().await;
    let catalog_doc = catalog::load_catalog(app_handle)?;
    let catalog_app = catalog::find_catalog_app(&catalog_doc, app_id)?;
    let installer = catalog_app.installer.ok_or_else(|| {
        format!(
            "App '{}' does not define installer metadata in apps.catalog.json",
            catalog_app.display_name
        )
    })?;

    let installer_path = resolve_installer_path(app_handle, app_id, &installer).await?;
    run_installer(&installer.kind, &installer_path)?;

    let resolved = catalog::resolve_app(app_handle, app_id)?;
    if resolved.install_state == LauncherInstallState::Installed {
        state.clear_install_tracking(app_id).await;
        return Ok(LauncherInstallResult {
            app_id: app_id.to_string(),
            outcome: LauncherInstallOutcome::Installed,
            message: format!(
                "App '{}' installed and registered.",
                catalog_app.display_name
            ),
            exe_path: resolved.registration.map(|entry| entry.exe_path),
        });
    }

    let attempts = state.record_missing_registration_attempt(app_id).await;
    let (outcome, message, requires_manual_registration) =
        missing_registration_outcome(&catalog_app.display_name, attempts);
    if requires_manual_registration {
        state.mark_manual_registration_required(app_id).await;
    }

    Ok(LauncherInstallResult {
        app_id: app_id.to_string(),
        outcome,
        message,
        exe_path: resolved.registration.map(|entry| entry.exe_path),
    })
}

pub async fn register_manual_path(
    app_id: &str,
    exe_path: &str,
    app_handle: &tauri::AppHandle,
    state: &LauncherState,
) -> Result<LauncherInstallResult, String> {
    let catalog_doc = catalog::load_catalog(app_handle)?;
    let catalog_app = catalog::find_catalog_app(&catalog_doc, app_id)?;

    let path = PathBuf::from(exe_path.trim());
    if !path.is_absolute() {
        return Err("Manual executable path must be absolute".to_string());
    }
    if !path.exists() {
        return Err(format!(
            "Manual executable path does not exist: {}",
            path.display()
        ));
    }

    #[cfg(target_os = "windows")]
    {
        let is_exe = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
        if !is_exe {
            return Err("Manual executable path must point to a .exe file".to_string());
        }
    }

    #[cfg(not(target_os = "windows"))]
    if !path.is_file() {
        return Err("Manual executable path must point to a file".to_string());
    }

    let existing = catalog::load_registry(app_handle)?
        .apps
        .into_iter()
        .find(|entry| entry.app_id == app_id);
    let args = existing
        .as_ref()
        .map_or_else(Vec::new, |entry| entry.args.clone());
    let launch_command = existing
        .as_ref()
        .and_then(|entry| entry.launch_command.clone());
    let focus_command = existing
        .as_ref()
        .and_then(|entry| entry.focus_command.clone());

    let entry = LauncherRegistryApp {
        app_id: app_id.to_string(),
        exe_path: path.display().to_string(),
        args,
        launch_command,
        focus_command,
        installed_at: catalog::now_utc_timestamp(),
        source: "manual".to_string(),
    };
    let app_handle = app_handle.clone();
    let entry_for_write = entry.clone();
    tokio::task::spawn_blocking(move || {
        catalog::upsert_registry_entry(&app_handle, entry_for_write)
    })
    .await
    .map_err(|error| format!("Failed to join registry write task: {error}"))??;
    state.clear_install_tracking(app_id).await;

    Ok(LauncherInstallResult {
        app_id: app_id.to_string(),
        outcome: LauncherInstallOutcome::Installed,
        message: format!(
            "Manual executable path saved for '{}'.",
            catalog_app.display_name
        ),
        exe_path: Some(entry.exe_path),
    })
}

pub async fn resolve_engine_client(
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
    app: &LauncherLaunchableApp,
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

/// Check if a process matching the given executable path is currently running.
/// Public so commands can call it for per-app status.
pub fn is_app_running(executable: &Path) -> bool {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);
    !matching_process_ids_in_system(&system, executable).is_empty()
}

pub fn is_app_running_in_snapshot(executable: &Path, running_names: &HashSet<String>) -> bool {
    let (target_file, target_stem, target_stem_exe) = executable_name_candidates(executable);
    target_file
        .as_ref()
        .is_some_and(|name| running_names.contains(name))
        || target_stem
            .as_ref()
            .is_some_and(|name| running_names.contains(name))
        || target_stem_exe
            .as_ref()
            .is_some_and(|name| running_names.contains(name))
}

fn process_names_from_system(system: &System) -> HashSet<String> {
    system
        .processes()
        .values()
        .map(|process| process.name().to_string_lossy().to_ascii_lowercase())
        .collect()
}

fn matching_process_ids(executable: &Path) -> Vec<u32> {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);
    matching_process_ids_in_system(&system, executable)
}

fn matching_process_ids_in_system(system: &System, executable: &Path) -> Vec<u32> {
    let (target_file, target_stem, target_stem_exe) = executable_name_candidates(executable);

    system
        .processes()
        .values()
        .filter_map(|process| {
            let process_name = process.name().to_string_lossy().to_ascii_lowercase();
            let matches_target = process_name_matches(
                &process_name,
                target_file.as_deref(),
                target_stem.as_deref(),
                target_stem_exe.as_deref(),
            );
            if matches_target {
                Some(process.pid().as_u32())
            } else {
                None
            }
        })
        .collect()
}

fn executable_name_candidates(
    executable: &Path,
) -> (Option<String>, Option<String>, Option<String>) {
    let target_file = executable
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase());
    let target_stem = executable
        .file_stem()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase());
    let target_stem_exe = target_stem.as_ref().map(|stem| format!("{stem}.exe"));

    (target_file, target_stem, target_stem_exe)
}

fn process_name_matches(
    process_name: &str,
    target_file: Option<&str>,
    target_stem: Option<&str>,
    target_stem_exe: Option<&str>,
) -> bool {
    target_file == Some(process_name)
        || target_stem == Some(process_name)
        || target_stem_exe == Some(process_name)
}

fn focus_existing_app(app: &LauncherLaunchableApp) -> Result<(), String> {
    if let Some(command_tokens) = app.focus_command.as_ref() {
        if !command_tokens.is_empty() && !command_tokens[0].trim().is_empty() {
            let mut command = Command::new(&command_tokens[0]);
            if command_tokens.len() > 1 {
                command.args(&command_tokens[1..]);
            }

            match command.status() {
                Ok(status) if status.success() => return Ok(()),
                Ok(_) | Err(_) => {
                    // Fall through to native window focus if command fails.
                }
            }
        }
    }

    focus_existing_window(&app.executable_path()).map_err(|error| {
        format!(
            "Failed to focus already running app '{}': {error}",
            app.app_id
        )
    })
}

#[cfg(target_os = "windows")]
fn focus_existing_window(executable: &Path) -> Result<(), String> {
    use std::collections::HashSet;
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow, ShowWindow,
        SW_RESTORE,
    };

    struct WindowSearch {
        target_pids: HashSet<u32>,
        matched_hwnd: HWND,
    }

    unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = unsafe { &mut *(lparam as *mut WindowSearch) };
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return 1;
        }

        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if pid != 0 && search.target_pids.contains(&pid) {
            search.matched_hwnd = hwnd;
            return 0;
        }

        1
    }

    let target_pids: HashSet<u32> = matching_process_ids(executable).into_iter().collect();
    if target_pids.is_empty() {
        return Err("no matching process IDs found".to_string());
    }

    let mut search = Box::new(WindowSearch {
        target_pids,
        matched_hwnd: std::ptr::null_mut(),
    });

    unsafe {
        EnumWindows(
            Some(enum_windows_callback),
            (&mut *search as *mut WindowSearch) as LPARAM,
        );
    }

    if search.matched_hwnd.is_null() {
        return Err("no visible window found for running process".to_string());
    }

    unsafe {
        ShowWindow(search.matched_hwnd, SW_RESTORE);
        if SetForegroundWindow(search.matched_hwnd) == 0 {
            return Err("failed to set foreground window".to_string());
        }
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn focus_existing_window(_executable: &Path) -> Result<(), String> {
    Err("native focus without focus_command is currently supported only on Windows".to_string())
}

fn launch_app_process(app: &LauncherLaunchableApp) -> Result<(), String> {
    let launch_target: String;
    let mut command = if cfg!(debug_assertions) {
        if let Some(command_tokens) = app.launch_command.as_ref() {
            if command_tokens.is_empty() || command_tokens[0].trim().is_empty() {
                return Err(format!(
                    "App '{}' launch_command must include an executable token",
                    app.app_id
                ));
            }

            launch_target = command_tokens[0].clone();
            let mut command = Command::new(&command_tokens[0]);
            if command_tokens.len() > 1 {
                command.args(&command_tokens[1..]);
            }
            command
        } else {
            let executable = app.executable_path();
            if !executable.exists() {
                return Err(format!(
                    "App '{}' executable path does not exist: {}",
                    app.app_id,
                    executable.display()
                ));
            }

            launch_target = executable.display().to_string();
            let mut command = Command::new(&executable);
            command.args(&app.args);
            command
        }
    } else {
        let executable = app.executable_path();
        if !executable.exists() {
            return Err(format!(
                "App '{}' executable path does not exist: {}",
                app.app_id,
                executable.display()
            ));
        }

        launch_target = executable.display().to_string();
        let mut command = Command::new(&executable);
        command.args(&app.args);
        command
    };

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }

    command.spawn().map_err(|error| {
        format!(
            "Failed to launch app '{}' ({}): {error}",
            app.app_id, launch_target
        )
    })?;

    Ok(())
}

/// Check if any managed app from the registry is currently running.
pub fn any_app_running(app_handle: &tauri::AppHandle) -> bool {
    let Ok(catalog_doc) = catalog::load_catalog(app_handle) else {
        return false;
    };
    let Ok(registry_doc) = catalog::load_registry(app_handle) else {
        return false;
    };

    catalog::merge_catalog_and_registry(&catalog_doc, &registry_doc)
        .iter()
        .filter_map(|entry| entry.registration.as_ref())
        .any(|entry| is_app_running(&entry.executable_path()))
}

/// Check if any managed app from the registry is running using a shared process snapshot.
pub async fn any_app_running_cached(app_handle: &tauri::AppHandle, state: &LauncherState) -> bool {
    let Ok(catalog_doc) = catalog::load_catalog(app_handle) else {
        return false;
    };
    let Ok(registry_doc) = catalog::load_registry(app_handle) else {
        return false;
    };
    let running_names = state.running_process_names().await;

    catalog::merge_catalog_and_registry(&catalog_doc, &registry_doc)
        .iter()
        .filter_map(|entry| entry.registration.as_ref())
        .any(|entry| is_app_running_in_snapshot(&entry.executable_path(), &running_names))
}

/// Shut down the cached engine client and clear the cache.
pub async fn shutdown_engine(state: &LauncherState) -> Result<(), String> {
    let client = state.client.lock().await.take();
    if let Some(client) = client {
        client
            .shutdown()
            .await
            .map_err(|e| format!("Engine shutdown request failed: {e}"))?;
        client
            .wait_for_shutdown(Duration::from_secs(10))
            .await
            .map_err(|e| format!("Engine shutdown wait failed: {e}"))?;
    }
    Ok(())
}

/// Resolve path to the engine host binary.
/// Launcher is at `launcher/src-tauri/`, so workspace root is `../../`.
fn resolve_host_binary_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("SMOLPC_ENGINE_HOST_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    // launcher/src-tauri -> launcher -> workspace root -> target
    let workspace_target = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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

async fn resolve_installer_path(
    app_handle: &tauri::AppHandle,
    app_id: &str,
    installer: &super::types::LauncherInstallerSpec,
) -> Result<PathBuf, String> {
    let url = installer.url.trim();
    if is_http_url(url) {
        return Err(format!(
            "Installer URL '{}' for app '{}' uses insecure http://; only https:// is allowed for remote installers",
            installer.url, app_id
        ));
    }

    if is_https_url(url) {
        return download_installer(app_id, installer).await;
    }

    if has_url_prefix(url, "file://") {
        let file_url = reqwest::Url::parse(url).map_err(|error| {
            format!("Invalid file:// installer URL '{}': {error}", installer.url)
        })?;
        let path = file_url.to_file_path().map_err(|_| {
            format!(
                "Installer URL '{}' is not a valid local file path",
                installer.url
            )
        })?;
        if !path.exists() {
            return Err(format!("Installer path does not exist: {}", path.display()));
        }
        return Ok(path);
    }

    let direct = PathBuf::from(url);
    if direct.is_absolute() {
        if direct.exists() {
            return Ok(direct);
        }
        return Err(format!(
            "Installer path does not exist: {}",
            direct.display()
        ));
    }

    let mut candidates = Vec::new();
    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        candidates.push(resource_dir.join("launcher").join(url));
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("launcher")
            .join(url),
    );

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(format!(
        "Installer URL '{}' for app '{}' is not a valid absolute path or downloadable URL",
        installer.url, app_id
    ))
}

async fn download_installer(
    app_id: &str,
    installer: &super::types::LauncherInstallerSpec,
) -> Result<PathBuf, String> {
    let installer_url = installer.url.trim();
    if !is_https_url(installer_url) {
        return Err(format!(
            "Installer URL '{}' is not a secure https:// URL",
            installer.url
        ));
    }

    let expected_hash = installer
        .sha256
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "Installer URL '{}' must define installer.sha256 for remote downloads",
                installer.url
            )
        })?;
    if !is_valid_sha256_hex(expected_hash) {
        return Err(format!(
            "Installer URL '{}' has invalid installer.sha256 digest '{}'",
            installer.url, expected_hash
        ));
    }

    let response = reqwest::get(installer_url)
        .await
        .map_err(|error| format!("Failed to download installer '{}': {error}", installer_url))?;
    if !response.status().is_success() {
        return Err(format!(
            "Installer download failed for '{}': HTTP {}",
            installer_url,
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Failed reading installer download bytes: {error}"))?;

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected_hash.to_ascii_lowercase() {
        return Err(format!(
            "Installer SHA-256 mismatch for '{}': expected {}, got {}",
            installer_url, expected_hash, actual
        ));
    }

    let Some(local_data_dir) = dirs::data_local_dir() else {
        return Err("Failed to resolve local data directory for installer cache".to_string());
    };
    let cache_dir = local_data_dir
        .join(SHARED_RUNTIME_VENDOR_DIR)
        .join(INSTALLER_CACHE_DIR);
    tokio::fs::create_dir_all(&cache_dir)
        .await
        .map_err(|error| {
            format!(
                "Failed to create installer cache directory {}: {error}",
                cache_dir.display()
            )
        })?;

    let extension = match installer.kind {
        InstallerKind::Exe => "exe",
        InstallerKind::Msi => "msi",
    };
    let filename = format!("{}-{}.{}", app_id, catalog::now_utc_timestamp(), extension);
    let path = cache_dir.join(filename);
    tokio::fs::write(&path, &bytes).await.map_err(|error| {
        format!(
            "Failed writing downloaded installer to {}: {error}",
            path.display()
        )
    })?;
    Ok(path)
}

fn is_http_url(value: &str) -> bool {
    has_url_prefix(value, "http://")
}

fn is_https_url(value: &str) -> bool {
    has_url_prefix(value, "https://")
}

fn has_url_prefix(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

fn is_valid_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn run_installer(kind: &InstallerKind, installer_path: &Path) -> Result<(), String> {
    let status = match kind {
        InstallerKind::Exe => Command::new(installer_path).status().map_err(|error| {
            format!(
                "Failed to execute installer {}: {error}",
                installer_path.display()
            )
        })?,
        InstallerKind::Msi => Command::new("msiexec")
            .args(["/i"])
            .arg(installer_path)
            .args(["/passive"])
            .status()
            .map_err(|error| {
                format!(
                    "Failed to execute MSI installer {}: {error}",
                    installer_path.display()
                )
            })?,
    };

    if !status.success() {
        return Err(format!(
            "Installer '{}' exited with status {}",
            installer_path.display(),
            status
        ));
    }

    Ok(())
}

fn missing_registration_outcome(
    display_name: &str,
    attempts: u8,
) -> (LauncherInstallOutcome, String, bool) {
    if attempts <= 1 {
        (
            LauncherInstallOutcome::RetryRequired,
            format!(
                "Installer finished, but launcher could not find a usable executable for '{}'. Retry install once.",
                display_name
            ),
            false,
        )
    } else {
        (
            LauncherInstallOutcome::ManualRequired,
            format!(
                "Installer did not register '{}' after retry. Use manual .exe browse to repair registration.",
                display_name
            ),
            true,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_required_major(required_major: u64) -> LauncherLaunchableApp {
        LauncherLaunchableApp {
            app_id: "codehelper".to_string(),
            display_name: "Code Helper".to_string(),
            exe_path: "C:\\Program Files\\SmolPC\\Code Helper\\SmolPC Code Helper.exe".to_string(),
            args: vec![],
            launch_command: None,
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

    #[test]
    fn missing_registration_outcome_requires_retry_before_manual() {
        let (first_outcome, _, first_manual) = missing_registration_outcome("Code Helper", 1);
        let (second_outcome, _, second_manual) = missing_registration_outcome("Code Helper", 2);

        assert!(matches!(
            first_outcome,
            LauncherInstallOutcome::RetryRequired
        ));
        assert!(!first_manual);
        assert!(matches!(
            second_outcome,
            LauncherInstallOutcome::ManualRequired
        ));
        assert!(second_manual);
    }

    #[test]
    fn installer_url_scheme_checks_are_case_insensitive() {
        assert!(is_http_url("HTTP://example.com/installer.exe"));
        assert!(is_https_url("HTTPS://example.com/installer.exe"));
        assert!(!is_http_url("file://C:/installer.exe"));
    }

    #[test]
    fn sha256_validator_enforces_hex_length_and_charset() {
        assert!(is_valid_sha256_hex(&"a".repeat(64)));
        assert!(!is_valid_sha256_hex("a"));
        assert!(!is_valid_sha256_hex(&"g".repeat(64)));
    }
}
