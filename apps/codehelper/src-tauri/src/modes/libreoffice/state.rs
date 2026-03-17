use smolpc_assistant_types::ToolDefinitionDto;
use smolpc_mcp_client::McpSession;
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct LibreOfficeProviderState {
    pub scaffold_dir: Option<PathBuf>,
    pub session: Option<McpSession>,
    pub tools: Vec<ToolDefinitionDto>,
    pub last_error: Option<String>,
    pub runtime_attempted: bool,
}
