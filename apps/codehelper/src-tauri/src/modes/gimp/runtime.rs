use crate::setup::gimp::{
    resolve_gimp_resource_layout, GIMP_PLUGIN_SOCKET_HOST, GIMP_PLUGIN_SOCKET_PORT,
};
use crate::setup::python::resolve_prepared_python_command;
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
    if let Some(command) = resolve_prepared_python_command(app_local_data_dir) {
        return Ok(command);
    }

    if cfg!(debug_assertions) {
        return Ok(DEFAULT_PYTHON_COMMAND.to_string());
    }

    Err(
        "Bundled Python is not prepared yet. Use setup_prepare() from the setup panel before starting GIMP."
            .to_string(),
    )
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
