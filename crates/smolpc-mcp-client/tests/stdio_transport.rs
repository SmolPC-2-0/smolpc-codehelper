use serde_json::json;
use smolpc_mcp_client::{McpSession, StdioTransportConfig};
use std::collections::BTreeMap;

#[tokio::test]
async fn stdio_client_round_trips_initialize_list_and_call() {
    let session = McpSession::connect_stdio(
        StdioTransportConfig {
            command: env!("CARGO_BIN_EXE_mcp_stdio_fixture").to_string(),
            args: Vec::new(),
            cwd: None,
            env: BTreeMap::new(),
        },
        "smolpc-test",
        "0.1.0",
    )
    .await
    .expect("connect stdio session");

    let tools = session.list_tools().await.expect("list tools");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "echo_text");

    let result = session
        .call_tool("echo_text", json!({ "message": "hello from stdio" }))
        .await
        .expect("call tool");
    assert_eq!(result["content"][0]["text"], "hello from stdio");
}
