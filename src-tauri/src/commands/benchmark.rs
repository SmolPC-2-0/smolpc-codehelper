use super::errors::Error;
use crate::benchmark::{
    create_readme, export_to_csv, run_benchmark_suite, BenchmarkResults,
};
use crate::commands::ollama::{HttpClient, OllamaConfig};
use tauri::{AppHandle, Emitter, State};

/// Run the complete benchmark suite
#[tauri::command]
pub async fn run_benchmark(
    app_handle: AppHandle,
    model: String,
    iterations: Option<usize>,
    client: State<'_, HttpClient>,
    config: State<'_, OllamaConfig>,
) -> Result<BenchmarkResults, Error> {
    let iterations = iterations.unwrap_or(3); // Default to 3 iterations

    // Create README if it doesn't exist
    create_readme()
        .map_err(|e| Error::Other(format!("Failed to create benchmark README file: {}", e)))?;

    // Run benchmark with progress updates
    let results = run_benchmark_suite(model.clone(), iterations, client.get(), &config, |progress| {
        // Emit progress event to frontend
        let _ = app_handle.emit("benchmark_progress", progress);
    })
    .await
    .map_err(|e| Error::Other(format!("Benchmark suite failed for model '{}': {}", model, e)))?;

    // Export to CSV
    let filepath = export_to_csv(&results, "benchmark")
        .map_err(|e| Error::Other(format!("Failed to export benchmark results to CSV: {}", e)))?;

    // Emit completion event with file path
    let _ = app_handle.emit(
        "benchmark_complete",
        filepath.to_string_lossy().to_string(),
    );

    Ok(results)
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
    let dir = crate::benchmark::get_benchmarks_dir_with_app_handle(&app)
        .map_err(|e| Error::Other(format!("Failed to locate benchmarks directory for opening: {}", e)))?;

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
