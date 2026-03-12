use crate::state::{BackendState, SceneData, SceneSnapshot};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Deserialize)]
pub struct SceneUpdateRequest {
    pub scene_data: SceneData,
}

#[derive(Debug, Clone, Serialize)]
pub struct SceneUpdateResponse {
    pub status: String,
    pub message: String,
}

fn current_scene_snapshot(state: &BackendState) -> SceneSnapshot {
    match state.scene_cache.lock() {
        Ok(cache) => cache.snapshot(),
        Err(poisoned) => poisoned.into_inner().snapshot(),
    }
}

#[tauri::command]
pub async fn scene_current(state: State<'_, BackendState>) -> Result<SceneSnapshot, String> {
    Ok(current_scene_snapshot(&state))
}

#[tauri::command]
pub async fn scene_update(
    request: SceneUpdateRequest,
    state: State<'_, BackendState>,
) -> Result<SceneUpdateResponse, String> {
    match state.scene_cache.lock() {
        Ok(mut cache) => cache.update(request.scene_data),
        Err(poisoned) => poisoned.into_inner().update(request.scene_data),
    };

    Ok(SceneUpdateResponse {
        status: "ok".to_string(),
        message: "Scene data updated".to_string(),
    })
}

pub fn current_scene_for_bridge(state: &BackendState) -> SceneSnapshot {
    current_scene_snapshot(state)
}
