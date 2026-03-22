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
    check_model_exists, check_model_readiness, get_current_model,
    get_inference_backend_status, inference_cancel, inference_generate,
    inference_generate_messages, is_generating, list_models, load_model,
    set_inference_runtime_mode, unload_model, InferenceState,
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
            Ok(())
        })
        .manage(AssistantState::default())
        .manage(HardwareCache::default())
        .manage(InferenceState::default())
        .manage(LauncherState::default())
        .manage(ModeProviderRegistry::default())
        .manage(SetupState::default())
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
            let _ = tauri::async_runtime::block_on(async {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    state.shutdown_engine(),
                )
                .await
                {
                    Ok(Ok(_)) => log::info!("Engine shut down gracefully"),
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

fn force_kill_engine() {
    let runtime_dir = dirs::data_local_dir()
        .map(|d| d.join("SmolPC").join("engine-runtime"));
    if let Some(dir) = runtime_dir {
        let pid_path = dir.join("engine.pid");
        if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                log::info!("Force-killing engine process (PID {pid})");
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("taskkill")
                        .args(["/F", "/PID", &pid.to_string()])
                        .output();
                }
                #[cfg(unix)]
                {
                    unsafe { libc::kill(pid as i32, libc::SIGKILL); }
                }
            }
        }
    }
}
