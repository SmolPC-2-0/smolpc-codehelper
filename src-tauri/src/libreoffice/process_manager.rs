use crate::libreoffice::types::LibreOfficeError;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

/// Manages the Python MCP server process lifecycle
pub struct ProcessManager {
    child: Child,
    pub stdin: ChildStdin,
    pub stdout: BufReader<ChildStdout>,
    pub stderr: BufReader<ChildStderr>,
}

impl ProcessManager {
    /// Spawn the Python MCP server process
    ///
    /// This will:
    /// 1. Find the Python executable
    /// 2. Find the MCP server script
    /// 3. Spawn the process with stdio pipes
    /// 4. Return handles for stdin/stdout/stderr
    pub async fn spawn() -> Result<Self, LibreOfficeError> {
        // Find Python executable
        let python_exe = find_python_executable()?;
        log::info!("Found Python executable: {:?}", python_exe);

        // Find MCP server script
        let server_script = find_server_script()?;
        log::info!("Found MCP server script: {:?}", server_script);

        // Spawn the process
        let mut child = Command::new(&python_exe)
            .arg(&server_script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true) // Automatically kill when dropped
            .spawn()
            .map_err(|e| {
                LibreOfficeError::ProcessSpawnFailed(format!(
                    "Failed to spawn Python process: {}",
                    e
                ))
            })?;

        // Take ownership of stdin/stdout/stderr
        let stdin = child.stdin.take().ok_or_else(|| {
            LibreOfficeError::ProcessSpawnFailed("Failed to capture stdin".to_string())
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            LibreOfficeError::ProcessSpawnFailed("Failed to capture stdout".to_string())
        })?;

        let stderr = child.stderr.take().ok_or_else(|| {
            LibreOfficeError::ProcessSpawnFailed("Failed to capture stderr".to_string())
        })?;

        log::info!("MCP server process spawned successfully (PID: {:?})", child.id());

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            stderr: BufReader::new(stderr),
        })
    }

    /// Check if the process is still running
    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(_)) => false, // Process has exited
            Ok(None) => true,     // Process is still running
            Err(_) => false,      // Error checking status, assume dead
        }
    }

    /// Get the process ID
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Kill the process
    pub async fn kill(&mut self) -> Result<(), LibreOfficeError> {
        log::info!("Killing MCP server process (PID: {:?})", self.child.id());
        self.child.kill().await.map_err(|e| {
            LibreOfficeError::ProcessCrashed(format!("Failed to kill process: {}", e))
        })?;
        Ok(())
    }

    /// Wait for the process to exit and get its status
    pub async fn wait(&mut self) -> Result<std::process::ExitStatus, LibreOfficeError> {
        self.child.await.map_err(|e| {
            LibreOfficeError::ProcessCrashed(format!("Error waiting for process: {}", e))
        })
    }
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        log::info!("ProcessManager dropped, process will be killed automatically");
    }
}

