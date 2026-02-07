mod benchmark;
mod commands;
mod hardware;
mod inference;
mod models;
mod security;
use commands::benchmark::{get_benchmarks_directory, open_benchmarks_folder, run_benchmark};
use commands::default::{read, save_code, write};
use commands::hardware::{detect_hardware, get_cached_hardware, HardwareCache};
use commands::inference::{
    check_model_exists, generate_text, get_current_model, inference_cancel, inference_generate,
    is_generating, list_models, load_model, unload_model, InferenceState,
};
use commands::ollama::{
    cancel_generation, check_ollama, generate_stream, get_ollama_models, HttpClient,
    OllamaConfig, StreamCancellation,
};
use tauri::Manager;

#[allow(clippy::missing_panics_doc)]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Resolve resource directory for bundled libraries
            let resource_dir = app.path().resource_dir().ok();
            if let Err(e) = inference::init_onnx_runtime(resource_dir.as_deref()) {
                log::error!("Failed to initialize ONNX Runtime: {}", e);

                if !cfg!(debug_assertions) {
                    // Production: ONNX Runtime is required — fail early with a clear message
                    return Err(format!("ONNX Runtime initialization failed: {}", e).into());
                }
                // Dev mode: continue — inference commands will return individual errors
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
            // ONNX inference commands
            load_model,
            unload_model,
            generate_text,
            inference_generate,
            inference_cancel,
            is_generating,
            list_models,
            get_current_model,
            check_model_exists
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
