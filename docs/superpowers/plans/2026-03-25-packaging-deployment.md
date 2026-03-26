# Packaging & Deployment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement first-run model provisioning (local extraction + internet download), portable mode, NSIS installer hooks, and a unified build pipeline producing 5 distribution artifacts.

**Architecture:** New `provisioning/` module in the Tauri app backend handles model source detection, ZIP extraction, HTTP download with resume, and SHA256 verification. Progress streams to a Svelte `SetupWizard` via Tauri Channel. Supervisor gains portable mode awareness. NSIS hooks write a breadcrumb and install VC++ Redistributable. A single `build-release.ps1` script replaces the two existing bundle scripts.

**Tech Stack:** Rust (sha2, zip, reqwest, windows crate), Svelte 5 runes, Tauri 2 IPC Channels, NSIS hooks, PowerShell

**Spec:** `docs/superpowers/specs/2026-03-25-packaging-deployment-design.md`

---

## File Structure

### New Files (Rust backend)

| File | Responsibility |
|------|---------------|
| `app/src-tauri/src/provisioning/mod.rs` | Module root: Tauri commands, re-exports |
| `app/src-tauri/src/provisioning/types.rs` | `ModelSource`, `ProvisioningEvent`, `ProvisioningError`, `ProvisioningErrorCode`, `ModelRecommendation`, `ProvisioningResult` |
| `app/src-tauri/src/provisioning/manifest.rs` | Parse `model-archives.json`, SHA256 verification |
| `app/src-tauri/src/provisioning/extractor.rs` | ZIP extraction with progress callbacks |
| `app/src-tauri/src/provisioning/downloader.rs` | HTTP download with range-request resume |
| `app/src-tauri/src/provisioning/source.rs` | Source detection: breadcrumb, removable drives, internet |
| `app/src-tauri/src/provisioning/singleton.rs` | Windows named mutex for concurrent provisioning guard |

### New Files (Frontend)

| File | Responsibility |
|------|---------------|
| `app/src/lib/stores/provisioning.svelte.ts` | Provisioning state: sources, progress, errors |
| `app/src/lib/components/setup/SetupWizard.svelte` | Full-screen first-run provisioning UI |
| `app/src/lib/components/setup/ProgressPanel.svelte` | Extraction/download progress bar panel |
| `app/src/lib/components/setup/SourceSelector.svelte` | Local vs internet source selection |

### New Files (Installer & Build)

| File | Responsibility |
|------|---------------|
| `app/src-tauri/nsis/hooks.nsh` | NSIS pre/post-install hooks: VC++ Redistributable + breadcrumb |
| `app/scripts/build-release.ps1` | Unified build script with `-Variant` parameter |

### Modified Files

| File | Change |
|------|--------|
| `app/src-tauri/Cargo.toml` | Add `sha2`, `zip`, `hex`, `windows` dependencies |
| `app/src-tauri/src/lib.rs` | Register provisioning commands, add startup model check |
| `app/src-tauri/src/engine/supervisor.rs` | Import `is_portable()` from provisioning, modify `resolve_paths()` for portable mode |
| `app/src-tauri/tauri.conf.json` | Add `installerHooks` to NSIS config |
| `app/src/App.svelte` | Show `SetupWizard` when no models detected |

---

## Task 1: Provisioning Types and Cargo Dependencies

**Files:**
- Create: `app/src-tauri/src/provisioning/types.rs`
- Create: `app/src-tauri/src/provisioning/mod.rs` (empty re-exports for now)
- Modify: `app/src-tauri/Cargo.toml` (add dependencies)

- [ ] **Step 1: Add dependencies to Cargo.toml**

Add under `[dependencies]` in `app/src-tauri/Cargo.toml`:

```toml
sha2 = "0.10"
hex = "0.4"
zip = "2.6"
```

Add Windows crate for singleton mutex (Windows-only):

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    "Win32_System_Threading",
    "Win32_Foundation",
] }
```

- [ ] **Step 2: Create provisioning/types.rs with all shared types**

```rust
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;

/// Where model archives can be found
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ModelSource {
    /// Archives found next to the installer (breadcrumb path or USB)
    Local { path: std::path::PathBuf },
    /// Download from HuggingFace
    Internet { base_url: String },
}

/// A single model archive entry from model-archives.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelArchiveEntry {
    pub id: String,
    pub backend: String,
    pub archive_name: String,
    pub archive_path: String,
    pub sha256: String,
}

/// The model-archives.json manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelArchivesManifest {
    pub version: u32,
    pub models: Vec<ModelArchiveEntry>,
}

/// Hardware-based model recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRecommendation {
    pub model_id: String,
    pub backend: String,
    pub display_name: String,
    pub download_size_bytes: u64,
    pub reason: String,
}

/// Result of a completed provisioning operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisioningResult {
    pub models_installed: Vec<String>,
    pub total_bytes: u64,
}

/// Structured error codes for frontend branching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProvisioningErrorCode {
    DiskFull,
    SourceUnavailable,
    NetworkError,
    ChecksumMismatch,
    ExtractionFailed,
    Cancelled,
    AlreadyRunning,
}

/// Provisioning errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisioningError {
    pub code: ProvisioningErrorCode,
    pub message: String,
}

impl std::fmt::Display for ProvisioningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Events streamed to the frontend via Tauri Channel
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum ProvisioningEvent {
    ArchiveStarted { name: String, total_bytes: u64 },
    Progress { bytes_done: u64, total_bytes: u64 },
    Verifying { name: String },
    ArchiveComplete { name: String },
    Error { code: ProvisioningErrorCode, message: String, retryable: bool },
    Complete { models_installed: Vec<String> },
}

/// Startup state passed to the frontend to determine whether to show SetupWizard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppBootState {
    pub models_provisioned: bool,
    pub portable: bool,
}

/// Detect portable mode: models/ directory exists next to the exe in release builds.
/// Used by both the provisioning startup check and the supervisor's path resolution.
pub fn is_portable() -> bool {
    if cfg!(debug_assertions) {
        return false;
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("models").exists()))
        .unwrap_or(false)
}

/// Get the exe's parent directory
pub fn exe_dir() -> Option<std::path::PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_owned()))
}
```

- [ ] **Step 3: Create provisioning/mod.rs with re-exports**

```rust
pub mod types;

pub use types::*;
```

- [ ] **Step 4: Wire module into lib.rs**

Add `mod provisioning;` to `app/src-tauri/src/lib.rs` (near the other module declarations).

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p smolpc-desktop`
Expected: compiles with no errors

- [ ] **Step 6: Commit**

```bash
git add app/src-tauri/Cargo.toml app/src-tauri/src/provisioning/
git add app/src-tauri/src/lib.rs
git commit -m "feat(provisioning): add types module and cargo dependencies"
```

---

## Task 2: Manifest Parser with Tests

**Files:**
- Create: `app/src-tauri/src/provisioning/manifest.rs`

- [ ] **Step 1: Write test for manifest parsing**

Add to `manifest.rs`:

