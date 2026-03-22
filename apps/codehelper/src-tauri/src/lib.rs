mod assistant;
mod benchmark;
mod commands;
mod hardware;
mod launcher;
mod modes;
mod security;
mod setup;

use assistant::state::AssistantState;
use commands::assistant::{assistant_cancel, assistant_send, mode_undo};
use commands::benchmark::{get_benchmarks_directory, open_benchmarks_folder, run_benchmark};
use commands::default::{read, save_code, write};
use commands::engine_client_adapter::{engine_ensure_started, engine_status};
use commands::hardware::{detect_hardware, get_cached_hardware, HardwareCache};
use commands::inference::{
    check_model_exists, check_model_readiness, get_current_model, get_inference_backend_status,
    inference_cancel, inference_generate, inference_generate_messages, is_generating, list_models,
    load_model, set_inference_runtime_mode, unload_model, InferenceState,
};
use commands::launcher::{launcher_launch_or_focus, launcher_list_apps};
use commands::modes::{list_modes, mode_refresh_tools, mode_status};
use commands::setup::{setup_prepare, setup_status};
use launcher::orchestrator::LauncherState;
use modes::registry::ModeProviderRegistry;
use setup::SetupState;
use tauri::Manager;

#[allow(clippy::missing_panics_doc)]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Debug)
                        .build(),
                )?;
            }

            log::info!("Hardware detection will occur on first request");

            let resource_dir = app.path().resource_dir().ok();
            let app_local_data_dir = app.path().app_local_data_dir().ok();
            app.manage(SetupState::new(resource_dir, app_local_data_dir));

            Ok(())
        })
        .manage(AssistantState::default())
        .manage(HardwareCache::default())
        .manage(InferenceState::default())
        .manage(LauncherState::default())
        .manage(ModeProviderRegistry::default())
        .invoke_handler(tauri::generate_handler![
            read,
            write,
            save_code,
            run_benchmark,
            get_benchmarks_directory,
            open_benchmarks_folder,
            detect_hardware,
            get_cached_hardware,
            load_model,
            unload_model,
            inference_generate,
            inference_generate_messages,
            inference_cancel,
            is_generating,
            list_models,
            get_current_model,
            check_model_readiness,
            check_model_exists,
            get_inference_backend_status,
            set_inference_runtime_mode,
            engine_ensure_started,
            engine_status,
            launcher_list_apps,
            launcher_launch_or_focus,
            assistant_send,
            assistant_cancel,
            mode_undo,
            list_modes,
            mode_status,
            mode_refresh_tools,
            setup_status,
            setup_prepare
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            log::info!("App exit requested, shutting down engine");
            let state = app_handle.state::<InferenceState>();
            tauri::async_runtime::block_on(async {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    state.shutdown_engine(),
                )
                .await
                {
                    Ok(Ok(_)) => {
                        log::info!("Engine shut down gracefully");
                        cleanup_engine_pid();
                    }
                    Ok(Err(e)) => {
                        log::warn!("Engine graceful shutdown failed: {e}");
                        force_kill_engine();
                    }
                    Err(_) => {
                        log::warn!("Engine shutdown timed out after 2s");
                        force_kill_engine();
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
    dirs::data_local_dir().map(|d| d.join("SmolPC").join("engine-runtime").join("engine.pid"))
}

fn cleanup_engine_pid() {
    if let Some(path) = engine_pid_path() {
        let _ = std::fs::remove_file(&path);
    }
}

fn force_kill_engine() {
    let Some(pid_path) = engine_pid_path() else {
        return;
    };
    let Ok(pid_str) = std::fs::read_to_string(&pid_path) else {
        return;
    };
    let Ok(pid) = pid_str.trim().parse::<u32>() else {
        return;
    };

    // Verify the PID is still an engine process before killing
    #[cfg(target_os = "windows")]
    {
        // Check if the process name matches before killing
        let check = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
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
                let _ = std::fs::remove_file(&pid_path);
                return;
            }
        }
        log::info!("Force-killing engine process (PID {pid})");
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
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

    let _ = std::fs::remove_file(&pid_path);
}
