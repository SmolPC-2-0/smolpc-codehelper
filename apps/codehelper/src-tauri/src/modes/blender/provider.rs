use super::bridge::{start_scene_bridge, BridgeConfig, SceneBridgeHandle};
use super::rag::{RagContext, RagIndex};
use super::state::{shared_scene_cache, SceneCache, SceneSnapshot};
use crate::assistant::MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION;
use crate::modes::provider::{provider_state, ToolProvider};
use async_trait::async_trait;
use serde_json::json;
use smolpc_assistant_types::{
    AppMode, ProviderStateDto, ToolDefinitionDto, ToolExecutionResultDto,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;

#[derive(Debug)]
struct RuntimeState {
    bridge_handle: Option<SceneBridgeHandle>,
    tools: Vec<ToolDefinitionDto>,
    rag_index: RagIndex,
    last_error: Option<String>,
    bridge_attempted: bool,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            bridge_handle: None,
            tools: Vec::new(),
            rag_index: RagIndex::disabled("Retrieval not loaded yet".to_string()),
            last_error: None,
            bridge_attempted: false,
        }
    }
}

#[derive(Debug)]
pub struct BlenderProvider {
    config: BridgeConfig,
    resource_dir: Option<PathBuf>,
    scene_cache: Arc<Mutex<SceneCache>>,
    state: AsyncMutex<RuntimeState>,
}

impl Default for BlenderProvider {
    fn default() -> Self {
        Self::new(None)
    }
}

impl BlenderProvider {
    pub fn new(resource_dir: Option<PathBuf>) -> Self {
        Self::with_config(BridgeConfig::default(), resource_dir)
    }

    pub fn with_config(config: BridgeConfig, resource_dir: Option<PathBuf>) -> Self {
        Self {
            config,
            resource_dir,
            scene_cache: shared_scene_cache(),
            state: AsyncMutex::new(RuntimeState::default()),
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
                description:
                    "Retrieve local Blender reference context for a workflow question."
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

    fn current_snapshot(&self) -> SceneSnapshot {
        match self.scene_cache.lock() {
            Ok(cache) => cache.snapshot(),
            Err(poisoned) => poisoned.into_inner().snapshot(),
        }
    }

    fn detail_for_connected_state(&self, rag_index: &RagIndex) -> Option<String> {
        let snapshot = self.current_snapshot();
        let mut details = Vec::new();

        if let Some(message) = snapshot.message {
            details.push(message);
        } else if let Some(scene_data) = snapshot.scene_data {
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

    fn rag_dir_candidates(&self) -> Vec<PathBuf> {
        let mut candidates = Vec::new();

        if let Some(resource_dir) = self.resource_dir.as_ref() {
            candidates.push(resource_dir.join("resources").join("blender").join("rag_system"));
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
            self.detail_for_connected_state(&state.rag_index),
        ))
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
        let mut state = self.state.lock().await;
        self.ensure_bridge_started(mode, &mut state).await
    }

    async fn status(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
        let mut state = self.state.lock().await;
        match self.ensure_bridge_started(mode, &mut state).await {
            Ok(provider_state) => Ok(provider_state),
            Err(_) => Ok(Self::disconnected_state(mode, &state)),
        }
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
                let result = Self::scene_tool_result(self.current_snapshot());
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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::BlenderProvider;
    use crate::modes::blender::bridge::BridgeConfig;
    use crate::modes::provider::ToolProvider;
    use serde_json::json;
    use smolpc_assistant_types::AppMode;
    use tempfile::tempdir;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn connected_state_includes_tool_definitions_when_bridge_starts() {
        let provider = BlenderProvider::with_config(
            BridgeConfig {
                host: "127.0.0.1".to_string(),
                port: 0,
            },
            None,
        );

        let state = provider.status(AppMode::Blender).await.expect("status");
        assert_eq!(state.state, "connected");

        let tools = provider.list_tools(AppMode::Blender).await.expect("tools");
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "scene_current");
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
        let rag_dir = temp_dir.path().join("resources").join("blender").join("rag_system").join("simple_db");
        std::fs::create_dir_all(&rag_dir).expect("rag dir");
        std::fs::write(
            rag_dir.join("metadata.json"),
            serde_json::to_string(&vec![json!({
                "text": "Use the bevel modifier to soften edges",
                "signature": "bpy.types.BevelModifier",
                "url": "/bpy.types.BevelModifier.html"
            })])
            .expect("metadata json"),
        )
        .expect("write metadata");

        let provider = BlenderProvider::with_config(
            BridgeConfig {
                host: "127.0.0.1".to_string(),
                port: 0,
            },
            Some(temp_dir.path().to_path_buf()),
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

    #[tokio::test]
    async fn retrieval_load_failure_degrades_gracefully() {
        let temp_dir = tempdir().expect("temp dir");
        let rag_dir = temp_dir
            .path()
            .join("resources")
            .join("blender")
            .join("rag_system")
            .join("simple_db");
        std::fs::create_dir_all(&rag_dir).expect("rag dir");
        std::fs::write(rag_dir.join("metadata.json"), "not valid json").expect("write metadata");

        let provider = BlenderProvider::with_config(
            BridgeConfig {
                host: "127.0.0.1".to_string(),
                port: 0,
            },
            Some(temp_dir.path().to_path_buf()),
        );

        let state = provider.status(AppMode::Blender).await.expect("status");
        assert_eq!(state.state, "connected");
        assert!(state.detail.expect("detail").contains("retrieval is unavailable"));
    }
}
