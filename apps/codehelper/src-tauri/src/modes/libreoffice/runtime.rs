use super::resources::LibreOfficeResourceLayout;
use crate::setup::python::resolve_prepared_python_command;
use smolpc_mcp_client::{McpSession, StdioTransportConfig};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const LIBREOFFICE_HELPER_SOCKET_ADDR: &str = "localhost:8765";
pub const LIBREOFFICE_OFFICE_SOCKET_ADDR: &str = "localhost:2002";
const LIBREOFFICE_CLIENT_NAME: &str = "smolpc-unified-libreoffice";
const LIBREOFFICE_CONNECT_HINT: &str =
    "Make sure bundled Python has been prepared and LibreOffice or Collabora is installed.";
const LIBREOFFICE_LOG_SUBDIR: &str = "libreoffice/logs";
const LIBREOFFICE_OFFICE_PATH_ENV: &str = "SMOLPC_LIBREOFFICE_OFFICE_PATH";

#[cfg(windows)]
const DEFAULT_PYTHON_COMMAND: &str = "python";
#[cfg(not(windows))]
const DEFAULT_PYTHON_COMMAND: &str = "python3";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibreOfficeRuntimeConfig {
    pub entrypoint: PathBuf,
    pub working_dir: PathBuf,
    pub helper_socket_addr: String,
    pub office_socket_addr: String,
    pub python_command: String,
    pub office_path: Option<PathBuf>,
    pub log_dir: PathBuf,
}

impl LibreOfficeRuntimeConfig {
    pub fn from_layout(
        layout: &LibreOfficeResourceLayout,
        app_local_data_dir: Option<&Path>,
        allow_system_python_fallback: bool,
        office_path: Option<PathBuf>,
    ) -> Result<Self, String> {
        Ok(Self {
            entrypoint: layout.main_py_path.clone(),
            working_dir: layout.mcp_server_dir.clone(),
            helper_socket_addr: LIBREOFFICE_HELPER_SOCKET_ADDR.to_string(),
            office_socket_addr: LIBREOFFICE_OFFICE_SOCKET_ADDR.to_string(),
            python_command: resolve_python_command(
                &layout.mcp_server_dir,
                app_local_data_dir,
                allow_system_python_fallback,
            )?,
            office_path,
            log_dir: resolve_log_dir(app_local_data_dir),
        })
    }

    pub fn stdio_transport_config(&self) -> Result<StdioTransportConfig, String> {
        let mut env = BTreeMap::new();
        env.insert(
            "SMOLPC_MCP_LOG_DIR".to_string(),
            ensure_log_dir(&self.log_dir)?.to_string_lossy().to_string(),
        );
        if let Some(office_path) = &self.office_path {
            env.insert(
                LIBREOFFICE_OFFICE_PATH_ENV.to_string(),
                office_path.to_string_lossy().to_string(),
            );
        }

        Ok(StdioTransportConfig {
            command: self.python_command.clone(),
            args: vec![self.entrypoint.to_string_lossy().to_string()],
            cwd: Some(self.working_dir.clone()),
            env,
        })
    }

    pub async fn connect_session(&self) -> Result<McpSession, String> {
        let transport = self.stdio_transport_config()?;
        McpSession::connect_stdio(
            transport,
            LIBREOFFICE_CLIENT_NAME,
            env!("CARGO_PKG_VERSION"),
        )
        .await
        .map_err(|error| {
            format!(
                "Unable to start the LibreOffice MCP runtime. {LIBREOFFICE_CONNECT_HINT} {error}"
            )
        })
    }

    pub fn summary(&self) -> String {
        format!(
            "shared LibreOffice MCP runtime over stdio via {} main.py with helper socket bridge on {}, office socket {}, and host app {}",
            self.python_command,
            self.helper_socket_addr,
            self.office_socket_addr,
            self.office_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "auto-detected LibreOffice".to_string())
        )
    }
}

