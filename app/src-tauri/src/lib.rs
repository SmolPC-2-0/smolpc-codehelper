mod app_paths;
mod assistant;
mod commands;
mod engine;
mod hardware;
mod modes;
mod provisioning;
mod security;
mod setup;

use app_paths::{
    bundled_resource_dir_path, bundled_resource_dir_source, default_dev_bundled_resource_dir,
    select_bundled_resource_dir_resolution, BundledResourceDirResolution,
};
use assistant::state::AssistantState;
use commands::assistant::{assistant_cancel, assistant_send, mode_undo};
use commands::default::{read, save_code, write};
use commands::engine_client_adapter::{engine_ensure_started, engine_status};
use commands::hardware::{detect_hardware, get_cached_hardware, HardwareCache};
use commands::inference::{
    check_model_exists, check_model_readiness, evaluate_memory_pressure, get_current_model,
    get_inference_backend_status, inference_cancel, inference_generate,
    inference_generate_messages, is_generating, list_models, load_model,
    set_inference_runtime_mode, unload_model,
};
use commands::modes::{list_modes, mode_open_host_app, mode_refresh_tools, mode_status};
use commands::setup::{setup_prepare, setup_status};
use engine::{EngineLifecycleState, EngineSupervisor, EngineSupervisorHandle};
use provisioning::ProvisioningCancel;
use modes::registry::ModeProviderRegistry;
use setup::SetupState;
use smolpc_engine_client::EngineClient;
use std::path::PathBuf;
use tauri::Manager;

const DIRS_LOCAL_DATA_SOURCE: &str = "dirs::data_local_dir()";
const PLATFORM_ENV_LOCAL_DATA_SOURCE: &str = "platform env fallback";

#[derive(Debug, Clone, PartialEq, Eq)]
enum AppLocalDataResolution {
    Direct(PathBuf),
    Fallback { path: PathBuf, source: &'static str },
}

fn build_managed_state(
    resource_dir: Option<PathBuf>,
    app_local_data_dir: Option<PathBuf>,
) -> (SetupState, ModeProviderRegistry) {
    (
        SetupState::new(resource_dir.clone(), app_local_data_dir.clone()),
        ModeProviderRegistry::new(resource_dir, app_local_data_dir),
    )
}

fn select_app_local_data_resolution(
    identifier: &str,
    tauri_result: Result<PathBuf, String>,
    is_debug: bool,
    dirs_local_data_root: Option<PathBuf>,
    last_resort_local_data_root: Option<PathBuf>,
) -> Option<AppLocalDataResolution> {
    match tauri_result {
        Ok(path) => Some(AppLocalDataResolution::Direct(path)),
        Err(_) if !is_debug => None,
        Err(_) => dirs_local_data_root
            .map(|base| AppLocalDataResolution::Fallback {
                path: base.join(identifier),
                source: DIRS_LOCAL_DATA_SOURCE,
            })
            .or_else(|| {
                last_resort_local_data_root.map(|base| AppLocalDataResolution::Fallback {
                    path: base.join(identifier),
                    source: PLATFORM_ENV_LOCAL_DATA_SOURCE,
                })
            }),
    }
}

fn ensure_fallback_app_local_data_dir(
    path: PathBuf,
    source: &str,
    tauri_error: &str,
) -> Option<PathBuf> {
    match std::fs::create_dir_all(&path) {
        Ok(()) => {
            log::warn!(
                "Using dev-mode app-local-data fallback at {} after Tauri resolution failed: {} (source: {})",
                path.display(),
                tauri_error,
                source
            );
            Some(path)
        }
        Err(create_error) => {
            log::warn!(
                "Unable to create dev-mode app-local-data fallback at {} after Tauri resolution failed: {} (source: {}, create_dir_all error: {})",
                path.display(),
                tauri_error,
                source,
                create_error
            );
            None
        }
    }
}

#[cfg(windows)]
fn debug_last_resort_local_data_root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn debug_last_resort_local_data_root() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Library").join("Application Support"))
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn debug_last_resort_local_data_root() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".local").join("share"))
        })
}

