use crate::hardware::{self, types::HardwareInfo};
use std::sync::Arc;
use tokio::sync::RwLock;

/// State for caching hardware detection results
#[derive(Default)]
pub struct HardwareCache {
    info: Arc<RwLock<Option<HardwareInfo>>>,
}

impl HardwareCache {
    /// Get cached hardware info
    pub async fn get(&self) -> Option<HardwareInfo> {
        self.info.read().await.clone()
    }

    /// Set cached hardware info
    pub async fn set(&self, info: HardwareInfo) {
        *self.info.write().await = Some(info);
    }
}

/// Detect all hardware (CPU, GPU, NPU)
/// Results are cached in app state
#[tauri::command]
pub async fn detect_hardware(
    cache: tauri::State<'_, HardwareCache>,
) -> Result<HardwareInfo, String> {
    let info = hardware::detect_all().await?;

    // Cache the result
    cache.set(info.clone()).await;

    Ok(info)
}

/// Get cached hardware info without re-detecting
#[tauri::command]
pub async fn get_cached_hardware(
    cache: tauri::State<'_, HardwareCache>,
) -> Result<Option<HardwareInfo>, String> {
    Ok(cache.get().await)
}