fn resolve_python_command(
    working_dir: &Path,
    app_local_data_dir: Option<&Path>,
    allow_system_python_fallback: bool,
) -> Result<String, String> {
    if let Some(command) = resolve_prepared_python_command(app_local_data_dir) {
        return Ok(command);
    }

    if allow_system_python_fallback {
        for candidate in development_python_candidates(working_dir) {
            if candidate.is_file() {
                return Ok(candidate.to_string_lossy().to_string());
            }
        }

        return Ok(DEFAULT_PYTHON_COMMAND.to_string());
    }

    Err(
        "Bundled Python is not prepared yet. Use setup_prepare() from the setup panel before starting Writer or Slides.".to_string(),
    )
}

fn development_python_candidates(working_dir: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let venv_dir = working_dir.join(".venv");

    #[cfg(windows)]
    {
        candidates.push(venv_dir.join("Scripts").join("python.exe"));
    }

    #[cfg(not(windows))]
    {
        candidates.push(venv_dir.join("bin").join("python3"));
        candidates.push(venv_dir.join("bin").join("python"));
    }

    candidates
}

fn resolve_log_dir(app_local_data_dir: Option<&Path>) -> PathBuf {
    app_local_data_dir
        .map(|base| base.join(LIBREOFFICE_LOG_SUBDIR))
        .unwrap_or_else(|| {
            std::env::temp_dir()
                .join("smolpc-unified-assistant")
                .join(LIBREOFFICE_LOG_SUBDIR)
        })
}

