//! LibreOffice MCP integration Tauri commands
//!
//! This module provides:
//! - `LibreOfficeState`: Managed state wrapper around MCPClient
//! - Tauri commands for connecting, listing tools, and calling tools
//! - Auto-recovery on crash (one retry)

use crate::libreoffice::{MCPClient, Tool, LibreOfficeError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

// ============================================================================
// State Management
// ============================================================================

/// Managed state for LibreOffice MCP client
///
/// Uses lazy initialization - client is only created on first `connect()` call.
/// Thread-safe with tokio::sync::Mutex for async operations.
pub struct LibreOfficeState {
    client: Arc<Mutex<Option<MCPClient>>>,
}

impl Default for LibreOfficeState {
    fn default() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
        }
    }
}

impl LibreOfficeState {
    /// Get the MCP client, returning error if not connected
    async fn get_client(&self) -> Result<tokio::sync::MutexGuard<'_, Option<MCPClient>>, LibreOfficeError> {
        let guard = self.client.lock().await;
        if guard.is_none() {
            return Err(LibreOfficeError::NotInitialized);
        }
        Ok(guard)
    }

    /// Check if client is connected and running
    async fn is_connected(&self) -> bool {
        let guard = self.client.lock().await;
        if let Some(client) = guard.as_ref() {
            client.is_running().await
        } else {
            false
        }
    }
}

// ============================================================================
// Response Types (for frontend)
// ============================================================================

/// Connection status response
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub connected: bool,
    pub server_name: Option<String>,
    pub server_version: Option<String>,
}

/// Tool information for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl From<Tool> for ToolInfo {
    fn from(tool: Tool) -> Self {
        Self {
            name: tool.name,
            description: tool.description,
            input_schema: tool.input_schema,
        }
    }
}

/// Tool call result for frontend
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResponse {
    pub success: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
}

// ============================================================================
// Error Conversion
// ============================================================================

