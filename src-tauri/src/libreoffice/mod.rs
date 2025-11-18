/// LibreOffice MCP client module
///
/// This module provides integration with LibreOffice via the MCP (Model Context Protocol).
/// It spawns a Python MCP server process and communicates with it via JSON-RPC 2.0 over stdio.
///
/// ## Architecture
///
/// ```text
/// Svelte UI → Tauri Commands → MCP Client → Python MCP Server → Helper → LibreOffice
/// ```
///
/// ## Modules
///
/// - `types`: JSON-RPC and MCP protocol types
/// - `process_manager`: Python process lifecycle management
/// - `mcp_client`: Core MCP client with JSON-RPC communication
/// - `commands`: Tauri command handlers (coming soon)

pub mod mcp_client;
pub mod process_manager;
pub mod types;

// Re-export commonly used types
#[allow(unused_imports)]
pub use mcp_client::MCPClient;
#[allow(unused_imports)]
pub use process_manager::ProcessManager;
#[allow(unused_imports)]
pub use types::{
    Content, InitializeParams, InitializeResult, JsonRpcError, JsonRpcNotification,
    JsonRpcRequest, JsonRpcResponse, LibreOfficeError, Tool, ToolCallParams, ToolCallResult,
    ToolsListResult,
};
