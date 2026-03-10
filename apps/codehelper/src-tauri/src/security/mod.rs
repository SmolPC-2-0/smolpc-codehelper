use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// Maximum file size for read/write operations (10 MB)
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Validates that a file path is within allowed directories
///
/// Uses canonicalization to resolve symlinks and normalize paths,
/// then verifies the canonical path starts with an allowed base directory.
///
/// # Security
/// - Prevents path traversal attacks (../ sequences)
/// - Resolves symlinks before validation (prevents symlink escape)
/// - Uses allowlist approach (only approved directories)
///
/// # Errors
/// Returns error if:
/// - Path doesn't exist (canonicalize requires existing paths)
/// - Path is outside allowed directories
/// - Path resolution fails
pub fn validate_path<P: AsRef<Path>>(path: P, app: &AppHandle) -> Result<PathBuf, String> {
    let path = path.as_ref();
    // Get allowed base directories from Tauri
    let allowed_bases = [
        app.path().app_data_dir(),
        app.path().app_cache_dir(),
        app.path().app_local_data_dir(),
    ];

    // Convert all Results to PathBufs, failing fast on first error
    let allowed_bases: Vec<PathBuf> = allowed_bases
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    // Canonicalize path - resolves symlinks and normalizes .. components
    let canonical = std::fs::canonicalize(path).map_err(|e| {
        log::warn!("Path canonicalization failed for '{}': {}", path.display(), e);
        format!("File not found or inaccessible: {e}")
    })?;

    // Verify canonical path is within at least one allowed directory
    for base in &allowed_bases {
        match std::fs::canonicalize(base) {
            Ok(base_canonical) => {
                if canonical.starts_with(&base_canonical) {
                    log::debug!("Path validated: {:?}", canonical);
                    return Ok(canonical);
                }
            }
            Err(e) => {
                log::warn!(
                    "Failed to canonicalize allowed base directory '{}': {}",
                    base.display(),
                    e
                );
            }
        }
    }

    // Path is outside all allowed directories
    log::warn!(
        "Access denied: path '{}' is outside allowed directories",
        canonical.display()
    );
    Err("Access denied: file outside allowed directories".to_string())
}

/// Validates file size to prevent memory exhaustion attacks
///
/// # Errors
/// Returns error if:
/// - File metadata cannot be read
/// - File size exceeds MAX_FILE_SIZE
pub async fn validate_file_size(path: &Path) -> Result<(), String> {
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|e| format!("Cannot read file metadata: {e}"))?;

    let file_size = metadata.len();
    if file_size > MAX_FILE_SIZE {
        const MB: f64 = 1024.0 * 1024.0;
        return Err(format!(
            "File too large: {:.2} MB (max {:.0} MB)",
            file_size as f64 / MB,
            MAX_FILE_SIZE as f64 / MB
        ));
    }

    Ok(())
}

/// Validates content size before writing to prevent memory exhaustion
///
/// # Errors
/// Returns error if content exceeds MAX_FILE_SIZE
pub fn validate_content_size(content: &str) -> Result<(), String> {
    let content_size = content.len() as u64;
    if content_size > MAX_FILE_SIZE {
        const MB: f64 = 1024.0 * 1024.0;
        return Err(format!(
            "Content too large: {:.2} MB (max {:.0} MB)",
            content_size as f64 / MB,
            MAX_FILE_SIZE as f64 / MB
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
