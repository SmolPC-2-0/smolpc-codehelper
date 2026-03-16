use crate::error::McpClientError;
use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse};
use async_trait::async_trait;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StdioTransportConfig {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TcpTransportConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportConfig {
    Stdio(StdioTransportConfig),
    Tcp(TcpTransportConfig),
}

impl TransportConfig {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Stdio(_) => "stdio",
            Self::Tcp(_) => "tcp",
        }
    }

    pub fn validate(&self) -> Result<(), McpClientError> {
        match self {
            Self::Stdio(config) => {
                if config.command.trim().is_empty() {
                    return Err(McpClientError::InvalidTransportConfig(
                        "stdio command must not be empty".to_string(),
                    ));
                }
            }
            Self::Tcp(config) => {
                if config.host.trim().is_empty() {
                    return Err(McpClientError::InvalidTransportConfig(
                        "tcp host must not be empty".to_string(),
                    ));
                }
                if config.port == 0 {
                    return Err(McpClientError::InvalidTransportConfig(
                        "tcp port must be greater than zero".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }
}

pub trait JsonRpcTransport: Send + Sync {
    fn config(&self) -> &TransportConfig;
}

#[async_trait]
pub trait JsonRpcClient: Send + Sync {
    fn transport(&self) -> &dyn JsonRpcTransport;

    async fn call(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, McpClientError>;
}
