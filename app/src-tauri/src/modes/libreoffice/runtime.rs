use super::resources::LibreOfficeResourceLayout;
use smolpc_connector_common::python::resolve_prepared_python_command;
use smolpc_mcp_client::{McpSession, StdioTransportConfig};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const LIBREOFFICE_CLIENT_NAME: &str = "smolpc-unified-libreoffice";
const LIBREOFFICE_CONNECT_HINT: &str = "Make sure bundled Python has been prepared first.";
const LIBREOFFICE_LOG_SUBDIR: &str = "libreoffice/logs";

#[cfg(windows)]
const DEFAULT_PYTHON_COMMAND: &str = "python";
#[cfg(not(windows))]
const DEFAULT_PYTHON_COMMAND: &str = "python3";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibreOfficeRuntimeConfig {
    pub entrypoint: PathBuf,
    pub working_dir: PathBuf,
    pub python_command: String,
    pub log_dir: PathBuf,
}

impl LibreOfficeRuntimeConfig {
    pub fn from_layout(
        layout: &LibreOfficeResourceLayout,
        app_local_data_dir: Option<&Path>,
        allow_system_python_fallback: bool,
    ) -> Result<Self, String> {
        Ok(Self {
            entrypoint: layout.mcp_server_py_path.clone(),
            working_dir: layout.mcp_server_dir.clone(),
            python_command: resolve_python_command(
                &layout.mcp_server_dir,
                app_local_data_dir,
                allow_system_python_fallback,
            )?,
            log_dir: resolve_log_dir(app_local_data_dir),
        })
    }

    pub fn stdio_transport_config(&self) -> Result<StdioTransportConfig, String> {
        let mut env = BTreeMap::new();
        env.insert(
            "SMOLPC_MCP_LOG_DIR".to_string(),
            ensure_log_dir(&self.log_dir)?.to_string_lossy().to_string(),
        );

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
                "Unable to start the LibreOffice document MCP server. {LIBREOFFICE_CONNECT_HINT} {error}"
            )
        })
    }

    pub fn summary(&self) -> String {
        format!(
            "LibreOffice document MCP server over stdio via {} mcp_server.py (python-docx/python-pptx/odfdo)",
            self.python_command,
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
    use super::LibreOfficeRuntimeConfig;
    use crate::modes::libreoffice::resources::LibreOfficeResourceLayout;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn runtime_config_from_layout_sets_entrypoint() {
        let tempdir = tempdir().expect("tempdir");
        let mcp_server_dir = tempdir.path().to_path_buf();
        let layout = LibreOfficeResourceLayout {
            mcp_server_dir: mcp_server_dir.clone(),
            readme_path: mcp_server_dir.join("README.md"),
            mcp_server_py_path: mcp_server_dir.join("mcp_server.py"),
            test_functions_py_path: mcp_server_dir.join("test_functions.py"),
        };

        let config =
            LibreOfficeRuntimeConfig::from_layout(&layout, None, true).expect("from_layout");

        assert_eq!(config.entrypoint, mcp_server_dir.join("mcp_server.py"));
        assert_eq!(config.working_dir, mcp_server_dir);
    }

    #[test]
    fn stdio_transport_config_sets_log_dir_env() {
        let tempdir = tempdir().expect("tempdir");
        let log_dir = tempdir.path().join("logs");
        let config = LibreOfficeRuntimeConfig {
            entrypoint: PathBuf::from("mcp_server.py"),
            working_dir: PathBuf::from("."),
            python_command: "python3".to_string(),
            log_dir,
        };

        let transport = config.stdio_transport_config().expect("transport config");
        assert!(transport.env.contains_key("SMOLPC_MCP_LOG_DIR"));
        assert_eq!(transport.args, vec!["mcp_server.py"]);
    }

    #[test]
    fn summary_describes_document_server() {
        let config = LibreOfficeRuntimeConfig {
            entrypoint: PathBuf::from("mcp_server.py"),
            working_dir: PathBuf::from("."),
            python_command: "python3".to_string(),
            log_dir: PathBuf::from("/tmp/logs"),
        };

        let summary = config.summary();
        assert!(summary.contains("python-docx"));
        assert!(summary.contains("mcp_server.py"));
    }
}
