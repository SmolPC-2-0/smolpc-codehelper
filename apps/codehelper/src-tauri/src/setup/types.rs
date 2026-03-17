use std::path::{Path, PathBuf};

pub const DEFAULT_BUNDLED_MODEL_ID: &str = "qwen3-4b-instruct-2507";

pub const SETUP_ITEM_ENGINE_RUNTIME: &str = "engine_runtime";
pub const SETUP_ITEM_BUNDLED_MODEL: &str = "bundled_model";
pub const SETUP_ITEM_BUNDLED_PYTHON: &str = "bundled_python";
pub const SETUP_ITEM_HOST_GIMP: &str = "host_gimp";
pub const SETUP_ITEM_HOST_BLENDER: &str = "host_blender";
pub const SETUP_ITEM_HOST_LIBREOFFICE: &str = "host_libreoffice";

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
