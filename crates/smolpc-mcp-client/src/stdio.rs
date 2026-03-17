use crate::error::McpClientError;
use crate::id::RequestIdGenerator;
use crate::jsonrpc::{JsonRpcId, JsonRpcRequest, JsonRpcResponse};
use crate::transport::{JsonRpcClient, JsonRpcTransport, StdioTransportConfig, TransportConfig};
use async_trait::async_trait;
use serde_json::Value;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StdioJsonRpcTransport {
    config: TransportConfig,
}

const STDIO_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
const STDERR_BUFFER_LIMIT: usize = 8;

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
    stderr_lines: Arc<Mutex<Vec<String>>>,
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
            let mut detail = match status {
                Some(status) => format!("child process exited with status {status}"),
                None => "child process closed stdio unexpectedly".to_string(),
            };
            let stderr_summary = self.stderr_summary().await;
            if !stderr_summary.is_empty() {
                detail.push_str(". stderr: ");
                detail.push_str(&stderr_summary);
            }
            return Err(McpClientError::Transport(detail));
        }

        serde_json::from_str(line.trim_end())
            .map_err(|error| McpClientError::JsonRpc(format!("invalid response json: {error}")))
    }

    async fn stderr_summary(&self) -> String {
        let lines = self.stderr_lines.lock().await;
        lines.join(" | ")
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
            .stderr(Stdio::piped())
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
        let stderr = child.stderr.take().ok_or_else(|| {
            McpClientError::Transport("stdio MCP command did not expose stderr".to_string())
        })?;
        let stderr_lines = Arc::new(Mutex::new(Vec::new()));
        spawn_stderr_drain(stderr, Arc::clone(&stderr_lines));

        Ok(Self {
            transport: StdioJsonRpcTransport::new(config),
            connection: Mutex::new(StdioConnection {
                child,
                reader: BufReader::new(stdout),
                writer: BufWriter::new(stdin),
                stderr_lines,
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
            let message = timeout(STDIO_RESPONSE_TIMEOUT, connection.read_json())
                .await
                .map_err(|_| {
                    McpClientError::Transport(format!(
                        "timed out waiting for stdio MCP response after {}s",
                        STDIO_RESPONSE_TIMEOUT.as_secs()
                    ))
                })??;
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

fn spawn_stderr_drain(stderr: ChildStderr, stderr_lines: Arc<Mutex<Vec<String>>>) {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    let entry = line.trim_end().to_string();
                    if entry.is_empty() {
                        continue;
                    }

                    let mut lines = stderr_lines.lock().await;
                    if lines.len() == STDERR_BUFFER_LIMIT {
                        lines.remove(0);
                    }
                    lines.push(entry);
                }
                Err(_) => break,
            }
        }
    });
}
