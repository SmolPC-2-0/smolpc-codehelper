use super::profiles::{allowed_tool_names, libreoffice_profile};
use super::resources::{resolve_mcp_server_layout, ResourceResolutionOptions};
use super::response::build_tool_execution_result;
use super::runtime::LibreOfficeRuntimeConfig;
use super::state::LibreOfficeProviderState;
use crate::assistant::MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION;
use crate::modes::provider::{provider_state, ToolProvider};
use async_trait::async_trait;
use smolpc_assistant_types::{
    AppMode, ProviderStateDto, ToolDefinitionDto, ToolExecutionResultDto,
};
use smolpc_mcp_client::McpTool;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

const RETRYABLE_RUNTIME_FAILURE_MARKERS: [&str; 4] = [
    "broken pipe",
    "channel closed",
    "connection reset",
    "process exited",
];

#[derive(Debug)]
pub struct LibreOfficeProvider {
    resource_dir: Option<PathBuf>,
    app_local_data_dir: Option<PathBuf>,
    resolution_options: ResourceResolutionOptions,
    state: Mutex<LibreOfficeProviderState>,
    connect_lock: Mutex<()>,
}

impl Default for LibreOfficeProvider {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl LibreOfficeProvider {
    pub fn new(resource_dir: Option<PathBuf>, app_local_data_dir: Option<PathBuf>) -> Self {
        Self::with_paths_and_resolution_options(
            resource_dir,
            app_local_data_dir,
            ResourceResolutionOptions::default(),
        )
    }

    #[cfg(test)]
    pub(crate) fn with_resolution_options(
        resource_dir: Option<PathBuf>,
        resolution_options: ResourceResolutionOptions,
    ) -> Self {
        Self::with_paths_and_resolution_options(resource_dir, None, resolution_options)
    }

    fn with_paths_and_resolution_options(
        resource_dir: Option<PathBuf>,
        app_local_data_dir: Option<PathBuf>,
        resolution_options: ResourceResolutionOptions,
    ) -> Self {
        Self {
            resource_dir,
            app_local_data_dir,
            resolution_options,
            state: Mutex::new(LibreOfficeProviderState::default()),
            connect_lock: Mutex::new(()),
        }
    }

    fn profile_for_mode(mode: AppMode) -> Result<super::LibreOfficeModeProfile, String> {
        libreoffice_profile(mode)
            .ok_or_else(|| format!("LibreOffice provider does not handle mode {mode:?}"))
    }

    fn tool_definition(tool: McpTool) -> ToolDefinitionDto {
        ToolDefinitionDto {
            name: tool.name,
            description: tool.description,
            input_schema: tool.input_schema,
        }
    }

    fn filtered_tools(mode: AppMode, raw_tools: Vec<McpTool>) -> Vec<ToolDefinitionDto> {
        let allowlist = allowed_tool_names(mode);
        raw_tools
            .into_iter()
            .filter(|tool| allowlist.iter().any(|name| *name == tool.name))
            .map(Self::tool_definition)
            .collect()
    }

    fn connected_state(mode: AppMode, detail: Option<String>) -> ProviderStateDto {
        provider_state(mode, "connected", detail.as_deref(), true, false)
    }

    fn disconnected_state(mode: AppMode, state: &LibreOfficeProviderState) -> ProviderStateDto {
        let label = if state.runtime_attempted && state.last_error.is_some() {
            "error"
        } else {
            "disconnected"
        };

        provider_state(mode, label, state.last_error.as_deref(), true, false)
    }

    fn friendly_runtime_error(error: &str) -> String {
        if error.contains("Bundled Python is not prepared yet") {
            return format!(
                "Document runtime is not ready yet. Prepare bundled Python from the setup panel before using Writer or Slides. {error}"
            );
        }

        if error.contains("spawn stdio MCP command") {
            return format!(
                "Unable to start the document MCP server. Bundled Python should be prepared first. {error}"
            );
        }

        format!("Document runtime failed. Make sure bundled Python is prepared. {error}")
    }

    fn is_retryable_runtime_failure(message: &str) -> bool {
        let normalized = message.to_ascii_lowercase();
        RETRYABLE_RUNTIME_FAILURE_MARKERS
            .iter()
            .any(|marker| normalized.contains(marker))
    }

