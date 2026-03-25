use serde::{Deserialize, Serialize};

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
