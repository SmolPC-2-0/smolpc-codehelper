mod benchmark;
mod commands;
mod hardware;
mod security;
use commands::benchmark::{get_benchmarks_directory, open_benchmarks_folder, run_benchmark};
use commands::default::{read, save_code, write};
use commands::hardware::{detect_hardware, get_cached_hardware, HardwareCache};
use commands::ollama::{
    cancel_generation, check_ollama, generate_stream, get_ollama_models, HttpClient,
    OllamaConfig, StreamCancellation,
};

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

            // Hardware detection now happens lazily on first request via OnceCell
            // This eliminates startup race conditions and ensures single detection
            // The first call to detect_hardware() or get_cached_hardware() will trigger detection
            log::info!("Hardware detection will occur on first request");

            Ok(())
        })
        .manage(StreamCancellation::default())
        .manage(HttpClient::default())
        .manage(OllamaConfig::default())
        .manage(HardwareCache::default())
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
            get_cached_hardware
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
