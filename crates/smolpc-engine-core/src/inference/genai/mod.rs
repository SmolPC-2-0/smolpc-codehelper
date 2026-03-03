#[cfg(target_os = "windows")]
pub mod directml;

#[cfg(target_os = "windows")]
pub use directml::GenAiDirectMlGenerator;
