mod benchmark;
mod commands;
use commands::benchmark::{get_benchmarks_directory, open_benchmarks_folder, run_benchmark};
use commands::default::{read, save_code, write};
use commands::ollama::{cancel_generation, check_ollama, generate_stream, get_ollama_models, StreamCancellation, HttpClient, OllamaConfig};

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
            Ok(())
        })
        .manage(StreamCancellation::default())
        .manage(HttpClient::default())
        .manage(OllamaConfig::default())
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
            open_benchmarks_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
