/// Model management module
///
/// Handles model file loading, metadata, and future download functionality.
///
/// # Phase 0
/// - Manual model placement (user downloads from Hugging Face)
/// - Basic model loading
///
/// # Phase 5
/// - Automatic downloads from Hugging Face Hub
/// - Model variant selection based on hardware
/// - Storage management
pub mod loader;
pub mod registry;

// Phase 5: Download manager
// pub mod downloader;

pub use loader::{ModelArtifactBackend, ModelLoader};
pub use registry::ModelRegistry;
