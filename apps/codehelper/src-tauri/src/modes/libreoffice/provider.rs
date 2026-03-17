use super::profiles::libreoffice_profile;
use super::resources::{resolve_mcp_server_layout, ResourceResolutionOptions};
use super::runtime::LibreOfficeRuntimeScaffold;
use super::state::LibreOfficeProviderState;
use crate::assistant::MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION;
use crate::modes::provider::{
    provider_state, ToolProvider, FOUNDATION_PROVIDER_EXECUTION_NOT_IMPLEMENTED,
};
use async_trait::async_trait;
use smolpc_assistant_types::{
    AppMode, ProviderStateDto, ToolDefinitionDto, ToolExecutionResultDto,
};
use std::path::PathBuf;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct LibreOfficeProvider {
    resource_dir: Option<PathBuf>,
    resolution_options: ResourceResolutionOptions,
    state: Mutex<LibreOfficeProviderState>,
}

impl Default for LibreOfficeProvider {
    fn default() -> Self {
        Self::new(None)
    }
}

impl LibreOfficeProvider {
    pub fn new(resource_dir: Option<PathBuf>) -> Self {
        Self::with_resolution_options(resource_dir, ResourceResolutionOptions::default())
    }

    pub(crate) fn with_resolution_options(
        resource_dir: Option<PathBuf>,
        resolution_options: ResourceResolutionOptions,
    ) -> Self {
        Self {
            resource_dir,
            resolution_options,
            state: Mutex::new(LibreOfficeProviderState::default()),
        }
    }

    fn profile_for_mode(mode: AppMode) -> Result<super::LibreOfficeModeProfile, String> {
        libreoffice_profile(mode)
            .ok_or_else(|| format!("LibreOffice provider does not handle mode {mode:?}"))
    }

    async fn validate_scaffold(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
        let profile = Self::profile_for_mode(mode)?;
        let mut state = self.state.lock().await;

        match resolve_mcp_server_layout(self.resource_dir.as_deref(), self.resolution_options) {
            Ok(layout) => {
                let runtime = LibreOfficeRuntimeScaffold::from_layout(&layout);
                let future_transport = runtime.stdio_transport_config();
                state.scaffold_dir = Some(layout.mcp_server_dir.clone());
                state.last_error = None;
                state.validated_once = true;

                let detail = format!(
                    "{} scaffold is present in the unified app, but live document actions are deferred to the activation branch. {} Future runtime: {}. Planned stdio entrypoint: {}.",
                    profile.label,
                    profile.source_coverage,
                    runtime.summary(),
                    future_transport.args.join(" ")
                );

                Ok(provider_state(
                    mode,
                    "disconnected",
                    Some(detail.as_str()),
                    true,
                    false,
                ))
            }
            Err(error) => {
                let detail = format!("LibreOffice scaffold validation failed: {error}");
                state.scaffold_dir = None;
                state.last_error = Some(detail.clone());
                state.validated_once = true;

                Ok(provider_state(
                    mode,
                    "error",
                    Some(detail.as_str()),
                    true,
                    false,
                ))
            }
        }
    }
}

#[async_trait]
impl ToolProvider for LibreOfficeProvider {
    async fn connect_if_needed(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
        self.validate_scaffold(mode).await
    }

    async fn status(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
        self.validate_scaffold(mode).await
    }

    async fn list_tools(&self, _mode: AppMode) -> Result<Vec<ToolDefinitionDto>, String> {
        Ok(Vec::new())
    }

    async fn execute_tool(
        &self,
        _mode: AppMode,
        _name: &str,
        _arguments: serde_json::Value,
    ) -> Result<ToolExecutionResultDto, String> {
        Err(FOUNDATION_PROVIDER_EXECUTION_NOT_IMPLEMENTED.to_string())
    }

    async fn undo_last_action(&self, _mode: AppMode) -> Result<(), String> {
        Err(MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION.to_string())
    }

    async fn disconnect_if_needed(&self, _mode: AppMode) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::LibreOfficeProvider;
    use crate::modes::libreoffice::resources::ResourceResolutionOptions;
    use crate::modes::provider::ToolProvider;
    use smolpc_assistant_types::AppMode;
    use std::fs;
    use tempfile::tempdir;

    fn staged_resource_root() -> tempfile::TempDir {
        let tempdir = tempdir().expect("tempdir");
        let staged_dir = tempdir
            .path()
            .join("resources")
            .join("libreoffice")
            .join("mcp_server");
        fs::create_dir_all(&staged_dir).expect("create staged dir");
        fs::write(staged_dir.join("README.md"), "placeholder").expect("write readme");
        tempdir
    }

    #[tokio::test]
    async fn libreoffice_provider_reports_scaffold_detail_for_each_submode() {
        let tempdir = staged_resource_root();
        let provider = LibreOfficeProvider::with_resolution_options(
            Some(tempdir.path().to_path_buf()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
            },
        );

        for (mode, label) in [
            (AppMode::Writer, "Writer"),
            (AppMode::Calc, "Calc"),
            (AppMode::Impress, "Slides"),
        ] {
            let state = provider.status(mode).await.expect("provider state");
            assert_eq!(state.mode, mode);
            assert_eq!(state.state, "disconnected");
            assert!(state.supports_tools);
            assert!(!state.supports_undo);
            assert!(state.detail.as_deref().expect("detail").contains(label));
        }
    }

    #[tokio::test]
    async fn connect_if_needed_revalidates_scaffold_without_tools() {
        let tempdir = staged_resource_root();
        let provider = LibreOfficeProvider::with_resolution_options(
            Some(tempdir.path().to_path_buf()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
            },
        );

        let state = provider
            .connect_if_needed(AppMode::Writer)
            .await
            .expect("connect state");
        let tools = provider.list_tools(AppMode::Writer).await.expect("tools");

        assert_eq!(state.state, "disconnected");
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn missing_staged_resource_directory_produces_honest_error_detail() {
        let tempdir = tempdir().expect("tempdir");
        let provider = LibreOfficeProvider::with_resolution_options(
            Some(tempdir.path().to_path_buf()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
            },
        );

        let state = provider
            .status(AppMode::Calc)
            .await
            .expect("provider state");

        assert_eq!(state.state, "error");
        assert_eq!(state.mode, AppMode::Calc);
        assert!(state
            .detail
            .as_deref()
            .expect("detail")
            .contains("scaffold validation failed"));
    }
}
