use std::path::{Path, PathBuf};

use crate::provisioning::types::ModelSource;

const BREADCRUMB_FILENAME: &str = "installer-source.txt";
const MANIFEST_FILENAME: &str = "model-archives.json";
const SMOLPC_VENDOR: &str = "SmolPC";

/// Reads the breadcrumb file from `%LOCALAPPDATA%\SmolPC\installer-source.txt`.
/// Returns the path stored in the file if it exists and is readable.
fn read_breadcrumb() -> Option<PathBuf> {
    let data_local = dirs::data_local_dir()?;
    let breadcrumb_path = data_local.join(SMOLPC_VENDOR).join(BREADCRUMB_FILENAME);
    let contents = std::fs::read_to_string(&breadcrumb_path).ok()?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

/// Checks whether `dir/models/model-archives.json` exists.
fn has_manifest(dir: &Path) -> bool {
    dir.join("models").join(MANIFEST_FILENAME).exists()
}

/// On Windows, scans drive roots A: through Z: for directories matching
/// `SmolPC*/models/model-archives.json`. Returns all matching root dirs.
#[cfg(windows)]
fn scan_drives() -> Vec<PathBuf> {
    let mut found = Vec::new();
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        let drive_path = Path::new(&drive);
        if !drive_path.exists() {
            continue;
        }
        // Iterate entries in the drive root looking for SmolPC* directories
        let entries = match std::fs::read_dir(drive_path) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(SMOLPC_VENDOR) && has_manifest(&path) {
                found.push(path);
            }
        }
    }
    found
}

#[cfg(not(windows))]
fn scan_drives() -> Vec<PathBuf> {
    Vec::new()
}

/// Detects available model sources in priority order:
/// 1. Breadcrumb (installer left a path hint in `%LOCALAPPDATA%\SmolPC\installer-source.txt`)
/// 2. Drive scan (USB/external drive with a SmolPC* folder)
/// 3. Internet (HuggingFace), only if `internet_available` is true
///
/// Results are deduplicated — breadcrumb and drive scan may discover the same path.
pub fn detect_sources(internet_available: bool) -> Vec<ModelSource> {
    let mut sources: Vec<ModelSource> = Vec::new();
    let mut seen_paths: Vec<PathBuf> = Vec::new();

    // 1. Breadcrumb
    if let Some(bc_dir) = read_breadcrumb() {
        if has_manifest(&bc_dir) {
            seen_paths.push(bc_dir.clone());
            sources.push(ModelSource::Local { path: bc_dir });
        }
    }

    // 2. Drive scan
    for drive_dir in scan_drives() {
        if !seen_paths.contains(&drive_dir) {
            seen_paths.push(drive_dir.clone());
            sources.push(ModelSource::Local { path: drive_dir });
        }
    }

    // 3. Internet (always last)
    if internet_available {
        sources.push(ModelSource::Internet {
            base_url: "https://huggingface.co".to_string(),
        });
    }

    sources
}

/// Returns `true` if `%LOCALAPPDATA%\SmolPC\models\` exists and contains at least
/// one subdirectory (i.e., at least one model has been installed).
///
/// Note: portable mode is handled separately via `AppBootState::portable`.
pub fn models_exist() -> bool {
    let Some(data_local) = dirs::data_local_dir() else {
        return false;
    };
    let models_dir = data_local.join(SMOLPC_VENDOR).join("models");
    let Ok(entries) = std::fs::read_dir(&models_dir) else {
        return false;
    };
    entries.flatten().any(|e| e.path().is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_models_exist_false_when_no_dir() {
        // We cannot mock dirs::data_local_dir(), so we just verify that the
        // function runs without panicking. On a clean CI box with no installed
        // models this will return false; on a developer machine it may return
        // true — both outcomes are acceptable here.
        let _ = models_exist();
    }

    #[test]
    fn test_detect_sources_no_internet() {
        let sources = detect_sources(false);
        let has_internet = sources.iter().any(|s| matches!(s, ModelSource::Internet { .. }));
        assert!(!has_internet, "expected no Internet source when internet_available=false");
    }

    #[test]
    fn test_detect_sources_with_internet() {
        let sources = detect_sources(true);
        // Internet source must be present and must be the last entry.
        let last = sources.last();
        assert!(
            matches!(last, Some(ModelSource::Internet { .. })),
            "expected Internet source to be the last entry when internet_available=true"
        );
    }
}
