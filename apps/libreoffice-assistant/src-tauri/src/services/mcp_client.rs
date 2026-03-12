use crate::models::mcp::*;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::fs;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct McpClient {
    process: Arc<Mutex<Option<Child>>>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    stdout_reader: Arc<Mutex<Option<BufReader<ChildStdout>>>>,
    next_id: AtomicU64,
    tools: Arc<Mutex<HashMap<String, McpTool>>>,
}

impl McpClient {
    pub fn new() -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
            stdin: Arc::new(Mutex::new(None)),
            stdout_reader: Arc::new(Mutex::new(None)),
            next_id: AtomicU64::new(1),
            tools: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn store_child_io(&self, child: &mut Child) -> Result<()> {
        let child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Failed to take stdin from child process"))?;
        let child_stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to take stdout from child process"))?;

        let mut stdin_guard = self.stdin.lock().expect("stdin mutex poisoned");
        *stdin_guard = Some(child_stdin);

        let mut reader_guard = self
            .stdout_reader
            .lock()
            .expect("stdout reader mutex poisoned");
        *reader_guard = Some(BufReader::new(child_stdout));

        Ok(())
    }

    fn prepare_python_env(command: &mut Command) {
        // Avoid writing __pycache__ into the watched mcp_server resource directory.
        command.env("PYTHONDONTWRITEBYTECODE", "1");

        // Route MCP Python logs outside of src-tauri/resources to avoid hot-reload loops.
        let log_dir = std::env::temp_dir().join("smolpc-libreoffice-mcp-logs");
        if let Err(error) = fs::create_dir_all(&log_dir) {
            log::warn!("Failed to create MCP log directory {:?}: {}", log_dir, error);
        } else {
            command.env("SMOLPC_MCP_LOG_DIR", &log_dir);
        }
    }

