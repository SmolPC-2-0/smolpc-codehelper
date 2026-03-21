use super::macros;
use super::response::build_tool_execution_result;
use super::runtime::GimpRuntimeConfig;
use super::transport::{connect_session, default_transport_config, tool_definition};
use crate::modes::provider::{provider_state, ToolProvider};
use crate::setup::gimp::{
    ensure_gimp_plugin_runtime_prepared, validate_supported_gimp, GimpPluginRuntimePrepareOutcome,
};
use crate::setup::host_apps::{detect_gimp, HostAppDetection};
use crate::setup::launch::{launch_gimp_if_needed, GimpLaunchOutcome};
use async_trait::async_trait;
use serde_json::json;
use smolpc_assistant_types::{
    AppMode, ProviderStateDto, ToolDefinitionDto, ToolExecutionResultDto,
};
use smolpc_mcp_client::{McpSession, TcpTransportConfig};
use std::path::{Path, PathBuf};
use std::process::Child;
use tokio::sync::Mutex;
use tokio::task;
use tokio::time::{sleep, Duration};

const GIMP_CONNECTION_RETRY_ATTEMPTS: usize = 24;
const GIMP_CONNECTION_RETRY_DELAY_MS: u64 = 250;

#[derive(Debug)]
struct RuntimeState {
    session: Option<McpSession>,
    runtime_child: Option<Child>,
    tools: Vec<ToolDefinitionDto>,
    last_error: Option<String>,
    ever_connected: bool,
    detected_gimp_path: Option<PathBuf>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            session: None,
            runtime_child: None,
            tools: Vec::new(),
            last_error: None,
            ever_connected: false,
            detected_gimp_path: None,
        }
    }
}

impl Drop for RuntimeState {
    fn drop(&mut self) {
        if let Some(child) = self.runtime_child.take() {
            terminate_child(child);
        }
    }
}

struct LiveSessionConnection {
    session: McpSession,
    tools: Vec<ToolDefinitionDto>,
    detail: String,
}

