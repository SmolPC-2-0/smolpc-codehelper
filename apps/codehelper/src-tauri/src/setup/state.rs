use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

const HOST_CACHE_VERSION: u32 = 1;
const HOST_CACHE_FILE: &str = "host-detection-cache.json";

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedSetupCache {
    version: u32,
    resolved_host_apps: HashMap<String, String>,
    last_error: Option<String>,
    updated_at: u64,
}

#[derive(Debug, Default)]
pub struct SetupCache {
    pub resolved_host_apps: HashMap<String, PathBuf>,
    pub last_error: Option<String>,
    pub loaded_from_disk: bool,
}

#[derive(Clone, Debug)]
pub struct SetupState {
    resource_dir: Option<PathBuf>,
    app_local_data_dir: Option<PathBuf>,
    allow_system_host_detection: bool,
    cache: Arc<Mutex<SetupCache>>,
}

impl Default for SetupState {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl SetupState {
    pub fn new(resource_dir: Option<PathBuf>, app_local_data_dir: Option<PathBuf>) -> Self {
        Self::with_host_detection(resource_dir, app_local_data_dir, true)
    }

    pub fn with_host_detection(
        resource_dir: Option<PathBuf>,
        app_local_data_dir: Option<PathBuf>,
        allow_system_host_detection: bool,
    ) -> Self {
        Self {
            resource_dir,
            app_local_data_dir,
            allow_system_host_detection,
            cache: Arc::new(Mutex::new(SetupCache::default())),
        }
    }

    pub fn resource_dir(&self) -> Option<&Path> {
        self.resource_dir.as_deref()
    }

    pub fn app_local_data_dir(&self) -> Option<&Path> {
        self.app_local_data_dir.as_deref()
    }

    pub fn setup_root(&self) -> Option<PathBuf> {
        self.app_local_data_dir().map(|path| path.join("setup"))
    }

    pub fn allow_system_host_detection(&self) -> bool {
        self.allow_system_host_detection
    }

    pub async fn cache(&self) -> tokio::sync::MutexGuard<'_, SetupCache> {
        self.cache.lock().await
    }

    fn host_cache_path(&self) -> Option<PathBuf> {
        self.app_local_data_dir()
            .map(|base| base.join("setup").join("state").join(HOST_CACHE_FILE))
    }

    fn host_cache_backup_path(path: &Path) -> PathBuf {
        path.with_extension("json.bak")
    }

    fn host_cache_tmp_path(path: &Path) -> PathBuf {
        path.with_extension("json.tmp")
    }

    fn deserialize_cache(raw: &str, source: &Path) -> Result<PersistedSetupCache, String> {
        let payload = serde_json::from_str::<PersistedSetupCache>(raw).map_err(|error| {
            format!(
                "Failed to parse setup host cache {}: {error}",
                source.display()
            )
        })?;

        if payload.version != HOST_CACHE_VERSION {
            return Err(format!(
                "Unsupported setup host cache version {} in {}",
                payload.version,
                source.display()
            ));
        }

        Ok(payload)
    }

    fn read_persisted_cache(path: &Path) -> Option<PersistedSetupCache> {
        let backup = Self::host_cache_backup_path(path);
        for candidate in [path, &backup] {
            if !candidate.exists() {
                continue;
            }

            let raw = match std::fs::read_to_string(candidate) {
                Ok(raw) => raw,
                Err(error) => {
                    log::warn!(
                        "Failed to read setup host cache {}: {}",
                        candidate.display(),
                        error
                    );
                    continue;
                }
            };

            match Self::deserialize_cache(&raw, candidate) {
                Ok(cache) => return Some(cache),
                Err(error) => {
                    log::warn!("{error}");
                }
            }
        }

        None
    }