    async fn reconnect_session_for_retry(
        &self,
        mode: AppMode,
    ) -> Result<Arc<smolpc_mcp_client::McpSession>, String> {
        {
            let mut state = self.state.lock().await;
            state.session = None;
            state.tools.clear();
        }

        self.connect_live(mode).await?;
        let state = self.state.lock().await;
        state.session.clone().ok_or_else(|| {
            "LibreOffice provider retry reconnect did not produce a session".to_string()
        })
    }

    async fn retry_tool_after_runtime_reconnect(
        &self,
        mode: AppMode,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<Option<ToolExecutionResultDto>, String> {
        let retry_session = match self.reconnect_session_for_retry(mode).await {
            Ok(session) => session,
            Err(error) => {
                log::warn!(
                    "LibreOffice runtime retry reconnect failed for tool {} in mode {:?}: {}",
                    name,
                    mode,
                    error
                );
                return Ok(None);
            }
        };

        match retry_session.call_tool(name, arguments).await {
            Ok(payload) => Ok(Some(build_tool_execution_result(name, payload))),
            Err(error) => {
                let message = Self::friendly_runtime_error(&error.to_string());
                let mut state = self.state.lock().await;
                state.session = None;
                state.tools.clear();
                state.last_error = Some(message.clone());
                Err(message)
            }
        }
    }

    fn live_connected_detail(
        profile: super::LibreOfficeModeProfile,
        layout: &super::resources::LibreOfficeResourceLayout,
        runtime: &LibreOfficeRuntimeConfig,
        tools: &[ToolDefinitionDto],
    ) -> String {
        format!(
            "{} is connected through the document MCP server. {} Scaffold: {}. {} tool(s) available in this mode.",
            profile.label,
            runtime.summary(),
            layout.mcp_server_dir.display(),
            tools.len()
        )
    }

    fn empty_allowlist_detail(profile: super::LibreOfficeModeProfile) -> String {
        format!(
            "{} connected, but the runtime did not expose any {} allowlisted tools.",
            profile.label, profile.label
        )
    }

    fn validate_runtime_prerequisites(
        &self,
    ) -> Result<
        (
            super::resources::LibreOfficeResourceLayout,
            LibreOfficeRuntimeConfig,
        ),
        String,
    > {
        let layout =
            resolve_mcp_server_layout(self.resource_dir.as_deref(), self.resolution_options)?;
        let runtime = LibreOfficeRuntimeConfig::from_layout(
            &layout,
            self.app_local_data_dir.as_deref(),
            self.resolution_options.allow_system_python_fallback,
        )?;
        log::debug!(
            "Resolved document runtime prerequisites: scaffold_dir={}, entrypoint={}, python_command={}",
            layout.mcp_server_dir.display(),
            layout.mcp_server_py_path.display(),
            runtime.python_command,
        );
        Ok((layout, runtime))
    }

    async fn connect_live(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
        let profile = Self::profile_for_mode(mode)?;
        let _connect_guard = self.connect_lock.lock().await;

        {
            let state = self.state.lock().await;
            if state.session.is_some() && !state.tools.is_empty() {
                return Ok(Self::connected_state(mode, state.last_error.clone()));
            }
        }

        let (layout, runtime) = self.validate_runtime_prerequisites()?;
        log::info!(
            "Connecting LibreOffice runtime from scaffold {} via entrypoint {}",
            layout.mcp_server_dir.display(),
            runtime.entrypoint.display()
        );
        {
            let mut state = self.state.lock().await;
            state.scaffold_dir = Some(layout.mcp_server_dir.clone());
            state.runtime_attempted = true;
        }
        let session = Arc::new(
            runtime
                .connect_session()
                .await
                .map_err(|error| Self::friendly_runtime_error(&error))?,
        );
        let raw_tools = session
            .list_tools()
            .await
            .map_err(|error| Self::friendly_runtime_error(&error.to_string()))?;
        let tools = Self::filtered_tools(mode, raw_tools);

        let mut state = self.state.lock().await;
        state.session = Some(Arc::clone(&session));

        if tools.is_empty() {
            let detail = Self::empty_allowlist_detail(profile);
            state.tools.clear();
            state.last_error = Some(detail.clone());
            return Ok(provider_state(
                mode,
                "error",
                Some(detail.as_str()),
                true,
                false,
            ));
        }

        state.tools = tools.clone();
        state.last_error = None;
        Ok(Self::connected_state(
            mode,
            Some(Self::live_connected_detail(
                profile, &layout, &runtime, &tools,
            )),
        ))
    }

    async fn refresh_live_state(&self, mode: AppMode) -> ProviderStateDto {
        let profile = Self::profile_for_mode(mode).expect("validated libreoffice mode");
        let (layout, runtime) = match self.validate_runtime_prerequisites() {
            Ok(value) => value,
            Err(error) => {
                let mut state = self.state.lock().await;
                state.scaffold_dir = None;
                state.last_error = Some(error.clone());
                return provider_state(mode, "error", Some(error.as_str()), true, false);
            }
        };

        {
            let mut state = self.state.lock().await;
            state.scaffold_dir = Some(layout.mcp_server_dir.clone());
        }

        let session = {
            let state = self.state.lock().await;
            state.session.clone()
        };

        if let Some(session) = session {
            match session.list_tools().await {
                Ok(raw_tools) => {
                    let tools = Self::filtered_tools(mode, raw_tools);
                    let mut state = self.state.lock().await;
                    if tools.is_empty() {
                        let detail = Self::empty_allowlist_detail(profile);
                        state.tools.clear();
                        state.last_error = Some(detail.clone());
                        return provider_state(mode, "error", Some(detail.as_str()), true, false);
                    }
                    state.tools = tools.clone();
                    state.last_error = None;
                    return Self::connected_state(
                        mode,
                        Some(Self::live_connected_detail(
                            profile, &layout, &runtime, &tools,
                        )),
                    );
                }
                Err(error) => {
                    let mut state = self.state.lock().await;
                    state.session = None;
                    state.tools.clear();
                    state.last_error = Some(Self::friendly_runtime_error(&error.to_string()));
                }
            }
        }

        match self.connect_live(mode).await {
            Ok(provider_state) => provider_state,
            Err(error) => {
                let mut state = self.state.lock().await;
                state.last_error = Some(error);
                Self::disconnected_state(mode, &state)
            }
        }
    }
}

#[async_trait]
impl ToolProvider for LibreOfficeProvider {
    async fn connect_if_needed(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
        Self::profile_for_mode(mode)?;
        let state = self.state.lock().await;

        if state.session.is_some() && !state.tools.is_empty() {
            return Ok(Self::connected_state(mode, state.last_error.clone()));
        }
        drop(state);

        self.connect_live(mode).await
    }

