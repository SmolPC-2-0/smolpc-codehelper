use super::errors::Error;
use crate::benchmark::BenchmarkResults;
use tauri::AppHandle;

/// Run the complete benchmark suite
#[tauri::command]
pub async fn run_benchmark(
    _app_handle: AppHandle,
    _model: String,
    _iterations: Option<usize>,
) -> Result<BenchmarkResults, Error> {
    Err(Error::Other(
        "Benchmarking is disabled for demo readiness because the legacy Ollama benchmark path is not active in this branch.".to_string(),
    ))
}

/// Get the benchmarks directory path
#[tauri::command]
pub fn get_benchmarks_directory(app: tauri::AppHandle) -> Result<String, Error> {
    let dir = crate::benchmark::get_benchmarks_dir_with_app_handle(&app)
        .map_err(|e| Error::Other(format!("Failed to locate benchmarks directory: {}", e)))?;
    Ok(dir.to_string_lossy().to_string())
}

/// Open the benchmarks folder in the system file manager
#[tauri::command]
pub fn open_benchmarks_folder(app: tauri::AppHandle) -> Result<(), Error> {
    let dir = crate::benchmark::get_benchmarks_dir_with_app_handle(&app).map_err(|e| {
        Error::Other(format!(
            "Failed to locate benchmarks directory for opening: {}",
            e
        ))
    })?;

    // Use platform-specific commands to open the folder
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(dir)
            .spawn()
            .map_err(|e| Error::Other(format!("Failed to open folder: {}", e)))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(dir)
            .spawn()
            .map_err(|e| Error::Other(format!("Failed to open folder: {}", e)))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map_err(|e| Error::Other(format!("Failed to open folder: {}", e)))?;
    }

    Ok(())
}
