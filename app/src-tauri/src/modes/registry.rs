use smolpc_connector_blender::BlenderProvider;
use super::code::CodeProvider;
use smolpc_connector_gimp::GimpProvider;
use super::libreoffice::LibreOfficeProvider;
use smolpc_connector_common::ToolProvider;
use smolpc_assistant_types::AppMode;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderFamily {
    Code,
    Gimp,
    Blender,
    LibreOffice,
}

pub struct ModeProviderRegistry {
    pub code: Arc<dyn ToolProvider>,
    pub gimp: Arc<dyn ToolProvider>,
    pub blender: Arc<dyn ToolProvider>,
    pub libreoffice: Arc<dyn ToolProvider>,
}

impl Default for ModeProviderRegistry {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl ModeProviderRegistry {
    pub fn new(resource_dir: Option<PathBuf>, app_local_data_dir: Option<PathBuf>) -> Self {
        Self {
            code: Arc::new(CodeProvider),
            gimp: Arc::new(GimpProvider::new(
                resource_dir.clone(),
                app_local_data_dir.clone(),
            )),
            blender: Arc::new(BlenderProvider::new(
                resource_dir.clone(),
                app_local_data_dir.clone(),
            )),
            libreoffice: Arc::new(LibreOfficeProvider::new(resource_dir, app_local_data_dir)),
        }
    }

    pub fn provider_family(mode: AppMode) -> ProviderFamily {
        match mode {
            AppMode::Code => ProviderFamily::Code,
            AppMode::Gimp => ProviderFamily::Gimp,
            AppMode::Blender => ProviderFamily::Blender,
            AppMode::Writer | AppMode::Impress => ProviderFamily::LibreOffice,
        }
    }

    pub fn provider_for_mode(&self, mode: AppMode) -> Arc<dyn ToolProvider> {
        match Self::provider_family(mode) {
            ProviderFamily::Code => Arc::clone(&self.code),
            ProviderFamily::Gimp => Arc::clone(&self.gimp),
            ProviderFamily::Blender => Arc::clone(&self.blender),
            ProviderFamily::LibreOffice => Arc::clone(&self.libreoffice),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ModeProviderRegistry, ProviderFamily};
    use smolpc_assistant_types::AppMode;

    #[test]
    fn libreoffice_modes_share_one_provider_family() {
        assert_eq!(
            ModeProviderRegistry::provider_family(AppMode::Writer),
            ProviderFamily::LibreOffice
        );
        assert_eq!(
            ModeProviderRegistry::provider_family(AppMode::Impress),
            ProviderFamily::LibreOffice
        );
    }
}
