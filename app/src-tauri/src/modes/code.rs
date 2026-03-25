use async_trait::async_trait;
use smolpc_assistant_types::{
    AppMode, ProviderStateDto, ToolDefinitionDto, ToolExecutionResultDto,
};
use smolpc_connector_common::{
    provider_state, ToolProvider, FOUNDATION_PROVIDER_EXECUTION_NOT_IMPLEMENTED,
    MODE_UNDO_NOT_SUPPORTED,
};

#[derive(Debug, Default)]
pub struct CodeProvider;

impl CodeProvider {
    fn idle_state() -> ProviderStateDto {
        provider_state(
            AppMode::Code,
            "idle",
            Some("Code provider scaffold ready"),
            false,
            false,
        )
    }
}

#[async_trait]
impl ToolProvider for CodeProvider {
    async fn connect_if_needed(&self, _mode: AppMode) -> Result<ProviderStateDto, String> {
        Ok(Self::idle_state())
    }

    async fn status(&self, _mode: AppMode) -> Result<ProviderStateDto, String> {
        Ok(Self::idle_state())
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
        Err(MODE_UNDO_NOT_SUPPORTED.to_string())
    }

    async fn disconnect_if_needed(&self, _mode: AppMode) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::CodeProvider;
    use smolpc_assistant_types::AppMode;
    use smolpc_connector_common::ToolProvider;

    #[tokio::test]
    async fn code_provider_returns_idle_state() {
        let provider = CodeProvider;
        let state = provider
            .status(AppMode::Code)
            .await
            .expect("provider state");

        assert_eq!(state.state, "idle");
        assert_eq!(
            state.detail.as_deref(),
            Some("Code provider scaffold ready")
        );
        assert!(!state.supports_tools);
        assert!(!state.supports_undo);
    }
}
