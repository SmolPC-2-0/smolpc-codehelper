use super::provider::{
    provider_state, ToolProvider, FOUNDATION_NOT_INTEGRATED_DETAIL,
    FOUNDATION_PROVIDER_EXECUTION_NOT_IMPLEMENTED,
};
use crate::assistant::MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION;
use async_trait::async_trait;
use smolpc_assistant_types::{
    AppMode, ProviderStateDto, ToolDefinitionDto, ToolExecutionResultDto,
};

#[derive(Debug, Default)]
pub struct LibreOfficeProvider;

impl LibreOfficeProvider {
    fn disconnected_state(mode: AppMode) -> ProviderStateDto {
        provider_state(
            mode,
            "disconnected",
            Some(FOUNDATION_NOT_INTEGRATED_DETAIL),
            true,
            false,
        )
    }
}

#[async_trait]
impl ToolProvider for LibreOfficeProvider {
    async fn connect_if_needed(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
        Ok(Self::disconnected_state(mode))
    }

    async fn status(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
        Ok(Self::disconnected_state(mode))
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
    use crate::modes::provider::{ToolProvider, FOUNDATION_NOT_INTEGRATED_DETAIL};
    use smolpc_assistant_types::AppMode;

    #[tokio::test]
    async fn libreoffice_provider_returns_placeholder_state() {
        let provider = LibreOfficeProvider;
        let state = provider
            .status(AppMode::Calc)
            .await
            .expect("provider state");

        assert_eq!(state.state, "disconnected");
        assert_eq!(state.mode, AppMode::Calc);
        assert_eq!(
            state.detail.as_deref(),
            Some(FOUNDATION_NOT_INTEGRATED_DETAIL)
        );
        assert!(state.supports_tools);
        assert!(!state.supports_undo);
    }
}
