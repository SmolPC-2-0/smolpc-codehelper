use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

fn response(id: &Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(_) => continue,
        };

        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();

        let maybe_response = match method {
            "initialize" => Some(response(
                &request["id"],
                json!({
                    "serverInfo": {
                        "name": "mcp-stdio-fixture",
                        "version": "0.1.0"
                    }
                }),
            )),
            "notifications/initialized" => None,
            "tools/list" => Some(response(
                &request["id"],
                json!({
                    "tools": [
                        {
                            "name": "echo_text",
                            "description": "Echo the provided message.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "message": { "type": "string" }
                                },
                                "required": ["message"],
                                "additionalProperties": false
                            }
                        }
                    ]
                }),
            )),
            "tools/call" => {
                let text = request["params"]["arguments"]["message"]
                    .as_str()
                    .unwrap_or("missing");
                Some(response(
                    &request["id"],
                    json!({
                        "content": [
                            {
                                "type": "text",
                                "text": text
                            }
                        ]
                    }),
                ))
            }
            _ => Some(json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "error": {
                    "code": -32601,
                    "message": format!("method not found: {method}")
                }
            })),
        };

        if let Some(payload) = maybe_response {
            writeln!(stdout, "{payload}").expect("write fixture response");
            stdout.flush().expect("flush fixture response");
        }
    }
}
