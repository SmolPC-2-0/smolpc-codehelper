use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpClientError {
    #[error("invalid transport configuration: {0}")]
    InvalidTransportConfig(String),
    #[error("json-rpc error: {0}")]
    JsonRpc(String),
    #[error("transport error: {0}")]
    Transport(String),
}
