use super::resources::LibreOfficeResourceLayout;
use smolpc_mcp_client::{McpSession, StdioTransportConfig};
use std::path::PathBuf;

pub const LIBREOFFICE_HELPER_SOCKET_ADDR: &str = "localhost:8765";
pub const LIBREOFFICE_OFFICE_SOCKET_ADDR: &str = "localhost:2002";
const LIBREOFFICE_CLIENT_NAME: &str = "smolpc-unified-libreoffice";
const LIBREOFFICE_CONNECT_HINT: &str =
    "Make sure Python 3 is available and LibreOffice or Collabora is installed.";

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
}

impl LibreOfficeRuntimeConfig {
    pub fn from_layout(layout: &LibreOfficeResourceLayout) -> Self {
        Self {
            entrypoint: layout.main_py_path.clone(),
            working_dir: layout.mcp_server_dir.clone(),
            helper_socket_addr: LIBREOFFICE_HELPER_SOCKET_ADDR.to_string(),
            office_socket_addr: LIBREOFFICE_OFFICE_SOCKET_ADDR.to_string(),
            python_command: resolve_python_command(&layout.mcp_server_dir),
        }
    }

    pub fn stdio_transport_config(&self) -> StdioTransportConfig {
        StdioTransportConfig {
            command: self.python_command.clone(),
            args: vec![self.entrypoint.to_string_lossy().to_string()],
            cwd: Some(self.working_dir.clone()),
        }
    }

    pub async fn connect_session(&self) -> Result<McpSession, String> {
        McpSession::connect_stdio(
            self.stdio_transport_config(),
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
            "shared LibreOffice MCP runtime over stdio via {} main.py with helper socket bridge on {} and office socket {}",
            self.python_command, self.helper_socket_addr, self.office_socket_addr
        )
    }
}

fn resolve_python_command(working_dir: &std::path::Path) -> String {
    for candidate in bundled_python_candidates(working_dir) {
        if candidate.is_file() {
            return candidate.to_string_lossy().to_string();
        }
    }

    DEFAULT_PYTHON_COMMAND.to_string()
}

fn bundled_python_candidates(working_dir: &std::path::Path) -> Vec<PathBuf> {
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

#[cfg(test)]
mod tests {
    use super::{
        LibreOfficeRuntimeConfig, LIBREOFFICE_HELPER_SOCKET_ADDR, LIBREOFFICE_OFFICE_SOCKET_ADDR,
    };
    use crate::modes::libreoffice::resources::LibreOfficeResourceLayout;
    use std::path::PathBuf;

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

        let runtime = LibreOfficeRuntimeConfig::from_layout(&layout);
        let config = runtime.stdio_transport_config();

        #[cfg(windows)]
        assert_eq!(config.command, "python");
        #[cfg(not(windows))]
        assert_eq!(config.command, "python3");
        assert_eq!(
            config.args,
            vec!["/tmp/libreoffice/mcp_server/main.py".to_string()]
        );
        assert_eq!(runtime.helper_socket_addr, LIBREOFFICE_HELPER_SOCKET_ADDR);
        assert_eq!(runtime.office_socket_addr, LIBREOFFICE_OFFICE_SOCKET_ADDR);
    }
}
