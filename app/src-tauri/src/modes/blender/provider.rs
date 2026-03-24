use super::bridge::{start_scene_bridge, BridgeConfig, SceneBridgeHandle};
use super::rag::{RagContext, RagIndex};
use super::state::{shared_scene_cache, SceneCache, SceneSnapshot};
use crate::assistant::MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION;
use crate::modes::provider::{provider_state, ToolProvider};
use crate::setup::blender::{ensure_blender_addon_prepared, BlenderAddonPrepareOutcome};
use crate::setup::host_apps::{detect_blender, HostAppDetection};
use crate::setup::launch::is_matching_blender_process_running;
use async_trait::async_trait;
use serde_json::json;
use smolpc_assistant_types::{
    AppMode, ProviderStateDto, ToolDefinitionDto, ToolExecutionResultDto,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex as AsyncMutex, RwLock};

#[derive(Debug)]
struct RuntimeState {
    bridge_handle: Option<SceneBridgeHandle>,
    tools: Vec<ToolDefinitionDto>,
    rag_index: RagIndex,
    last_error: Option<String>,
    bridge_attempted: bool,
    detected_blender_path: Option<PathBuf>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            bridge_handle: None,
            tools: Vec::new(),
            rag_index: RagIndex::disabled("Retrieval not loaded yet".to_string()),
            last_error: None,
            bridge_attempted: false,
            detected_blender_path: None,
        }
    }
}

#[derive(Debug)]
pub struct BlenderProvider {
    config: BridgeConfig,
    resource_dir: Option<PathBuf>,
    app_local_data_dir: Option<PathBuf>,
    scene_cache: Arc<RwLock<SceneCache>>,
    state: AsyncMutex<RuntimeState>,
    connect_lock: AsyncMutex<()>,
    host_blender_override: Option<PathBuf>,
}

impl Default for BlenderProvider {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl BlenderProvider {
    pub fn new(resource_dir: Option<PathBuf>, app_local_data_dir: Option<PathBuf>) -> Self {
        Self::with_config(BridgeConfig::default(), resource_dir, app_local_data_dir)
    }

    pub fn with_config(
        config: BridgeConfig,
        resource_dir: Option<PathBuf>,
        app_local_data_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            config,
            resource_dir,
            app_local_data_dir,
            scene_cache: shared_scene_cache(),
            state: AsyncMutex::new(RuntimeState::default()),
            connect_lock: AsyncMutex::new(()),
            host_blender_override: None,
        }
    }

    #[cfg(test)]
    pub fn with_host_override(
        config: BridgeConfig,
        resource_dir: Option<PathBuf>,
        app_local_data_dir: Option<PathBuf>,
        host_blender_override: Option<PathBuf>,
    ) -> Self {
        Self {
            config,
            resource_dir,
            app_local_data_dir,
            scene_cache: shared_scene_cache(),
            state: AsyncMutex::new(RuntimeState::default()),
            connect_lock: AsyncMutex::new(()),
            host_blender_override,
        }
    }

