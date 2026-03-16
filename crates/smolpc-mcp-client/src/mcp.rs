use crate::error::McpClientError;
use crate::jsonrpc::JsonRpcRequest;
use crate::tcp::TcpJsonRpcClient;
use crate::transport::TcpTransportConfig;
use crate::JsonRpcClient;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug)]
pub struct McpSession {
    client: TcpJsonRpcClient,
}

impl McpSession {
    pub async fn connect_tcp(
        config: TcpTransportConfig,
        client_name: &str,
        client_version: &str,
    ) -> Result<Self, McpClientError> {
        let client = TcpJsonRpcClient::connect(config).await?;
        let initialize_request = JsonRpcRequest::new(
            client.next_request_id(),
            "initialize",
            Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": { "listChanged": true },
                    "roots": { "listChanged": true }
                },
                "clientInfo": {
                    "name": client_name,
                    "version": client_version
                }
            })),
        );
        let response = client.call(initialize_request).await?;
        if let Some(error) = response.error {
            return Err(McpClientError::JsonRpc(format!(
                "initialize failed: {} ({})",
                error.message, error.code
            )));
        }

        client
            .notify("notifications/initialized", Some(json!({})))
            .await?;

        Ok(Self { client })
    }

    pub fn transport_config(&self) -> &TcpTransportConfig {
        self.client.transport_config()
    }

    pub async fn list_tools(&self) -> Result<Vec<McpTool>, McpClientError> {
        let response = self
            .client
            .call(JsonRpcRequest::new(
                self.client.next_request_id(),
                "tools/list",
                Some(json!({ "cursor": null })),
            ))
            .await?;
        if let Some(error) = response.error {
            return Err(McpClientError::JsonRpc(format!(
                "tools/list failed: {} ({})",
                error.message, error.code
            )));
        }

        let tools = response
            .result
            .and_then(|result| result.get("tools").cloned())
            .ok_or_else(|| McpClientError::JsonRpc("tools/list missing tools array".to_string()))?;

        serde_json::from_value(tools)
            .map_err(|error| McpClientError::JsonRpc(format!("decode tools/list: {error}")))
    }

    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, McpClientError> {
        let response = self
            .client
            .call(JsonRpcRequest::new(
                self.client.next_request_id(),
                "tools/call",
                Some(json!({
                    "name": name,
                    "arguments": arguments
                })),
            ))
            .await?;
        if let Some(error) = response.error {
            return Err(McpClientError::JsonRpc(format!(
                "tools/call failed for {name}: {} ({})",
                error.message, error.code
            )));
        }

        response
            .result
            .ok_or_else(|| McpClientError::JsonRpc("tools/call missing result".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::McpSession;
    use crate::transport::TcpTransportConfig;
    use serde_json::json;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn mcp_session_initializes_lists_tools_and_calls_tool() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            reader.read_line(&mut line).await.expect("read initialize");
            let init: serde_json::Value = serde_json::from_str(line.trim_end()).expect("init json");
            let init_id = init["id"].clone();
            writer
                .write_all(
                    format!(
                        "{}\n",
                        json!({
                            "jsonrpc": "2.0",
                            "id": init_id,
                            "result": { "serverInfo": { "name": "gimp-mcp", "version": "test" } }
                        })
                    )
                    .as_bytes(),
                )
                .await
                .expect("write init response");

            line.clear();
            reader.read_line(&mut line).await.expect("read initialized");
            let initialized: serde_json::Value =
                serde_json::from_str(line.trim_end()).expect("initialized json");
            assert_eq!(initialized["method"], "notifications/initialized");

            line.clear();
            reader.read_line(&mut line).await.expect("read tools list");
            let list_request: serde_json::Value =
                serde_json::from_str(line.trim_end()).expect("tools list json");
            writer
                .write_all(
                    format!(
                        "{}\n",
                        json!({
                            "jsonrpc": "2.0",
                            "id": list_request["id"],
                            "result": {
                                "tools": [
                                    {
                                        "name": "get_image_metadata",
                                        "description": "Current image metadata",
                                        "inputSchema": { "type": "object" }
                                    }
                                ]
                            }
                        })
                    )
                    .as_bytes(),
                )
                .await
                .expect("write tools list response");

            line.clear();
            reader.read_line(&mut line).await.expect("read tools call");
            let call_request: serde_json::Value =
                serde_json::from_str(line.trim_end()).expect("tools call json");
            assert_eq!(call_request["params"]["name"], "get_image_metadata");
            writer
                .write_all(
                    format!(
                        "{}\n",
                        json!({
                            "jsonrpc": "2.0",
                            "id": call_request["id"],
                            "result": {
                                "content": [{ "type": "text", "text": "{\"basic\":{\"width\":640}}" }]
                            }
                        })
                    )
                    .as_bytes(),
                )
                .await
                .expect("write tools call response");
        });

        let session = McpSession::connect_tcp(
            TcpTransportConfig {
                host: "127.0.0.1".to_string(),
                port: addr.port(),
            },
            "smolpc-test",
            "0.1.0",
        )
        .await
        .expect("connect session");

        let tools = session.list_tools().await.expect("list tools");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "get_image_metadata");

        let result = session
            .call_tool("get_image_metadata", json!({}))
            .await
            .expect("call tool");
        assert_eq!(result["content"][0]["type"], "text");

        server.await.expect("server task");
    }
}
