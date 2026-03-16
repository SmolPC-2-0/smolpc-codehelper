pub mod error;
pub mod id;
pub mod jsonrpc;
pub mod mcp;
pub mod tcp;
pub mod transport;

pub use error::McpClientError;
pub use id::RequestIdGenerator;
pub use jsonrpc::{JsonRpcError, JsonRpcId, JsonRpcRequest, JsonRpcResponse};
pub use mcp::{McpSession, McpTool};
pub use tcp::TcpJsonRpcClient;
pub use transport::{
    JsonRpcClient, JsonRpcTransport, StdioTransportConfig, TcpTransportConfig, TransportConfig,
};