fn resolve_app_local_data_dir<R: tauri::Runtime>(app: &tauri::App<R>) -> Option<PathBuf> {
    let tauri_result = app.path().app_local_data_dir().map_err(|error| {
        let message = error.to_string();
        log::warn!("Unable to resolve Tauri app-local-data directory: {message}");
        message
    });
    let tauri_error = tauri_result.as_ref().err().cloned();

    match select_app_local_data_resolution(
        &app.config().identifier,
        tauri_result,
        cfg!(debug_assertions),
        dirs::data_local_dir(),
        debug_last_resort_local_data_root(),
    ) {
        Some(AppLocalDataResolution::Direct(path)) => Some(path),
        Some(AppLocalDataResolution::Fallback { path, source }) => {
            let tauri_error = tauri_error
                .unwrap_or_else(|| "unknown Tauri app-local-data resolution failure".to_string());
            ensure_fallback_app_local_data_dir(path, source, &tauri_error)
        }
        None => {
            if let Some(tauri_error) = tauri_error {
                log::warn!(
                    "Dev-mode app-local-data fallback is unavailable after Tauri resolution failed: {tauri_error}"
                );
            }
            None
        }
    }
}

fn resolve_bundled_resource_dir<R: tauri::Runtime>(app: &tauri::App<R>) -> Option<PathBuf> {
    let tauri_result = app.path().resource_dir().map_err(|error| {
        let message = error.to_string();
        log::warn!("Unable to resolve Tauri resource directory: {message}");
        message
    });
    let tauri_error = tauri_result.as_ref().err().cloned();
    let tauri_path = tauri_result.as_ref().ok().cloned();

    match select_bundled_resource_dir_resolution(
        tauri_result,
        cfg!(debug_assertions),
        Some(default_dev_bundled_resource_dir()),
    ) {
        Some(
            resolution @ (BundledResourceDirResolution::Direct(_)
            | BundledResourceDirResolution::NestedResources(_)),
        ) => {
            let path = bundled_resource_dir_path(&resolution).to_path_buf();
            let source = bundled_resource_dir_source(&resolution);
            log::info!(
                "Resolved bundled resource base at {} (source: {})",
                path.display(),
                source
            );
            Some(path)
        }
        Some(BundledResourceDirResolution::DevFallback(path)) => {
            if let Some(tauri_path) = tauri_path {
                log::warn!(
                    "Using dev bundled-resource fallback at {} because Tauri resource directory {} did not contain bundled resources directly or under /resources",
                    path.display(),
                    tauri_path.display()
                );
            } else if let Some(tauri_error) = tauri_error {
                log::warn!(
                    "Using dev bundled-resource fallback at {} after Tauri resource directory resolution failed: {}",
                    path.display(),
                    tauri_error
                );
            }
            Some(path)
        }
        None => {
            if let Some(tauri_path) = tauri_path {
                log::warn!(
                    "Bundled resource base is unavailable because Tauri resource directory {} did not contain bundled resources directly or under /resources, and no dev fallback was usable",
                    tauri_path.display()
                );
            } else if let Some(tauri_error) = tauri_error {
                log::warn!(
                    "Bundled resource base is unavailable after Tauri resource directory resolution failed: {tauri_error}"
                );
            }
            None
        }
    }
}