    pub fn start(&self, python_path: Option<&str>, mcp_dir: std::path::PathBuf) -> Result<()> {
        #[cfg(all(target_os = "macos", debug_assertions))]
        {
            log::warn!("macOS dev mode: starting libre.py only");
            log::info!("Ensure LibreOffice headless (2002) and helper.py (8765) are running");

            let libre_script = mcp_dir.join("libre.py");
            let venv_python = mcp_dir.join(".venv/bin/python");
            let default_cmd = if venv_python.exists() {
                venv_python.to_string_lossy().to_string()
            } else {
                "python3".to_string()
            };
            let python_cmd = python_path
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .unwrap_or(default_cmd);

            let mut process = Command::new(&python_cmd);
            process
                .arg(&libre_script)
                .current_dir(&mcp_dir)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            Self::prepare_python_env(&mut process);
            let mut child = process.spawn()?;

            self.store_child_io(&mut child)?;
            let mut process_guard = self.process.lock().expect("process mutex poisoned");
            *process_guard = Some(child);
            std::thread::sleep(Duration::from_millis(500));
            return Ok(());
        }

        #[cfg(not(all(target_os = "macos", debug_assertions)))]
        {
            let run_script = mcp_dir.join("run.sh");
            let main_script = mcp_dir.join("main.py");

            let explicit_python = python_path
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);

            let (command, args): (String, Vec<String>) = if run_script.exists() && cfg!(unix) {
                log::info!("Starting MCP server via wrapper: {}", run_script.display());
                (run_script.to_string_lossy().to_string(), vec![])
            } else if main_script.exists() {
                let venv_python = if cfg!(target_os = "windows") {
                    mcp_dir.join(".venv/Scripts/python.exe")
                } else {
                    mcp_dir.join(".venv/bin/python")
                };
                let discovered_cmd = if venv_python.exists() {
                    venv_python.to_string_lossy().to_string()
                } else if cfg!(target_os = "windows") {
                    "python".to_string()
                } else {
                    "python3".to_string()
                };
                let python_cmd = explicit_python.unwrap_or(discovered_cmd);
                log::info!(
                    "Starting MCP server: {} {}",
                    python_cmd,
                    main_script.display()
                );
                (python_cmd, vec![main_script.to_string_lossy().to_string()])
            } else {
                return Err(anyhow!("MCP server scripts not found at {:?}", mcp_dir));
            };

            let mut process = Command::new(&command);
            process
                .args(&args)
                .current_dir(&mcp_dir)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            Self::prepare_python_env(&mut process);
            let mut child = process.spawn()?;

            self.store_child_io(&mut child)?;

            let mut process_guard = self.process.lock().expect("process mutex poisoned");
            *process_guard = Some(child);
            std::thread::sleep(Duration::from_millis(3000));
            Ok(())
        }
    }

    pub fn stop(&self) -> Result<()> {
        {
            let mut stdin_guard = self.stdin.lock().expect("stdin mutex poisoned");
            *stdin_guard = None;
        }
        {
            let mut reader_guard = self
                .stdout_reader
                .lock()
                .expect("stdout reader mutex poisoned");
            *reader_guard = None;
        }

        let mut process_guard = self.process.lock().expect("process mutex poisoned");
        if let Some(mut child) = process_guard.take() {
            log::info!("Stopping MCP server");
            let _ = child.kill();
            child.wait()?;
        }
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        let mut process_guard = self.process.lock().expect("process mutex poisoned");
        if let Some(ref mut child) = *process_guard {
            match child.try_wait() {
                Ok(Some(_)) => {
                    log::warn!("MCP server process exited unexpectedly");
                    false
                }
                Ok(None) => true,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    pub fn initialize(&self) -> Result<()> {
        let params = McpInitParams::default();
        let max_attempts = 10;
        let retry_delay = Duration::from_millis(2000);
        let start_time = Instant::now();
        let max_wait = Duration::from_secs(30);

        for attempt in 1..=max_attempts {
            if start_time.elapsed() > max_wait {
                return Err(anyhow!("MCP initialization timed out after {:?}", max_wait));
            }

            let request = McpRequest::new(
                self.next_id(),
                "initialize".to_string(),
                Some(serde_json::to_value(&params)?),
            );

            match self.send_request(request) {
                Ok(response) => {
                    if let Some(error) = response.error {
                        return Err(anyhow!("MCP initialization failed: {}", error.message));
                    }
                    log::info!(
                        "MCP initialized successfully on attempt {} ({:?} elapsed)",
                        attempt,
                        start_time.elapsed()
                    );
                    return Ok(());
                }
                Err(error) => {
                    log::warn!(
                        "MCP initialize attempt {}/{} failed: {}. Retrying in {:?}",
                        attempt,
                        max_attempts,
                        error,
                        retry_delay
                    );
                    if attempt < max_attempts {
                        std::thread::sleep(retry_delay);
                    }
                }
            }
        }

        Err(anyhow!(
            "MCP initialization failed after {} attempts",
            max_attempts
        ))
    }

    pub fn list_tools(&self) -> Result<Vec<McpTool>> {
        let request = McpRequest::new(self.next_id(), "tools/list".to_string(), None);
        let response = self.send_request(request)?;

        if let Some(error) = response.error {
            return Err(anyhow!("Failed to list tools: {}", error.message));
        }

        if let Some(result) = response.result {
            let tools: Vec<McpTool> = serde_json::from_value(
                result.get("tools").cloned().unwrap_or(Value::Array(vec![])),
            )?;

            let mut tools_map = self.tools.lock().expect("tools mutex poisoned");
            tools_map.clear();
            for tool in &tools {
                tools_map.insert(tool.name.clone(), tool.clone());
            }
            log::info!("Discovered {} MCP tools", tools.len());
            Ok(tools)
        } else {
            Ok(vec![])
        }
    }

    pub fn call_tool(&self, name: String, arguments: Value) -> Result<ToolResult> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments
        });

        let request = McpRequest::new(self.next_id(), "tools/call".to_string(), Some(params));
        let response = self.send_request(request)?;

        if let Some(error) = response.error {
            return Err(anyhow!("Tool invocation failed: {}", error.message));
        }

        if let Some(result) = response.result {
            let tool_result: ToolResult = serde_json::from_value(result)?;
            Ok(tool_result)
        } else {
            Err(anyhow!("No result from tool invocation"))
        }
    }

    pub fn get_tools(&self) -> Vec<McpTool> {
        let tools_map = self.tools.lock().expect("tools mutex poisoned");
        tools_map.values().cloned().collect()
    }

    fn send_request(&self, request: McpRequest) -> Result<McpResponse> {
        {
            let mut stdin_guard = self.stdin.lock().expect("stdin mutex poisoned");
            let stdin = stdin_guard
                .as_mut()
                .ok_or_else(|| anyhow!("MCP server stdin not available"))?;
            let request_json = serde_json::to_string(&request)?;
            log::debug!("Sending MCP request: {}", request_json);
            writeln!(stdin, "{}", request_json)?;
            stdin.flush()?;
        }

        let reader_arc = Arc::clone(&self.stdout_reader);
        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let mut reader_guard = reader_arc.lock().expect("stdout reader mutex poisoned");
            if let Some(reader) = reader_guard.as_mut() {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(_) => {
                        let _ = tx.send(Ok(line));
                    }
                    Err(error) => {
                        let _ = tx.send(Err(error));
                    }
                }
            } else {
                let _ = tx.send(Err(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "MCP stdout reader not available",
                )));
            }
        });

        let timeout = Duration::from_secs(30);
        let response_line = match rx.recv_timeout(timeout) {
            Ok(Ok(line)) if line.trim().is_empty() => {
                return Err(anyhow!("MCP server closed stdout (empty response)"));
            }
            Ok(Ok(line)) => line,
            Ok(Err(error)) => return Err(anyhow!("Failed to read MCP response: {}", error)),
            Err(_) => return Err(anyhow!("MCP server response timed out after {:?}", timeout)),
        };

        log::debug!("Received MCP response: {}", response_line.trim());
        let response: McpResponse = serde_json::from_str(response_line.trim())?;
        Ok(response)
    }

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