    fn default_tools() -> Vec<ToolDefinitionDto> {
        vec![
            ToolDefinitionDto {
                name: "scene_current".to_string(),
                description: "Return the current Blender scene snapshot from the bridge runtime."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            },
            ToolDefinitionDto {
                name: "retrieve_rag_context".to_string(),
                description: "Retrieve local Blender reference context for a workflow question."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" },
                        "nResults": { "type": "integer", "minimum": 1, "maximum": 10 }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }),
            },
        ]
    }

    fn bridge_connected_state(mode: AppMode, detail: Option<String>) -> ProviderStateDto {
        provider_state(mode, "connected", detail.as_deref(), true, false)
    }

    fn disconnected_state(mode: AppMode, state: &RuntimeState) -> ProviderStateDto {
        let status = if state.bridge_attempted && state.last_error.is_some() {
            "error"
        } else {
            "disconnected"
        };

        provider_state(mode, status, state.last_error.as_deref(), true, false)
    }

    fn host_detection(&self, cached: Option<&Path>) -> HostAppDetection {
        if let Some(path) = self.host_blender_override.as_deref() {
            let resolved = path.exists().then(|| path.to_path_buf());
            let detail = resolved
                .as_ref()
                .map(|value| format!("Blender detected at {}", value.display()))
                .or_else(|| {
                    Some(format!(
                        "Blender is not installed or could not be detected yet. Expected {}.",
                        path.display()
                    ))
                });

            return HostAppDetection {
                id: "host_blender",
                label: "Blender",
                path: resolved,
                detail,
            };
        }

        detect_blender(cached)
    }

    async fn current_snapshot(&self) -> SceneSnapshot {
        self.scene_cache.read().await.snapshot()
    }

    fn detail_for_snapshot(
        snapshot: &SceneSnapshot,
        rag_index: &RagIndex,
        prefix: Option<String>,
    ) -> Option<String> {
        let mut details = Vec::new();

        if let Some(prefix) = prefix {
            details.push(prefix);
        }

        if let Some(message) = snapshot.message.as_ref() {
            details.push(message.clone());
        } else if let Some(scene_data) = snapshot.scene_data.as_ref() {
            details.push(format!(
                "Live scene available with {} object(s); active object: {}.",
                scene_data.object_count,
                scene_data.active_object.as_deref().unwrap_or("None")
            ));
        }

        if let Some(rag_error) = rag_index.load_error() {
            details.push(format!(
                "Local Blender reference retrieval is unavailable: {rag_error}"
            ));
        } else if rag_index.is_loaded() {
            details.push(format!(
                "Loaded {} local Blender reference document(s).",
                rag_index.document_count()
            ));
        }

        if details.is_empty() {
            None
        } else {
            Some(details.join(" · "))
        }
    }

    fn missing_blender_detail(detection: &HostAppDetection) -> String {
        detection.detail.clone().unwrap_or_else(|| {
            "Blender is not installed or could not be detected yet. Install Blender to use scene-aware Blender mode."
                .to_string()
        })
    }

    fn provision_error_detail(error: &str) -> String {
        format!("Unable to provision the bundled Blender addon automatically. {error}")
    }

    fn connection_note(
        snapshot: &SceneSnapshot,
        blender_path: &Path,
        prepare_outcome: &BlenderAddonPrepareOutcome,
        blender_running: bool,
    ) -> Option<String> {
        if snapshot.connected {
            return match prepare_outcome {
                BlenderAddonPrepareOutcome::Prepared(_) => Some(format!(
                    "Bundled Blender addon was provisioned for {}.",
                    blender_path.display()
                )),
                BlenderAddonPrepareOutcome::AlreadyReady(_) => None,
            };
        }

        let mut notes = Vec::new();
        match prepare_outcome {
            BlenderAddonPrepareOutcome::Prepared(_) => notes.push(format!(
                "Bundled Blender addon was provisioned and enabled for {}.",
                blender_path.display()
            )),
            BlenderAddonPrepareOutcome::AlreadyReady(_) => notes.push(format!(
                "Bundled Blender addon is provisioned for {}.",
                blender_path.display()
            )),
        }

        if blender_running {
            notes.push(
                "Blender is already running. If this session started before the addon was provisioned, reopen Blender once so the addon can load."
                    .to_string(),
            );
        } else {
            notes.push(
                "Blender is not running yet. Use Open App to launch Blender when you want live scene tools."
                    .to_string(),
            );
        }

        if notes.is_empty() {
            None
        } else {
            Some(notes.join(" "))
        }
    }

    fn rag_dir_candidates(&self) -> Vec<PathBuf> {
        let mut candidates = Vec::new();

        if let Some(resource_dir) = self.resource_dir.as_ref() {
            candidates.push(
                resource_dir
                    .join("resources")
                    .join("blender")
                    .join("rag_system"),
            );
            candidates.push(resource_dir.join("blender").join("rag_system"));
        }

        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join("blender")
                .join("rag_system"),
        );

        candidates
    }

    fn load_rag_index(&self) -> RagIndex {
        for candidate in self.rag_dir_candidates() {
            if candidate.exists() {
                return RagIndex::load_from_dir(&candidate);
            }
        }

        RagIndex::disabled(
            "Blender retrieval metadata was not bundled with the unified app.".to_string(),
        )
    }

    async fn ensure_bridge_started(
        &self,
        mode: AppMode,
        state: &mut RuntimeState,
    ) -> Result<ProviderStateDto, String> {
        if state.bridge_handle.is_none() {
            state.bridge_attempted = true;
            state.rag_index = self.load_rag_index();
            match start_scene_bridge(Arc::clone(&self.scene_cache), &self.config).await {
                Ok(handle) => {
                    state.bridge_handle = Some(handle);
                    state.tools = Self::default_tools();
                    state.last_error = None;
                }
                Err(error) => {
                    state.tools.clear();
                    state.last_error = Some(error.clone());
                    return Err(error);
                }
            }
        }

        Ok(Self::bridge_connected_state(
            mode,
            Self::detail_for_snapshot(&self.current_snapshot().await, &state.rag_index, None),
        ))
    }

    async fn ensure_provider_ready(&self, mode: AppMode) -> ProviderStateDto {
        let _connect_guard = self.connect_lock.lock().await;

        let cached_blender_path = {
            let mut state = self.state.lock().await;
            if self.ensure_bridge_started(mode, &mut state).await.is_err() {
                return Self::disconnected_state(mode, &state);
            }
            state.detected_blender_path.clone()
        };

        let detection = self.host_detection(cached_blender_path.as_deref());
        let Some(blender_path) = detection.path.clone() else {
            let detail = Self::missing_blender_detail(&detection);
            let mut state = self.state.lock().await;
            state.detected_blender_path = None;
            state.last_error = Some(detail.clone());
            return provider_state(mode, "disconnected", Some(detail.as_str()), true, false);
        };

        let prepare_outcome = match ensure_blender_addon_prepared(
            self.resource_dir.as_deref(),
            self.app_local_data_dir.as_deref(),
            &blender_path,
        ) {
            Ok(outcome) => outcome,
            Err(error) => {
                let detail = Self::provision_error_detail(&error);
                let mut state = self.state.lock().await;
                state.detected_blender_path = Some(blender_path);
                state.last_error = Some(detail.clone());
                return provider_state(mode, "error", Some(detail.as_str()), true, false);
            }
        };

        let snapshot = self.current_snapshot().await;
        let blender_running = is_matching_blender_process_running(&blender_path);
        let rag_index = {
            let mut state = self.state.lock().await;
            state.detected_blender_path = Some(blender_path.clone());
            state.tools = Self::default_tools();
            state.last_error = None;
            state.rag_index.clone()
        };

        let note =
            Self::connection_note(&snapshot, &blender_path, &prepare_outcome, blender_running);
        Self::bridge_connected_state(mode, Self::detail_for_snapshot(&snapshot, &rag_index, note))
    }

    fn scene_tool_result(snapshot: SceneSnapshot) -> ToolExecutionResultDto {
        let summary = if let Some(scene) = snapshot.scene_data.as_ref() {
            format!(
                "Scene snapshot available with {} object(s); active object: {}.",
                scene.object_count,
                scene.active_object.as_deref().unwrap_or("None")
            )
        } else {
            snapshot
                .message
                .clone()
                .unwrap_or_else(|| "No live Blender scene data is available yet.".to_string())
        };

        ToolExecutionResultDto {
            name: "scene_current".to_string(),
            ok: true,
            summary,
            payload: serde_json::to_value(snapshot).unwrap_or_else(|_| json!({})),
        }
    }

    fn rag_tool_result(
        query: &str,
        contexts: Vec<RagContext>,
        rag_index: &RagIndex,
    ) -> ToolExecutionResultDto {
        let summary = if !contexts.is_empty() {
            format!("Retrieved {} Blender reference context(s).", contexts.len())
        } else if let Some(load_error) = rag_index.load_error() {
            format!("Blender reference retrieval is unavailable: {load_error}")
        } else {
            format!("No Blender reference matches were found for \"{query}\".")
        };

        ToolExecutionResultDto {
            name: "retrieve_rag_context".to_string(),
            ok: true,
            summary,
            payload: json!({
                "contexts": contexts,
                "ragEnabled": rag_index.is_loaded(),
                "loadError": rag_index.load_error(),
            }),
        }
    }
}