    fn write_persisted_cache(path: &Path, cache: &PersistedSetupCache) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to create setup cache directory {}: {error}",
                    parent.display()
                )
            })?;
        }

        let backup = Self::host_cache_backup_path(path);
        let tmp = Self::host_cache_tmp_path(path);
        let serialized = serde_json::to_string_pretty(cache)
            .map_err(|error| format!("Failed to serialize setup host cache: {error}"))?;
        std::fs::write(&tmp, serialized).map_err(|error| {
            format!(
                "Failed to write temporary setup host cache {}: {error}",
                tmp.display()
            )
        })?;

        if path.exists() {
            let _ = std::fs::copy(path, &backup);
            let _ = std::fs::remove_file(path);
        }

        std::fs::rename(&tmp, path).map_err(|error| {
            format!(
                "Failed to promote setup host cache {} to {}: {error}",
                tmp.display(),
                path.display()
            )
        })?;

        Ok(())
    }

    pub async fn load_cache_from_disk_if_needed(&self) {
        {
            let cache = self.cache.lock().await;
            if cache.loaded_from_disk {
                return;
            }
        }

        let loaded = self
            .host_cache_path()
            .as_deref()
            .and_then(Self::read_persisted_cache);

        let mut cache = self.cache.lock().await;
        if cache.loaded_from_disk {
            return;
        }

        if let Some(loaded) = loaded {
            cache.resolved_host_apps = loaded
                .resolved_host_apps
                .into_iter()
                .map(|(id, path)| (id, PathBuf::from(path)))
                .filter(|(_, path)| path.exists())
                .collect();
            cache.last_error = loaded.last_error;
        }

        cache.loaded_from_disk = true;
    }

    pub async fn persist_cache_to_disk(&self) {
        let Some(path) = self.host_cache_path() else {
            return;
        };

        let snapshot = {
            let cache = self.cache.lock().await;
            PersistedSetupCache {
                version: HOST_CACHE_VERSION,
                resolved_host_apps: cache
                    .resolved_host_apps
                    .iter()
                    .map(|(id, path)| (id.clone(), path.to_string_lossy().to_string()))
                    .collect(),
                last_error: cache.last_error.clone(),
                updated_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_secs())
                    .unwrap_or_default(),
            }
        };

        if let Err(error) = Self::write_persisted_cache(&path, &snapshot) {
            log::warn!("{error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SetupState;
    use crate::setup::types::SETUP_ITEM_HOST_BLENDER;
    use tempfile::TempDir;

    #[tokio::test]
    async fn setup_cache_round_trips_through_disk() {
        let app_temp = TempDir::new().expect("app temp");
        let blender_path = app_temp.path().join("blender.exe");
        std::fs::write(&blender_path, "blender").expect("write blender");

        let state =
            SetupState::with_host_detection(None, Some(app_temp.path().to_path_buf()), false);
        {
            let mut cache = state.cache().await;
            cache.loaded_from_disk = true;
            cache
                .resolved_host_apps
                .insert(SETUP_ITEM_HOST_BLENDER.to_string(), blender_path.clone());
            cache.last_error = Some("cached error".to_string());
        }
        state.persist_cache_to_disk().await;

        let loaded_state =
            SetupState::with_host_detection(None, Some(app_temp.path().to_path_buf()), false);
        loaded_state.load_cache_from_disk_if_needed().await;
        let cache = loaded_state.cache().await;

        assert_eq!(
            cache
                .resolved_host_apps
                .get(SETUP_ITEM_HOST_BLENDER)
                .cloned(),
            Some(blender_path)
        );
        assert_eq!(cache.last_error.as_deref(), Some("cached error"));
    }

    #[tokio::test]
    async fn setup_cache_uses_backup_when_primary_is_corrupt() {
        let app_temp = TempDir::new().expect("app temp");
        let blender_path_a = app_temp.path().join("blender-a.exe");
        let blender_path_b = app_temp.path().join("blender-b.exe");
        std::fs::write(&blender_path_a, "blender-a").expect("write blender a");
        std::fs::write(&blender_path_b, "blender-b").expect("write blender b");

        let state =
            SetupState::with_host_detection(None, Some(app_temp.path().to_path_buf()), false);
        {
            let mut cache = state.cache().await;
            cache.loaded_from_disk = true;
            cache
                .resolved_host_apps
                .insert(SETUP_ITEM_HOST_BLENDER.to_string(), blender_path_a.clone());
        }
        state.persist_cache_to_disk().await;

        {
            let mut cache = state.cache().await;
            cache
                .resolved_host_apps
                .insert(SETUP_ITEM_HOST_BLENDER.to_string(), blender_path_b.clone());
        }
        state.persist_cache_to_disk().await;

        let cache_path = state.host_cache_path().expect("cache path");
        std::fs::write(&cache_path, "{invalid-json").expect("corrupt primary cache");

        let loaded_state =
            SetupState::with_host_detection(None, Some(app_temp.path().to_path_buf()), false);
        loaded_state.load_cache_from_disk_if_needed().await;
        let cache = loaded_state.cache().await;

        assert_eq!(
            cache
                .resolved_host_apps
                .get(SETUP_ITEM_HOST_BLENDER)
                .cloned(),
            Some(blender_path_a)
        );
    }
}
