use super::state::{SceneCache, SceneData};
use axum::extract::State as AxumState;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::routing::{get, post};
use axum::{Json, Router};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

#[derive(Debug, Clone)]
pub struct BridgeConfig {
    pub host: String,
    pub port: u16,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 5179,
        }
    }
}

#[derive(Clone)]
struct BridgeState {
    scene_cache: Arc<Mutex<SceneCache>>,
    bridge_token: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Deserialize)]
struct SceneUpdateRequest {
    scene_data: SceneData,
}

#[derive(Debug)]
pub struct SceneBridgeHandle {
    shutdown: Option<oneshot::Sender<()>>,
    task: tauri::async_runtime::JoinHandle<()>,
}

impl SceneBridgeHandle {
    pub fn stop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.abort();
    }
}

impl Drop for SceneBridgeHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

fn generate_bridge_token() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .map(char::from)
        .take(48)
        .collect()
}

fn bridge_token_dir() -> Result<PathBuf, String> {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        return Ok(PathBuf::from(local_app_data)
            .join("SmolPC")
            .join("engine-runtime"));
    }

    dirs::data_local_dir()
        .map(|path| path.join("SmolPC").join("engine-runtime"))
        .ok_or_else(|| "Unable to resolve a local data directory for the Blender bridge token".to_string())
}

fn write_bridge_token(token: &str) -> Result<(), String> {
    let directory = bridge_token_dir()?;
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Failed to create bridge token dir: {error}"))?;
    std::fs::write(directory.join("bridge-token.txt"), token)
        .map_err(|error| format!("Failed to write bridge token: {error}"))?;
    Ok(())
}

async fn auth_middleware(
    AxumState(state): AxumState<BridgeState>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    if request.uri().path() == "/health" {
        return Ok(next.run(request).await);
    }

    let auth_header = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let token = auth_header.strip_prefix("Bearer ").unwrap_or("");

    if token == state.bridge_token {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

pub async fn start_scene_bridge(
    scene_cache: Arc<Mutex<SceneCache>>,
    config: &BridgeConfig,
) -> Result<SceneBridgeHandle, String> {
    let bridge_token = generate_bridge_token();
    write_bridge_token(&bridge_token)?;

    let state = BridgeState {
        scene_cache,
        bridge_token,
    };
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/scene/update", post(scene_update_handler))
        .route("/scene/current", get(scene_current_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);

    let bind_address = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&bind_address)
        .await
        .map_err(|error| {
            let text = error.to_string();
            if text.contains("address already in use") || text.contains("Address already in use") {
                format!(
                    "Unable to start the Blender bridge because port {} is already in use. Close the conflicting process and refresh Blender mode.",
                    config.port
                )
            } else {
                format!("Failed to bind the Blender bridge on {bind_address}: {error}")
            }
        })?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let task = tauri::async_runtime::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        });

        if let Err(error) = server.await {
            log::error!("[BlenderBridge] Server error: {error}");
        }
    });

    Ok(SceneBridgeHandle {
        shutdown: Some(shutdown_tx),
        task,
    })
}

async fn health_handler(
    AxumState(state): AxumState<BridgeState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let snapshot = match state.scene_cache.lock() {
        Ok(cache) => cache.snapshot(),
        Err(poisoned) => poisoned.into_inner().snapshot(),
    };

    Ok(Json(json!({
        "status": "ok",
        "sceneConnected": snapshot.connected,
        "sceneMessage": snapshot.message,
        "lastUpdate": snapshot.last_update,
    })))
}

async fn scene_update_handler(
    AxumState(state): AxumState<BridgeState>,
    Json(request): Json<SceneUpdateRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match state.scene_cache.lock() {
        Ok(mut cache) => cache.update(request.scene_data),
        Err(poisoned) => poisoned.into_inner().update(request.scene_data),
    }

    Ok(Json(json!({
        "status": "ok",
        "message": "Scene data updated"
    })))
}

async fn scene_current_handler(
    AxumState(state): AxumState<BridgeState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let snapshot = match state.scene_cache.lock() {
        Ok(cache) => cache.snapshot(),
        Err(poisoned) => poisoned.into_inner().snapshot(),
    };

    serde_json::to_value(snapshot)
        .map(Json)
        .map_err(internal_error)
}

fn internal_error(error: impl std::fmt::Display) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::{start_scene_bridge, BridgeConfig};
    use crate::modes::blender::state::shared_scene_cache;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn bridge_returns_friendly_port_conflict_error() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind occupied port");
        let port = listener.local_addr().expect("local addr").port();

        let error = start_scene_bridge(
            shared_scene_cache(),
            &BridgeConfig {
                host: "127.0.0.1".to_string(),
                port,
            },
        )
        .await
        .expect_err("port conflict");

        assert!(error.contains("already in use"));
    }
}