fn terminate_child(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[derive(Debug)]
pub struct GimpProvider {
    config: TcpTransportConfig,
    resource_dir: Option<PathBuf>,
    app_local_data_dir: Option<PathBuf>,
    state: Mutex<RuntimeState>,
    connect_lock: Mutex<()>,
    host_gimp_override: Option<PathBuf>,
}

impl Default for GimpProvider {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl GimpProvider {
    pub fn new(resource_dir: Option<PathBuf>, app_local_data_dir: Option<PathBuf>) -> Self {
        Self::with_config(default_transport_config(), resource_dir, app_local_data_dir)
    }

    pub fn with_config(
        config: TcpTransportConfig,
        resource_dir: Option<PathBuf>,
        app_local_data_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            config,
            resource_dir,
            app_local_data_dir,
            state: Mutex::new(RuntimeState::default()),
            connect_lock: Mutex::new(()),
            host_gimp_override: None,
        }
    }

    #[cfg(test)]
    pub fn with_host_override(
        config: TcpTransportConfig,
        resource_dir: Option<PathBuf>,
        app_local_data_dir: Option<PathBuf>,
        host_gimp_override: Option<PathBuf>,
    ) -> Self {
        Self {
            config,
            resource_dir,
            app_local_data_dir,
            state: Mutex::new(RuntimeState::default()),
            connect_lock: Mutex::new(()),
            host_gimp_override,
        }
    }

    fn connected_state(mode: AppMode, detail: Option<String>) -> ProviderStateDto {
        provider_state(mode, "connected", detail.as_deref(), true, true)
    }

    fn disconnected_state(mode: AppMode, state: &RuntimeState) -> ProviderStateDto {
        let label = if state.ever_connected && state.last_error.is_some() {
            "error"
        } else {
            "disconnected"
        };

        provider_state(mode, label, state.last_error.as_deref(), true, true)
    }

    fn raw_transport_error(error: &str) -> String {
        if error.contains("connect tcp") || error.contains("connection closed") {
            return format!(
                "Unable to reach the GIMP MCP bridge on {}:{}. {}",
                super::transport::DEFAULT_GIMP_HOST,
                super::transport::DEFAULT_GIMP_PORT,
                "Make sure the bundled GIMP bridge is running and GIMP is available."
            );
        }

        format!("GIMP MCP request failed: {error}")
    }

    fn host_detection(&self, cached: Option<&Path>) -> HostAppDetection {
        if let Some(path) = self.host_gimp_override.as_deref() {
            let resolved = path.exists().then(|| path.to_path_buf());
            let detail = resolved
                .as_ref()
                .map(|value| format!("GIMP detected at {}", value.display()))
                .or_else(|| {
                    Some(format!(
                        "GIMP is not installed or could not be detected yet. Expected {}.",
                        path.display()
                    ))
                });

            return HostAppDetection {
                id: "host_gimp",
                label: "GIMP",
                path: resolved,
                detail,
            };
        }

        detect_gimp(cached)
    }

    fn is_self_contained_mode(&self) -> bool {
        self.resource_dir.is_some() && self.app_local_data_dir.is_some()
    }

    async fn connect_locked_raw(
        &self,
        mode: AppMode,
        state: &mut RuntimeState,
    ) -> Result<ProviderStateDto, String> {
        let session = connect_session(&self.config)
            .await
            .map_err(|error| Self::raw_transport_error(&error))?;
        let tools = session
            .list_tools()
            .await
            .map_err(|error| Self::raw_transport_error(&error.to_string()))?;

        state.tools = tools.into_iter().map(tool_definition).collect();
        state.session = Some(session);
        state.last_error = None;
        state.ever_connected = true;

        Ok(Self::connected_state(mode, None))
    }

    async fn refresh_raw_state(&self, mode: AppMode, state: &mut RuntimeState) -> ProviderStateDto {
        if let Some(session) = state.session.as_ref() {
            match session.list_tools().await {
                Ok(tools) => {
                    state.tools = tools.into_iter().map(tool_definition).collect();
                    state.last_error = None;
                    return Self::connected_state(mode, None);
                }
                Err(error) => {
                    state.session = None;
                    state.tools.clear();
                    state.last_error = Some(Self::raw_transport_error(&error.to_string()));
                }
            }
        }

        match self.connect_locked_raw(mode, state).await {
            Ok(provider_state) => provider_state,
            Err(error) => {
                state.last_error = Some(error);
                Self::disconnected_state(mode, state)
            }
        }
    }

    fn runtime_spawn_note(restarted: bool) -> &'static str {
        if restarted {
            "Bundled GIMP MCP bridge was restarted."
        } else {
            "Bundled GIMP MCP bridge was started."
        }
    }

    fn prepared_note(outcome: &GimpPluginRuntimePrepareOutcome) -> Option<&'static str> {
        match outcome {
            GimpPluginRuntimePrepareOutcome::Prepared(_) => {
                Some("Bundled GIMP plugin/runtime was provisioned.")
            }
            GimpPluginRuntimePrepareOutcome::AlreadyReady(_) => None,
        }
    }

    fn launch_note(outcome: GimpLaunchOutcome) -> &'static str {
        match outcome {
            GimpLaunchOutcome::AlreadyRunning => "Reused an already-running GIMP session.",
            GimpLaunchOutcome::Launched => "GIMP was launched automatically.",
        }
    }

    fn connected_detail(
        runtime: &GimpRuntimeConfig,
        tools: &[ToolDefinitionDto],
        extra_notes: &[String],
    ) -> String {
        let mut notes = extra_notes.to_vec();
        notes.push(format!(
            "GIMP is connected through the {}. {} tool(s) available.",
            runtime.summary(),
            tools.len()
        ));
        notes.join(" ")
    }

    fn connection_retry_error(
        gimp_path: &Path,
        launch_outcome: GimpLaunchOutcome,
        error: &str,
    ) -> String {
        match launch_outcome {
            GimpLaunchOutcome::AlreadyRunning => format!(
                "Unable to connect to the bundled GIMP runtime for {}. If this GIMP session started before the bundled plugin was provisioned, reopen GIMP once and try again. {error}",
                gimp_path.display()
            ),
            GimpLaunchOutcome::Launched => format!(
                "Unable to connect to the bundled GIMP runtime for {} after launching GIMP automatically. {error}",
                gimp_path.display()
            ),
        }
    }

    fn runtime_error_detail(error: &str) -> String {
        if error.contains("Bundled Python is not prepared yet") {
            return format!(
                "GIMP runtime is not ready yet. Prepare bundled Python from the setup panel before using GIMP. {error}"
            );
        }

        format!("Unable to start the bundled GIMP MCP bridge. {error}")
    }

    fn ensure_runtime_child_started(
        &self,
        state: &mut RuntimeState,
        runtime: &GimpRuntimeConfig,
    ) -> Result<Option<String>, String> {
        let mut restarted = false;
        if let Some(child) = state.runtime_child.as_mut() {
            match child.try_wait() {
                Ok(None) => return Ok(None),
                Ok(Some(_)) => {
                    state.runtime_child = None;
                    restarted = true;
                }
                Err(error) => {
                    if let Some(child) = state.runtime_child.take() {
                        terminate_child(child);
                    }
                    return Err(format!(
                        "Unable to inspect the GIMP bridge process state: {error}"
                    ));
                }
            }
        }

        let child = runtime.spawn_bridge()?;
        state.runtime_child = Some(child);
        Ok(Some(Self::runtime_spawn_note(restarted).to_string()))
    }

    async fn connect_live_session(
        &self,
        gimp_path: &Path,
        runtime: &GimpRuntimeConfig,
        prepare_outcome: &GimpPluginRuntimePrepareOutcome,
        launch_outcome: GimpLaunchOutcome,
        runtime_note: Option<String>,
    ) -> Result<LiveSessionConnection, String> {
        let mut last_error = None;

        for attempt in 0..GIMP_CONNECTION_RETRY_ATTEMPTS {
            match connect_session(&self.config).await {
                Ok(session) => match session.list_tools().await {
                    Ok(raw_tools) => {
                        if let Err(error) = session.call_tool("get_gimp_info", json!({})).await {
                            last_error = Some(Self::raw_transport_error(&error.to_string()));
                        } else {
                            let tools: Vec<ToolDefinitionDto> =
                                raw_tools.into_iter().map(tool_definition).collect();
                            let mut notes = Vec::new();
                            if let Some(note) = runtime_note.clone() {
                                notes.push(note);
                            }
                            if let Some(note) = Self::prepared_note(prepare_outcome) {
                                notes.push(note.to_string());
                            }
                            notes.push(Self::launch_note(launch_outcome).to_string());
                            let detail = Self::connected_detail(runtime, &tools, &notes);
                            return Ok(LiveSessionConnection {
                                session,
                                tools,
                                detail,
                            });
                        }
                    }
                    Err(error) => {
                        last_error = Some(Self::raw_transport_error(&error.to_string()));
                    }
                },
                Err(error) => {
                    last_error = Some(Self::raw_transport_error(&error));
                }
            }

            if attempt + 1 < GIMP_CONNECTION_RETRY_ATTEMPTS {
                sleep(Duration::from_millis(GIMP_CONNECTION_RETRY_DELAY_MS)).await;
            }
        }

        let detail = Self::connection_retry_error(
            gimp_path,
            launch_outcome,
            last_error
                .as_deref()
                .unwrap_or("Timed out waiting for the bundled GIMP runtime."),
        );
        Err(detail)
    }

    async fn ensure_provider_ready(&self, mode: AppMode) -> ProviderStateDto {
        let _connect_guard = self.connect_lock.lock().await;

        let cached_gimp_path = {
            let state = self.state.lock().await;
            state.detected_gimp_path.clone()
        };
        let detection = self.host_detection(cached_gimp_path.as_deref());
        let Some(gimp_path) = detection.path.clone() else {
            let detail = detection.detail.unwrap_or_else(|| {
                "GIMP is not installed or could not be detected yet.".to_string()
            });
            let mut state = self.state.lock().await;
            state.detected_gimp_path = None;
            state.last_error = Some(detail.clone());
            return provider_state(mode, "disconnected", Some(detail.as_str()), true, true);
        };

        let gimp_path_for_validation = gimp_path.clone();
        let validation_result = task::spawn_blocking(move || {
            validate_supported_gimp(gimp_path_for_validation.as_path())
        })
        .await;
        if let Err(detail) = match validation_result {
            Ok(result) => result,
            Err(error) => Err(format!(
                "Unable to validate the detected GIMP install in a background task. {error}"
            )),
        } {
            let mut state = self.state.lock().await;
            state.detected_gimp_path = Some(gimp_path);
            state.last_error = Some(detail.clone());
            return provider_state(mode, "error", Some(detail.as_str()), true, true);
        }

        let resource_dir = self.resource_dir.clone();
        let app_local_data_dir = self.app_local_data_dir.clone();
        let gimp_path_for_prepare = gimp_path.clone();
        let prepare_result = task::spawn_blocking(move || {
            ensure_gimp_plugin_runtime_prepared(
                resource_dir.as_deref(),
                app_local_data_dir.as_deref(),
                gimp_path_for_prepare.as_path(),
            )
        })
        .await;
        let prepare_outcome = match match prepare_result {
            Ok(result) => result,
            Err(error) => Err(format!(
                "Unable to provision the bundled GIMP plugin/runtime in a background task. {error}"
            )),
        } {
            Ok(outcome) => outcome,
            Err(error) => {
                let detail = format!(
                    "Unable to provision the bundled GIMP plugin/runtime automatically. {error}"
                );
                let mut state = self.state.lock().await;
                state.detected_gimp_path = Some(gimp_path);
                state.last_error = Some(detail.clone());
                return provider_state(mode, "error", Some(detail.as_str()), true, true);
            }
        };

        let runtime = match GimpRuntimeConfig::from_paths(
            self.resource_dir.as_deref(),
            self.app_local_data_dir.as_deref(),
        ) {
            Ok(runtime) => runtime,
            Err(error) => {
                let detail = Self::runtime_error_detail(&error);
                let mut state = self.state.lock().await;
                state.detected_gimp_path = Some(gimp_path);
                state.last_error = Some(detail.clone());
                return provider_state(mode, "error", Some(detail.as_str()), true, true);
            }
        };

        let runtime_note = {
            let mut state = self.state.lock().await;
            state.detected_gimp_path = Some(gimp_path.clone());
            match self.ensure_runtime_child_started(&mut state, &runtime) {
                Ok(note) => note,
                Err(error) => {
                    let detail = Self::runtime_error_detail(&error);
                    state.last_error = Some(detail.clone());
                    return provider_state(mode, "error", Some(detail.as_str()), true, true);
                }
            }
        };

        let launch_outcome = match launch_gimp_if_needed(&gimp_path) {
            Ok(outcome) => outcome,
            Err(error) => {
                let detail = format!(
                    "The bundled GIMP plugin/runtime is provisioned, but the app could not launch GIMP at {}. {error}",
                    gimp_path.display()
                );
                let mut state = self.state.lock().await;
                state.detected_gimp_path = Some(gimp_path);
                state.last_error = Some(detail.clone());
                return provider_state(mode, "error", Some(detail.as_str()), true, true);
            }
        };

        match self
            .connect_live_session(
                &gimp_path,
                &runtime,
                &prepare_outcome,
                launch_outcome,
                runtime_note,
            )
            .await
        {
            Ok(connection) => {
                let mut state = self.state.lock().await;
                state.tools = connection.tools;
                state.session = Some(connection.session);
                state.last_error = None;
                state.ever_connected = true;
                state.detected_gimp_path = Some(gimp_path);
                Self::connected_state(mode, Some(connection.detail))
            }
            Err(error) => {
                let mut state = self.state.lock().await;
                state.session = None;
                state.tools.clear();
                state.last_error = Some(error.clone());
                state.detected_gimp_path = Some(gimp_path);
                provider_state(mode, "error", Some(error.as_str()), true, true)
            }
        }
    }
}

