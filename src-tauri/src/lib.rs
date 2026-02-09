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
    cancel_generation, check_ollama, generate_stream, get_ollama_models, HttpClient, OllamaConfig,
    StreamCancellation,
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

            if let Some(res_dir) = resource_dir.as_deref() {
                let bundled_models_dir = res_dir.join("models");
                if bundled_models_dir.exists() {
                    if let Err(e) =
                        models::ModelLoader::set_runtime_models_dir(bundled_models_dir.clone())
                    {
                        log::warn!("Failed to configure runtime models directory: {}", e);
                    } else {
                        log::info!(
                            "Configured runtime models directory: {}",
                            bundled_models_dir.display()
                        );
                    }

                    let bundled_model_file = bundled_models_dir
                        .join("qwen2.5-coder-1.5b")
                        .join("model.onnx");
                    let bundled_tokenizer_file = bundled_models_dir
                        .join("qwen2.5-coder-1.5b")
                        .join("tokenizer.json");

                    if !bundled_model_file.exists() || !bundled_tokenizer_file.exists() {
                        let msg = format!(
                            "Bundled model assets are missing. Expected files:\n- {}\n- {}",
                            bundled_model_file.display(),
                            bundled_tokenizer_file.display()
                        );
                        log::error!("{}", msg);
                        if !cfg!(debug_assertions) {
                            return Err(msg.into());
                        }
                    }
                } else if !cfg!(debug_assertions) {
                    let msg = format!(
                        "Bundled models directory not found: {}",
                        bundled_models_dir.display()
                    );
                    log::error!("{}", msg);
                    return Err(msg.into());
                }
            }

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
