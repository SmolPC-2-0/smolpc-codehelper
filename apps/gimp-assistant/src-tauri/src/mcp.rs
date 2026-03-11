use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

/// Folder where gimp_mcp_server.py lives
const GIMP_MCP_PATH: &str = "/Users/aishah/gimp-mcp";

struct McpConnection {
    #[allow(dead_code)]
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    initialized: bool,
}

impl McpConnection {
    fn new() -> Result<Self, String> {
        eprintln!("[MCP] Starting gimp-mcp server (real mode)…");

        let mut child = Command::new("uv")
            .arg("run")
            .arg("--directory")
            .arg(GIMP_MCP_PATH)
            .arg("gimp_mcp_server.py")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("Failed to start gimp-mcp server: {e}"))?;

        eprintln!("[MCP] gimp-mcp server started (pid={})", child.id());

        let stdin = child
            .stdin
            .take()
            .ok_or("Failed to open stdin for gimp-mcp")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to open stdout for gimp-mcp")?;

        Ok(Self {
            child,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
            next_id: 1,
            initialized: false,
        })
    }

    fn ensure_initialized(&mut self) -> Result<(), String> {
        if self.initialized {
            return Ok(());
        }

        let id = self.next_id;
        self.next_id += 1;

        let init_req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                // spec-ish version string; most servers just ignore this
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": { "listChanged": true },
                    "roots": { "listChanged": true }
                },
                "clientInfo": {
                    "name": "smolpc-gimp-assistant",
                    "version": "0.1.0"
                }
            }
        });

        eprintln!("[MCP] Sending initialize request (id={id})…");
        self.send_message(&init_req)?;
        let resp = self.read_response_for_id(id)?;

        eprintln!("[MCP] Initialize response: {resp}");

        if let Some(err) = resp.get("error") {
            return Err(format!("Initialize error from server: {err}"));
        }

        // Send notifications/initialized (no response expected)
        let initialized = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });
        self.send_message(&initialized)?;

        self.initialized = true;
        Ok(())
    }

    fn send_request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        use std::time::Instant;
        self.ensure_initialized()?;

        let id = self.next_id;
        self.next_id += 1;

        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        eprintln!("[MCP] Sending request id={id}, method={method}");
        self.send_message(&req)?;
        let resp = self.read_response_for_id(id)?;
        eprintln!("[MCP] Got response for id={id}: {resp}");

        if let Some(err) = resp.get("error") {
            return Err(format!("Server error: {err}"));
        }

        return resp.get("result")
            .cloned()
            .ok_or_else(|| "Missing result in MCP response".to_string());
        

    }

    /// Write one JSON object per line (MCP stdio format)
    fn send_message(&mut self, value: &Value) -> Result<(), String> {
        let json = serde_json::to_string(value)
            .map_err(|e| format!("Failed to serialize MCP request: {e}"))?;

        self.stdin
            .write_all(json.as_bytes())
            .and_then(|_| self.stdin.write_all(b"\n"))
            .and_then(|_| self.stdin.flush())
            .map_err(|e| format!("Failed to write to MCP server: {e}"))?;

        Ok(())
    }

    /// Read one line of JSON from stdout
    fn read_message(&mut self) -> Result<Value, String> {
        let mut line = String::new();
        let bytes_read = self
            .stdout
            .read_line(&mut line)
            .map_err(|e| format!("[MCP] Failed to read from MCP server: {e}"))?;

        if bytes_read == 0 {
            return Err(
                "MCP server closed the connection. Make sure GIMP is running, an image is open, and Tools → Start MCP Server has been clicked."
                    .to_string(),
            );
        }

        let line_trimmed = line.trim_end();

        serde_json::from_str(line_trimmed).map_err(|e| {
            format!(
                "[MCP] Invalid JSON from MCP server: {e}\nLine was: {line_trimmed}"
            )
        })
    }

    fn read_response_for_id(&mut self, target_id: u64) -> Result<Value, String> {
        loop {
            let msg = self.read_message()?;

            if let Some(id_val) = msg.get("id") {
                if let Some(id) = id_val.as_u64() {
                    if id == target_id {
                        return Ok(msg);
                    } else {
                        eprintln!("[MCP] Ignoring response for other id={id}");
                        continue;
                    }
                }
            }

            // Notifications etc: ignore
            eprintln!("[MCP] Ignoring notification/unknown message: {msg}");
        }
    }
}

// Global singleton connection
static MCP: Lazy<Mutex<Option<McpConnection>>> = Lazy::new(|| Mutex::new(None));

fn with_connection<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&mut McpConnection) -> Result<R, String>,
{
    let mut guard = MCP
        .lock()
        .map_err(|_| "MCP mutex poisoned".to_string())?;

    if guard.is_none() {
        *guard = Some(McpConnection::new()?);
    }

    let conn = guard.as_mut().unwrap();
    f(conn)
}

/// Public API used by Tauri commands

pub fn list_tools() -> Result<Value, String> {
    with_connection(|conn| conn.send_request("tools/list", json!({ "cursor": null })))
}

// pub fn call_tool(name: &str, arguments: Value) -> Result<Value, String> {
//     with_connection(|conn| {
//         conn.send_request(
//             "tools/call",
//             json!({
//                 "name": name,
//                 "arguments": arguments
//             }),
//         )
//     })
// }
pub fn call_tool(name: &str, arguments: Value) -> Result<Value, String> {
    let start = std::time::Instant::now();

    println!("\n================ MCP CALL ================");
    println!("Tool: {}", name);
    println!("Arguments: {}", arguments);
    println!("------------------------------------------");

    let result = with_connection(|conn| {
        conn.send_request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments
            }),
        )
    });

    match result {
        Ok(response) => {
            println!("MCP OK ({} ms)", start.elapsed().as_millis());
            println!("Response: {:#?}", response);
            println!("==========================================\n");
            Ok(response)
        }
        Err(err) => {
            println!("MCP ERROR ({} ms)", start.elapsed().as_millis());
            println!("Error: {}", err);
            println!("==========================================\n");
            Err(err)
        }
    }
}