    async fn status(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
        Self::profile_for_mode(mode)?;
        let mut state = self.state.lock().await;

        match self.validate_runtime_prerequisites() {
            Ok((layout, _runtime)) => {
                state.scaffold_dir = Some(layout.mcp_server_dir);
            }
            Err(error) => {
                state.scaffold_dir = None;
                state.last_error = Some(error.clone());
                return Ok(provider_state(
                    mode,
                    "error",
                    Some(error.as_str()),
                    true,
                    false,
                ));
            }
        }
        drop(state);

        Ok(self.refresh_live_state(mode).await)
    }

    async fn list_tools(&self, mode: AppMode) -> Result<Vec<ToolDefinitionDto>, String> {
        Self::profile_for_mode(mode)?;

        let state = self.state.lock().await;
        Ok(state.tools.clone())
    }

    async fn execute_tool(
        &self,
        mode: AppMode,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolExecutionResultDto, String> {
        let profile = Self::profile_for_mode(mode)?;

        if !allowed_tool_names(mode).contains(&name) {
            return Err(format!(
                "{name} is not available in {} mode.",
                profile.label
            ));
        }

        let session = {
            let state = self.state.lock().await;
            state.session.clone()
        };
        let session = match session {
            Some(session) => session,
            None => {
                self.connect_live(mode).await?;
                let state = self.state.lock().await;
                state
                    .session
                    .clone()
                    .ok_or_else(|| "LibreOffice provider is not connected".to_string())?
            }
        };
        let retry_arguments = arguments.clone();

        match session.call_tool(name, arguments).await {
            Ok(payload) => {
                let mut tool_result = build_tool_execution_result(name, payload);
                if !tool_result.ok && Self::is_retryable_runtime_failure(&tool_result.summary) {
                    log::warn!(
                        "Retrying LibreOffice tool {} in mode {:?} after transient runtime failure: {}",
                        name,
                        mode,
                        tool_result.summary
                    );
                    if let Some(retry_result) = self
                        .retry_tool_after_runtime_reconnect(mode, name, retry_arguments)
                        .await?
                    {
                        tool_result = retry_result;
                    }
                }
                let mut state = self.state.lock().await;
                if tool_result.ok {
                    state.last_error = None;
                } else {
                    state.last_error = Some(tool_result.summary.clone());
                }
                Ok(tool_result)
            }
            Err(error) => {
                let error_text = error.to_string();
                if Self::is_retryable_runtime_failure(&error_text) {
                    log::warn!(
                        "Retrying LibreOffice tool {} in mode {:?} after runtime transport error: {}",
                        name,
                        mode,
                        error_text
                    );
                    if let Some(tool_result) = self
                        .retry_tool_after_runtime_reconnect(mode, name, retry_arguments)
                        .await?
                    {
                        let mut state = self.state.lock().await;
                        if tool_result.ok {
                            state.last_error = None;
                        } else {
                            state.last_error = Some(tool_result.summary.clone());
                        }
                        return Ok(tool_result);
                    }
                }

                let message = Self::friendly_runtime_error(&error.to_string());
                let mut state = self.state.lock().await;
                state.session = None;
                state.tools.clear();
                state.last_error = Some(message.clone());
                Err(message)
            }
        }
    }

    async fn undo_last_action(&self, _mode: AppMode) -> Result<(), String> {
        Err(MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION.to_string())
    }

    async fn disconnect_if_needed(&self, mode: AppMode) -> Result<(), String> {
        Self::profile_for_mode(mode)?;

        let mut state = self.state.lock().await;
        state.session = None;
        state.tools.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::LibreOfficeProvider;
    use crate::modes::libreoffice::resources::ResourceResolutionOptions;
    use crate::modes::provider::ToolProvider;
    use serde_json::json;
    use smolpc_assistant_types::AppMode;
    use std::fs;
    use std::sync::Arc;
    use tempfile::tempdir;

    const TEST_RUNTIME: &str = r#"#!/usr/bin/env python3
import json
import os
import sys

TOOLS = [
    {"name": "add_heading", "description": "Add a heading", "inputSchema": {"type": "object"}},
    {"name": "add_slide", "description": "Add a slide", "inputSchema": {"type": "object"}}
]

def send(value):
    sys.stdout.write(json.dumps(value) + "\n")
    sys.stdout.flush()

assert os.getenv("SMOLPC_MCP_LOG_DIR")

for line in sys.stdin:
    payload = json.loads(line)
    method = payload.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": payload["id"], "result": {"serverInfo": {"name": "libre-mcp", "version": "test"}}})
    elif method == "notifications/initialized":
        continue
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": payload["id"], "result": {"tools": TOOLS}})
    elif method == "tools/call":
        name = payload["params"]["name"]
        send({"jsonrpc": "2.0", "id": payload["id"], "result": {"content": [{"type": "text", "text": f"{name} completed"}]}})
"#;

    fn staged_runtime_root(mcp_server_py: &str) -> tempfile::TempDir {
        let tempdir = tempdir().expect("tempdir");
        let staged_dir = tempdir
            .path()
            .join("resources")
            .join("libreoffice")
            .join("mcp_server");
        fs::create_dir_all(&staged_dir).expect("create staged dir");
        fs::write(staged_dir.join("README.md"), "placeholder").expect("write readme");
        fs::write(staged_dir.join("mcp_server.py"), mcp_server_py).expect("write mcp_server");
        fs::write(staged_dir.join("test_functions.py"), "placeholder")
            .expect("write test_functions");
        tempdir
    }

    #[tokio::test]
    async fn writer_status_connects_and_filters_writer_tools() {
        let tempdir = staged_runtime_root(TEST_RUNTIME);
        let provider = LibreOfficeProvider::with_resolution_options(
            Some(tempdir.path().to_path_buf()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
                allow_system_python_fallback: true,
            },
        );

        let state = provider.status(AppMode::Writer).await.expect("status");
        let tools = provider.list_tools(AppMode::Writer).await.expect("tools");

        assert_eq!(state.state, "connected");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "add_heading");
    }

    #[tokio::test]
    async fn impress_status_connects_and_filters_slides_tools() {
        let tempdir = staged_runtime_root(TEST_RUNTIME);
        let provider = LibreOfficeProvider::with_resolution_options(
            Some(tempdir.path().to_path_buf()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
                allow_system_python_fallback: true,
            },
        );

        let state = provider.status(AppMode::Impress).await.expect("status");
        let tools = provider.list_tools(AppMode::Impress).await.expect("tools");

        assert_eq!(state.state, "connected");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "add_slide");
    }

