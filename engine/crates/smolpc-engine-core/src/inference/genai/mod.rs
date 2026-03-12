#[cfg(target_os = "windows")]
pub mod directml;
#[cfg(target_os = "windows")]
pub mod openvino;

#[cfg(target_os = "windows")]
pub use directml::GenAiDirectMlGenerator;
#[cfg(target_os = "windows")]
pub use openvino::OpenVinoGenAiGenerator;