#[async_trait]
impl ToolProvider for GimpProvider {
    async fn connect_if_needed(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
        if self.is_self_contained_mode() {
            return Ok(self.ensure_provider_ready(mode).await);
        }

        let mut state = self.state.lock().await;
        if state.session.is_some() {
            return Ok(Self::connected_state(mode, None));
        }

        self.connect_locked_raw(mode, &mut state).await
    }

    async fn status(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
        if self.is_self_contained_mode() {
            return Ok(self.ensure_provider_ready(mode).await);
        }

        let mut state = self.state.lock().await;
        Ok(self.refresh_raw_state(mode, &mut state).await)
    }

    async fn list_tools(&self, _mode: AppMode) -> Result<Vec<ToolDefinitionDto>, String> {
        let state = self.state.lock().await;
        Ok(state.tools.clone())
    }

    async fn execute_tool(
        &self,
        mode: AppMode,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolExecutionResultDto, String> {
        if self.is_self_contained_mode() {
            let state = self.ensure_provider_ready(mode).await;
            if state.state != "connected" {
                return Err(state
                    .detail
                    .unwrap_or_else(|| "GIMP provider is not connected".to_string()));
            }
        } else {
            let mut state = self.state.lock().await;
            if state.session.is_none() {
                self.connect_locked_raw(mode, &mut state).await?;
            }
        }

        let mut state = self.state.lock().await;
        let session = state
            .session
            .as_ref()
            .ok_or_else(|| "GIMP provider is not connected".to_string())?;

        match session.call_tool(name, arguments).await {
            Ok(payload) => {
                let tool_result = build_tool_execution_result(name, payload);
                if tool_result.ok {
                    state.last_error = None;
                } else {
                    state.last_error = Some(tool_result.summary.clone());
                }
                Ok(tool_result)
            }
            Err(error) => {
                let message = Self::raw_transport_error(&error.to_string());
                state.session = None;
                state.tools.clear();
                state.last_error = Some(message.clone());
                Err(message)
            }
        }
    }

    async fn undo_last_action(&self, mode: AppMode) -> Result<(), String> {
        let payload = macros::undo();
        let name = payload
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "Undo payload missing tool name".to_string())?;
        let arguments = payload
            .get("arguments")
            .cloned()
            .ok_or_else(|| "Undo payload missing arguments".to_string())?;
        let result = self.execute_tool(mode, name, arguments).await?;
        if result.ok {
            Ok(())
        } else {
            Err(result.summary)
        }
    }

