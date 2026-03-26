use std::path::{Path, PathBuf};

pub const DEFAULT_BUNDLED_MODEL_ID: &str = "qwen2.5-1.5b-instruct";

pub const SETUP_ITEM_ENGINE_RUNTIME: &str = "engine_runtime";
pub const SETUP_ITEM_BUNDLED_MODEL: &str = "bundled_model";
#[allow(dead_code)]
pub trait OwnedIntegration {
    fn id(&self) -> &'static str;
    fn resource_root(&self, resource_dir: &Path) -> PathBuf;
}

#[allow(dead_code)]
pub trait Provisioner {
    fn id(&self) -> &'static str;
    fn prepare(&self, resource_dir: &Path, app_local_data_dir: &Path) -> Result<(), String>;
}

#[allow(dead_code)]
pub trait HostAppLocator {
    fn id(&self) -> &'static str;
    fn detect(&self) -> Option<PathBuf>;
}

#[allow(dead_code)]
pub trait RuntimeSupervisor {
    fn id(&self) -> &'static str;
    fn status(&self) -> String;
}

#[allow(dead_code)]
pub trait BundledPythonRuntime {
    fn prepared_root(&self, app_local_data_dir: &Path) -> PathBuf;
}
