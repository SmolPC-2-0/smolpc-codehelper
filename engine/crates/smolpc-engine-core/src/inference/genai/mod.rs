#[cfg(target_os = "windows")]
pub mod directml;
#[cfg(target_os = "windows")]
mod directml_ffi;
#[cfg(target_os = "windows")]
pub mod openvino;
#[cfg(target_os = "windows")]
mod openvino_ffi;

#[cfg(target_os = "windows")]
pub use directml::GenAiDirectMlGenerator;
#[cfg(target_os = "windows")]
pub use openvino::{OpenVinoGenAiGenerator, OpenVinoGenerationControls, OpenVinoPipelineConfig};

#[cfg(target_os = "windows")]
mod whisper_ffi;
#[cfg(target_os = "windows")]
pub mod whisper;