fn ensure_log_dir(path: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(path)
        .map_err(|error| format!("Unable to create LibreOffice log directory: {error}"))?;
    std::fs::canonicalize(path)
        .map_err(|error| format!("Unable to canonicalize LibreOffice log directory: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        LibreOfficeRuntimeConfig, LIBREOFFICE_HELPER_SOCKET_ADDR, LIBREOFFICE_OFFICE_SOCKET_ADDR,
    };
    use crate::modes::libreoffice::resources::LibreOfficeResourceLayout;
    use serde_json::{json, Value};
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::sync::{Mutex as StdMutex, OnceLock};
    use std::thread;
    use std::time::{Duration, Instant};
    use tempfile::tempdir;

    const HELPER_PORT: u16 = 8765;
    const MAX_HELPER_FRAME_SIZE: usize = 10 * 1024 * 1024;
    const HELPER_AUTH_TOKEN_ENV: &str = "SMOLPC_LIBREOFFICE_HELPER_AUTH_TOKEN";
    static LIBREOFFICE_RUNTIME_TEST_LOCK: OnceLock<StdMutex<()>> = OnceLock::new();

    struct ChildGuard(Child);

    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn python_command() -> &'static str {
        #[cfg(windows)]
        {
            "python"
        }
        #[cfg(not(windows))]
        {
            "python3"
        }
    }

    fn helper_script_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("libreoffice")
            .join("mcp_server")
            .join("helper.py")
    }

    fn libre_script_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("libreoffice")
            .join("mcp_server")
            .join("libre.py")
    }

    fn main_script_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("libreoffice")
            .join("mcp_server")
            .join("main.py")
    }

    fn port_test_lock() -> &'static StdMutex<()> {
        LIBREOFFICE_RUNTIME_TEST_LOCK.get_or_init(|| StdMutex::new(()))
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directories");
        }
        fs::write(path, contents).expect("write file");
    }

    fn read_exact(stream: &mut TcpStream, size: usize) -> Vec<u8> {
        let mut buf = vec![0u8; size];
        stream.read_exact(&mut buf).expect("read exact");
        buf
    }

    fn read_json_frame(stream: &mut TcpStream) -> Value {
        let length_bytes = read_exact(stream, 4);
        let length = u32::from_be_bytes(length_bytes.try_into().expect("header")) as usize;
        let payload = read_exact(stream, length);
        serde_json::from_slice(&payload).expect("json frame")
    }

    fn send_json_frame(stream: &mut TcpStream, payload: &Value) {
        let bytes = serde_json::to_vec(payload).expect("serialize payload");
        let length = u32::try_from(bytes.len()).expect("payload length");
        stream
            .write_all(&length.to_be_bytes())
            .expect("write length header");
        stream.write_all(&bytes).expect("write payload");
    }

    fn send_raw_frame(stream: &mut TcpStream, payload: &[u8]) {
        let length = u32::try_from(payload.len()).expect("payload length");
        stream
            .write_all(&length.to_be_bytes())
            .expect("write length header");
        stream.write_all(payload).expect("write payload");
    }

    fn wait_for_port(port: u16) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        panic!("port {port} did not become ready in time");
    }

    fn fake_uno_tree(root: &Path) {
        for package in [
            root.join("com"),
            root.join("com").join("sun"),
            root.join("com").join("sun").join("star"),
            root.join("com").join("sun").join("star").join("beans"),
            root.join("com").join("sun").join("star").join("connection"),
            root.join("com").join("sun").join("star").join("text"),
            root.join("com").join("sun").join("star").join("awt"),
            root.join("com").join("sun").join("star").join("lang"),
            root.join("com").join("sun").join("star").join("style"),
            root.join("com").join("sun").join("star").join("table"),
        ] {
            write_file(&package.join("__init__.py"), "");
        }

        write_file(
            &root.join("uno.py"),
            r#"
class Bool(int):
    pass

def systemPathToFileUrl(path):
    return path

class _ServiceManager:
    def createInstanceWithContext(self, *args, **kwargs):
        raise RuntimeError("UNO services are unavailable in tests")

class _Context:
    ServiceManager = _ServiceManager()

def getComponentContext():
    return _Context()
"#,
        );
        write_file(
            &root
                .join("com")
                .join("sun")
                .join("star")
                .join("beans")
                .join("__init__.py"),
            "class PropertyValue:\n    pass\n",
        );
        write_file(
            &root
                .join("com")
                .join("sun")
                .join("star")
                .join("connection")
                .join("__init__.py"),
            "class NoConnectException(Exception):\n    pass\n",
        );
        write_file(
            &root
                .join("com")
                .join("sun")
                .join("star")
                .join("text")
                .join("__init__.py"),
            "class ControlCharacter:\n    pass\n",
        );
        write_file(
            &root
                .join("com")
                .join("sun")
                .join("star")
                .join("text")
                .join("TextContentAnchorType.py"),
            "AS_CHARACTER = 0\n",
        );
        write_file(
            &root
                .join("com")
                .join("sun")
                .join("star")
                .join("text")
                .join("ControlCharacter.py"),
            "PARAGRAPH_BREAK = 0\n",
        );
        write_file(
            &root
                .join("com")
                .join("sun")
                .join("star")
                .join("awt")
                .join("__init__.py"),
            "class Size:\n    pass\n",
        );
        write_file(
            &root
                .join("com")
                .join("sun")
                .join("star")
                .join("awt")
                .join("FontSlant.py"),
            "ITALIC = 2\n",
        );
        write_file(
            &root
                .join("com")
                .join("sun")
                .join("star")
                .join("lang")
                .join("__init__.py"),
            "class Locale:\n    pass\n",
        );
        write_file(
            &root
                .join("com")
                .join("sun")
                .join("star")
                .join("style")
                .join("ParagraphAdjust.py"),
            "CENTER = 0\nLEFT = 1\nRIGHT = 2\nBLOCK = 3\n",
        );
        write_file(
            &root
                .join("com")
                .join("sun")
                .join("star")
                .join("style")
                .join("BreakType.py"),
            "PAGE_BEFORE = 0\n",
        );
        write_file(
            &root
                .join("com")
                .join("sun")
                .join("star")
                .join("table")
                .join("__init__.py"),
            "class BorderLine2:\n    pass\n\nclass TableBorder2:\n    pass\n",
        );
        write_file(
            &root
                .join("com")
                .join("sun")
                .join("star")
                .join("table")
                .join("BorderLineStyle.py"),
            "SOLID = 0\n",
        );
    }

    fn fake_mcp_tree(root: &Path) {
        for package in [root.join("mcp"), root.join("mcp").join("server")] {
            write_file(&package.join("__init__.py"), "");
        }

        write_file(
            &root.join("mcp").join("server").join("fastmcp.py"),
            r#"
class FastMCP:
    def __init__(self, *_args, **_kwargs):
        pass

    def tool(self):
        def decorator(func):
            return func
        return decorator

    def resource(self, *_args, **_kwargs):
        def decorator(func):
            return func
        return decorator
"#,
        );
    }

    fn start_helper_process(fake_modules_dir: &Path, log_dir: &Path, token: &str) -> ChildGuard {
        let child = Command::new(python_command())
            .arg(helper_script_path())
            .env("PYTHONPATH", fake_modules_dir)
            .env(HELPER_AUTH_TOKEN_ENV, token)
            .env("SMOLPC_MCP_LOG_DIR", log_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn helper process");
        let guard = ChildGuard(child);
        wait_for_port(HELPER_PORT);
        guard
    }

    fn run_libre_client_with_server(
        response_handler: impl FnOnce(TcpStream) + Send + 'static,
        set_token_env: bool,
    ) -> (Value, Option<Value>) {
        let _guard = port_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let tempdir = tempdir().expect("tempdir");
        let fake_mcp_root = tempdir.path().join("fake_mcp");
        fake_mcp_tree(&fake_mcp_root);
        let log_dir = tempdir.path().join("logs");
        fs::create_dir_all(&log_dir).expect("create log dir");
        let runner_path = tempdir.path().join("runner.py");
        write_file(
            &runner_path,
            &format!(
                r#"
import asyncio
import importlib.util
import json
import sys

spec = importlib.util.spec_from_file_location("smolpc_libre_runtime", r"{path}")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

async def main():
    response = await module.call_libreoffice_helper({{"action": "ping"}})
    sys.stdout.write(json.dumps(response))
    sys.stdout.flush()

asyncio.run(main())
"#,
                path = libre_script_path().display()
            ),
        );

        let captured_request = std::sync::Arc::new(StdMutex::new(None::<Value>));
        let server = if set_token_env {
            let request_slot = std::sync::Arc::clone(&captured_request);
            let listener = TcpListener::bind(("127.0.0.1", HELPER_PORT)).expect("bind helper port");
            Some(thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("accept helper connection");
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .expect("set read timeout");
                let request = read_json_frame(&mut stream);
                *request_slot.lock().expect("request slot") = Some(request);
                response_handler(stream);
            }))
        } else {
            None
        };

        let mut command = Command::new(python_command());
        command
            .arg(&runner_path)
            .env("PYTHONPATH", &fake_mcp_root)
            .env("SMOLPC_MCP_LOG_DIR", &log_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if set_token_env {
            command.env(HELPER_AUTH_TOKEN_ENV, "test-token");
        }

        let output = command.output().expect("run libre client");
        if let Some(server) = server {
            server.join().expect("join helper server");
        }
        assert!(
            output.status.success(),
            "libre client failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let response: Value =
            serde_json::from_slice(&output.stdout).expect("parse libre client response");
        let request = captured_request.lock().expect("captured request").clone();
        (response, request)
    }

    #[test]
    fn runtime_config_produces_stdio_setup() {
        let layout = LibreOfficeResourceLayout {
            mcp_server_dir: PathBuf::from("/tmp/libreoffice/mcp_server"),
            readme_path: PathBuf::from("/tmp/libreoffice/mcp_server/README.md"),
            main_py_path: PathBuf::from("/tmp/libreoffice/mcp_server/main.py"),
            libre_py_path: PathBuf::from("/tmp/libreoffice/mcp_server/libre.py"),
            helper_py_path: PathBuf::from("/tmp/libreoffice/mcp_server/helper.py"),
            helper_utils_py_path: PathBuf::from("/tmp/libreoffice/mcp_server/helper_utils.py"),
            helper_test_functions_py_path: PathBuf::from(
                "/tmp/libreoffice/mcp_server/helper_test_functions.py",
            ),
        };

        let runtime = LibreOfficeRuntimeConfig::from_layout(
            &layout,
            Some(Path::new("/tmp/smolpc")),
            true,
            Some(PathBuf::from("/tmp/libreoffice/program/soffice")),
        )
        .expect("runtime");
        let config = runtime
            .stdio_transport_config()
            .expect("stdio transport config");

        #[cfg(windows)]
        assert_eq!(config.command, "python");
        #[cfg(not(windows))]
        assert_eq!(config.command, "python3");
        assert_eq!(
            config.args,
            vec!["/tmp/libreoffice/mcp_server/main.py".to_string()]
        );
        assert_eq!(
            config.env.get("SMOLPC_LIBREOFFICE_OFFICE_PATH"),
            Some(&"/tmp/libreoffice/program/soffice".to_string())
        );
        assert_eq!(runtime.helper_socket_addr, LIBREOFFICE_HELPER_SOCKET_ADDR);
        assert_eq!(runtime.office_socket_addr, LIBREOFFICE_OFFICE_SOCKET_ADDR);
        let expected_log_dir =
            std::fs::canonicalize("/tmp/smolpc/libreoffice/logs").expect("canonical log dir");
        assert_eq!(
            config.env.get("SMOLPC_MCP_LOG_DIR"),
            Some(&expected_log_dir.to_string_lossy().to_string())
        );
    }

    #[test]
    fn runtime_config_requires_prepared_python_when_system_fallback_disabled() {
        let layout = LibreOfficeResourceLayout {
            mcp_server_dir: PathBuf::from("/tmp/libreoffice/mcp_server"),
            readme_path: PathBuf::from("/tmp/libreoffice/mcp_server/README.md"),
            main_py_path: PathBuf::from("/tmp/libreoffice/mcp_server/main.py"),
            libre_py_path: PathBuf::from("/tmp/libreoffice/mcp_server/libre.py"),
            helper_py_path: PathBuf::from("/tmp/libreoffice/mcp_server/helper.py"),
            helper_utils_py_path: PathBuf::from("/tmp/libreoffice/mcp_server/helper_utils.py"),
            helper_test_functions_py_path: PathBuf::from(
                "/tmp/libreoffice/mcp_server/helper_test_functions.py",
            ),
        };

        let error = LibreOfficeRuntimeConfig::from_layout(
            &layout,
            Some(Path::new("/tmp/phase3-missing-python")),
            false,
            Some(PathBuf::from("/tmp/libreoffice/program/soffice")),
        )
        .expect_err("strict packaged mode should require prepared python");

        assert!(error.contains("Bundled Python is not prepared yet"));
    }

    #[test]
    fn helper_protocol_requires_auth_and_rejects_bad_frames() {
        let _guard = port_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let tempdir = tempdir().expect("tempdir");
        let fake_modules = tempdir.path().join("fake_uno");
        fake_uno_tree(&fake_modules);
        let log_dir = tempdir.path().join("logs");
        fs::create_dir_all(&log_dir).expect("create log dir");
        let _helper = start_helper_process(&fake_modules, &log_dir, "secret-token");

        let mut stream = TcpStream::connect(("127.0.0.1", HELPER_PORT)).expect("connect helper");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set read timeout");
        send_json_frame(
            &mut stream,
            &json!({"action": "no_such_action", "_smolpc_auth_token": "secret-token"}),
        );
        let response = read_json_frame(&mut stream);
        assert_eq!(response["status"], "error");
        assert!(response["error"]
            .as_str()
            .expect("error string")
            .contains("Unknown action"));
        drop(stream);

        let mut stream = TcpStream::connect(("127.0.0.1", HELPER_PORT)).expect("connect helper");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set read timeout");
        send_json_frame(&mut stream, &json!({"action": "no_such_action"}));
        let response = read_json_frame(&mut stream);
        assert_eq!(response["status"], "error");
        assert!(response["error"]
            .as_str()
            .expect("error string")
            .contains("Unauthorized helper request"));
        drop(stream);

        let mut stream = TcpStream::connect(("127.0.0.1", HELPER_PORT)).expect("connect helper");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set read timeout");
        send_json_frame(
            &mut stream,
            &json!({"action": "no_such_action", "_smolpc_auth_token": "wrong-token"}),
        );
        let response = read_json_frame(&mut stream);
        assert_eq!(response["status"], "error");
        assert!(response["error"]
            .as_str()
            .expect("error string")
            .contains("Unauthorized helper request"));
        drop(stream);

        let mut stream = TcpStream::connect(("127.0.0.1", HELPER_PORT)).expect("connect helper");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set read timeout");
        send_raw_frame(&mut stream, b"{not json");
        let response = read_json_frame(&mut stream);
        assert_eq!(response["status"], "error");
        assert!(response["error"]
            .as_str()
            .expect("error string")
            .contains("Invalid JSON received"));
        drop(stream);

        let mut stream = TcpStream::connect(("127.0.0.1", HELPER_PORT)).expect("connect helper");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set read timeout");
        stream
            .write_all(&((MAX_HELPER_FRAME_SIZE as u32) + 1).to_be_bytes())
            .expect("write oversized header");
        let response = read_json_frame(&mut stream);
        assert_eq!(response["status"], "error");
        assert!(response["error"]
            .as_str()
            .expect("error string")
            .contains("maximum frame size"));
    }

    #[test]
    fn libre_runtime_validates_helper_responses_and_sends_auth_token() {
        let (response, request) = run_libre_client_with_server(
            |mut stream| {
                let payload = json!({"result": "missing status"});
                send_json_frame(&mut stream, &payload);
            },
            true,
        );
        assert_eq!(response["status"], "error");
        assert!(response["message"]
            .as_str()
            .expect("message string")
            .contains("missing valid status field"));
        assert_eq!(
            request.expect("captured request")["_smolpc_auth_token"],
            json!("test-token")
        );

        let (response, _) = run_libre_client_with_server(
            |mut stream| {
                let payload = b"not-json";
                let length = u32::try_from(payload.len()).expect("payload length");
                stream
                    .write_all(&length.to_be_bytes())
                    .expect("write malformed header");
                stream.write_all(payload).expect("write malformed body");
            },
            true,
        );
        assert_eq!(response["status"], "error");
        assert!(response["message"]
            .as_str()
            .expect("message string")
            .contains("Invalid helper response JSON"));

        let (response, _) = run_libre_client_with_server(
            |mut stream| {
                stream
                    .write_all(&((MAX_HELPER_FRAME_SIZE as u32) + 1).to_be_bytes())
                    .expect("write oversized header");
            },
            true,
        );
        assert_eq!(response["status"], "error");
        assert!(response["message"]
            .as_str()
            .expect("message string")
            .contains("maximum frame size"));
    }

    #[test]
    fn libre_runtime_requires_helper_auth_token_in_environment() {
        let (response, request) = run_libre_client_with_server(
            |mut stream| {
                let payload = json!({"status": "success", "result": "ok"});
                send_json_frame(&mut stream, &payload);
            },
            false,
        );

        assert_eq!(response["status"], "error");
        assert!(response["message"]
            .as_str()
            .expect("message string")
            .contains("Missing helper auth token"));
        assert!(
            request.is_none(),
            "client should fail before opening a socket"
        );
    }

    #[test]
    fn main_runtime_cleanup_terminates_child_processes() {
        let tempdir = tempdir().expect("tempdir");
        let runner_path = tempdir.path().join("cleanup_check.py");
        write_file(
            &runner_path,
            &format!(
                r#"
import importlib.util
import subprocess
import sys

spec = importlib.util.spec_from_file_location("smolpc_main_runtime", r"{path}")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
module.helper_process = child
module.cleanup_processes()
print("terminated" if child.poll() is not None else "alive")
"#,
                path = main_script_path().display()
            ),
        );

        let output = Command::new(python_command())
            .arg(&runner_path)
            .output()
            .expect("run cleanup checker");
        assert!(
            output.status.success(),
            "cleanup checker failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "terminated");
    }
}
