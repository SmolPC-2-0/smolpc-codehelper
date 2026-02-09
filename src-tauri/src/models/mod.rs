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
pub mod runtime_spec;

// Phase 5: Download manager
// pub mod downloader;

pub use loader::ModelLoader;
pub use registry::ModelRegistry;
pub use runtime_spec::ModelRuntimeSpec;