/// Convert LibreOfficeError to frontend-friendly string
impl From<LibreOfficeError> for String {
    fn from(err: LibreOfficeError) -> Self {
        err.to_string()
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Connect to the LibreOffice MCP server
///
/// Creates a new MCPClient and performs the initialization handshake.
/// If already connected, returns success without reconnecting.
#[tauri::command]
pub async fn libreoffice_connect(
    state: State<'_, LibreOfficeState>,
) -> Result<StatusResponse, String> {
    log::info!("libreoffice_connect called");

    let mut guard = state.client.lock().await;

    // Check if already connected
    if let Some(client) = guard.as_ref() {
        if client.is_running().await {
            let info = client.server_info();
            return Ok(StatusResponse {
                connected: true,
                server_name: Some(info.server_info.name.clone()),
                server_version: Some(info.server_info.version.clone()),
            });
        }
        // Client exists but not running - will reconnect
        log::warn!("Existing client is not running, reconnecting...");
    }

    // Create new client
    match MCPClient::new().await {
        Ok(client) => {
            let info = client.server_info();
            let response = StatusResponse {
                connected: true,
                server_name: Some(info.server_info.name.clone()),
                server_version: Some(info.server_info.version.clone()),
            };
            *guard = Some(client);
            log::info!("LibreOffice MCP client connected successfully");
            Ok(response)
        }
        Err(e) => {
            log::error!("Failed to connect to LibreOffice MCP: {}", e);
            Err(e.to_string())
        }
    }
}

/// Disconnect from the LibreOffice MCP server
#[tauri::command]
pub async fn libreoffice_disconnect(
    state: State<'_, LibreOfficeState>,
) -> Result<(), String> {
    log::info!("libreoffice_disconnect called");

    let mut guard = state.client.lock().await;

    if let Some(client) = guard.take() {
        if let Err(e) = client.shutdown().await {
            log::warn!("Error during shutdown: {}", e);
        }
    }

    log::info!("LibreOffice MCP client disconnected");
    Ok(())
}

/// Get the current connection status
#[tauri::command]
pub async fn libreoffice_status(
    state: State<'_, LibreOfficeState>,
) -> Result<StatusResponse, String> {
    log::info!(">>> libreoffice_status command called");
    println!(">>> libreoffice_status command called (println)");
    let guard = state.client.lock().await;

    if let Some(client) = guard.as_ref() {
        if client.is_running().await {
            let info = client.server_info();
            return Ok(StatusResponse {
                connected: true,
                server_name: Some(info.server_info.name.clone()),
                server_version: Some(info.server_info.version.clone()),
            });
        }
    }

    Ok(StatusResponse {
        connected: false,
        server_name: None,
        server_version: None,
    })
}

/// List all available MCP tools
#[tauri::command]
pub async fn libreoffice_list_tools(
    state: State<'_, LibreOfficeState>,
) -> Result<Vec<ToolInfo>, String> {
    log::info!("libreoffice_list_tools called");
    println!(">>> libreoffice_list_tools called");

    let guard = state.client.lock().await;
    println!(">>> Got lock");

    let client = guard.as_ref()
        .ok_or_else(|| {
            println!(">>> Client not initialized!");
            LibreOfficeError::NotInitialized.to_string()
        })?;

    println!(">>> Calling client.list_tools()...");
    let tools = client.list_tools().await
        .map_err(|e| {
            println!(">>> Error listing tools: {}", e);
            e.to_string()
        })?;

    println!(">>> Got {} tools", tools.len());
    Ok(tools.into_iter().map(ToolInfo::from).collect())
}

/// Call an MCP tool with arguments
///
/// This is the generic tool call command - works with any tool.
#[tauri::command]
pub async fn libreoffice_call_tool(
    state: State<'_, LibreOfficeState>,
    tool_name: String,
    arguments: Value,
) -> Result<ToolCallResponse, String> {
    log::info!("libreoffice_call_tool called: {}", tool_name);

    let guard = state.client.lock().await;

    let client = guard.as_ref()
        .ok_or_else(|| LibreOfficeError::NotInitialized.to_string())?;

    match client.call_tool(&tool_name, arguments).await {
        Ok(result) => {
            log::info!("Tool {} completed successfully", tool_name);
            Ok(ToolCallResponse {
                success: true,
                result: Some(result),
                error: None,
            })
        }
        Err(e) => {
            log::error!("Tool {} failed: {}", tool_name, e);
            Ok(ToolCallResponse {
                success: false,
                result: None,
                error: Some(e.to_string()),
            })
        }
    }
}

// ============================================================================
// Convenience Commands (typed wrappers for common operations)
// ============================================================================

/// Create a new blank document
///
/// Convenience wrapper for `create_blank_document` tool.
#[tauri::command]
pub async fn libreoffice_create_document(
    state: State<'_, LibreOfficeState>,
    filename: String,
    title: Option<String>,
    doc_type: Option<String>,
) -> Result<ToolCallResponse, String> {
    log::info!("libreoffice_create_document called: {}", filename);

    let guard = state.client.lock().await;

    let client = guard.as_ref()
        .ok_or_else(|| LibreOfficeError::NotInitialized.to_string())?;

    let mut args = serde_json::json!({
        "filename": filename
    });

    if let Some(t) = title {
        args["title"] = Value::String(t);
    }
    if let Some(dt) = doc_type {
        args["doc_type"] = Value::String(dt);
    }

    match client.call_tool("create_blank_document", args).await {
        Ok(result) => {
            log::info!("Document {} created successfully", filename);
            Ok(ToolCallResponse {
                success: true,
                result: Some(result),
                error: None,
            })
        }
        Err(e) => {
            log::error!("Failed to create document {}: {}", filename, e);
            Ok(ToolCallResponse {
                success: false,
                result: None,
                error: Some(e.to_string()),
            })
        }
    }
}

/// Add text to the current document
///
/// Convenience wrapper for `add_text` tool.
#[tauri::command]
pub async fn libreoffice_add_text(
    state: State<'_, LibreOfficeState>,
    text: String,
) -> Result<ToolCallResponse, String> {
    log::info!("libreoffice_add_text called");

    let guard = state.client.lock().await;

    let client = guard.as_ref()
        .ok_or_else(|| LibreOfficeError::NotInitialized.to_string())?;

    let args = serde_json::json!({
        "text": text
    });

    match client.call_tool("add_text", args).await {
        Ok(result) => Ok(ToolCallResponse {
            success: true,
            result: Some(result),
            error: None,
        }),
        Err(e) => Ok(ToolCallResponse {
            success: false,
            result: None,
            error: Some(e.to_string()),
        }),
    }
}

/// Save the current document
///
/// Convenience wrapper for `save_document` tool.
#[tauri::command]
pub async fn libreoffice_save_document(
    state: State<'_, LibreOfficeState>,
    path: Option<String>,
) -> Result<ToolCallResponse, String> {
    log::info!("libreoffice_save_document called");

    let guard = state.client.lock().await;

    let client = guard.as_ref()
        .ok_or_else(|| LibreOfficeError::NotInitialized.to_string())?;

    let args = if let Some(p) = path {
        serde_json::json!({ "path": p })
    } else {
        serde_json::json!({})
    };

    match client.call_tool("save_document", args).await {
        Ok(result) => Ok(ToolCallResponse {
            success: true,
            result: Some(result),
            error: None,
        }),
        Err(e) => Ok(ToolCallResponse {
            success: false,
            result: None,
            error: Some(e.to_string()),
        }),
    }
}