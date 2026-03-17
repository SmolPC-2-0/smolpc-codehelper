use super::resources::LibreOfficeResourceLayout;
use smolpc_mcp_client::StdioTransportConfig;
use std::path::PathBuf;

pub const LIBREOFFICE_HELPER_SOCKET_ADDR: &str = "localhost:8765";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibreOfficeRuntimeScaffold {
    pub entrypoint: PathBuf,
    pub working_dir: PathBuf,
    pub helper_socket_addr: String,
}

impl LibreOfficeRuntimeScaffold {
    pub fn from_layout(layout: &LibreOfficeResourceLayout) -> Self {
        Self {
            entrypoint: layout.mcp_server_dir.join("main.py"),
            working_dir: layout.mcp_server_dir.clone(),
            helper_socket_addr: LIBREOFFICE_HELPER_SOCKET_ADDR.to_string(),
        }
    }

    pub fn stdio_transport_config(&self) -> StdioTransportConfig {
        StdioTransportConfig {
            command: "python".to_string(),
            args: vec![self.entrypoint.to_string_lossy().to_string()],
            cwd: Some(self.working_dir.clone()),
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "shared LibreOffice MCP runtime over stdio via Python main.py with helper socket bridge on {}",
            self.helper_socket_addr
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{LibreOfficeRuntimeScaffold, LIBREOFFICE_HELPER_SOCKET_ADDR};
    use crate::modes::libreoffice::resources::LibreOfficeResourceLayout;
    use std::path::PathBuf;

    #[test]
    fn runtime_scaffold_produces_future_stdio_config() {
        let layout = LibreOfficeResourceLayout {
            mcp_server_dir: PathBuf::from("/tmp/libreoffice/mcp_server"),
            readme_path: PathBuf::from("/tmp/libreoffice/mcp_server/README.md"),
        };

        let runtime = LibreOfficeRuntimeScaffold::from_layout(&layout);
        let config = runtime.stdio_transport_config();

        assert_eq!(config.command, "python");
        assert_eq!(
            config.args,
            vec!["/tmp/libreoffice/mcp_server/main.py".to_string()]
        );
        assert_eq!(runtime.helper_socket_addr, LIBREOFFICE_HELPER_SOCKET_ADDR);
    }
}
