mod commands;
use commands::default::{read, write};
use commands::ollama::{check_ollama, generate_stream, get_ollama_models};

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
        .invoke_handler(tauri::generate_handler![
            read,
            write,
            check_ollama,
            get_ollama_models,
            generate_stream
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
