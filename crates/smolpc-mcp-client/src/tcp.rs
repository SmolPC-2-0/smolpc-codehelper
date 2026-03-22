use crate::error::McpClientError;
use crate::id::RequestIdGenerator;
use crate::jsonrpc::{JsonRpcId, JsonRpcRequest, JsonRpcResponse};
use crate::transport::{JsonRpcClient, JsonRpcTransport, TcpTransportConfig, TransportConfig};
use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TcpJsonRpcTransport {
    config: TransportConfig,
}

impl TcpJsonRpcTransport {
    fn new(config: TcpTransportConfig) -> Self {
        Self {
            config: TransportConfig::Tcp(config),
        }
    }
}

impl JsonRpcTransport for TcpJsonRpcTransport {
    fn config(&self) -> &TransportConfig {
        &self.config
    }
}

#[derive(Debug)]
struct TcpConnection {
    reader: BufReader<OwnedReadHalf>,
    writer: BufWriter<OwnedWriteHalf>,
}

impl TcpConnection {
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
            return Err(McpClientError::Transport(
                "connection closed by MCP server".to_string(),
            ));
        }

        serde_json::from_str(line.trim_end())
            .map_err(|error| McpClientError::JsonRpc(format!("invalid response json: {error}")))
    }
}

#[derive(Debug)]
pub struct TcpJsonRpcClient {
    transport: TcpJsonRpcTransport,
    connection: Mutex<TcpConnection>,
    request_ids: RequestIdGenerator,
}

impl TcpJsonRpcClient {
    pub async fn connect(config: TcpTransportConfig) -> Result<Self, McpClientError> {
        TransportConfig::Tcp(config.clone()).validate()?;

        let stream = TcpStream::connect((config.host.as_str(), config.port))
            .await
            .map_err(|error| {
                McpClientError::Transport(format!(
                    "connect tcp {}:{}: {error}",
                    config.host, config.port
                ))
            })?;
        let (reader, writer) = stream.into_split();

        Ok(Self {
            transport: TcpJsonRpcTransport::new(config),
            connection: Mutex::new(TcpConnection {
                reader: BufReader::new(reader),
                writer: BufWriter::new(writer),
            }),
            request_ids: RequestIdGenerator::new(),
        })
    }

    pub fn transport_config(&self) -> &TcpTransportConfig {
        match self.transport.config() {
            TransportConfig::Tcp(config) => config,
            TransportConfig::Stdio(_) => unreachable!("tcp transport stored stdio config"),
        }
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

    pub fn next_request_id(&self) -> JsonRpcId {
        JsonRpcId::Number(self.request_ids.next())
    }
}

#[async_trait]
impl JsonRpcClient for TcpJsonRpcClient {
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

#[cfg(test)]
mod tests {
    use super::TcpJsonRpcClient;
    use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse};
    use crate::transport::TcpTransportConfig;
    use crate::JsonRpcClient;
    use serde_json::json;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn tcp_client_round_trips_call_and_notification() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            reader.read_line(&mut line).await.expect("read call");
            let request: JsonRpcRequest =
                serde_json::from_str(line.trim_end()).expect("decode request");
            assert_eq!(request.method, "tools/list");

            let response = JsonRpcResponse::success(request.id.clone(), json!({ "tools": [] }));
            writer
                .write_all(format!("{}\n", serde_json::to_string(&response).unwrap()).as_bytes())
                .await
                .expect("write response");

            line.clear();
            reader.read_line(&mut line).await.expect("read notify");
            let notify: serde_json::Value =
                serde_json::from_str(line.trim_end()).expect("decode notification");
            assert_eq!(notify["method"], "notifications/initialized");
        });

        let client = TcpJsonRpcClient::connect(TcpTransportConfig {
            host: "127.0.0.1".to_string(),
            port: addr.port(),
        })
        .await
        .expect("connect client");

        let response = client
            .call(JsonRpcRequest::new(
                client.next_request_id(),
                "tools/list",
                Some(json!({ "cursor": null })),
            ))
            .await
            .expect("call succeeds");
        assert_eq!(response.result.expect("result")["tools"], json!([]));

        client
            .notify("notifications/initialized", Some(json!({})))
            .await
            .expect("notify succeeds");

        server.await.expect("server task");
    }
}
