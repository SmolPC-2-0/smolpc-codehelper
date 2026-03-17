use crate::error::McpClientError;
use crate::id::RequestIdGenerator;
use crate::jsonrpc::{JsonRpcId, JsonRpcRequest, JsonRpcResponse};
use crate::transport::{JsonRpcClient, JsonRpcTransport, StdioTransportConfig, TransportConfig};
use async_trait::async_trait;
use serde_json::Value;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StdioJsonRpcTransport {
    config: TransportConfig,
}

impl StdioJsonRpcTransport {
    fn new(config: StdioTransportConfig) -> Self {
        Self {
            config: TransportConfig::Stdio(config),
        }
    }
}

impl JsonRpcTransport for StdioJsonRpcTransport {
    fn config(&self) -> &TransportConfig {
        &self.config
    }
}

#[derive(Debug)]
struct StdioConnection {
    child: Child,
    reader: BufReader<ChildStdout>,
    writer: BufWriter<ChildStdin>,
}

impl StdioConnection {
    async fn send_json(&mut self, value: &Value) -> Result<(), McpClientError> {
        let encoded = serde_json::to_string(value)
            .map_err(|error| McpClientError::JsonRpc(format!("serialize request: {error}")))?;
        self.writer
            .write_all(encoded.as_bytes())
            .await
            .map_err(|error| McpClientError::Transport(format!("write request: {error}")))?;
        self.writer
            .write_all(b"\n")
            .await
            .map_err(|error| McpClientError::Transport(format!("write newline: {error}")))?;
        self.writer
            .flush()
            .await
            .map_err(|error| McpClientError::Transport(format!("flush request: {error}")))?;
        Ok(())
    }

    async fn read_json(&mut self) -> Result<Value, McpClientError> {
        let mut line = String::new();
        let bytes_read = self
            .reader
            .read_line(&mut line)
            .await
            .map_err(|error| McpClientError::Transport(format!("read response: {error}")))?;

        if bytes_read == 0 {
            let status = self.child.try_wait().map_err(|error| {
                McpClientError::Transport(format!("check child status: {error}"))
            })?;
            let detail = match status {
                Some(status) => format!("child process exited with status {status}"),
                None => "child process closed stdio unexpectedly".to_string(),
            };
            return Err(McpClientError::Transport(detail));
        }

        serde_json::from_str(line.trim_end())
            .map_err(|error| McpClientError::JsonRpc(format!("invalid response json: {error}")))
    }
}

#[derive(Debug)]
pub struct StdioJsonRpcClient {
    transport: StdioJsonRpcTransport,
    connection: Mutex<StdioConnection>,
    request_ids: RequestIdGenerator,
}

impl StdioJsonRpcClient {
    pub async fn connect(config: StdioTransportConfig) -> Result<Self, McpClientError> {
        TransportConfig::Stdio(config.clone()).validate()?;

        let mut command = Command::new(&config.command);
        command
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        if let Some(cwd) = config.cwd.as_ref() {
            command.current_dir(cwd);
        }

        let mut child = command.spawn().map_err(|error| {
            McpClientError::Transport(format!(
                "spawn stdio MCP command {}: {error}",
                config.command
            ))
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            McpClientError::Transport("stdio MCP command did not expose stdout".to_string())
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            McpClientError::Transport("stdio MCP command did not expose stdin".to_string())
        })?;

        Ok(Self {
            transport: StdioJsonRpcTransport::new(config),
            connection: Mutex::new(StdioConnection {
                child,
                reader: BufReader::new(stdout),
                writer: BufWriter::new(stdin),
            }),
            request_ids: RequestIdGenerator::new(),
        })
    }

    pub fn transport_config(&self) -> &StdioTransportConfig {
        match self.transport.config() {
            TransportConfig::Stdio(config) => config,
            TransportConfig::Tcp(_) => unreachable!("stdio transport stored tcp config"),
        }
    }

    pub fn next_request_id(&self) -> JsonRpcId {
        JsonRpcId::Number(self.request_ids.next())
    }

    pub async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), McpClientError> {
        let mut payload = serde_json::Map::new();
        payload.insert("jsonrpc".to_string(), Value::String("2.0".to_string()));
        payload.insert("method".to_string(), Value::String(method.to_string()));
        if let Some(params) = params {
            payload.insert("params".to_string(), params);
        }

        let mut connection = self.connection.lock().await;
        connection.send_json(&Value::Object(payload)).await
    }
}

#[async_trait]
impl JsonRpcClient for StdioJsonRpcClient {
    fn transport(&self) -> &dyn JsonRpcTransport {
        &self.transport
    }

    async fn call(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, McpClientError> {
        let target_id = request.id.clone();
        let request_value = serde_json::to_value(&request)
            .map_err(|error| McpClientError::JsonRpc(format!("serialize request: {error}")))?;

        let mut connection = self.connection.lock().await;
        connection.send_json(&request_value).await?;

        loop {
            let message = connection.read_json().await?;
            let response: JsonRpcResponse = match serde_json::from_value(message.clone()) {
                Ok(response) => response,
                Err(_) => {
                    continue;
                }
            };

            if response.id == target_id {
                return Ok(response);
            }
        }
    }
}
