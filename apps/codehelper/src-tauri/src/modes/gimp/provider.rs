use super::macros;
use super::response::build_tool_execution_result;
use super::transport::{connect_session, default_transport_config, tool_definition};
use crate::modes::provider::{provider_state, ToolProvider};
use async_trait::async_trait;
use smolpc_assistant_types::{
    AppMode, ProviderStateDto, ToolDefinitionDto, ToolExecutionResultDto,
};
use smolpc_mcp_client::{McpSession, TcpTransportConfig};
use tokio::sync::Mutex;

const GIMP_CONNECT_HINT: &str =
    "Make sure GIMP is running and the MCP plugin/server is active on 127.0.0.1:10008.";

#[derive(Debug)]
struct RuntimeState {
    session: Option<McpSession>,
    tools: Vec<ToolDefinitionDto>,
    last_error: Option<String>,
    ever_connected: bool,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            session: None,
            tools: Vec::new(),
            last_error: None,
            ever_connected: false,
        }
    }
}

#[derive(Debug)]
pub struct GimpProvider {
    config: TcpTransportConfig,
    state: Mutex<RuntimeState>,
}

impl Default for GimpProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GimpProvider {
    pub fn new() -> Self {
        Self::with_config(default_transport_config())
    }

    pub fn with_config(config: TcpTransportConfig) -> Self {
        Self {
            config,
            state: Mutex::new(RuntimeState::default()),
        }
    }

    fn connected_state(mode: AppMode) -> ProviderStateDto {
        provider_state(mode, "connected", None, true, true)
    }

    fn disconnected_state(mode: AppMode, state: &RuntimeState) -> ProviderStateDto {
        let label = if state.ever_connected && state.last_error.is_some() {
            "error"
        } else {
            "disconnected"
        };

        provider_state(mode, label, state.last_error.as_deref(), true, true)
    }

    fn friendly_error(&self, error: &str) -> String {
        if error.contains("connect tcp") || error.contains("connection closed") {
            return format!("Unable to reach the GIMP MCP server. {GIMP_CONNECT_HINT}");
        }

        format!("GIMP MCP request failed: {error}")
    }

    async fn connect_locked(
        &self,
        mode: AppMode,
        state: &mut RuntimeState,
    ) -> Result<ProviderStateDto, String> {
        let session = connect_session(&self.config)
            .await
            .map_err(|error| self.friendly_error(&error))?;
        let tools = session
            .list_tools()
            .await
            .map_err(|error| self.friendly_error(&error.to_string()))?;

        state.tools = tools.into_iter().map(tool_definition).collect();
        state.session = Some(session);
        state.last_error = None;
        state.ever_connected = true;

        Ok(Self::connected_state(mode))
    }

    async fn refresh_connected_state(
        &self,
        mode: AppMode,
        state: &mut RuntimeState,
    ) -> ProviderStateDto {
        if let Some(session) = state.session.as_ref() {
            match session.list_tools().await {
                Ok(tools) => {
                    state.tools = tools.into_iter().map(tool_definition).collect();
                    state.last_error = None;
                    return Self::connected_state(mode);
                }
                Err(error) => {
                    state.session = None;
                    state.tools.clear();
                    state.last_error = Some(self.friendly_error(&error.to_string()));
                }
            }
        }

        match self.connect_locked(mode, state).await {
            Ok(provider_state) => provider_state,
            Err(error) => {
                state.last_error = Some(error);
                Self::disconnected_state(mode, state)
            }
        }
    }
}

#[async_trait]
impl ToolProvider for GimpProvider {
    async fn connect_if_needed(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
        let mut state = self.state.lock().await;
        if state.session.is_some() {
            return Ok(Self::connected_state(mode));
        }

        self.connect_locked(mode, &mut state).await
    }

    async fn status(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
        let mut state = self.state.lock().await;
        Ok(self.refresh_connected_state(mode, &mut state).await)
    }