    #[tokio::test]
    async fn unsupported_mode_returns_error_without_starting_runtime() {
        let tempdir = staged_runtime_root(
            "from pathlib import Path\nPath('runtime-started.txt').write_text('started')\n",
        );
        let provider = LibreOfficeProvider::with_resolution_options(
            Some(tempdir.path().to_path_buf()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
                allow_system_python_fallback: true,
            },
        );

        let error = provider
            .status(AppMode::Code)
            .await
            .expect_err("unsupported mode");

        assert!(error.contains("does not handle mode"));
        assert!(
            !tempdir.path().join("runtime-started.txt").exists(),
            "Unsupported mode status should not start the runtime"
        );
    }

    #[tokio::test]
    async fn runtime_start_failure_produces_honest_error_detail() {
        let tempdir =
            staged_runtime_root("import sys\nsys.stderr.write('runtime failed')\nsys.exit(1)\n");
        let provider = LibreOfficeProvider::with_resolution_options(
            Some(tempdir.path().to_path_buf()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
                allow_system_python_fallback: true,
            },
        );

        let state = provider.status(AppMode::Writer).await.expect("status");

        assert_eq!(state.state, "error");
        assert!(state
            .detail
            .expect("detail")
            .contains("Document runtime failed"));
    }

    #[tokio::test]
    async fn missing_runtime_resources_produces_error_detail() {
        let tempdir = tempdir().expect("tempdir");
        let provider = LibreOfficeProvider::with_resolution_options(
            Some(tempdir.path().to_path_buf()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
                allow_system_python_fallback: true,
            },
        );

        let state = provider.status(AppMode::Writer).await.expect("status");

        assert_eq!(state.state, "error");
        assert!(state
            .detail
            .expect("detail")
            .contains("resources are not bundled yet"));
    }

