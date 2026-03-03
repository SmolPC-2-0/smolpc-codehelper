use super::errors::Error;
use crate::security;
use rfd::FileDialog;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFileInput {
    pub file_name: String,
    pub contents: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceWriteResult {
    pub written_paths: Vec<String>,
    pub conflicts: Vec<String>,
}

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
    let validated_path = security::validate_path(&path, &app).map_err(Error::Other)?;

    // Validate file size before reading
    security::validate_file_size(&validated_path)
        .await
        .map_err(Error::Other)?;

    // Read file asynchronously
    tokio::fs::read_to_string(&validated_path)
        .await
        .map_err(Into::into)
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

/// Pick a workspace folder using a native folder dialog.
#[tauri::command]
pub fn pick_workspace_folder() -> Result<Option<String>, Error> {
    Ok(FileDialog::new()
        .pick_folder()
        .map(|path| path.to_string_lossy().to_string()))
}

fn validate_workspace_filename(file_name: &str) -> Result<String, Error> {
    let trimmed = file_name.trim();
    if trimmed.is_empty() {
        return Err(Error::Other("Invalid file name: empty".to_string()));
    }

    if trimmed == "." || trimmed == ".." {
        return Err(Error::Other("Invalid file name".to_string()));
    }

    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err(Error::Other(
            "Invalid file name: nested paths are not allowed".to_string(),
        ));
    }

    let file_path = Path::new(trimmed);
    let Some(name) = file_path.file_name() else {
        return Err(Error::Other("Invalid file name".to_string()));
    };

    let normalized = name.to_string_lossy().trim().to_string();
    if normalized.is_empty() {
        return Err(Error::Other("Invalid file name".to_string()));
    }

    Ok(normalized)
}

/// Write one or more files to a user-selected workspace directory.
///
/// The workspace path must point to an existing directory chosen by the user.
/// File names are limited to basename-only values to prevent path traversal.
#[tauri::command]
pub async fn write_workspace_files(
    workspace_path: String,
    files: Vec<WorkspaceFileInput>,
    overwrite: bool,
) -> Result<WorkspaceWriteResult, Error> {
    if files.is_empty() {
        return Err(Error::Other("No files provided".to_string()));
    }

    let workspace = PathBuf::from(&workspace_path);
    let canonical_workspace = std::fs::canonicalize(&workspace)
        .map_err(|e| Error::Other(format!("Workspace path is invalid: {e}")))?;
    if !canonical_workspace.is_dir() {
        return Err(Error::Other(
            "Workspace path must be a directory".to_string(),
        ));
    }

    let mut unique_names = HashSet::new();
    let mut pending_writes = Vec::with_capacity(files.len());

    for file in files {
        security::validate_content_size(&file.contents).map_err(Error::Other)?;

        let validated_name = validate_workspace_filename(&file.file_name)?;
        if !unique_names.insert(validated_name.clone()) {
            return Err(Error::Other(format!(
                "Duplicate filename in request: {}",
                validated_name
            )));
        }

        let target_path = canonical_workspace.join(&validated_name);
        pending_writes.push((target_path, file.contents));
    }

    let mut conflicts = Vec::new();
    for (path, _) in &pending_writes {
        if path.exists() {
            conflicts.push(path.to_string_lossy().to_string());
        }
    }

    if !overwrite && !conflicts.is_empty() {
        return Ok(WorkspaceWriteResult {
            written_paths: Vec::new(),
            conflicts,
        });
    }

    let mut written_paths = Vec::with_capacity(pending_writes.len());
    for (path, contents) in pending_writes {
        tokio::fs::write(&path, contents).await?;
        written_paths.push(path.to_string_lossy().to_string());
    }

    Ok(WorkspaceWriteResult {
        written_paths,
        conflicts: Vec::new(),
    })
}