```rust
use crate::provisioning::types::{ModelArchiveEntry, ModelArchivesManifest};
use std::path::Path;

/// Parse a model-archives.json file
pub fn parse_manifest(path: &Path) -> Result<ModelArchivesManifest, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read manifest at {}: {}", path.display(), e))?;
    let manifest: ModelArchivesManifest = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse manifest: {}", e))?;
    if manifest.version != 1 {
        return Err(format!("Unsupported manifest version: {}", manifest.version));
    }
    Ok(manifest)
}

/// Verify SHA256 checksum of a file
pub fn verify_sha256(path: &Path, expected_hex: &str) -> Result<bool, String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("Cannot open {} for checksum: {}", path.display(), e))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)
            .map_err(|e| format!("Read error during checksum: {}", e))?;
        if n == 0 { break; }
        hasher.update(&buffer[..n]);
    }
    let result = hex::encode(hasher.finalize());
    Ok(result == expected_hex.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_valid_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_path = dir.path().join("model-archives.json");
        std::fs::write(&manifest_path, r#"{
            "version": 1,
            "models": [{
                "id": "qwen2.5-1.5b-instruct",
                "backend": "openvino",
                "archive_name": "qwen2.5-1.5b-instruct-openvino.zip",
                "archive_path": "models/qwen2.5-1.5b-instruct-openvino.zip",
                "sha256": "abc123"
            }]
        }"#).unwrap();

        let manifest = parse_manifest(&manifest_path).unwrap();
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.models.len(), 1);
        assert_eq!(manifest.models[0].id, "qwen2.5-1.5b-instruct");
        assert_eq!(manifest.models[0].backend, "openvino");
    }

    #[test]
    fn test_reject_unsupported_version() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_path = dir.path().join("model-archives.json");
        std::fs::write(&manifest_path, r#"{"version": 99, "models": []}"#).unwrap();

        let result = parse_manifest(&manifest_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported manifest version"));
    }

    #[test]
    fn test_verify_sha256_correct() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.bin");
        std::fs::write(&file_path, b"hello world").unwrap();
        // SHA256 of "hello world"
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert!(verify_sha256(&file_path, expected).unwrap());
    }

    #[test]
    fn test_verify_sha256_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.bin");
        std::fs::write(&file_path, b"hello world").unwrap();
        assert!(!verify_sha256(&file_path, "0000000000000000").unwrap());
    }
}
```

- [ ] **Step 2: Add tempfile dev-dependency to Cargo.toml**

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Add module to mod.rs**

```rust
pub mod manifest;
pub mod types;

pub use types::*;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p smolpc-desktop -- provisioning::manifest`
Expected: 4 tests pass

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/src/provisioning/manifest.rs app/src-tauri/src/provisioning/mod.rs
git add app/src-tauri/Cargo.toml
git commit -m "feat(provisioning): add manifest parser with SHA256 verification"
```

---

## Task 3: ZIP Extractor with Progress

**Files:**
- Create: `app/src-tauri/src/provisioning/extractor.rs`

- [ ] **Step 1: Write extractor with progress callback**

```rust
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Callback for extraction progress
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send>;

