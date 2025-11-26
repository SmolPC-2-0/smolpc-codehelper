use crate::hardware::{self, types::HardwareInfo};
use std::sync::Arc;
use tokio::sync::OnceCell;

/// State for caching hardware detection results
/// Uses OnceCell for write-once semantics to eliminate race conditions
pub struct HardwareCache {
    info: Arc<OnceCell<HardwareInfo>>,
}

impl Default for HardwareCache {
    fn default() -> Self {
        Self {
            info: Arc::new(OnceCell::new()),    
        }
    }
}

impl HardwareCache {
    /// Get cached hardware info, or detect if not yet initialized
    /// This ensures single detection even with concurrent requests
    pub async fn get_or_detect(&self) -> Result<Arc<HardwareInfo>, String> {
        self.info
            .get_or_try_init(|| async {
                log::info!("Detecting hardware for the first time");
                hardware::detect_all().await.map_err(|e| e.to_string())
            })
            .await
            .map(|info| Arc::new(info.clone()))
    }

    /// Get cached hardware info if available, without triggering detection
    pub fn get(&self) -> Option<Arc<HardwareInfo>> {
        self.info.get().map(|info| Arc::new(info.clone()))
    }
}

/// Detect hardware or return cached results
/// Uses get_or_detect to ensure single detection even with concurrent requests
#[tauri::command]
pub async fn detect_hardware(
    cache: tauri::State<'_, HardwareCache>,
) -> Result<HardwareInfo, String> {
    let info = cache.get_or_detect().await?;
    Ok((*info).clone())
}

/// Get cached hardware info without triggering detection
/// Returns None if hardware hasn't been detected yet
#[tauri::command]
pub async fn get_cached_hardware(
    cache: tauri::State<'_, HardwareCache>,
) -> Result<Option<HardwareInfo>, String> {
    Ok(cache.get().map(|arc| (*arc).clone()))
}
