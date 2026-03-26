use crate::setup::{GIMP_BRIDGE_HOST, GIMP_BRIDGE_PORT};
use smolpc_assistant_types::ToolDefinitionDto;
use smolpc_mcp_client::{McpSession, McpTool, TcpTransportConfig};

pub const DEFAULT_GIMP_HOST: &str = GIMP_BRIDGE_HOST;
pub const DEFAULT_GIMP_PORT: u16 = GIMP_BRIDGE_PORT;

pub fn default_transport_config() -> TcpTransportConfig {
    TcpTransportConfig {
        host: DEFAULT_GIMP_HOST.to_string(),
        port: DEFAULT_GIMP_PORT,
    }
}

pub fn tool_definition(tool: McpTool) -> ToolDefinitionDto {
    ToolDefinitionDto {
        name: tool.name,
        description: tool.description,
        input_schema: tool.input_schema,
    }
}

pub async fn connect_session(config: &TcpTransportConfig) -> Result<McpSession, String> {
    McpSession::connect_tcp(
        config.clone(),
        "smolpc-unified-assistant",
        env!("CARGO_PKG_VERSION"),
    )
    .await
    .map_err(|error| format!("Unable to initialize the GIMP MCP session: {error}"))
}
