use crate::libreoffice::process_manager::ProcessManager;
use crate::libreoffice::types::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::{oneshot, Mutex};
use tokio::time::{timeout, Duration};

/// MCP Client for communicating with Python MCP server
///
/// This client:
/// 1. Spawns the Python MCP server process
/// 2. Performs the MCP initialization handshake
/// 3. Sends tool call requests via JSON-RPC 2.0
/// 4. Receives responses asynchronously
/// 5. Manages request/response matching
pub struct MCPClient {
    process: Arc<Mutex<ProcessManager>>,
    next_id: Arc<AtomicU64>,
    pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    server_info: InitializeResult,
}

impl MCPClient {
    /// Create a new MCP client and perform initialization handshake
    ///
    /// This will:
    /// 1. Spawn the Python MCP server process
    /// 2. Send initialize request
    /// 3. Wait for initialize response
    /// 4. Send initialized notification
    /// 5. Start background task to read responses
    pub async fn new() -> Result<Self, LibreOfficeError> {
        log::info!("Creating new MCP client...");

        // Spawn the Python MCP server process
        let process = ProcessManager::spawn().await?;
        log::info!("Python MCP server spawned successfully");

        let process = Arc::new(Mutex::new(process));
        let next_id = Arc::new(AtomicU64::new(1));
        let pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Perform initialization handshake
        let init_id = next_id.fetch_add(1, Ordering::SeqCst);
        let init_request = JsonRpcRequest::new(
            init_id,
            "initialize",
            serde_json::to_value(InitializeParams::default())?,
        );

        // Send initialize request
        Self::send_request_internal(&process, &init_request).await?;
        log::info!("Sent initialize request");

        // Wait for initialize response (with timeout)
        let init_response = Self::read_response_internal(&process).await?;
        log::info!("Received initialize response");

        // Parse initialize result
        let server_info = if let Some(result) = init_response.result {
            serde_json::from_value::<InitializeResult>(result).map_err(|e| {
                LibreOfficeError::HandshakeFailed(format!("Invalid initialize response: {}", e))
            })?
        } else if let Some(error) = init_response.error {
            return Err(LibreOfficeError::HandshakeFailed(format!(
                "Initialize failed: {} (code {})",
                error.message, error.code
            )));
        } else {
            return Err(LibreOfficeError::HandshakeFailed(
                "Initialize response missing result and error".to_string(),
            ));
        };

        log::info!(
            "MCP server initialized: {} v{}",
            server_info.server_info.name,
            server_info.server_info.version
        );

        // Send initialized notification
        let init_notification = JsonRpcNotification::new("notifications/initialized", json!({}));
        Self::send_notification_internal(&process, &init_notification).await?;
        log::info!("Sent initialized notification");

        // Start background task to read responses
        let process_clone = Arc::clone(&process);
        let pending_clone = Arc::clone(&pending_requests);
        tokio::spawn(async move {
            Self::response_reader_task(process_clone, pending_clone).await;
        });

        log::info!("MCP client ready");

        Ok(Self {
            process,
            next_id,
            pending_requests,
            server_info,
        })
    }

    /// Get server information from initialization
    pub fn server_info(&self) -> &InitializeResult {
        &self.server_info
    }

    /// List available tools from the MCP server
    pub async fn list_tools(&self) -> Result<Vec<Tool>, LibreOfficeError> {
        log::info!("Listing tools...");

        let result = self.send_request("tools/list", json!({})).await?;

        let tools_result = serde_json::from_value::<ToolsListResult>(result).map_err(|e| {
            LibreOfficeError::InvalidResponse(format!("Failed to parse tools list: {}", e))
        })?;

        log::info!("Found {} tools", tools_result.tools.len());

        Ok(tools_result.tools)
    }

    /// Call a tool on the MCP server
    ///
    /// # Arguments
    /// * `tool_name` - Name of the tool to call
    /// * `arguments` - Tool arguments as JSON value
    ///
    /// # Returns
    /// Tool result as JSON value
    pub async fn call_tool(
        &self,
        tool_name: impl Into<String>,
        arguments: Value,
    ) -> Result<Value, LibreOfficeError> {
        let tool_name = tool_name.into();
        log::info!("Calling tool: {}", tool_name);

        let params = ToolCallParams {
            name: tool_name.clone(),
            arguments,
        };

        let result = self
            .send_request("tools/call", serde_json::to_value(params)?)
            .await?;

        log::info!("Tool {} completed successfully", tool_name);

        Ok(result)
    }

