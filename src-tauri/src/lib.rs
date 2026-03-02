mod benchmark;
mod commands;
mod hardware;
mod security;

use commands::benchmark::{get_benchmarks_directory, open_benchmarks_folder, run_benchmark};
use commands::default::{read, save_code, write};
use commands::hardware::{detect_hardware, get_cached_hardware, HardwareCache};
use commands::inference::{
    check_model_exists, generate_text, get_current_model, get_inference_backend_status,
    inference_cancel, inference_generate, is_generating, list_models, load_model,
    set_inference_runtime_mode, unload_model, InferenceState,
};
use commands::ollama::{
    cancel_generation, check_ollama, generate_stream, get_ollama_models, HttpClient, OllamaConfig,
    StreamCancellation,
};
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
        .manage(StreamCancellation::default())
        .manage(HttpClient::default())
        .manage(OllamaConfig::default())
        .manage(HardwareCache::default())
        .manage(InferenceState::default())
        .invoke_handler(tauri::generate_handler![
            read,
            write,
            save_code,
            check_ollama,
            get_ollama_models,
            generate_stream,
            cancel_generation,
            run_benchmark,
            get_benchmarks_directory,
            open_benchmarks_folder,
            detect_hardware,
            get_cached_hardware,
            load_model,
            unload_model,
            generate_text,
            inference_generate,
            inference_cancel,
            is_generating,
            list_models,
            get_current_model,
            check_model_exists,
            get_inference_backend_status,
            set_inference_runtime_mode
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
