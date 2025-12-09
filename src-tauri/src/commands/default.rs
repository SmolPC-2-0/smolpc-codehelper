use super::errors::Error;
use crate::security;
use rfd::FileDialog;
use std::path::PathBuf;
use tauri::AppHandle;

/// Read a file from disk with security validation
///
/// # Security
/// - Validates path is within allowed directories (app data/cache)
/// - Checks file size before reading (max 10 MB)
/// - Uses async I/O to prevent blocking
///
/// # Errors
/// Returns error if:
/// - Path is outside allowed directories
/// - File doesn't exist or is inaccessible
/// - File exceeds size limit
/// - File contains invalid UTF-8
#[tauri::command]
pub async fn read(path: String, app: AppHandle) -> Result<String, Error> {
    // Validate path is within allowed directories
    let validated_path = security::validate_path(&path, &app)
        .map_err(Error::Other)?;

    // Validate file size before reading
    security::validate_file_size(&validated_path)
        .await
        .map_err(Error::Other)?;

    // Read file asynchronously
    tokio::fs::read_to_string(&validated_path).await.map_err(Into::into)
}

/// Write content to a file with security validation
///
/// # Security
/// - Validates parent directory is within allowed directories
/// - Validates content size (max 10 MB)
/// - Uses async I/O to prevent blocking
///
/// # Errors
/// Returns error if:
/// - Path parent directory is outside allowed directories
/// - Path is invalid (no parent or filename)
/// - Content exceeds size limit
/// - Write operation fails
#[tauri::command]
pub async fn write(path: String, contents: String, app: AppHandle) -> Result<(), Error> {
    // Validate content size first (before path operations)
    security::validate_content_size(&contents).map_err(Error::Other)?;

    // For write operations, validate the parent directory exists and is allowed
    // (can't canonicalize non-existent files)
    let path_buf = PathBuf::from(&path);
    let parent = path_buf
        .parent()
        .ok_or_else(|| Error::Other("Invalid path: no parent directory".to_string()))?;

    // Validate parent directory is in allowed locations
    let validated_parent = security::validate_path(parent, &app).map_err(Error::Other)?;

    // Reconstruct full path with validated parent
    let file_name = path_buf
        .file_name()
        .ok_or_else(|| Error::Other("Invalid path: no filename".to_string()))?;
    let validated_path = validated_parent.join(file_name);

    // Write file asynchronously
    tokio::fs::write(&validated_path, contents).await?;

    Ok(())
}

/// Save code to a file with native file dialog
///
/// Note: This uses a file dialog, so user explicitly chooses the location.
/// Path validation is not applied here since user consent is implicit.
#[tauri::command]
pub async fn save_code(code: String) -> Result<(), Error> {
    // Validate content size
    security::validate_content_size(&code).map_err(Error::Other)?;

    let file_path = FileDialog::new()
        .add_filter("All Files", &["*"])
        .add_filter("Python", &["py"])
        .add_filter("JavaScript", &["js"])
        .add_filter("TypeScript", &["ts"])
        .add_filter("Rust", &["rs"])
        .add_filter("HTML", &["html"])
        .add_filter("CSS", &["css"])
        .add_filter("Text", &["txt"])
        .set_file_name("code.txt")
        .save_file();

    if let Some(path) = file_path {
        tokio::fs::write(path, code).await?;
        Ok(())
    } else {
        Err(Error::Other("File save cancelled".to_string()))
    }
}