#[allow(clippy::missing_panics_doc)]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            {
                let log_level = if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                };
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log_level)
                        .level_for("hyper_util", log::LevelFilter::Warn)
                        .level_for("reqwest", log::LevelFilter::Warn)
                        .level_for("hyper", log::LevelFilter::Warn)
                        .level_for("tungstenite", log::LevelFilter::Warn)
                        .level_for("tokio_tungstenite", log::LevelFilter::Warn)
                        .level_for("h2", log::LevelFilter::Warn)
                        .level_for("rustls", log::LevelFilter::Warn)
                        .build(),
                )?;
            }

            app.handle().plugin(tauri_plugin_opener::init())?;

            log::info!("Hardware detection will occur on first request");

            let resource_dir = resolve_bundled_resource_dir(app);
            let app_local_data_dir = resolve_app_local_data_dir(app);
            let (setup_state, mode_provider_registry) =
                build_managed_state(resource_dir, app_local_data_dir);
            app.manage(setup_state);
            app.manage(mode_provider_registry);

            // --- Engine Supervisor Setup ---
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(16);
            let (state_tx, state_rx) = tokio::sync::watch::channel(EngineLifecycleState::Idle);
            let (client_tx, client_rx) = tokio::sync::watch::channel::<Option<EngineClient>>(None);
            let (pid_tx, pid_rx) = tokio::sync::watch::channel::<Option<u32>>(None);

            let handle = EngineSupervisorHandle::new(cmd_tx, state_rx, client_rx, pid_rx);

            let supervisor =
                EngineSupervisor::new(cmd_rx, state_tx, client_tx, pid_tx, app.handle().clone());
            tauri::async_runtime::spawn(supervisor.run());
            log::info!("Engine supervisor spawned");

            app.manage(handle);

            Ok(())
        })
        .manage(AssistantState::default())
        .manage(HardwareCache::default())
        .manage(ProvisioningCancel::default())
        .manage(commands::audio::AudioState::default())
        .invoke_handler(tauri::generate_handler![
            read,
            write,
            save_code,
            detect_hardware,
            get_cached_hardware,
            load_model,
            unload_model,
            inference_generate,
            inference_generate_messages,
            inference_cancel,
            is_generating,
            evaluate_memory_pressure,
            list_models,
            get_current_model,
            check_model_readiness,
            check_model_exists,
            get_inference_backend_status,
            set_inference_runtime_mode,
            engine_ensure_started,
            engine_status,
            assistant_send,
            assistant_cancel,
            mode_undo,
            list_modes,
            mode_status,
            mode_refresh_tools,
            mode_open_host_app,
            setup_status,
            setup_prepare,
            commands::audio::start_recording,
            commands::audio::stop_recording,
            commands::audio::speak_text,
            commands::audio::stop_playback,
            commands::audio::is_playing,
            provisioning::get_boot_state,
            provisioning::detect_model_sources,
            provisioning::get_recommended_model,
            provisioning::provision_models,
            provisioning::cancel_provisioning,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            log::info!("App exit requested, shutting down engine");

            // [I2 fix] Stop active audio before engine shutdown to avoid
            // leaving WASAPI sessions in a bad state on Windows.
            let audio = app_handle.state::<commands::audio::AudioState>();
            commands::audio::stop_recording_sync(&audio);
            commands::audio::stop_playback_sync(&audio);

            // Primary: shut down via supervisor handle.
            let supervisor_handle = app_handle.state::<EngineSupervisorHandle>();
            // Snapshot the PID before shutdown clears it — used as fallback for force-kill.
            let last_pid = supervisor_handle.last_engine_pid();
            tauri::async_runtime::block_on(async {
                match tokio::time::timeout(
                    // Must exceed the supervisor's internal 5s shutdown_and_wait
                    // timeout, otherwise this outer timeout always fires first and
                    // graceful shutdown never completes.
                    std::time::Duration::from_secs(8),
                    supervisor_handle.shutdown(),
                )
                .await
                {
                    Ok(Ok(_)) => {
                        log::info!("Engine shut down gracefully via supervisor");
                        cleanup_engine_pid();
                    }
                    Ok(Err(e)) => {
                        log::warn!("Supervisor shutdown returned error: {e}");
                        force_kill_engine(last_pid);
                    }
                    Err(_) => {
                        log::warn!("Supervisor shutdown timed out after 8s");
                        force_kill_engine(last_pid);
                    }
                }
            });
        }
    });
}

/// Matches the write path in engine-client spawn.rs which uses
/// `options.shared_runtime_dir.join("engine.pid")`, where shared_runtime_dir
/// is set to `dirs::data_local_dir()/SmolPC/engine-runtime` in inference.rs:164.
fn engine_pid_path() -> Option<std::path::PathBuf> {
    dirs::data_local_dir().map(|d| d.join("SmolPC 2.0").join("engine-runtime").join("engine.pid"))
}

fn cleanup_engine_pid() {
    if let Some(path) = engine_pid_path() {
        let _ = std::fs::remove_file(&path);
    }
}

