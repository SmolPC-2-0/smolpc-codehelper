mod benchmark;
mod commands;
mod hardware;
mod launcher;
mod security;

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
use launcher::orchestrator::LauncherState;
#[allow(clippy::missing_panics_doc)]
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

            log::info!("Hardware detection will occur on first request");
            Ok(())
        })
        .manage(HardwareCache::default())
        .manage(InferenceState::default())
        .manage(LauncherState::default())
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
            launcher_launch_or_focus
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
