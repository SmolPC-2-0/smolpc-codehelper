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
    /// When using the bundled standalone Python (not a venv Python), this points
    /// to the pre-installed `.venv/Lib/site-packages/` directory so that
    /// third-party packages are available via PYTHONPATH.
    pub site_packages_dir: Option<PathBuf>,
    pub log_dir: PathBuf,
}

impl LibreOfficeRuntimeConfig {
    pub fn from_layout(
        layout: &LibreOfficeResourceLayout,
        app_local_data_dir: Option<&Path>,
        allow_system_python_fallback: bool,
    ) -> Result<Self, String> {
        let uses_prepared_python = resolve_prepared_python_command(app_local_data_dir).is_some();
        let python_command = resolve_python_command(
            &layout.mcp_server_dir,
            app_local_data_dir,
            allow_system_python_fallback,
        )?;
        let site_packages_dir = if uses_prepared_python {
            resolve_site_packages_dir(&layout.mcp_server_dir)
        } else {
            None
        };
        Ok(Self {
            entrypoint: layout.mcp_server_py_path.clone(),
            working_dir: layout.mcp_server_dir.clone(),
            python_command,
            site_packages_dir,
            log_dir: resolve_log_dir(app_local_data_dir),
        })
    }

    pub fn stdio_transport_config(&self) -> Result<StdioTransportConfig, String> {
        let mut env = BTreeMap::new();
        env.insert(
            "SMOLPC_MCP_LOG_DIR".to_string(),
            ensure_log_dir(&self.log_dir)?.to_string_lossy().to_string(),
        );

        // When using the bundled standalone Python (not a venv), inject PYTHONPATH
        // so that pre-installed packages from the bundled .venv are importable.
        if let Some(ref site_packages) = self.site_packages_dir {
            env.insert(
                "PYTHONPATH".to_string(),
                build_python_path(site_packages),
            );
            // Prevent importing from user site-packages on the target machine.
            env.insert("PYTHONNOUSERSITE".to_string(), "1".to_string());
            // pywin32 needs its DLL directory (pywintypes313.dll etc.) discoverable.
            let pywin32_dll_dir = site_packages.join("pywin32_system32");
            if pywin32_dll_dir.is_dir() {
                let existing_path = std::env::var("PATH").unwrap_or_default();
                env.insert(
                    "PATH".to_string(),
                    format!("{};{}", pywin32_dll_dir.to_string_lossy(), existing_path),
                );
            }
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
    // 1. Try the bundled full CPython prepared by setup_prepare().
    if let Some(prepared) = resolve_prepared_python_command(app_local_data_dir) {
        return Ok(prepared);
    }

    // 2. Dev fallback: try .venv from the working dir or source tree.
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

/// When using the bundled standalone Python, resolve the site-packages directory
/// from the pre-installed .venv shipped alongside the MCP server.
fn resolve_site_packages_dir(mcp_server_dir: &Path) -> Option<PathBuf> {
    let site_packages = mcp_server_dir
        .join(".venv")
        .join("Lib")
        .join("site-packages");
    if site_packages.is_dir() {
        Some(site_packages)
    } else {
        None
    }
}

/// Build a PYTHONPATH value that includes site-packages and the additional
/// directories required by pywin32 (win32, win32/lib).
fn build_python_path(site_packages: &Path) -> String {
    let sep = ";";
    let mut parts = vec![site_packages.to_string_lossy().to_string()];

    let win32_dir = site_packages.join("win32");
    if win32_dir.is_dir() {
        parts.push(win32_dir.to_string_lossy().to_string());
        let win32_lib = win32_dir.join("lib");
        if win32_lib.is_dir() {
            parts.push(win32_lib.to_string_lossy().to_string());
        }
    }

    parts.join(sep)
}

fn development_python_candidates(working_dir: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let venv_dir = working_dir.join(".venv");

    let source_venv_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("mcp_server")
        .join(".venv");

    #[cfg(windows)]
    {
        candidates.push(venv_dir.join("Scripts").join("python.exe"));
        candidates.push(source_venv_dir.join("Scripts").join("python.exe"));
    }

    #[cfg(not(windows))]
    {
        candidates.push(venv_dir.join("bin").join("python3"));
        candidates.push(venv_dir.join("bin").join("python"));
        candidates.push(source_venv_dir.join("bin").join("python3"));
        candidates.push(source_venv_dir.join("bin").join("python"));
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
    use crate::resources::LibreOfficeResourceLayout;
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
            site_packages_dir: None,
            log_dir,
        };

        let transport = config.stdio_transport_config().expect("transport config");
        assert!(transport.env.contains_key("SMOLPC_MCP_LOG_DIR"));
        assert!(!transport.env.contains_key("PYTHONPATH"));
        assert_eq!(transport.args, vec!["mcp_server.py"]);
    }

    #[test]
    fn stdio_transport_config_injects_pythonpath_when_site_packages_set() {
        let tempdir = tempdir().expect("tempdir");
        let log_dir = tempdir.path().join("logs");
        let site_packages = tempdir.path().join("site-packages");
        std::fs::create_dir_all(&site_packages).expect("create site-packages");

        let config = LibreOfficeRuntimeConfig {
            entrypoint: PathBuf::from("mcp_server.py"),
            working_dir: PathBuf::from("."),
            python_command: "/bundled/python.exe".to_string(),
            site_packages_dir: Some(site_packages.clone()),
            log_dir,
        };

        let transport = config.stdio_transport_config().expect("transport config");
        let pythonpath = transport.env.get("PYTHONPATH").expect("PYTHONPATH");
        assert!(pythonpath.contains(&site_packages.to_string_lossy().to_string()));
        assert_eq!(
            transport.env.get("PYTHONNOUSERSITE").expect("PYTHONNOUSERSITE"),
            "1"
        );
    }

    #[test]
    fn summary_describes_document_server() {
        let config = LibreOfficeRuntimeConfig {
            entrypoint: PathBuf::from("mcp_server.py"),
            working_dir: PathBuf::from("."),
            python_command: "python3".to_string(),
            site_packages_dir: None,
            log_dir: PathBuf::from("/tmp/logs"),
        };

        let summary = config.summary();
        assert!(summary.contains("python-docx"));
        assert!(summary.contains("mcp_server.py"));
    }

    #[test]
    fn source_tree_venv_candidate_is_included() {
        let candidates = super::development_python_candidates(&PathBuf::from("/some/target/dir"));
        let source_candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("mcp_server")
            .join(".venv");

        #[cfg(windows)]
        let expected = source_candidate.join("Scripts").join("python.exe");
        #[cfg(not(windows))]
        let expected = source_candidate.join("bin").join("python3");

        assert!(
            candidates.contains(&expected),
            "Should include source-tree .venv candidate: {}",
            expected.display()
        );
    }
}