/// Extract a ZIP archive to target_dir, reporting progress.
/// Extracts to a temp directory first, then atomically renames.
pub fn extract_zip(
    archive_path: &Path,
    target_dir: &Path,
    cancel: Arc<AtomicBool>,
    on_progress: ProgressCallback,
) -> Result<PathBuf, String> {
    let file = std::fs::File::open(archive_path)
        .map_err(|e| format!("Cannot open archive {}: {}", archive_path.display(), e))?;
    let total_bytes = file.metadata()
        .map_err(|e| format!("Cannot read archive size: {}", e))?.len();

    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Invalid ZIP archive: {}", e))?;

    // Extract to temp dir alongside target to ensure same filesystem (for rename)
    let temp_dir = target_dir.with_extension("extracting");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)
            .map_err(|e| format!("Cannot clean temp dir: {}", e))?;
    }
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Cannot create temp dir: {}", e))?;

    let mut bytes_written: u64 = 0;

    for i in 0..archive.len() {
        if cancel.load(Ordering::Relaxed) {
            // Clean up temp dir on cancel
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err("Extraction cancelled".to_string());
        }

        let mut entry = archive.by_index(i)
            .map_err(|e| {
                // If the source archive became unavailable (USB removed), return a
                // specific error so the Tauri command can send ProvisioningEvent::Error
                // with SourceUnavailable code and retryable: true, prompting the
                // frontend to show "Please reinsert the USB drive" with a Retry button.
                format!("Cannot read archive entry {}: {}", i, e)
            })?;

        let entry_path = entry.enclosed_name()
            .ok_or_else(|| format!("Invalid entry name in archive at index {}", i))?
            .to_owned();
        let out_path = temp_dir.join(&entry_path);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| format!("Cannot create dir {}: {}", out_path.display(), e))?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Cannot create parent dir: {}", e))?;
            }
            let mut outfile = std::fs::File::create(&out_path)
                .map_err(|e| format!("Cannot create file {}: {}", out_path.display(), e))?;
            let copied = std::io::copy(&mut entry, &mut outfile)
                .map_err(|e| format!("Write error for {}: {}", entry_path.display(), e))?;
            bytes_written += copied;
            on_progress(bytes_written, total_bytes);
        }
    }

    // Atomic rename: remove target if exists, rename temp → target
    if target_dir.exists() {
        std::fs::remove_dir_all(target_dir)
            .map_err(|e| format!("Cannot remove existing target dir: {}", e))?;
    }
    std::fs::rename(&temp_dir, target_dir)
        .map_err(|e| format!("Cannot rename temp to target: {}", e))?;

    Ok(target_dir.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_test_zip(dir: &Path) -> PathBuf {
        let zip_path = dir.join("test.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();

        writer.start_file("hello.txt", options).unwrap();
        writer.write_all(b"hello world").unwrap();

        writer.start_file("subdir/nested.txt", options).unwrap();
        writer.write_all(b"nested content").unwrap();

        writer.finish().unwrap();
        zip_path
    }

    #[test]
    fn test_extract_zip_creates_files() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = create_test_zip(dir.path());
        let target = dir.path().join("output");
        let cancel = Arc::new(AtomicBool::new(false));
        let progress_calls = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let pc = progress_calls.clone();

        extract_zip(&zip_path, &target, cancel, Box::new(move |_, _| {
            pc.fetch_add(1, Ordering::Relaxed);
        })).unwrap();

        assert!(target.join("hello.txt").exists());
        assert!(target.join("subdir/nested.txt").exists());
        assert_eq!(std::fs::read_to_string(target.join("hello.txt")).unwrap(), "hello world");
        assert!(progress_calls.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn test_extract_zip_cancel() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = create_test_zip(dir.path());
        let target = dir.path().join("output");
        let cancel = Arc::new(AtomicBool::new(true)); // pre-cancelled

        let result = extract_zip(&zip_path, &target, cancel, Box::new(|_, _| {}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cancelled"));
    }
}
```

- [ ] **Step 2: Add module to mod.rs**

```rust
pub mod extractor;
pub mod manifest;
pub mod types;

pub use types::*;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p smolpc-desktop -- provisioning::extractor`
Expected: 2 tests pass

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/src/provisioning/extractor.rs app/src-tauri/src/provisioning/mod.rs
git commit -m "feat(provisioning): add ZIP extractor with progress and cancellation"
```

---

## Task 4: HTTP Downloader with Resume

**Files:**
- Create: `app/src-tauri/src/provisioning/downloader.rs`

- [ ] **Step 1: Write downloader with range-request resume**

```rust
use reqwest::Client;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

pub type AsyncProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

/// Download a file from `url` to `dest_path` with resume support.
/// If `dest_path` already exists partially, resumes from the last byte.
pub async fn download_file(
    client: &Client,
    url: &str,
    dest_path: &Path,
    cancel: Arc<AtomicBool>,
    on_progress: AsyncProgressCallback,
) -> Result<PathBuf, String> {
    let partial_path = dest_path.with_extension("partial");

    // Check for existing partial download
    let existing_bytes = if partial_path.exists() {
        std::fs::metadata(&partial_path)
            .map(|m| m.len())
            .unwrap_or(0)
    } else {
        0
    };

    // Build request with Range header for resume
    let mut request = client.get(url);
    if existing_bytes > 0 {
        request = request.header("Range", format!("bytes={}-", existing_bytes));
    }

    let response = request.send().await
        .map_err(|e| format!("Download request failed: {}", e))?;

    if !response.status().is_success() && response.status().as_u16() != 206 {
        return Err(format!("Download failed with status: {}", response.status()));
    }

    // Determine total size
    let total_bytes = if response.status().as_u16() == 206 {
        // Partial content — total is existing + remaining
        response.content_length().unwrap_or(0) + existing_bytes
    } else {
        // Full download (server doesn't support range, or fresh start)
        response.content_length().unwrap_or(0)
    };

    // Open file for append (resume) or create fresh
    let mut opts = tokio::fs::OpenOptions::new();
    opts.create(true).write(true);
    if existing_bytes > 0 && response.status().as_u16() == 206 {
        opts.append(true);
    } else {
        opts.truncate(true);
    }
    let mut file = opts.open(&partial_path)
        .await
        .map_err(|e| format!("Cannot open download file: {}", e))?;

    let mut bytes_downloaded = if response.status().as_u16() == 206 { existing_bytes } else { 0 };
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            // Keep partial file for resume on next attempt
            return Err("Download cancelled".to_string());
        }

        let chunk = chunk.map_err(|e| format!("Download stream error: {}", e))?;
        file.write_all(&chunk).await
            .map_err(|e| format!("Write error during download: {}", e))?;
        bytes_downloaded += chunk.len() as u64;
        on_progress(bytes_downloaded, total_bytes);
    }

    file.flush().await.map_err(|e| format!("Flush error: {}", e))?;
    drop(file);

    // Rename partial → final
    std::fs::rename(&partial_path, dest_path)
        .map_err(|e| format!("Cannot finalize download: {}", e))?;

    Ok(dest_path.to_owned())
}

/// Check if a URL is reachable (HEAD request with timeout)
pub async fn check_internet(client: &Client, url: &str) -> bool {
    client.head(url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}
```

- [ ] **Step 2: Add futures-util dependency to Cargo.toml**

Check if `futures-util` or `futures` is already in `Cargo.toml`. If not, add:

```toml
futures-util = "0.3"
```

- [ ] **Step 3: Add module to mod.rs**

```rust
pub mod downloader;
pub mod extractor;
pub mod manifest;
pub mod types;

pub use types::*;
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p smolpc-desktop`
Expected: compiles (no unit tests for downloader — requires network; tested via integration)

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/src/provisioning/downloader.rs app/src-tauri/src/provisioning/mod.rs
git add app/src-tauri/Cargo.toml
git commit -m "feat(provisioning): add HTTP downloader with range-request resume"
```

---

## Task 5: Source Detection

**Files:**
- Create: `app/src-tauri/src/provisioning/source.rs`

- [ ] **Step 1: Write source detection logic**

```rust
use crate::provisioning::types::ModelSource;
use std::path::{Path, PathBuf};

const BREADCRUMB_FILENAME: &str = "installer-source.txt";
const MANIFEST_FILENAME: &str = "model-archives.json";
const SMOLPC_VENDOR: &str = "SmolPC";

/// Read the breadcrumb file written by the NSIS installer.
/// Returns the directory the installer was launched from.
fn read_breadcrumb() -> Option<PathBuf> {
    let local_app_data = dirs::data_local_dir()?;
    let breadcrumb_path = local_app_data.join(SMOLPC_VENDOR).join(BREADCRUMB_FILENAME);
    let content = std::fs::read_to_string(&breadcrumb_path).ok()?;
    let path = PathBuf::from(content.trim());
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Check if a directory contains a valid model manifest
fn has_manifest(dir: &Path) -> bool {
    dir.join("models").join(MANIFEST_FILENAME).exists()
}

/// Scan removable/fixed drives for SmolPC model folders
#[cfg(windows)]
fn scan_drives() -> Vec<PathBuf> {
    let mut found = Vec::new();
    // Check drive letters A-Z
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        let drive_path = PathBuf::from(&drive);
        if !drive_path.exists() {
            continue;
        }
        // Look for SmolPC*/models/model-archives.json
        if let Ok(entries) = std::fs::read_dir(&drive_path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("SmolPC") && entry.path().is_dir() {
                    if has_manifest(&entry.path()) {
                        found.push(entry.path());
                    }
                }
            }
        }
    }
    found
}

#[cfg(not(windows))]
fn scan_drives() -> Vec<PathBuf> {
    Vec::new()
}

/// Detect all available model sources, ordered by priority.
pub fn detect_sources(internet_available: bool) -> Vec<ModelSource> {
    let mut sources = Vec::new();

    // 1. Breadcrumb path (from NSIS installer)
    if let Some(installer_dir) = read_breadcrumb() {
        if has_manifest(&installer_dir) {
            sources.push(ModelSource::Local {
                path: installer_dir.join("models"),
            });
        }
    }

    // 2. Removable/fixed drive scan
    for drive_dir in scan_drives() {
        let models_path = drive_dir.join("models");
        // Avoid duplicating breadcrumb source
        if sources.iter().any(|s| matches!(s, ModelSource::Local { path } if path == &models_path)) {
            continue;
        }
        sources.push(ModelSource::Local { path: models_path });
    }

    // 3. Internet (if reachable)
    if internet_available {
        sources.push(ModelSource::Internet {
            base_url: "https://huggingface.co".to_string(),
        });
    }

    sources
}

/// Check if models are already provisioned
pub fn models_exist() -> bool {
    if let Some(local_app_data) = dirs::data_local_dir() {
        let models_dir = local_app_data.join(SMOLPC_VENDOR).join("models");
        if models_dir.exists() {
            // Check for at least one model subdirectory with content
            if let Ok(entries) = std::fs::read_dir(&models_dir) {
                return entries
                    .flatten()
                    .any(|e| e.path().is_dir() && e.file_name() != "." && e.file_name() != "..");
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_models_exist_false_when_no_dir() {
        // This tests the function logic — on CI/dev machines without models installed,
        // it should return false (no models in %LOCALAPPDATA%\SmolPC\models\)
        // We can't easily mock dirs::data_local_dir, so just verify it doesn't panic
        let _ = models_exist();
    }

    #[test]
    fn test_detect_sources_no_internet() {
        let sources = detect_sources(false);
        // Should not contain Internet source
        assert!(sources.iter().all(|s| matches!(s, ModelSource::Local { .. })));
    }

    #[test]
    fn test_detect_sources_with_internet() {
        let sources = detect_sources(true);
        // Last source should be Internet
        if let Some(last) = sources.last() {
            if sources.iter().any(|s| matches!(s, ModelSource::Internet { .. })) {
                assert!(matches!(last, ModelSource::Internet { .. }));
            }
        }
    }
}
```

- [ ] **Step 2: Add module to mod.rs**

- [ ] **Step 3: Run tests**

Run: `cargo test -p smolpc-desktop -- provisioning::source`
Expected: 3 tests pass

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/src/provisioning/source.rs app/src-tauri/src/provisioning/mod.rs
git commit -m "feat(provisioning): add model source detection (breadcrumb, drives, internet)"
```

---

## Task 6: Singleton Guard

**Files:**
- Create: `app/src-tauri/src/provisioning/singleton.rs`

- [ ] **Step 1: Write Windows named mutex singleton**

```rust
#[cfg(windows)]
mod platform {
    use windows::core::w;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;

    pub struct SingletonGuard {
        handle: HANDLE,
    }

    impl SingletonGuard {
        /// Attempt to acquire the global provisioning mutex.
        /// Returns Ok(guard) if acquired, Err if another instance is already provisioning.
        pub fn acquire() -> Result<Self, String> {
            let handle = unsafe {
                CreateMutexW(None, true, w!("Global\\SmolPC-Provisioning"))
            }.map_err(|e| format!("CreateMutexW failed: {}", e))?;

            // Check if the mutex already existed (another instance holds it)
            if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                unsafe { let _ = CloseHandle(handle); }
                return Err("Another SmolPC instance is already setting up models".to_string());
            }

            Ok(Self { handle })
        }
    }

    impl Drop for SingletonGuard {
        fn drop(&mut self) {
            unsafe { let _ = CloseHandle(self.handle); }
        }
    }
}

#[cfg(not(windows))]
mod platform {
    pub struct SingletonGuard;

    impl SingletonGuard {
        pub fn acquire() -> Result<Self, String> {
            Ok(Self)
        }
    }
}

pub use platform::SingletonGuard;
```

- [ ] **Step 2: Add module to mod.rs**

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p smolpc-desktop`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/src/provisioning/singleton.rs app/src-tauri/src/provisioning/mod.rs
git commit -m "feat(provisioning): add Windows named mutex singleton guard"
```

---

## Task 7: Tauri Commands and Wiring

**Files:**
- Modify: `app/src-tauri/src/provisioning/mod.rs` (add Tauri commands)
- Modify: `app/src-tauri/src/lib.rs` (register commands, startup check)

- [ ] **Step 1: Add shared cancellation state**

Add to `mod.rs` — a managed state struct for the cancel flag:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Shared cancellation flag, managed by Tauri
pub struct ProvisioningCancel(pub Arc<AtomicBool>);

impl Default for ProvisioningCancel {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}
```

- [ ] **Step 2: Implement get_boot_state command**

```rust
#[tauri::command]
pub fn get_boot_state() -> AppBootState {
    AppBootState {
        models_provisioned: source::models_exist(),
        portable: is_portable(),
    }
}
```

- [ ] **Step 3: Implement detect_model_sources command**

```rust
#[tauri::command]
pub async fn detect_model_sources() -> Result<Vec<ModelSource>, ProvisioningError> {
    let client = reqwest::Client::new();
    let internet = downloader::check_internet(
        &client,
        "https://huggingface.co/api/models/OpenVINO/Qwen2.5-1.5B-Instruct-int4-ov",
    ).await;

    Ok(source::detect_sources(internet))
}
```

- [ ] **Step 4: Implement get_recommended_model command**

Uses the existing `HardwareCache` from app state (defined in `app/src-tauri/src/commands/hardware.rs`). This is a **download recommendation**, not a backend selection override — the engine owns backend policy.

```rust
#[tauri::command]
pub async fn get_recommended_model(
    hardware_cache: tauri::State<'_, crate::hardware::HardwareCache>,
) -> Result<ModelRecommendation, ProvisioningError> {
    let hw = hardware_cache.get_or_detect().await
        .map_err(|e| ProvisioningError {
            code: ProvisioningErrorCode::ExtractionFailed,
            message: format!("Hardware detection failed: {}", e),
        })?;

    let total_ram = hw.memory.total_gb;
    let has_npu = hw.npu.is_some();
    let has_discrete_gpu = hw.gpus.iter().any(|g| g.is_discrete);

    // Recommendation table from spec
    let (model_id, backend, display_name, size) = if has_discrete_gpu && total_ram >= 16.0 {
        ("qwen3-4b", "dml", "Qwen 3 4B (DirectML)", 2_900_000_000u64)
    } else if has_discrete_gpu {
        ("qwen2.5-1.5b-instruct", "dml", "Qwen 2.5 1.5B (DirectML)", 1_300_000_000)
    } else if has_npu && total_ram >= 16.0 {
        ("qwen3-4b", "openvino", "Qwen 3 4B (OpenVINO NPU)", 2_200_000_000)
    } else if has_npu {
        ("qwen2.5-1.5b-instruct", "openvino", "Qwen 2.5 1.5B (OpenVINO)", 900_000_000)
    } else {
        ("qwen2.5-1.5b-instruct", "openvino", "Qwen 2.5 1.5B (OpenVINO CPU)", 900_000_000)
    };

    Ok(ModelRecommendation {
        model_id: model_id.to_string(),
        backend: backend.to_string(),
        display_name: display_name.to_string(),
        download_size_bytes: size,
        reason: format!("{:.0} GB RAM, {}", total_ram,
            if has_discrete_gpu { "discrete GPU detected" }
            else if has_npu { "Intel NPU detected" }
            else { "CPU-only" }),
    })
}
```

- [ ] **Step 5: Implement cancel_provisioning command**

```rust
#[tauri::command]
pub fn cancel_provisioning(
    cancel: tauri::State<'_, ProvisioningCancel>,
) {
    cancel.0.store(true, Ordering::Relaxed);
}
```

- [ ] **Step 6: Implement provision_models command (the orchestrator)**

This is the core integration point. It ties together singleton guard, manifest parsing, disk space check, extraction/download, checksum verification, and progress streaming.

```rust
use crate::provisioning::{
    downloader, extractor, manifest, singleton::SingletonGuard, source, types::*,
};

/// Check available disk space at target path. Returns bytes available.
fn available_space(path: &std::path::Path) -> Result<u64, String> {
    // Ensure the directory exists for the check
    std::fs::create_dir_all(path).ok();
    fs2::available_space(path)
        .map_err(|e| format!("Cannot check disk space: {}", e))
}

/// Resolve the models target directory
fn models_target_dir() -> Result<std::path::PathBuf, ProvisioningError> {
    dirs::data_local_dir()
        .map(|d| d.join("SmolPC").join("models"))
        .ok_or_else(|| ProvisioningError {
            code: ProvisioningErrorCode::ExtractionFailed,
            message: "Cannot determine local app data directory".to_string(),
        })
}

#[tauri::command]
pub async fn provision_models(
    app: tauri::AppHandle,
    source: ModelSource,
    model_ids: Vec<String>,
    channel: tauri::ipc::Channel<ProvisioningEvent>,
    cancel_state: tauri::State<'_, ProvisioningCancel>,
) -> Result<ProvisioningResult, ProvisioningError> {
    // 1. Acquire singleton guard
    let _guard = SingletonGuard::acquire()
        .map_err(|msg| ProvisioningError {
            code: ProvisioningErrorCode::AlreadyRunning,
            message: msg,
        })?;

    // Reset cancel flag
    cancel_state.0.store(false, Ordering::Relaxed);
    let cancel = cancel_state.0.clone();

    let target_dir = models_target_dir()?;
    std::fs::create_dir_all(&target_dir).ok();

    let mut installed = Vec::new();
    let mut total_bytes_processed: u64 = 0;

    match &source {
        ModelSource::Local { path } => {
            // 2a. LOCAL: parse manifest, check disk space, extract each archive
            let manifest_path = path.join("model-archives.json");
            let manifest = manifest::parse_manifest(&manifest_path)
                .map_err(|e| ProvisioningError {
                    code: ProvisioningErrorCode::SourceUnavailable,
                    message: e,
                })?;

            // Filter by model_ids if provided (empty = all)
            let archives: Vec<_> = if model_ids.is_empty() {
                manifest.models.clone()
            } else {
                manifest.models.iter()
                    .filter(|m| model_ids.contains(&m.id))
                    .cloned()
                    .collect()
            };

            // Disk space check: sum archive sizes, compare to available
            let total_needed: u64 = archives.iter()
                .filter_map(|a| {
                    let archive_path = path.join(&a.archive_path);
                    std::fs::metadata(&archive_path).ok().map(|m| m.len())
                })
                .sum();

            let available = available_space(&target_dir)
                .unwrap_or(0);
            // Archives extract to roughly their compressed size (models are mostly incompressible)
            if available < total_needed {
                return Err(ProvisioningError {
                    code: ProvisioningErrorCode::DiskFull,
                    message: format!(
                        "Need {} MB free space, only {} MB available",
                        total_needed / 1_048_576,
                        available / 1_048_576
                    ),
                });
            }

            // Extract each archive
            for entry in &archives {
                if cancel.load(Ordering::Relaxed) {
                    return Err(ProvisioningError {
                        code: ProvisioningErrorCode::Cancelled,
                        message: "Cancelled by user".to_string(),
                    });
                }

                let archive_path = path.join(&entry.archive_path);
                let archive_size = std::fs::metadata(&archive_path)
                    .map(|m| m.len()).unwrap_or(0);

                let _ = channel.send(ProvisioningEvent::ArchiveStarted {
                    name: entry.archive_name.clone(),
                    total_bytes: archive_size,
                });

                // Verify checksum before extraction
                let _ = channel.send(ProvisioningEvent::Verifying {
                    name: entry.archive_name.clone(),
                });
                match manifest::verify_sha256(&archive_path, &entry.sha256) {
                    Ok(true) => {},
                    Ok(false) => {
                        let _ = channel.send(ProvisioningEvent::Error {
                            code: ProvisioningErrorCode::ChecksumMismatch,
                            message: format!("{} checksum mismatch", entry.archive_name),
                            retryable: false,
                        });
                        return Err(ProvisioningError {
                            code: ProvisioningErrorCode::ChecksumMismatch,
                            message: format!("Checksum mismatch for {}", entry.archive_name),
                        });
                    },
                    Err(e) => {
                        // Source became unavailable (USB removed?)
                        let _ = channel.send(ProvisioningEvent::Error {
                            code: ProvisioningErrorCode::SourceUnavailable,
                            message: format!("Cannot read {}: {}", entry.archive_name, e),
                            retryable: true,
                        });
                        return Err(ProvisioningError {
                            code: ProvisioningErrorCode::SourceUnavailable,
                            message: e,
                        });
                    },
                }

                // Extract (target_dir is the root models dir; ZIP contains model_id/backend/ structure)
                let cancel_clone = cancel.clone();
                let channel_ref = channel.clone();
                extractor::extract_zip(
                    &archive_path,
                    &target_dir.join(&entry.id).join(&entry.backend),
                    cancel_clone,
                    Box::new(move |done, total| {
                        let _ = channel_ref.send(ProvisioningEvent::Progress {
                            bytes_done: done,
                            total_bytes: total,
                        });
                    }),
                ).map_err(|e| ProvisioningError {
                    code: ProvisioningErrorCode::ExtractionFailed,
                    message: e,
                })?;

                total_bytes_processed += archive_size;
                installed.push(format!("{}/{}", entry.id, entry.backend));

                let _ = channel.send(ProvisioningEvent::ArchiveComplete {
                    name: entry.archive_name.clone(),
                });
            }
        },

        ModelSource::Internet { base_url } => {
            // 2b. INTERNET: download archives from HuggingFace, then extract
            // URL pattern: https://huggingface.co/OpenVINO/{ModelId}-int4-ov/resolve/main/{filename}
            // The exact URL mapping depends on the HuggingFace repo naming convention.
            // For now, use a URL template that the build-release script bakes into a
            // download-manifest.json alongside the app resources.
            //
            // Implementation: download .zip to a temp file, verify SHA256,
            // extract to target_dir, delete the .zip.
            // Same loop structure as local but with download_file() before extract_zip().

            let client = reqwest::Client::builder()
                .user_agent("SmolPC-CodeHelper")
                .build()
                .map_err(|e| ProvisioningError {
                    code: ProvisioningErrorCode::NetworkError,
                    message: format!("HTTP client error: {}", e),
                })?;

            // TODO: Load download manifest from app resources for URL + SHA256 mapping.
            // For initial implementation, construct URLs from model_id + backend:
            // https://huggingface.co/OpenVINO/Qwen2.5-1.5B-Instruct-int4-ov/resolve/main/model.zip
            // This will be refined in the build pipeline task when the manifest format is finalized.

            return Err(ProvisioningError {
                code: ProvisioningErrorCode::NetworkError,
                message: "Internet download not yet implemented — use local USB bundle".to_string(),
            });
        },
    }

    let _ = channel.send(ProvisioningEvent::Complete {
        models_installed: installed.clone(),
    });

    Ok(ProvisioningResult {
        models_installed: installed,
        total_bytes: total_bytes_processed,
    })
}
```

Note: The `ModelSource::Internet` arm is stubbed — it requires a download manifest with HuggingFace URLs and checksums that will be finalized in the build pipeline task. The local extraction path is fully implemented and is the priority for USB testing.

- [ ] **Step 7: Add fs2 dependency to Cargo.toml**

```toml
fs2 = "0.4"
```

- [ ] **Step 8: Update mod.rs with all modules and re-exports**

```rust
pub mod downloader;
pub mod extractor;
pub mod manifest;
pub mod singleton;
pub mod source;
pub mod types;

pub use types::*;

// ... Tauri command implementations above ...
```

- [ ] **Step 9: Register commands in lib.rs**

At `app/src-tauri/src/lib.rs`, add to the `generate_handler![]` macro (around line 275):

```rust
provisioning::get_boot_state,
provisioning::detect_model_sources,
provisioning::get_recommended_model,
provisioning::provision_models,
provisioning::cancel_provisioning,
```

And in the `setup` closure (around line 225), add managed state:

```rust
app.manage(provisioning::ProvisioningCancel::default());
```

- [ ] **Step 10: Verify compilation**

Run: `cargo check -p smolpc-desktop`
Expected: compiles

- [ ] **Step 11: Commit**

```bash
git add app/src-tauri/src/provisioning/ app/src-tauri/src/lib.rs app/src-tauri/Cargo.toml
git commit -m "feat(provisioning): add Tauri commands with full orchestration"
```

---

## Task 8: Portable Mode in Supervisor

**Files:**
- Modify: `app/src-tauri/src/engine/supervisor.rs`

Note: `is_portable()` and `exe_dir()` are defined in `provisioning::types` (Task 1). The supervisor imports them.

- [ ] **Step 1: Import is_portable from provisioning module**

At the top of `supervisor.rs`, add:

```rust
use crate::provisioning::{is_portable, exe_dir};
```

- [ ] **Step 2: Modify resolve_paths() for portable mode**

In `resolve_paths()` (line ~468), add portable branch:

```rust
fn resolve_paths(&self) -> Result<SpawnPaths, String> {
    if is_portable() {
        let exe_dir = exe_dir().ok_or("Cannot determine exe directory")?;
        return Ok(SpawnPaths {
            port: DEFAULT_ENGINE_PORT,
            app_version: self.app_version.clone(),
            shared_runtime_dir: exe_dir.join("data").join("engine-runtime"),
            data_dir: exe_dir.join("data"),
            resource_dir: None, // Key: forces engine to use exe-relative DLL resolution
            models_dir: Some(exe_dir.join("models")),
            host_binary: Some(exe_dir.join("smolpc-engine-host.exe")),
        });
    }
    // ... existing installed-mode logic unchanged ...
}
```

- [ ] **Step 3: Add network drive warning for portable mode**

In the portable branch of `resolve_paths()`, after determining `exe_dir`, check for UNC paths or mapped network drives. If detected, log a warning (the frontend can surface this later):

```rust
if is_portable() {
    let exe_dir = exe_dir().ok_or("Cannot determine exe directory")?;

    // Warn if running from a network drive (slow, may break)
    let path_str = exe_dir.to_string_lossy();
    if path_str.starts_with("\\\\") {
        tracing::warn!("Portable mode running from network path: {} — performance may be degraded", path_str);
    }

    return Ok(SpawnPaths { /* ... same as above ... */ });
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p smolpc-desktop`
Expected: compiles

- [ ] **Step 5: Test portable mode does not break installed mode**

Run: `cargo test -p smolpc-desktop`
Expected: all existing tests pass (portable mode only activates in release builds, tests run in debug)

- [ ] **Step 6: Commit**

```bash
git add app/src-tauri/src/engine/supervisor.rs
git commit -m "feat(engine): add portable mode awareness to supervisor path resolution"
```

---

## Task 9: NSIS Hooks

**Files:**
- Create: `app/src-tauri/nsis/hooks.nsh`
- Modify: `app/src-tauri/tauri.conf.json`

- [ ] **Step 1: Create the NSIS hooks directory and file**

Run: `ls app/src-tauri/` to confirm directory exists, then create `nsis/` subdirectory.

Write `app/src-tauri/nsis/hooks.nsh`:

```nsis
!macro NSIS_HOOK_PREINSTALL
  ; Check for VC++ Redistributable (required by ORT, OpenVINO, DirectML DLLs)
  IfFileExists "$SYSDIR\vcruntime140.dll" vcredist_ok
    ; VC++ Runtime not found — install it
    SetDetailsPrint textonly
    DetailPrint "Installing Visual C++ Redistributable..."
    SetDetailsPrint listonly
    File "/oname=$TEMP\vc_redist.x64.exe" "${NSISDIR}\..\prereqs\vc_redist.x64.exe"
    nsExec::ExecToLog '"$TEMP\vc_redist.x64.exe" /install /quiet /norestart'
    Pop $0
    Delete "$TEMP\vc_redist.x64.exe"
  vcredist_ok:
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Write breadcrumb so the app knows where the installer was launched from.
  ; $EXEDIR = directory containing the installer .exe (e.g., E:\SmolPC-Lite\)
  CreateDirectory "$LOCALAPPDATA\SmolPC"
  FileOpen $0 "$LOCALAPPDATA\SmolPC\installer-source.txt" w
  FileWrite $0 "$EXEDIR"
  FileClose $0
!macroend
```

- [ ] **Step 2: Update tauri.conf.json**

Add `installerHooks` to the NSIS config section (around line 48):

```json
"nsis": {
    "installerHooks": "nsis/hooks.nsh",
    "installMode": "currentUser"
}
```

- [ ] **Step 3: Verify Tauri build configuration**

Run: `cargo check -p smolpc-desktop`
Expected: compiles (hooks are only used during `tauri build`, not `cargo check`)

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/nsis/hooks.nsh app/src-tauri/tauri.conf.json
git commit -m "feat(installer): add NSIS hooks for VC++ redistributable and breadcrumb"
```

---

## Task 10: Provisioning Frontend — Store

**Files:**
- Create: `app/src/lib/stores/provisioning.svelte.ts`

- [ ] **Step 1: Create provisioning store with Svelte 5 runes**

```typescript
import { invoke } from '@tauri-apps/api/core';
import { Channel } from '@tauri-apps/api/core';

// Types matching Rust ProvisioningEvent
interface ProvisioningEvent {
  kind: 'ArchiveStarted' | 'Progress' | 'Verifying' | 'ArchiveComplete' | 'Error' | 'Complete';
  name?: string;
  total_bytes?: number;
  bytes_done?: number;
  code?: string;
  message?: string;
  retryable?: boolean;
  models_installed?: string[];
}

interface ModelSource {
  kind: 'Local' | 'Internet';
  path?: string;
  base_url?: string;
}

interface ModelRecommendation {
  model_id: string;
  backend: string;
  display_name: string;
  download_size_bytes: number;
  reason: string;
}

function createProvisioningStore() {
  let sources = $state<ModelSource[]>([]);
  let recommendation = $state<ModelRecommendation | null>(null);
  let currentArchive = $state<string>('');
  let bytesDown = $state<number>(0);
  let totalBytes = $state<number>(0);
  let phase = $state<'detecting' | 'ready' | 'provisioning' | 'verifying' | 'complete' | 'error'>('detecting');
  let errorMessage = $state<string>('');
  let errorRetryable = $state<boolean>(false);
  let modelsInstalled = $state<string[]>([]);

  let progress = $derived(totalBytes > 0 ? bytesDown / totalBytes : 0);

  async function detectSources() {
    phase = 'detecting';
    try {
      sources = await invoke<ModelSource[]>('detect_model_sources');
      recommendation = await invoke<ModelRecommendation>('get_recommended_model');
      phase = 'ready';
    } catch (e: any) {
      errorMessage = e?.message || String(e);
      phase = 'error';
    }
  }

  async function startProvisioning(source: ModelSource, modelIds: string[]) {
    phase = 'provisioning';
    bytesDown = 0;
    totalBytes = 0;

    const channel = new Channel<ProvisioningEvent>();
    channel.onmessage = (event) => {
      switch (event.kind) {
        case 'ArchiveStarted':
          currentArchive = event.name || '';
          totalBytes = event.total_bytes || 0;
          bytesDown = 0;
          break;
        case 'Progress':
          bytesDown = event.bytes_done || 0;
          totalBytes = event.total_bytes || totalBytes;
          break;
        case 'Verifying':
          phase = 'verifying';
          currentArchive = event.name || '';
          break;
        case 'ArchiveComplete':
          break;
        case 'Error':
          errorMessage = event.message || 'Unknown error';
          errorRetryable = event.retryable || false;
          phase = 'error';
          break;
        case 'Complete':
          modelsInstalled = event.models_installed || [];
          phase = 'complete';
          break;
      }
    };

    try {
      await invoke('provision_models', {
        source,
        modelIds,
        channel,
      });
    } catch (e: any) {
      errorMessage = e?.message || String(e);
      phase = 'error';
    }
  }

  async function cancel() {
    await invoke('cancel_provisioning');
  }

  return {
    get sources() { return sources; },
    get recommendation() { return recommendation; },
    get currentArchive() { return currentArchive; },
    get bytesDown() { return bytesDown; },
    get totalBytes() { return totalBytes; },
    get progress() { return progress; },
    get phase() { return phase; },
    get errorMessage() { return errorMessage; },
    get errorRetryable() { return errorRetryable; },
    get modelsInstalled() { return modelsInstalled; },
    detectSources,
    startProvisioning,
    cancel,
  };
}

export const provisioningStore = createProvisioningStore();
```

- [ ] **Step 2: Verify TypeScript**

Run: `cd app && npm run check`
Expected: passes

- [ ] **Step 3: Commit**

```bash
git add app/src/lib/stores/provisioning.svelte.ts
git commit -m "feat(frontend): add provisioning store with Tauri channel streaming"
```

---

## Task 11: Provisioning Frontend — SetupWizard UI

**Files:**
- Create: `app/src/lib/components/setup/SetupWizard.svelte`
- Create: `app/src/lib/components/setup/ProgressPanel.svelte`
- Create: `app/src/lib/components/setup/SourceSelector.svelte`
- Modify: `app/src/App.svelte`

Note: `app/src/lib/components/setup/` already contains `SetupBanner.svelte` and `SetupPanel.svelte` — these handle connector/model setup within the running app (post-provisioning). The new `SetupWizard.svelte` is distinct: it's the full-screen first-run experience shown BEFORE the main app loads.

- [ ] **Step 1: Create ProgressPanel.svelte**

A reusable progress panel showing: current file, progress bar, speed (for downloads), cancel button.

```svelte
<script lang="ts">
  interface Props {
    archiveName: string;
    bytesDown: number;
    totalBytes: number;
    phase: string;
    oncancel: () => void;
  }
  let { archiveName, bytesDown, totalBytes, phase, oncancel }: Props = $props();

  let progress = $derived(totalBytes > 0 ? (bytesDown / totalBytes) * 100 : 0);
  let sizeLabel = $derived(
    `${(bytesDown / 1024 / 1024).toFixed(0)} MB / ${(totalBytes / 1024 / 1024).toFixed(0)} MB`
  );
</script>

<div class="flex flex-col gap-4 w-full max-w-md">
  <p class="text-sm text-zinc-400">
    {phase === 'verifying' ? 'Verifying...' : 'Extracting...'} {archiveName}
  </p>
  <div class="w-full bg-zinc-800 rounded-full h-3">
    <div
      class="bg-blue-500 h-3 rounded-full transition-all duration-200"
      style="width: {progress}%"
    ></div>
  </div>
  <div class="flex justify-between text-xs text-zinc-500">
    <span>{sizeLabel}</span>
    <span>{progress.toFixed(0)}%</span>
  </div>
  <button
    class="text-sm text-zinc-500 hover:text-zinc-300 underline"
    onclick={oncancel}
  >
    Cancel
  </button>
</div>
```

- [ ] **Step 2: Create SourceSelector.svelte**

Shows detected sources with a "Start" button for local or "Download" for internet.

```svelte
<script lang="ts">
  interface ModelSource {
    kind: 'Local' | 'Internet';
    path?: string;
    base_url?: string;
  }
  interface ModelRecommendation {
    model_id: string;
    backend: string;
    display_name: string;
    download_size_bytes: number;
    reason: string;
  }
  interface Props {
    sources: ModelSource[];
    recommendation: ModelRecommendation | null;
    onselect: (source: ModelSource) => void;
  }
  let { sources, recommendation, onselect }: Props = $props();
</script>

<div class="flex flex-col gap-6 w-full max-w-md">
  {#each sources as source}
    {#if source.kind === 'Local'}
      <button
        class="p-4 rounded-lg border border-zinc-700 hover:border-blue-500 text-left transition-colors"
        onclick={() => onselect(source)}
      >
        <p class="font-medium text-zinc-200">Install from local media</p>
        <p class="text-sm text-zinc-400 mt-1">{source.path}</p>
      </button>
    {:else if source.kind === 'Internet' && recommendation}
      <button
        class="p-4 rounded-lg border border-zinc-700 hover:border-blue-500 text-left transition-colors"
        onclick={() => onselect(source)}
      >
        <p class="font-medium text-zinc-200">Download {recommendation.display_name}</p>
        <p class="text-sm text-zinc-400 mt-1">
          {(recommendation.download_size_bytes / 1024 / 1024).toFixed(0)} MB — {recommendation.reason}
        </p>
      </button>
    {/if}
  {/each}

  {#if sources.length === 0}
    <p class="text-zinc-400 text-center">No model sources found. Connect to the internet or insert the SmolPC USB drive.</p>
  {/if}
</div>
```

- [ ] **Step 3: Create SetupWizard.svelte**

Full-screen wizard that ties SourceSelector and ProgressPanel together:

```svelte
<script lang="ts">
  import { provisioningStore } from '$lib/stores/provisioning.svelte';
  import ProgressPanel from './ProgressPanel.svelte';
  import SourceSelector from './SourceSelector.svelte';
  import { onMount } from 'svelte';

  interface Props {
    oncomplete: () => void;
  }
  let { oncomplete }: Props = $props();

  onMount(() => {
    provisioningStore.detectSources();
  });

  function handleSourceSelect(source: any) {
    // For local: provision all models in manifest
    // For internet: provision recommended model only
    const modelIds = source.kind === 'Internet' && provisioningStore.recommendation
      ? [provisioningStore.recommendation.model_id]
      : []; // empty = all from manifest
    provisioningStore.startProvisioning(source, modelIds);
  }

  $effect(() => {
    if (provisioningStore.phase === 'complete') {
      oncomplete();
    }
  });
</script>

<div class="fixed inset-0 bg-zinc-950 flex flex-col items-center justify-center p-8">
  <h1 class="text-2xl font-bold text-zinc-100 mb-2">SmolPC Setup</h1>

  {#if provisioningStore.phase === 'detecting'}
    <p class="text-zinc-400">Checking for AI models...</p>
  {:else if provisioningStore.phase === 'ready'}
    <p class="text-zinc-400 mb-6">SmolPC needs AI models to get started.</p>
    <SourceSelector
      sources={provisioningStore.sources}
      recommendation={provisioningStore.recommendation}
      onselect={handleSourceSelect}
    />
  {:else if provisioningStore.phase === 'provisioning' || provisioningStore.phase === 'verifying'}
    <p class="text-zinc-400 mb-6">Setting up AI models...</p>
    <ProgressPanel
      archiveName={provisioningStore.currentArchive}
      bytesDown={provisioningStore.bytesDown}
      totalBytes={provisioningStore.totalBytes}
      phase={provisioningStore.phase}
      oncancel={provisioningStore.cancel}
    />
  {:else if provisioningStore.phase === 'error'}
    <div class="text-center">
      <p class="text-red-400 mb-4">{provisioningStore.errorMessage}</p>
      {#if provisioningStore.errorRetryable}
        <button
          class="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-500"
          onclick={() => provisioningStore.detectSources()}
        >
          Retry
        </button>
      {/if}
    </div>
  {/if}
</div>
```

- [ ] **Step 4: Wire into App.svelte**

In `app/src/App.svelte`, add the provisioning check. Read `AppBootState` from the Tauri backend at startup. If `models_provisioned` is false and `portable` is false, show `SetupWizard` instead of the main app:

```svelte
<script lang="ts">
  import SetupWizard from '$lib/components/setup/SetupWizard.svelte';
  import { invoke } from '@tauri-apps/api/core';
  // ... existing imports ...

  let needsSetup = $state(false);
  let setupChecked = $state(false);

  onMount(async () => {
    // Check if models need provisioning
    const bootState = await invoke<{ models_provisioned: boolean; portable: boolean }>('get_boot_state');
    needsSetup = !bootState.models_provisioned && !bootState.portable;
    setupChecked = true;
    // ... rest of existing onMount ...
  });

  function handleSetupComplete() {
    needsSetup = false;
  }
</script>

{#if !setupChecked}
  <!-- Loading -->
{:else if needsSetup}
  <SetupWizard oncomplete={handleSetupComplete} />
{:else}
  <!-- Existing app UI -->
{/if}
```

Note: Add a `get_boot_state` Tauri command that returns the `AppBootState` struct managed in `lib.rs`.

- [ ] **Step 5: Verify TypeScript and lint**

Run: `cd app && npm run check && npm run lint`
Expected: passes

- [ ] **Step 6: Commit**

```bash
git add app/src/lib/components/setup/ app/src/lib/stores/provisioning.svelte.ts
git add app/src/App.svelte
git commit -m "feat(frontend): add SetupWizard with source selection and progress UI"
```

---

## Task 12: Build Pipeline Script

**Files:**
- Create: `app/scripts/build-release.ps1`

- [ ] **Step 1: Write build-release.ps1**

The script orchestrates all build steps with a `-Variant` parameter. Structure:

```powershell
[CmdletBinding()]
param(
    [ValidateSet('Online', 'Lite', 'Standard', 'Full', 'Portable')]
    [string]$Variant = 'Online'
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path "$PSScriptRoot/../..").Path
$AppRoot = Join-Path $RepoRoot 'app'
$ScriptsDir = Join-Path $AppRoot 'scripts'
$DistDir = Join-Path $RepoRoot 'dist'

# Step 1: Stage runtimes (idempotent)
Write-Host "=== Staging runtimes ===" -ForegroundColor Cyan
& "$ScriptsDir/setup-directml-runtime.ps1"
& "$ScriptsDir/setup-openvino-runtime.ps1"
& "$ScriptsDir/setup-bundled-python-runtime.ps1"

# Step 2: Stage engine sidecar
Write-Host "=== Building engine sidecar ===" -ForegroundColor Cyan
& "$ScriptsDir/stage-engine-sidecar.ps1"

# Step 3: Build NSIS installer
Write-Host "=== Building Tauri app ===" -ForegroundColor Cyan
Push-Location $AppRoot
try {
    npm run tauri build
} finally {
    Pop-Location
}

# Find the generated installer
$InstallerDir = Join-Path $AppRoot 'src-tauri/target/release/bundle/nsis'
$Installer = Get-ChildItem $InstallerDir -Filter '*.exe' | Select-Object -First 1
if (-not $Installer) { throw "NSIS installer not found in $InstallerDir" }

# Size guard: fail if installer exceeds 1.5 GB
$SizeGB = $Installer.Length / 1GB
if ($SizeGB -gt 1.5) { throw "Installer size ${SizeGB:F2} GB exceeds 1.5 GB limit" }

# Step 4: Variant-specific packaging
switch ($Variant) {
    'Online' {
        $OutDir = Join-Path $DistDir 'online'
        New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
        Copy-Item $Installer.FullName $OutDir
        Write-Host "Online installer: $OutDir/$($Installer.Name)" -ForegroundColor Green
    }
    'Lite' {
        Build-OfflineBundle -Models @('qwen2.5-1.5b-instruct') -BundleName 'SmolPC-Lite'
    }
    'Standard' {
        Build-OfflineBundle -Models @('qwen3-4b') -BundleName 'SmolPC-Standard'
    }
    'Full' {
        Build-OfflineBundle -Models @('qwen2.5-1.5b-instruct', 'qwen3-4b') -BundleName 'SmolPC-Full'
    }
    'Portable' {
        Build-PortableBundle
    }
}

function Build-OfflineBundle {
    param([string[]]$Models, [string]$BundleName)

    # Build model archives (calls existing scripts)
    & "$ScriptsDir/build-dml-model-archives.ps1"
    & "$ScriptsDir/build-openvino-model-archives.ps1"

    $OutDir = Join-Path $DistDir "offline/$BundleName"
    New-Item -ItemType Directory -Path "$OutDir/models" -Force | Out-Null
    Copy-Item $Installer.FullName $OutDir

    # Copy relevant model archives based on $Models filter
    # ... (filter model-archives.json entries, copy matching ZIPs)

    # ZIP the bundle
    $ZipPath = Join-Path $DistDir "offline/$BundleName.zip"
    Compress-Archive -Path "$OutDir/*" -DestinationPath $ZipPath -Force
    Write-Host "Offline bundle: $ZipPath" -ForegroundColor Green
}

function Build-PortableBundle {
    # Extract NSIS installer contents to a flat folder
    # Add pre-extracted Qwen 2.5 1.5B models
    # ZIP the result
    $OutDir = Join-Path $DistDir 'portable/SmolPC-Portable'
    # ... (copy app exe, engine, libs, python, models)

    $ZipPath = Join-Path $DistDir 'portable/SmolPC-Portable.zip'
    Compress-Archive -Path "$OutDir/*" -DestinationPath $ZipPath -Force
    Write-Host "Portable bundle: $ZipPath" -ForegroundColor Green
}
```

Note: The exact implementation will need to handle:
- Locating model archives from the existing build scripts' output directories
- Filtering `model-archives.json` entries by model ID for variant-specific manifests
- Extracting the NSIS installer contents for the portable variant (7z or manual extraction)

- [ ] **Step 2: Test the Online variant**

Run: `cd app && pwsh scripts/build-release.ps1 -Variant Online`
Expected: `dist/online/SmolPC_x.x.x_x64-setup.exe` exists, size < 1.5 GB

- [ ] **Step 3: Commit**

```bash
git add app/scripts/build-release.ps1
git commit -m "feat(build): add unified build-release.ps1 with variant support"
```

---

## Task 13: Integration Verification

- [ ] **Step 1: Run full pre-commit checks**

```bash
cargo check --workspace && cargo clippy --workspace
cargo test -p smolpc-engine-core && cargo test -p smolpc-engine-host
cargo test -p smolpc-desktop
cd app && npm run check && npm run lint
```

Expected: all pass

- [ ] **Step 2: Manual test — installed mode with no models**

1. Start the app in dev mode: `cd app && npm run tauri:dev`
2. If models exist at `%LOCALAPPDATA%\SmolPC\models\`, temporarily rename the directory
3. Verify the SetupWizard appears instead of the main chat UI
4. Verify source detection runs (may find no sources in dev)
5. Restore the models directory
6. Restart — verify the app goes straight to the main UI

- [ ] **Step 3: Manual test — provisioning from local source**

1. Create a test folder with a valid `model-archives.json` and a small test ZIP
2. Write the folder path to `%LOCALAPPDATA%\SmolPC\installer-source.txt` (simulating breadcrumb)
3. Remove models directory
4. Start the app — verify it detects the local source
5. Click "Install from local media" — verify extraction runs with progress

- [ ] **Step 4: Commit any fixes from integration testing**

```bash
git add -A
git commit -m "fix(provisioning): integration test fixes"
```

---

## Dependency Graph

```
Task 1 (types + deps + is_portable)
  ├─→ Task 2 (manifest parser)
  ├─→ Task 3 (extractor)
  ├─→ Task 4 (downloader)
  ├─→ Task 5 (source detection)
  ├─→ Task 6 (singleton guard)
  ├─→ Task 7 (Tauri commands) ← depends on 2-6
  │     └─→ Task 10 (frontend store) ← depends on 7
  │           └─→ Task 11 (SetupWizard UI) ← depends on 10
  └─→ Task 8 (portable mode) ← depends on 1 only (imports is_portable)

Task 9 (NSIS hooks) ← independent of 1-8, can run in parallel
Task 12 (build script) ← depends on all of 1-11 compiling (calls tauri build)
Task 13 (integration) ← depends on all above
```

**Parallel opportunities:**
- Tasks 2-6 and 8 can all run in parallel after Task 1
- Task 9 can run in parallel with everything except Task 13
- Tasks 8 and 9 are independent of each other and of Tasks 2-6