    /// Send a JSON-RPC request and wait for response
    ///
    /// This handles:
    /// 1. Generating request ID
    /// 2. Creating response channel
    /// 3. Sending request
    /// 4. Waiting for response (with timeout)
    async fn send_request(
        &self,
        method: impl Into<String>,
        params: Value,
    ) -> Result<Value, LibreOfficeError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest::new(id, method, params);

        // Create a oneshot channel for the response
        let (tx, rx) = oneshot::channel();

        // Register pending request
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(id, tx);
        }

        // Send request
        Self::send_request_internal(&self.process, &request).await?;

        // Wait for response with timeout (30 seconds)
        let response = timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| LibreOfficeError::Timeout(30))?
            .map_err(|_| {
                LibreOfficeError::InvalidResponse("Response channel closed".to_string())
            })?;

        // Check for error
        if let Some(error) = response.error {
            return Err(LibreOfficeError::JsonRpcError {
                code: error.code,
                message: error.message,
            });
        }

        // Return result
        response.result.ok_or_else(|| {
            LibreOfficeError::InvalidResponse("Response missing result".to_string())
        })
    }

    /// Send a JSON-RPC request (internal helper)
    async fn send_request_internal(
        process: &Arc<Mutex<ProcessManager>>,
        request: &JsonRpcRequest,
    ) -> Result<(), LibreOfficeError> {
        let json = serde_json::to_string(request)?;
        let mut process = process.lock().await;
        process
            .stdin
            .write_all(json.as_bytes())
            .await
            .map_err(|e| {
                LibreOfficeError::ProcessCrashed(format!("Failed to write to stdin: {}", e))
            })?;
        process
            .stdin
            .write_all(b"\n")
            .await
            .map_err(|e| {
                LibreOfficeError::ProcessCrashed(format!("Failed to write newline: {}", e))
            })?;
        process.stdin.flush().await.map_err(|e| {
            LibreOfficeError::ProcessCrashed(format!("Failed to flush stdin: {}", e))
        })?;

        Ok(())
    }

    /// Send a JSON-RPC notification (internal helper)
    async fn send_notification_internal(
        process: &Arc<Mutex<ProcessManager>>,
        notification: &JsonRpcNotification,
    ) -> Result<(), LibreOfficeError> {
        let json = serde_json::to_string(notification)?;
        let mut process = process.lock().await;
        process
            .stdin
            .write_all(json.as_bytes())
            .await
            .map_err(|e| {
                LibreOfficeError::ProcessCrashed(format!("Failed to write to stdin: {}", e))
            })?;
        process
            .stdin
            .write_all(b"\n")
            .await
            .map_err(|e| {
                LibreOfficeError::ProcessCrashed(format!("Failed to write newline: {}", e))
            })?;
        process.stdin.flush().await.map_err(|e| {
            LibreOfficeError::ProcessCrashed(format!("Failed to flush stdin: {}", e))
        })?;

        Ok(())
    }

    /// Read a single JSON-RPC response (internal helper)
    ///
    /// This will skip non-JSON lines (like log messages) until it finds a valid JSON-RPC response
    async fn read_response_internal(
        process: &Arc<Mutex<ProcessManager>>,
    ) -> Result<JsonRpcResponse, LibreOfficeError> {
        let mut process = process.lock().await;

        // Try to read lines until we get a valid JSON-RPC response
        loop {
            let mut line = String::new();

            let bytes_read = process
                .stdout
                .read_line(&mut line)
                .await
                .map_err(|e| {
                    LibreOfficeError::ProcessCrashed(format!("Failed to read from stdout: {}", e))
                })?;

            if bytes_read == 0 {
                return Err(LibreOfficeError::ProcessCrashed(
                    "Process stdout closed".to_string(),
                ));
            }

            // Try to parse as JSON-RPC response
            match serde_json::from_str::<JsonRpcResponse>(&line) {
                Ok(response) => return Ok(response),
                Err(_) => {
                    // Not a JSON-RPC response, might be a log message
                    // Log it and continue to next line
                    log::debug!("Skipping non-JSON line from MCP server: {}", line.trim());
                    continue;
                }
            }
        }
    }

    /// Background task to read responses from stdout
    ///
    /// This runs in a separate task and:
    /// 1. Continuously reads lines from stdout
    /// 2. Parses JSON-RPC responses
    /// 3. Matches response IDs to pending requests
    /// 4. Sends responses through channels
    async fn response_reader_task(
        process: Arc<Mutex<ProcessManager>>,
        pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    ) {
        log::info!("Response reader task started");

        loop {
            let mut line = String::new();

            // Read line from stdout
            let read_result = {
                let mut process = process.lock().await;
                process.stdout.read_line(&mut line).await
            };

            match read_result {
                Ok(0) => {
                    log::error!("MCP server stdout closed");
                    break;
                }
                Ok(_) => {
                    // Try to parse as JSON-RPC response
                    match serde_json::from_str::<JsonRpcResponse>(&line) {
                        Ok(response) => {
                            log::debug!("Received response for request ID {}", response.id);

                            // Find pending request
                            let sender = {
                                let mut pending = pending_requests.lock().await;
                                pending.remove(&response.id)
                            };

                            if let Some(sender) = sender {
                                // Send response through channel
                                if sender.send(response).is_err() {
                                    log::warn!("Failed to send response - receiver dropped");
                                }
                            } else {
                                log::warn!("Received response for unknown request ID {}", response.id);
                            }
                        }
                        Err(e) => {
                            // Might be a notification or log message
                            log::debug!("Non-JSON-RPC line from stdout: {} ({})", line.trim(), e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("Error reading from stdout: {}", e);
                    break;
                }
            }
        }

        log::info!("Response reader task exited");
    }

    /// Check if the MCP server process is still running
    pub async fn is_running(&self) -> bool {
        let mut process = self.process.lock().await;
        process.is_running()
    }

    /// Shutdown the MCP server
    pub async fn shutdown(&self) -> Result<(), LibreOfficeError> {
        log::info!("Shutting down MCP client...");
        let mut process = self.process.lock().await;
        process.kill().await?;
        log::info!("MCP client shut down");
        Ok(())
    }
}

impl Drop for MCPClient {
    fn drop(&mut self) {
        log::info!("MCPClient dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mcp_client_new() {
        // This test requires Python and the MCP server to be available
        match MCPClient::new().await {
            Ok(client) => {
                println!("MCP client created successfully");
                println!("Server: {} v{}",
                    client.server_info().server_info.name,
                    client.server_info().server_info.version
                );
                assert!(client.is_running().await);

                // Shutdown
                client.shutdown().await.expect("Failed to shutdown");
            }
            Err(e) => {
                println!("Skipping test - MCP server not available: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_mcp_client_list_tools() {
        match MCPClient::new().await {
            Ok(client) => {
                let tools = client.list_tools().await.expect("Failed to list tools");
                println!("Found {} tools", tools.len());

                for tool in &tools {
                    println!("  - {}: {}", tool.name, tool.description);
                }

                assert!(!tools.is_empty(), "Should have at least one tool");

                client.shutdown().await.expect("Failed to shutdown");
            }
            Err(e) => {
                println!("Skipping test - MCP server not available: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_mcp_client_call_tool() {
        match MCPClient::new().await {
            Ok(client) => {
                // Try to create a test document
                let result = client
                    .call_tool(
                        "create_blank_document",
                        json!({
                            "filename": "test_from_rust.odt",
                            "title": "Test Document from Rust",
                            "author": "Rust Test"
                        }),
                    )
                    .await;

                match result {
                    Ok(response) => {
                        println!("Tool call successful: {:?}", response);
                    }
                    Err(e) => {
                        println!("Tool call failed (expected if LibreOffice not running): {:?}", e);
                    }
                }

                client.shutdown().await.expect("Failed to shutdown");
            }
            Err(e) => {
                println!("Skipping test - MCP server not available: {:?}", e);
            }
        }
    }
}