    async fn list_tools(&self, _mode: AppMode) -> Result<Vec<ToolDefinitionDto>, String> {
        let state = self.state.lock().await;
        Ok(state.tools.clone())
    }

    async fn execute_tool(
        &self,
        mode: AppMode,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolExecutionResultDto, String> {
        {
            let mut state = self.state.lock().await;
            if state.session.is_none() {
                self.connect_locked(mode, &mut state).await?;
            }
        }

        let mut state = self.state.lock().await;
        let session = state
            .session
            .as_ref()
            .ok_or_else(|| "GIMP provider is not connected".to_string())?;

        match session.call_tool(name, arguments).await {
            Ok(payload) => {
                let tool_result = build_tool_execution_result(name, payload);
                if tool_result.ok {
                    state.last_error = None;
                } else {
                    state.last_error = Some(tool_result.summary.clone());
                }
                Ok(tool_result)
            }
            Err(error) => {
                let message = self.friendly_error(&error.to_string());
                state.session = None;
                state.tools.clear();
                state.last_error = Some(message.clone());
                Err(message)
            }
        }
    }

    async fn undo_last_action(&self, mode: AppMode) -> Result<(), String> {
        let payload = macros::undo();
        let name = payload
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "Undo payload missing tool name".to_string())?;
        let arguments = payload
            .get("arguments")
            .cloned()
            .ok_or_else(|| "Undo payload missing arguments".to_string())?;
        let result = self.execute_tool(mode, name, arguments).await?;
        if result.ok {
            Ok(())
        } else {
            Err(result.summary)
        }
    }

    async fn disconnect_if_needed(&self, _mode: AppMode) -> Result<(), String> {
        let mut state = self.state.lock().await;
        state.session = None;
        state.tools.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::GimpProvider;
    use crate::modes::provider::ToolProvider;
    use serde_json::json;
    use smolpc_assistant_types::AppMode;
    use smolpc_mcp_client::TcpTransportConfig;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn disconnected_state_is_honest_when_server_is_unreachable() {
        let provider = GimpProvider::with_config(TcpTransportConfig {
            host: "127.0.0.1".to_string(),
            port: 1,
        });

        let state = provider.status(AppMode::Gimp).await.expect("status");
        assert_eq!(state.state, "disconnected");
        assert!(state.detail.expect("detail").contains("Unable to reach"));
    }

    #[tokio::test]
    async fn connected_state_includes_tool_definitions() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let port = listener.local_addr().expect("listener addr").port();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            reader.read_line(&mut line).await.expect("read initialize");
            let initialize: serde_json::Value =
                serde_json::from_str(line.trim_end()).expect("init json");
            writer
                .write_all(
                    format!(
                        "{}\n",
                        json!({
                            "jsonrpc": "2.0",
                            "id": initialize["id"],
                            "result": { "serverInfo": { "name": "gimp-mcp", "version": "test" } }
                        })
                    )
                    .as_bytes(),
                )
                .await
                .expect("write init response");

            line.clear();
            reader.read_line(&mut line).await.expect("read initialized");

            for _ in 0..2 {
                line.clear();
                reader.read_line(&mut line).await.expect("read tools/list");
                let request: serde_json::Value =
                    serde_json::from_str(line.trim_end()).expect("tools list request");
                writer
                    .write_all(
                        format!(
                            "{}\n",
                            json!({
                                "jsonrpc": "2.0",
                                "id": request["id"],
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
                    .expect("write tools/list response");
            }
        });

        let provider = GimpProvider::with_config(TcpTransportConfig {
            host: "127.0.0.1".to_string(),
            port,
        });

        let state = provider.status(AppMode::Gimp).await.expect("status");
        assert_eq!(state.state, "connected");

        let tools = provider.list_tools(AppMode::Gimp).await.expect("tools");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "get_image_metadata");

        server.await.expect("server task");
    }
}
