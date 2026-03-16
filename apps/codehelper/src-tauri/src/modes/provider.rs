use async_trait::async_trait;
use smolpc_assistant_types::{
    AppMode, ProviderStateDto, ToolDefinitionDto, ToolExecutionResultDto,
};

pub const FOUNDATION_NOT_INTEGRATED_DETAIL: &str = "Provider not integrated yet";
pub const FOUNDATION_PROVIDER_EXECUTION_NOT_IMPLEMENTED: &str =
    "UNIFIED_PROVIDER_EXECUTION_NOT_IMPLEMENTED";

#[allow(dead_code)]
#[async_trait]
pub trait ToolProvider: Send + Sync {
    async fn connect_if_needed(&self) -> Result<ProviderStateDto, String>;
    async fn status(&self) -> Result<ProviderStateDto, String>;
    async fn list_tools(&self) -> Result<Vec<ToolDefinitionDto>, String>;
    async fn execute_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolExecutionResultDto, String>;
    async fn undo_last_action(&self) -> Result<(), String>;
    async fn disconnect_if_needed(&self) -> Result<(), String>;
}

pub fn provider_state(
    mode: AppMode,
    state: &str,
    detail: Option<&str>,
    supports_tools: bool,
    supports_undo: bool,
) -> ProviderStateDto {
    ProviderStateDto {
        mode,
        state: state.to_string(),
        detail: detail.map(str::to_string),
        supports_tools,
        supports_undo,
    }
}

pub fn provider_state_for_mode(mode: AppMode, mut state: ProviderStateDto) -> ProviderStateDto {
    state.mode = mode;
    state
}