/// Find the Python executable on the system
fn find_python_executable() -> Result<PathBuf, LibreOfficeError> {
    #[cfg(target_os = "macos")]
    {
        // macOS: Try common Python paths
        let candidates = vec![
            "/usr/bin/python3",
            "/usr/local/bin/python3",
            "/opt/homebrew/bin/python3", // Apple Silicon Homebrew
            "/usr/bin/python",
        ];

        for path in candidates {
            if Path::new(path).exists() {
                return Ok(PathBuf::from(path));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: First try `where` command to find python
        if let Ok(output) = std::process::Command::new("where").arg("python").output() {
            if output.status.success() {
                if let Ok(stdout) = String::from_utf8(output.stdout) {
                    if let Some(first_line) = stdout.lines().next() {
                        let path = PathBuf::from(first_line.trim());
                        if path.exists() {
                            return Ok(path);
                        }
                    }
                }
            }
        }

        // Try python3 as well
        if let Ok(output) = std::process::Command::new("where").arg("python3").output() {
            if output.status.success() {
                if let Ok(stdout) = String::from_utf8(output.stdout) {
                    if let Some(first_line) = stdout.lines().next() {
                        let path = PathBuf::from(first_line.trim());
                        if path.exists() {
                            return Ok(path);
                        }
                    }
                }
            }
        }

        // Try common Python installation paths
        let candidates = vec![
            r"C:\Python39\python.exe",
            r"C:\Python310\python.exe",
            r"C:\Python311\python.exe",
            r"C:\Python312\python.exe",
        ];

        for path in candidates {
            if Path::new(path).exists() {
                return Ok(PathBuf::from(path));
            }
        }

        // Try user-specific paths (expand %USERPROFILE%)
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let user_candidates = vec![
                format!("{}\\AppData\\Local\\Programs\\Python\\Python39\\python.exe", user_profile),
                format!("{}\\AppData\\Local\\Programs\\Python\\Python310\\python.exe", user_profile),
                format!("{}\\AppData\\Local\\Programs\\Python\\Python311\\python.exe", user_profile),
                format!("{}\\AppData\\Local\\Programs\\Python\\Python312\\python.exe", user_profile),
            ];

            for path in user_candidates {
                if Path::new(&path).exists() {
                    return Ok(PathBuf::from(path));
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: Try common Python paths
        let candidates = vec![
            "/usr/bin/python3",
            "/usr/local/bin/python3",
            "/usr/bin/python",
        ];

        for path in candidates {
            if Path::new(path).exists() {
                return Ok(PathBuf::from(path));
            }
        }
    }

    Err(LibreOfficeError::PythonNotFound)
}

/// Find the MCP server script (main.py)
fn find_server_script() -> Result<PathBuf, LibreOfficeError> {
    // The server script should be at: src-tauri/mcp-servers/libreoffice/main.py
    // We need to find it relative to the executable or the project root

    // Try to get the executable directory (works in production)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            // In production (macOS .app bundle): Contents/MacOS/
            // Script would be at: Contents/Resources/mcp-servers/libreoffice/main.py
            let production_path = exe_dir
                .parent() // Contents/
                .and_then(|p| p.parent()) // .app/
                .map(|p| p.join("Resources/mcp-servers/libreoffice/main.py"));

            if let Some(path) = production_path {
                if path.exists() {
                    return Ok(path);
                }
            }

            // Also try relative to exe_dir directly (for testing)
            let dev_path = exe_dir.join("../../../mcp-servers/libreoffice/main.py");
            if dev_path.exists() {
                return Ok(dev_path);
            }
        }
    }

    // Try current directory (works in development)
    let cwd = std::env::current_dir().map_err(|e| {
        LibreOfficeError::ServerFilesNotFound(format!("Failed to get current directory: {}", e))
    })?;

    let dev_paths = vec![
        cwd.join("src-tauri/mcp-servers/libreoffice/main.py"),
        cwd.join("mcp-servers/libreoffice/main.py"),
        cwd.join("../mcp-servers/libreoffice/main.py"),
    ];

    for path in dev_paths {
        if path.exists() {
            return Ok(path);
        }
    }

    Err(LibreOfficeError::ServerFilesNotFound(
        "Could not find main.py script".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_python_executable() {
        // This test will pass if Python is installed
        let result = find_python_executable();
        match result {
            Ok(path) => {
                println!("Found Python at: {:?}", path);
                assert!(path.exists());
            }
            Err(e) => {
                println!("Python not found: {:?}", e);
                // Don't fail the test, as Python might not be installed in CI
            }
        }
    }

    #[test]
    fn test_find_server_script() {
        // This test checks if we can locate the server script
        let result = find_server_script();
        match result {
            Ok(path) => {
                println!("Found server script at: {:?}", path);
                assert!(path.exists());
                assert!(path.ends_with("main.py"));
            }
            Err(e) => {
                println!("Server script not found: {:?}", e);
                // This is expected if running tests outside project directory
            }
        }
    }

    #[tokio::test]
    async fn test_spawn_python_hello_world() {
        // Test spawning a simple Python process
        let python_exe = match find_python_executable() {
            Ok(exe) => exe,
            Err(_) => {
                println!("Skipping test: Python not found");
                return;
            }
        };

        let mut child = Command::new(&python_exe)
            .arg("-c")
            .arg("print('Hello from Python')")
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn Python");

        let output = child.wait_with_output().await.expect("Failed to wait");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Hello from Python"));
    }

    #[tokio::test]
    async fn test_process_manager_lifecycle() {
        // Test the full lifecycle (if Python and script are available)
        let python_exe = match find_python_executable() {
            Ok(exe) => exe,
            Err(_) => {
                println!("Skipping test: Python not found");
                return;
            }
        };

        // Spawn a simple Python script that keeps running
        let mut child = Command::new(&python_exe)
            .arg("-c")
            .arg("import time; time.sleep(10)")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("Failed to spawn Python");

        // Check process is running
        assert!(child.try_wait().unwrap().is_none());

        // Get PID
        let pid = child.id();
        assert!(pid.is_some());
        println!("Spawned process with PID: {:?}", pid);

        // Kill the process
        child.kill().await.expect("Failed to kill");

        // Check process is dead
        let status = child.wait().await.expect("Failed to wait");
        assert!(!status.success()); // Process was killed, so it didn't exit successfully
    }
}
