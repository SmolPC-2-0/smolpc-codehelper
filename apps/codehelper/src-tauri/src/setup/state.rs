use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Default)]
pub struct SetupCache {
    pub resolved_host_apps: HashMap<String, PathBuf>,
    pub last_error: Option<String>,
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
}
