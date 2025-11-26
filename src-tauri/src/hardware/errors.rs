use thiserror::Error;

/// Errors that can occur during hardware detection
#[derive(Error, Debug)]
pub enum HardwareError {
    /// Hardware query failed
    #[error("Hardware query failed: {0}")]
    QueryFailed(String),

    /// No hardware detected
    #[error("No hardware detected: {0}")]
    NoHardware(String),

    /// Invalid hardware data
    #[error("Invalid hardware data: {0}")]
    InvalidData(String),
}

/// Convert HardwareError to String for Tauri IPC boundary
impl From<HardwareError> for String {
    fn from(error: HardwareError) -> String {
        error.to_string()
    }
}
