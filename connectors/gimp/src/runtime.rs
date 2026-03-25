use crate::setup::{
    resolve_gimp_resource_layout, GIMP_PLUGIN_SOCKET_HOST, GIMP_PLUGIN_SOCKET_PORT,
};
use smolpc_connector_common::python::resolve_prepared_python_command;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use super::transport::{DEFAULT_GIMP_HOST, DEFAULT_GIMP_PORT};

const GIMP_LOG_SUBDIR: &str = "gimp/logs";

#[cfg(windows)]
const DEFAULT_PYTHON_COMMAND: &str = "python";
#[cfg(not(windows))]
const DEFAULT_PYTHON_COMMAND: &str = "python3";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GimpRuntimeConfig {
    pub bridge_entrypoint: PathBuf,
    pub working_dir: PathBuf,
    pub python_command: String,
    pub bridge_host: String,
    pub bridge_port: u16,
    pub plugin_host: String,
    pub plugin_port: u16,
    pub log_dir: PathBuf,
}

impl GimpRuntimeConfig {
    pub fn from_paths(
        resource_dir: Option<&Path>,
        app_local_data_dir: Option<&Path>,
    ) -> Result<Self, String> {
        let layout = resolve_gimp_resource_layout(resource_dir)?;
        Ok(Self {
            bridge_entrypoint: layout.bridge_entry,
            working_dir: layout.root,
            python_command: resolve_python_command(app_local_data_dir)?,
            bridge_host: DEFAULT_GIMP_HOST.to_string(),
            bridge_port: DEFAULT_GIMP_PORT,
            plugin_host: GIMP_PLUGIN_SOCKET_HOST.to_string(),
            plugin_port: GIMP_PLUGIN_SOCKET_PORT,
            log_dir: resolve_log_dir(app_local_data_dir),
        })
    }

    pub fn spawn_bridge(&self) -> Result<Child, String> {
        let log_dir = ensure_log_dir(&self.log_dir)?;
        let stdout_log = File::options()
            .create(true)
            .append(true)
            .open(log_dir.join("bridge.stdout.log"))
            .map_err(|error| format!("Unable to open GIMP bridge stdout log: {error}"))?;
        let stderr_log = File::options()
            .create(true)
            .append(true)
            .open(log_dir.join("bridge.stderr.log"))
            .map_err(|error| format!("Unable to open GIMP bridge stderr log: {error}"))?;

        Command::new(&self.python_command)
            .arg(&self.bridge_entrypoint)
            .current_dir(&self.working_dir)
            .env("SMOLPC_GIMP_BRIDGE_HOST", &self.bridge_host)
            .env("SMOLPC_GIMP_BRIDGE_PORT", self.bridge_port.to_string())
            .env("SMOLPC_GIMP_PLUGIN_HOST", &self.plugin_host)
            .env("SMOLPC_GIMP_PLUGIN_PORT", self.plugin_port.to_string())
            .stdout(Stdio::from(stdout_log))
            .stderr(Stdio::from(stderr_log))
            .spawn()
            .map_err(|error| {
                format!(
                    "Unable to start the GIMP MCP bridge via {} {}: {error}",
                    self.python_command,
                    self.bridge_entrypoint.display()
                )
            })
    }

    pub fn summary(&self) -> String {
        format!(
            "bundled GIMP MCP bridge on {}:{} via {} talking to the plugin socket on {}:{}",
            self.bridge_host,
            self.bridge_port,
            self.python_command,
            self.plugin_host,
            self.plugin_port
        )
    }
}

fn resolve_python_command(app_local_data_dir: Option<&Path>) -> Result<String, String> {
    resolve_python_command_with_candidates(
        app_local_data_dir,
        cfg!(debug_assertions),
        development_python_candidates(&development_repo_root()),
    )
}

fn resolve_python_command_with_candidates(
    app_local_data_dir: Option<&Path>,
    allow_dev_fallback: bool,
    development_candidates: Vec<PathBuf>,
) -> Result<String, String> {
    if let Some(command) = resolve_prepared_python_command(app_local_data_dir) {
        return Ok(command);
    }

    if allow_dev_fallback {
        for candidate in development_candidates {
            if candidate.is_file() {
                return Ok(candidate.to_string_lossy().to_string());
            }
        }

        return Ok(DEFAULT_PYTHON_COMMAND.to_string());
    }

    Err(
        "Bundled Python is not prepared yet. Use setup_prepare() from the setup panel before starting GIMP."
            .to_string(),
    )
}

fn development_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}

fn development_python_candidates(repo_root: &Path) -> Vec<PathBuf> {
    let venv_dir = repo_root.join(".venv");
    let mut candidates = Vec::new();

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
        .map(|base| base.join("setup").join("logs").join("gimp"))
        .unwrap_or_else(|| {
            std::env::temp_dir()
                .join("smolpc-unified-assistant")
                .join(GIMP_LOG_SUBDIR)
        })
}

fn ensure_log_dir(path: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(path)
        .map_err(|error| format!("Unable to create GIMP log directory: {error}"))?;
    std::fs::canonicalize(path)
        .map_err(|error| format!("Unable to canonicalize GIMP log directory: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        development_python_candidates, resolve_python_command_with_candidates,
        DEFAULT_PYTHON_COMMAND,
    };
    use smolpc_connector_common::python::prepared_python_root;
    use std::path::Path;
    use tempfile::TempDir;

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs");
        }
        std::fs::write(path, contents).expect("write file");
    }

    fn prepared_python_candidate_path(root: &Path) -> std::path::PathBuf {
        #[cfg(windows)]
        {
            root.join("payload").join("python.exe")
        }

        #[cfg(not(windows))]
        {
            root.join("payload").join("bin").join("python3")
        }
    }

    #[test]
    fn resolve_python_command_prefers_prepared_runtime_over_dev_candidates() {
        let app_temp = TempDir::new().expect("app temp");
        let prepared_root =
            prepared_python_root(Some(app_temp.path())).expect("prepared python root");
        let prepared_python = prepared_python_candidate_path(&prepared_root);
        write_file(&prepared_python, "python");

        let repo_temp = TempDir::new().expect("repo temp");
        let candidates = development_python_candidates(repo_temp.path());
        let dev_python = candidates.first().expect("dev candidate").clone();
        write_file(&dev_python, "python");

        let command =
            resolve_python_command_with_candidates(Some(app_temp.path()), true, candidates)
                .expect("resolve python command");

        assert_eq!(command, prepared_python.to_string_lossy().to_string());
    }

    #[test]
    fn resolve_python_command_prefers_repo_venv_before_path_python() {
        let repo_temp = TempDir::new().expect("repo temp");
        let candidates = development_python_candidates(repo_temp.path());
        let dev_python = candidates.first().expect("dev candidate").clone();
        write_file(&dev_python, "python");

        let command = resolve_python_command_with_candidates(None, true, candidates)
            .expect("resolve python command");

        assert_eq!(command, dev_python.to_string_lossy().to_string());
    }

    #[test]
    fn resolve_python_command_falls_back_to_path_python_in_debug_mode() {
        let command = resolve_python_command_with_candidates(None, true, Vec::new())
            .expect("resolve python command");

        assert_eq!(command, DEFAULT_PYTHON_COMMAND);
    }

    #[test]
    fn resolve_python_command_requires_prepared_python_when_dev_fallback_is_disabled() {
        let error = resolve_python_command_with_candidates(None, false, Vec::new())
            .expect_err("strict mode should require prepared python");

        assert!(error.contains("Bundled Python is not prepared yet"));
    }
}