    #[tokio::test]
    async fn execute_tool_survives_concurrent_status_refresh() {
        let tempdir = staged_runtime_root(TEST_RUNTIME);
        let provider = Arc::new(LibreOfficeProvider::with_resolution_options(
            Some(tempdir.path().to_path_buf()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
                allow_system_python_fallback: true,
            },
        ));

        let initial = provider
            .status(AppMode::Writer)
            .await
            .expect("initial status");
        assert_eq!(initial.state, "connected");

        let execute_provider = Arc::clone(&provider);
        let status_provider = Arc::clone(&provider);

        let (tool_result, refreshed_state) = tokio::join!(
            async move {
                execute_provider
                    .execute_tool(AppMode::Writer, "add_heading", json!({"text": "Hello"}))
                    .await
            },
            async move { status_provider.status(AppMode::Writer).await }
        );

        let tool_result = tool_result.expect("tool result");
        let refreshed_state = refreshed_state.expect("refreshed state");

        assert!(tool_result.ok);
        assert_eq!(tool_result.name, "add_heading");
        assert_eq!(refreshed_state.state, "connected");
    }

    #[test]
    fn retryable_runtime_failure_detection_matches_known_messages() {
        assert!(LibreOfficeProvider::is_retryable_runtime_failure(
            "Error: broken pipe"
        ));
        assert!(LibreOfficeProvider::is_retryable_runtime_failure(
            "Error: channel closed unexpectedly"
        ));
        assert!(LibreOfficeProvider::is_retryable_runtime_failure(
            "connection reset by peer"
        ));
        assert!(LibreOfficeProvider::is_retryable_runtime_failure(
            "process exited with code 1"
        ));
        assert!(!LibreOfficeProvider::is_retryable_runtime_failure(
            "Mode is unavailable in this build"
        ));
    }
}
