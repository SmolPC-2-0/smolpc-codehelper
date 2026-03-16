mod assistant;
mod benchmark;
mod commands;
mod hardware;
mod launcher;
mod modes;
mod security;

use assistant::state::AssistantState;
use commands::assistant::{assistant_cancel, assistant_send, mode_undo};
use commands::benchmark::{get_benchmarks_directory, open_benchmarks_folder, run_benchmark};
use commands::default::{read, save_code, write};
use commands::engine_client_adapter::{engine_ensure_started, engine_status};
use commands::hardware::{detect_hardware, get_cached_hardware, HardwareCache};
use commands::inference::{
    check_model_exists, check_model_readiness, generate_text, get_current_model,
    get_inference_backend_status, inference_cancel, inference_generate,
    inference_generate_messages, is_generating, list_models, load_model,
    set_inference_runtime_mode, unload_model, InferenceState,
};
use commands::launcher::{launcher_launch_or_focus, launcher_list_apps};
use commands::modes::{list_modes, mode_refresh_tools, mode_status};
use launcher::orchestrator::LauncherState;
use modes::registry::ModeProviderRegistry;
use tauri::Manager;
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
            let resource_dir = app.path().resource_dir().ok();
            app.manage(ModeProviderRegistry::new(resource_dir));
            Ok(())
        })
        .manage(AssistantState::default())
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
            generate_text,
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
            list_modes,
            mode_status,
            mode_refresh_tools,
            assistant_send,
            assistant_cancel,
            mode_undo,
            launcher_list_apps,
            launcher_launch_or_focus
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
