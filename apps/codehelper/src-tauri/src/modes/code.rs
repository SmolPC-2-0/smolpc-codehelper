use super::provider::{
    provider_state, ToolProvider, FOUNDATION_PROVIDER_EXECUTION_NOT_IMPLEMENTED,
};
use crate::assistant::MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION;
use async_trait::async_trait;
use smolpc_assistant_types::{
    AppMode, ProviderStateDto, ToolDefinitionDto, ToolExecutionResultDto,
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
    async fn connect_if_needed(&self) -> Result<ProviderStateDto, String> {
        Ok(Self::idle_state())
    }

    async fn status(&self) -> Result<ProviderStateDto, String> {
        Ok(Self::idle_state())
    }

    async fn list_tools(&self) -> Result<Vec<ToolDefinitionDto>, String> {
        Ok(Vec::new())
    }

    async fn execute_tool(
        &self,
        _name: &str,
        _arguments: serde_json::Value,
    ) -> Result<ToolExecutionResultDto, String> {
        Err(FOUNDATION_PROVIDER_EXECUTION_NOT_IMPLEMENTED.to_string())
    }

    async fn undo_last_action(&self) -> Result<(), String> {
        Err(MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION.to_string())
    }

    async fn disconnect_if_needed(&self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::CodeProvider;
    use crate::modes::provider::ToolProvider;

    #[tokio::test]
    async fn code_provider_returns_idle_state() {
        let provider = CodeProvider;
        let state = provider.status().await.expect("provider state");

        assert_eq!(state.state, "idle");
        assert_eq!(
            state.detail.as_deref(),
            Some("Code provider scaffold ready")
        );
        assert!(!state.supports_tools);
        assert!(!state.supports_undo);
    }
}
