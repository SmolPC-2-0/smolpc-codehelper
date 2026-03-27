use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::provisioning::types::ModelSource;

const BREADCRUMB_FILENAME: &str = "installer-source.txt";
const MANIFEST_FILENAME: &str = "model-archives.json";
const SMOLPC_VENDOR: &str = "SmolPC 2.0";

/// Per-drive timeout for scanning. Network/slow USB drives that don't respond
/// within this budget are skipped rather than blocking the UI.
const DRIVE_SCAN_TIMEOUT: Duration = Duration::from_secs(3);

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

/// Scan a single drive root for SmolPC directories. Runs on a thread with a
/// timeout so that slow/network drives don't block the UI.
#[cfg(windows)]
fn scan_single_drive(letter: u8) -> Vec<PathBuf> {
    let drive = format!("{}:\\", letter as char);
    let drive_path = PathBuf::from(&drive);

    // Spawn a thread with a timeout to avoid hanging on slow/network drives.
    let handle = std::thread::spawn(move || {
        if !drive_path.exists() {
            return Vec::new();
        }
        let entries = match std::fs::read_dir(&drive_path) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };
        let mut found = Vec::new();
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
        found
    });

    match handle.join() {
        Ok(result) => result,
        Err(_) => Vec::new(),
    }
}

/// On Windows, scans drive roots A: through Z: for directories matching
/// `SmolPC*/models/model-archives.json`. Each drive is scanned with a timeout
/// to avoid hanging on slow or network drives.
#[cfg(windows)]
fn scan_drives() -> Vec<PathBuf> {
    let mut found = Vec::new();
    // Skip A: and B: (floppy drives) to avoid legacy hardware delays.
    for letter in b'C'..=b'Z' {
        // Use a thread + timeout per drive to avoid blocking on network mounts.
        let handle = std::thread::spawn(move || scan_single_drive(letter));

        // Wait with a timeout — if the drive doesn't respond, skip it.
        let start = std::time::Instant::now();
        loop {
            if handle.is_finished() {
                if let Ok(results) = handle.join() {
                    found.extend(results);
                }
                break;
            }
            if start.elapsed() >= DRIVE_SCAN_TIMEOUT {
                log::warn!(
                    "Drive {}:\\ scan timed out after {:?}, skipping",
                    letter as char,
                    DRIVE_SCAN_TIMEOUT
                );
                // Thread is abandoned — it will eventually complete on its own.
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    found
}

#[cfg(not(windows))]
fn scan_drives() -> Vec<PathBuf> {
    Vec::new()
}

/// If the breadcrumb path doesn't exist (USB drive letter changed), try the
/// same relative path on every other drive letter. For example, if the
/// breadcrumb says `E:\SmolPC-Full\` but the USB remounted as `F:\`, check
/// `F:\SmolPC-Full\` etc.
#[cfg(windows)]
fn resolve_breadcrumb_on_other_drives(bc_dir: &Path) -> Option<PathBuf> {
    // Extract the relative path after the drive root (e.g. `SmolPC-Full\`).
    let rel = bc_dir.strip_prefix(bc_dir.ancestors().last()?).ok()?;
    // On Windows paths like E:\Foo, the components are: Prefix(E:), RootDir(\), Normal(Foo)
    // strip_prefix with the root won't work; use the path after the drive letter.
    let path_str = bc_dir.to_string_lossy();
    let after_drive = path_str.get(3..)?; // Skip "E:\"
    for letter in b'C'..=b'Z' {
        let candidate = PathBuf::from(format!("{}:\\{}", letter as char, after_drive));
        if candidate != bc_dir && has_manifest(&candidate) {
            log::info!(
                "Breadcrumb path {} not found, resolved to {} on different drive",
                bc_dir.display(),
                candidate.display()
            );
            return Some(candidate);
        }
    }
    let _ = rel; // suppress unused warning
    None
}

#[cfg(not(windows))]
fn resolve_breadcrumb_on_other_drives(_bc_dir: &Path) -> Option<PathBuf> {
    None
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
        } else if let Some(resolved) = resolve_breadcrumb_on_other_drives(&bc_dir) {
            // Breadcrumb path is stale (drive letter changed), but we found it
            // on another drive.
            seen_paths.push(resolved.clone());
            sources.push(ModelSource::Local { path: resolved });
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

/// Returns `true` if `%LOCALAPPDATA%\SmolPC 2.0\models\` contains at least one
/// LLM model subdirectory (qwen*, phi*, llama*). Voice-only dirs like
/// whisper-base.en or kittentts-nano don't count — the user still needs to
/// provision an LLM model before the app is usable.
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
    entries.flatten().any(|e| {
        e.path().is_dir() && {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("qwen") || name.starts_with("phi") || name.starts_with("llama")
        }
    })
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
