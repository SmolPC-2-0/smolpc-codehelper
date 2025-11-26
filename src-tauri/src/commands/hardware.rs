use crate::hardware::{self, types::HardwareInfo};
use std::sync::Arc;
use tokio::sync::OnceCell;

/// State for caching hardware detection results
/// Uses OnceCell for write-once semantics to eliminate race conditions
/// Stores Arc<HardwareInfo> internally for efficient sharing without cloning data
pub struct HardwareCache {
    info: OnceCell<Arc<HardwareInfo>>,
}

impl Default for HardwareCache {
    fn default() -> Self {
        Self {
            info: OnceCell::new(),
        }
    }
}

impl HardwareCache {
    /// Get cached hardware info, or detect if not yet initialized
    /// This ensures single detection even with concurrent requests
    /// Returns Arc for efficient sharing - only clones the pointer, not the data
    pub async fn get_or_detect(&self) -> Result<Arc<HardwareInfo>, String> {
        self.info
            .get_or_try_init(|| async {
                log::info!("Detecting hardware for the first time");
                hardware::detect_all()
                    .await
                    .map(Arc::new)
                    .map_err(|e| e.to_string())
            })
            .await
            .map(Arc::clone)
    }

    /// Get cached hardware info if available, without triggering detection
    /// Returns Arc for efficient sharing - only clones the pointer, not the data
    pub fn get(&self) -> Option<Arc<HardwareInfo>> {
        self.info.get().map(Arc::clone)
    }
}

/// Detect hardware or return cached results
/// Uses get_or_detect to ensure single detection even with concurrent requests
///
/// Arc strategy: Detection wraps result in Arc once, all accesses clone the Arc pointer (cheap),
/// this command clones the actual data once at the serialization boundary for Tauri IPC
#[tauri::command]
pub async fn detect_hardware(
    cache: tauri::State<'_, HardwareCache>,
) -> Result<HardwareInfo, String> {
    let info = cache.get_or_detect().await?;
    // Clone the data once for serialization - Arc::clone above was just pointer increment
    Ok(Arc::unwrap_or_clone(info))
}

/// Get cached hardware info without triggering detection
/// Returns None if hardware hasn't been detected yet
///
/// Arc strategy: If cached, clones Arc pointer (cheap), then clones data once for serialization
#[tauri::command]
pub async fn get_cached_hardware(
    cache: tauri::State<'_, HardwareCache>,
) -> Result<Option<HardwareInfo>, String> {
    Ok(cache.get().map(Arc::unwrap_or_clone))
}
