mod assistant;
mod benchmark;
mod commands;
mod hardware;
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
    check_model_exists, check_model_readiness, generate_text, get_current_model,
    get_inference_backend_status, inference_cancel, inference_generate,
    inference_generate_messages, is_generating, list_models, load_model,
    set_inference_runtime_mode, unload_model, InferenceState,
};
use commands::modes::{list_modes, mode_refresh_tools, mode_status};
use commands::setup::{setup_prepare, setup_status};
use modes::registry::ModeProviderRegistry;
use setup::SetupState;
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
            let resource_dir = match app.path().resource_dir() {
                Ok(path) => Some(path),
                Err(error) => {
                    log::warn!("Unable to resolve Tauri resource directory: {error}");
                    None
                }
            };
            let app_local_data_dir = match app.path().app_local_data_dir() {
                Ok(path) => Some(path),
                Err(error) => {
                    log::warn!("Unable to resolve Tauri app-local-data directory: {error}");
                    None
                }
            };
            app.manage(SetupState::new(
                resource_dir.clone(),
                app_local_data_dir.clone(),
            ));
            app.manage(ModeProviderRegistry::new(resource_dir, app_local_data_dir));
            Ok(())
        })
        .manage(AssistantState::default())
        .manage(HardwareCache::default())
        .manage(InferenceState::default())
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
            setup_status,
            setup_prepare,
            list_modes,
            mode_status,
            mode_refresh_tools,
            assistant_send,
            assistant_cancel,
            mode_undo
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    #[test]
    fn tauri_config_uses_unified_branding_and_resources() {
        let raw = include_str!("../tauri.conf.json");
        let value: serde_json::Value = serde_json::from_str(raw).expect("parse tauri config");

        assert_eq!(value["productName"], "SmolPC Unified Assistant");
        assert_eq!(value["identifier"], "com.smolpc.codehelper");
        assert_eq!(
            value["app"]["windows"][0]["title"],
            "SmolPC Unified Assistant"
        );

        let resources = value["bundle"]["resources"]
            .as_array()
            .expect("bundle resources");
        assert!(resources
            .iter()
            .any(|entry| entry == "resources/libreoffice"));
        assert!(resources.iter().any(|entry| entry == "resources/blender"));
        assert!(resources.iter().any(|entry| entry == "resources/gimp"));
        assert!(resources.iter().any(|entry| entry == "resources/python"));
        assert!(resources.iter().any(|entry| entry == "resources/models"));
        assert!(!resources.iter().any(|entry| {
            entry
                .as_str()
                .map(|value| value.contains("resources/launcher"))
                .unwrap_or(false)
        }));
    }
}
