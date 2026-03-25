pub mod cancellation;
pub mod host_apps;
pub mod launch;
pub mod manifests;
pub mod provider;
pub mod python;

pub use cancellation::{CancellationToken, MockCancellationToken};
pub use provider::{provider_state, ToolProvider, FOUNDATION_PROVIDER_EXECUTION_NOT_IMPLEMENTED};

// Constants migrated from app setup/types.rs
pub const SETUP_ITEM_HOST_GIMP: &str = "host_gimp";
pub const SETUP_ITEM_HOST_BLENDER: &str = "host_blender";
pub const SETUP_ITEM_HOST_LIBREOFFICE: &str = "host_libreoffice";
pub const SETUP_ITEM_BLENDER_ADDON: &str = "blender_addon";
pub const SETUP_ITEM_GIMP_PLUGIN_RUNTIME: &str = "gimp_plugin_runtime";
pub const SETUP_ITEM_BUNDLED_PYTHON: &str = "bundled_python";

// Constant migrated from app assistant/mod.rs (string value unchanged for frontend compat)
pub const MODE_UNDO_NOT_SUPPORTED: &str = "MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION";