#[async_trait]
impl ToolProvider for BlenderProvider {
    async fn connect_if_needed(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
        Ok(self.ensure_provider_ready(mode).await)
    }

    async fn status(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
        Ok(self.ensure_provider_ready(mode).await)
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
        {
            let mut state = self.state.lock().await;
            self.ensure_bridge_started(mode, &mut state).await?;
        }

        let mut state = self.state.lock().await;
        match name {
            "scene_current" => {
                let result = Self::scene_tool_result(self.current_snapshot().await);
                state.last_error = None;
                Ok(result)
            }
            "retrieve_rag_context" => {
                let query = arguments
                    .get("query")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|query| !query.is_empty())
                    .ok_or_else(|| "retrieve_rag_context requires a non-empty query".to_string())?;
                let n_results = arguments
                    .get("nResults")
                    .and_then(serde_json::Value::as_u64)
                    .map(|value| value as usize)
                    .unwrap_or(3);

                let contexts = state.rag_index.retrieve_context(query, n_results)?;
                state.last_error = None;
                Ok(Self::rag_tool_result(query, contexts, &state.rag_index))
            }
            _ => Err(format!("Unsupported Blender provider tool: {name}")),
        }
    }

    async fn undo_last_action(&self, _mode: AppMode) -> Result<(), String> {
        Err(MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION.to_string())
    }

    async fn disconnect_if_needed(&self, _mode: AppMode) -> Result<(), String> {
        let mut state = self.state.lock().await;
        if let Some(mut handle) = state.bridge_handle.take() {
            handle.stop();
        }
        state.tools.clear();
        state.rag_index = self.load_rag_index();
        state.last_error = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::BlenderProvider;
    use crate::modes::blender::bridge::BridgeConfig;
    use crate::modes::blender::state::SceneData;
    use crate::modes::provider::ToolProvider;
    use serde_json::json;
    use smolpc_assistant_types::AppMode;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex as StdMutex, OnceLock};
    use tempfile::{tempdir, TempDir};
    use tokio::net::TcpListener;
    use tokio::time::{sleep, Duration};

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn write_blender_resources(root: &Path, metadata_contents: Option<&str>) {
        let blender_root = root.join("blender");
        std::fs::create_dir_all(blender_root.join("addon")).expect("addon dir");
        std::fs::create_dir_all(blender_root.join("rag_system").join("simple_db"))
            .expect("rag dir");
        std::fs::write(blender_root.join("README.md"), "phase4 blender resources").expect("readme");
        std::fs::write(
            blender_root.join("addon").join("blender_helper_http.py"),
            "# blender addon snapshot\n",
        )
        .expect("addon");
        std::fs::write(
            blender_root.join("manifest.json"),
            r#"{
              "version": "phase4-blender",
              "source": "tests",
              "expectedPaths": ["README.md", "rag_system", "addon/blender_helper_http.py"],
              "status": "tracked"
            }"#,
        )
        .expect("manifest");

        if let Some(metadata_contents) = metadata_contents {
            std::fs::write(
                blender_root
                    .join("rag_system")
                    .join("simple_db")
                    .join("metadata.json"),
                metadata_contents,
            )
            .expect("metadata");
        }
    }

    #[cfg(unix)]
    fn write_fake_blender_script(root: &Path, addon_dir: &Path, log_path: &Path) -> PathBuf {
        let blender_path = root.join("fake-blender");
        let script = format!(
            r#"#!/bin/sh
ARGS="$*"
TOKEN_PATH="${{LOCALAPPDATA}}/SmolPC/engine-runtime/bridge-token.txt"
if printf '%s' "$ARGS" | grep -q "smolpc_blender_addon_dir_probe"; then
  if [ -f "$TOKEN_PATH" ]; then printf 'token_present\n' >> "{log_path}"; else printf 'token_missing\n' >> "{log_path}"; fi
  echo '{{"status":"ok","addonDir":"{addon_dir}"}}'
  exit 0
fi
if printf '%s' "$ARGS" | grep -q "smolpc_blender_addon_enable"; then
  if [ -f "$TOKEN_PATH" ]; then printf 'token_present\n' >> "{log_path}"; else printf 'token_missing\n' >> "{log_path}"; fi
  printf 'enabled\n' >> "{log_path}"
  echo '{{"status":"ok","loaded":true,"module":"blender_helper_http"}}'
  exit 0
fi
printf 'launched\n' >> "{log_path}"
exit 0
"#,
            addon_dir = addon_dir.display(),
            log_path = log_path.display(),
        );
        std::fs::write(&blender_path, script).expect("script");
        let mut permissions = std::fs::metadata(&blender_path)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&blender_path, permissions).expect("chmod");
        blender_path
    }

    fn env_lock() -> &'static StdMutex<()> {
        static LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| StdMutex::new(()))
    }

    #[cfg(unix)]
    fn provider_with_fake_blender(
        resource_temp: &TempDir,
        app_temp: &TempDir,
        blender_path: PathBuf,
    ) -> BlenderProvider {
        BlenderProvider::with_host_override(
            BridgeConfig {
                host: "127.0.0.1".to_string(),
                port: 0,
            },
            Some(resource_temp.path().to_path_buf()),
            Some(app_temp.path().to_path_buf()),
            Some(blender_path),
        )
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn connected_state_includes_tool_definitions_when_bridge_starts() {
        let _env_guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let host_temp = TempDir::new().expect("host temp");
        write_blender_resources(resource_temp.path(), None);
        let log_path = host_temp.path().join("fake-blender.log");
        let blender_path = write_fake_blender_script(
            host_temp.path(),
            &host_temp.path().join("addons"),
            &log_path,
        );
        let previous_local_app_data = std::env::var_os("LOCALAPPDATA");
        std::env::set_var("LOCALAPPDATA", host_temp.path());

        let provider = provider_with_fake_blender(&resource_temp, &app_temp, blender_path);

        let state = provider.status(AppMode::Blender).await.expect("status");
        sleep(Duration::from_millis(75)).await;
        match previous_local_app_data {
            Some(value) => std::env::set_var("LOCALAPPDATA", value),
            None => std::env::remove_var("LOCALAPPDATA"),
        }

        assert_eq!(state.state, "connected");

        let tools = provider.list_tools(AppMode::Blender).await.expect("tools");
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "scene_current");
        assert!(state
            .detail
            .expect("detail")
            .contains("Bundled Blender addon"));
    }

    #[tokio::test]
    async fn status_returns_friendly_error_when_port_is_occupied() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind occupied port");
        let port = listener.local_addr().expect("local addr").port();
        let provider = BlenderProvider::with_config(
            BridgeConfig {
                host: "127.0.0.1".to_string(),
                port,
            },
            None,
            None,
        );

        let state = provider.status(AppMode::Blender).await.expect("status");
        assert_eq!(state.state, "error");
        assert!(state.detail.expect("detail").contains("already in use"));
    }

    #[tokio::test]
    async fn scene_current_tool_returns_snapshot_payload() {
        let provider = BlenderProvider::with_config(
            BridgeConfig {
                host: "127.0.0.1".to_string(),
                port: 0,
            },
            None,
            None,
        );

        let result = provider
            .execute_tool(AppMode::Blender, "scene_current", json!({}))
            .await
            .expect("scene_current result");

        assert!(result.ok);
        assert_eq!(result.name, "scene_current");
        assert!(result.payload.get("connected").is_some());
    }

    #[tokio::test]
    async fn retrieve_rag_context_returns_results_when_metadata_is_loaded() {
        let temp_dir = tempdir().expect("temp dir");
        write_blender_resources(
            temp_dir.path(),
            Some(
                &serde_json::to_string(&vec![json!({
                    "text": "Use the bevel modifier to soften edges",
                    "signature": "bpy.types.BevelModifier",
                    "url": "/bpy.types.BevelModifier.html"
                })])
                .expect("metadata json"),
            ),
        );

        let provider = BlenderProvider::with_config(
            BridgeConfig {
                host: "127.0.0.1".to_string(),
                port: 0,
            },
            Some(temp_dir.path().to_path_buf()),
            None,
        );

        let result = provider
            .execute_tool(
                AppMode::Blender,
                "retrieve_rag_context",
                json!({ "query": "How do I add a bevel modifier?" }),
            )
            .await
            .expect("retrieve result");

        assert!(result.ok);
        assert_eq!(result.payload["contexts"].as_array().map(Vec::len), Some(1));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn retrieval_load_failure_degrades_gracefully() {
        let _env_guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let host_temp = TempDir::new().expect("host temp");
        write_blender_resources(resource_temp.path(), Some("not valid json"));
        let log_path = host_temp.path().join("fake-blender.log");
        let blender_path = write_fake_blender_script(
            host_temp.path(),
            &host_temp.path().join("addons"),
            &log_path,
        );
        let previous_local_app_data = std::env::var_os("LOCALAPPDATA");
        std::env::set_var("LOCALAPPDATA", host_temp.path());

        let provider = provider_with_fake_blender(&resource_temp, &app_temp, blender_path);

        let state = provider.status(AppMode::Blender).await.expect("status");
        sleep(Duration::from_millis(75)).await;
        match previous_local_app_data {
            Some(value) => std::env::set_var("LOCALAPPDATA", value),
            None => std::env::remove_var("LOCALAPPDATA"),
        }
        assert_eq!(state.state, "connected");
        assert!(state
            .detail
            .expect("detail")
            .contains("retrieval is unavailable"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn status_reports_live_scene_detail_when_scene_cache_has_data() {
        let _env_guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let host_temp = TempDir::new().expect("host temp");
        write_blender_resources(resource_temp.path(), None);
        let log_path = host_temp.path().join("fake-blender.log");
        let blender_path = write_fake_blender_script(
            host_temp.path(),
            &host_temp.path().join("addons"),
            &log_path,
        );
        let previous_local_app_data = std::env::var_os("LOCALAPPDATA");
        std::env::set_var("LOCALAPPDATA", host_temp.path());

        let provider = provider_with_fake_blender(&resource_temp, &app_temp, blender_path);

        provider
            .status(AppMode::Blender)
            .await
            .expect("initial status");
        {
            let mut cache = provider.scene_cache.write().await;
            cache.update(SceneData {
                object_count: 2,
                active_object: Some("Cube".to_string()),
                mode: "OBJECT".to_string(),
                render_engine: Some("BLENDER_EEVEE".to_string()),
                objects: Vec::new(),
            });
        }

        let state = provider.status(AppMode::Blender).await.expect("status");
        sleep(Duration::from_millis(75)).await;
        match previous_local_app_data {
            Some(value) => std::env::set_var("LOCALAPPDATA", value),
            None => std::env::remove_var("LOCALAPPDATA"),
        }
        let detail = state.detail.expect("detail");
        assert!(detail.contains("Live scene available"));
        assert!(detail.contains("active object: Cube"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn reconnect_restores_tool_definitions_after_disconnect() {
        let _env_guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let host_temp = TempDir::new().expect("host temp");
        write_blender_resources(resource_temp.path(), None);
        let log_path = host_temp.path().join("fake-blender.log");
        let blender_path = write_fake_blender_script(
            host_temp.path(),
            &host_temp.path().join("addons"),
            &log_path,
        );
        let previous_local_app_data = std::env::var_os("LOCALAPPDATA");
        std::env::set_var("LOCALAPPDATA", host_temp.path());

        let provider = provider_with_fake_blender(&resource_temp, &app_temp, blender_path);

        provider.status(AppMode::Blender).await.expect("status");
        sleep(Duration::from_millis(75)).await;
        assert_eq!(
            provider
                .list_tools(AppMode::Blender)
                .await
                .expect("tools")
                .len(),
            2
        );

        provider
            .disconnect_if_needed(AppMode::Blender)
            .await
            .expect("disconnect");
        assert!(provider
            .list_tools(AppMode::Blender)
            .await
            .expect("tools after disconnect")
            .is_empty());

        provider
            .connect_if_needed(AppMode::Blender)
            .await
            .expect("reconnect");
        assert_eq!(
            provider
                .list_tools(AppMode::Blender)
                .await
                .expect("tools after reconnect")
                .len(),
            2
        );
        sleep(Duration::from_millis(75)).await;

        match previous_local_app_data {
            Some(value) => std::env::set_var("LOCALAPPDATA", value),
            None => std::env::remove_var("LOCALAPPDATA"),
        }
    }

    #[tokio::test]
    async fn status_reports_missing_when_blender_host_is_unavailable() {
        let provider = BlenderProvider::with_host_override(
            BridgeConfig {
                host: "127.0.0.1".to_string(),
                port: 0,
            },
            None,
            None,
            Some(PathBuf::from("/definitely/missing/blender")),
        );

        let state = provider.status(AppMode::Blender).await.expect("status");
        assert_eq!(state.state, "disconnected");
        assert!(state
            .detail
            .expect("detail")
            .contains("could not be detected"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn provider_starts_bridge_before_provisioning_the_addon() {
        let _env_guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let host_temp = TempDir::new().expect("host temp");
        write_blender_resources(resource_temp.path(), None);
        let log_path = host_temp.path().join("fake-blender.log");
        let blender_path = write_fake_blender_script(
            host_temp.path(),
            &host_temp.path().join("addons"),
            &log_path,
        );
        let previous_local_app_data = std::env::var_os("LOCALAPPDATA");
        std::env::set_var("LOCALAPPDATA", host_temp.path());

        let provider = provider_with_fake_blender(&resource_temp, &app_temp, blender_path);
        let _ = provider.status(AppMode::Blender).await.expect("status");
        sleep(Duration::from_millis(75)).await;

        match previous_local_app_data {
            Some(value) => std::env::set_var("LOCALAPPDATA", value),
            None => std::env::remove_var("LOCALAPPDATA"),
        }

        let log = std::fs::read_to_string(&log_path).expect("log");
        assert!(log.contains("token_present"));
    }
}