    async fn disconnect_if_needed(&self, _mode: AppMode) -> Result<(), String> {
        let mut state = self.state.lock().await;
        state.session = None;
        if let Some(child) = state.runtime_child.take() {
            terminate_child(child);
        }
        state.tools.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::GimpProvider;
    use crate::modes::provider::ToolProvider;
    use serde_json::json;
    use smolpc_assistant_types::AppMode;
    use smolpc_mcp_client::TcpTransportConfig;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn disconnected_state_is_honest_when_server_is_unreachable() {
        let provider = GimpProvider::with_config(
            TcpTransportConfig {
                host: "127.0.0.1".to_string(),
                port: 1,
            },
            None,
            None,
        );

        let state = provider.status(AppMode::Gimp).await.expect("status");
        assert_eq!(state.state, "disconnected");
        assert!(state.detail.expect("detail").contains("Unable to reach"));
    }

    #[tokio::test]
    async fn connected_state_includes_tool_definitions() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let port = listener.local_addr().expect("listener addr").port();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            reader.read_line(&mut line).await.expect("read initialize");
            let initialize: serde_json::Value =
                serde_json::from_str(line.trim_end()).expect("init json");
            writer
                .write_all(
                    format!(
                        "{}\n",
                        json!({
                            "jsonrpc": "2.0",
                            "id": initialize["id"],
                            "result": { "serverInfo": { "name": "gimp-mcp", "version": "test" } }
                        })
                    )
                    .as_bytes(),
                )
                .await
                .expect("write init response");

            line.clear();
            reader.read_line(&mut line).await.expect("read initialized");

            line.clear();
            reader.read_line(&mut line).await.expect("read tools/list");
            let request: serde_json::Value =
                serde_json::from_str(line.trim_end()).expect("tools list request");
            writer
                .write_all(
                    format!(
                        "{}\n",
                        json!({
                            "jsonrpc": "2.0",
                            "id": request["id"],
                            "result": {
                                "tools": [
                                    {
                                        "name": "get_image_metadata",
                                        "description": "Current image metadata",
                                        "inputSchema": { "type": "object" }
                                    }
                                ]
                            }
                        })
                    )
                    .as_bytes(),
                )
                .await
                .expect("write tools/list response");
        });

        let provider = GimpProvider::with_config(
            TcpTransportConfig {
                host: "127.0.0.1".to_string(),
                port,
            },
            None,
            None,
        );

        let state = provider.status(AppMode::Gimp).await.expect("status");
        assert_eq!(state.state, "connected");

        let tools = provider.list_tools(AppMode::Gimp).await.expect("tools");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "get_image_metadata");

        server.await.expect("server task");
    }

    #[tokio::test]
    async fn self_contained_status_reports_missing_gimp_when_host_is_not_detected() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let provider = GimpProvider::with_host_override(
            TcpTransportConfig {
                host: "127.0.0.1".to_string(),
                port: 10008,
            },
            Some(resource_temp.path().to_path_buf()),
            Some(app_temp.path().to_path_buf()),
            Some(PathBuf::from("/missing/gimp-3.exe")),
        );

        let state = provider.status(AppMode::Gimp).await.expect("status");
        assert_eq!(state.state, "disconnected");
        assert!(state
            .detail
            .expect("detail")
            .contains("Expected /missing/gimp-3.exe"));
    }

    #[tokio::test]
    async fn self_contained_status_rejects_gimp_2_installs() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let host_temp = TempDir::new().expect("host temp");
        let gimp_path = host_temp.path().join("gimp-2.10.exe");
        std::fs::write(&gimp_path, "gimp").expect("fake gimp");

        let provider = GimpProvider::with_host_override(
            TcpTransportConfig {
                host: "127.0.0.1".to_string(),
                port: 10008,
            },
            Some(resource_temp.path().to_path_buf()),
            Some(app_temp.path().to_path_buf()),
            Some(gimp_path),
        );

        let state = provider.status(AppMode::Gimp).await.expect("status");
        assert_eq!(state.state, "error");
        assert!(state.detail.expect("detail").contains("GIMP 3.x only"));
    }
}
