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
/// - `process_manager`: Python process lifecycle management (coming soon)
/// - `mcp_client`: Core MCP client with JSON-RPC communication (coming soon)
/// - `commands`: Tauri command handlers (coming soon)

pub mod types;

// Re-export commonly used types
pub use types::{
    Content, InitializeParams, InitializeResult, JsonRpcError, JsonRpcNotification,
    JsonRpcRequest, JsonRpcResponse, LibreOfficeError, Tool, ToolCallParams, ToolCallResult,
    ToolsListResult,
};