fn force_kill_engine(handle_pid: Option<u32>) {
    let pid_path = engine_pid_path();

    // Prefer the in-memory PID from the supervisor handle (always current),
    // fall back to the PID file (may be stale after restarts).
    let pid = if let Some(pid) = handle_pid {
        pid
    } else {
        let Some(ref path) = pid_path else {
            return;
        };
        let Ok(pid_str) = std::fs::read_to_string(path) else {
            return;
        };
        let Ok(pid) = pid_str.trim().parse::<u32>() else {
            return;
        };
        pid
    };

    // Verify the PID is still an engine process before killing
    #[cfg(target_os = "windows")]
    {
        // Check if the process name matches before killing
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let check = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        if let Ok(output) = check {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // tasklist /FO CSV outputs: "Image Name","PID",... — check first column only
            let is_engine = stdout.lines().any(|line| {
                line.split(',')
                    .next()
                    .is_some_and(|name| name.contains("smolpc-engine-host"))
            });
            if !is_engine {
                log::warn!("PID {pid} is not an engine process, skipping force-kill");
                if let Some(ref path) = pid_path {
                    let _ = std::fs::remove_file(path);
                }
                return;
            }
        }
        log::info!("Force-killing engine process (PID {pid})");
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    #[cfg(unix)]
    {
        // SIGKILL because the graceful path already failed — no point in SIGTERM
        log::info!("Force-killing engine process (PID {pid})");
        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }

    if let Some(ref path) = pid_path {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_managed_state, ensure_fallback_app_local_data_dir, select_app_local_data_resolution,
        AppLocalDataResolution, DIRS_LOCAL_DATA_SOURCE, PLATFORM_ENV_LOCAL_DATA_SOURCE,
    };
    use smolpc_assistant_types::AppMode;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn app_local_data_resolution_prefers_tauri_path_when_available() {
        let expected = PathBuf::from("/resolved/by/tauri");

        let resolution = select_app_local_data_resolution(
            "com.smolpc.codehelper",
            Ok(expected.clone()),
            true,
            Some(PathBuf::from("/dirs/local/data")),
            Some(PathBuf::from("/env/local/data")),
        );

        assert_eq!(resolution, Some(AppLocalDataResolution::Direct(expected)));
    }

    #[test]
    fn app_local_data_resolution_uses_dirs_local_data_fallback_in_debug_mode() {
        let resolution = select_app_local_data_resolution(
            "com.smolpc.codehelper",
            Err("tauri path unavailable".to_string()),
            true,
            Some(PathBuf::from("/dirs/local/data")),
            Some(PathBuf::from("/env/local/data")),
        );

        assert_eq!(
            resolution,
            Some(AppLocalDataResolution::Fallback {
                path: PathBuf::from("/dirs/local/data").join("com.smolpc.codehelper"),
                source: DIRS_LOCAL_DATA_SOURCE,
            })
        );
    }

    #[test]
    fn app_local_data_resolution_uses_last_resort_fallback_when_dirs_root_is_missing() {
        let resolution = select_app_local_data_resolution(
            "com.smolpc.codehelper",
            Err("tauri path unavailable".to_string()),
            true,
            None,
            Some(PathBuf::from("/env/local/data")),
        );

        assert_eq!(
            resolution,
            Some(AppLocalDataResolution::Fallback {
                path: PathBuf::from("/env/local/data").join("com.smolpc.codehelper"),
                source: PLATFORM_ENV_LOCAL_DATA_SOURCE,
            })
        );
    }

    #[test]
    fn app_local_data_resolution_stays_unavailable_outside_debug_mode() {
        let resolution = select_app_local_data_resolution(
            "com.smolpc.codehelper",
            Err("tauri path unavailable".to_string()),
            false,
            Some(PathBuf::from("/dirs/local/data")),
            Some(PathBuf::from("/env/local/data")),
        );

        assert_eq!(resolution, None);
    }

    #[test]
    fn ensure_fallback_app_local_data_dir_creates_directory_on_disk() {
        let temp = TempDir::new().expect("temp dir");
        let fallback_path = temp.path().join("com.smolpc.codehelper");

        let resolved = ensure_fallback_app_local_data_dir(
            fallback_path.clone(),
            DIRS_LOCAL_DATA_SOURCE,
            "tauri path unavailable",
        )
        .expect("fallback path");

        assert_eq!(resolved, fallback_path);
        assert!(resolved.is_dir());
    }

    #[tokio::test]
    async fn build_managed_state_passes_app_local_data_dir_to_setup_and_providers() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let (setup_state, registry) = build_managed_state(
            Some(resource_temp.path().to_path_buf()),
            Some(app_temp.path().to_path_buf()),
        );

        assert_eq!(setup_state.resource_dir(), Some(resource_temp.path()));
        assert_eq!(setup_state.app_local_data_dir(), Some(app_temp.path()));

        let provider_state = registry
            .provider_for_mode(AppMode::Gimp)
            .status(AppMode::Gimp)
            .await
            .expect("provider status");

        assert_eq!(provider_state.state, "disconnected");
        assert!(provider_state
            .detail
            .expect("provider detail")
            .contains("GIMP is not installed or could not be detected yet"));
    }
}
