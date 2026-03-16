use super::state::{SceneCache, SceneData};
use axum::extract::{DefaultBodyLimit, State as AxumState};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::routing::{get, post};
use axum::{Json, Router};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
#[cfg(test)]
use std::net::SocketAddr;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};

const BRIDGE_BODY_LIMIT_BYTES: usize = 1024 * 1024;

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
    scene_cache: Arc<RwLock<SceneCache>>,
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
    #[cfg(test)]
    local_addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: tauri::async_runtime::JoinHandle<()>,
}

impl SceneBridgeHandle {
    #[cfg(test)]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

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
        .ok_or_else(|| {
            "Unable to resolve a local data directory for the Blender bridge token".to_string()
        })
}

fn write_bridge_token(token: &str) -> Result<(), String> {
    let directory = bridge_token_dir()?;
    write_bridge_token_in_dir(&directory, token)
}

fn write_bridge_token_in_dir(directory: &Path, token: &str) -> Result<(), String> {
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("Failed to create bridge token dir: {error}"))?;
    let token_path = directory.join("bridge-token.txt");

    #[cfg(unix)]
    {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&token_path)
            .map_err(|error| format!("Failed to write bridge token: {error}"))?;
        use std::io::Write;
        file.write_all(token.as_bytes())
            .and_then(|()| file.flush())
            .map_err(|error| format!("Failed to write bridge token: {error}"))?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(&token_path, token)
            .map_err(|error| format!("Failed to write bridge token: {error}"))?;
    }

    Ok(())
}

fn tokens_match(expected: &str, provided: &str) -> bool {
    let expected = expected.as_bytes();
    let provided = provided.as_bytes();
    let max_len = expected.len().max(provided.len());
    let mut diff = expected.len() ^ provided.len();

    for index in 0..max_len {
        let left = expected.get(index).copied().unwrap_or_default();
        let right = provided.get(index).copied().unwrap_or_default();
        diff |= usize::from(left ^ right);
    }

    diff == 0
}

async fn auth_middleware(
    AxumState(state): AxumState<BridgeState>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    // Keep /health unauthenticated so the addon can verify bridge reachability
    // before it has loaded the token file.
    if request.uri().path() == "/health" {
        return Ok(next.run(request).await);
    }

    let auth_header = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let token = auth_header.strip_prefix("Bearer ").unwrap_or("");

    if tokens_match(&state.bridge_token, token) {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn build_router(state: BridgeState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/scene/update", post(scene_update_handler))
        .route("/scene/current", get(scene_current_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(DefaultBodyLimit::max(BRIDGE_BODY_LIMIT_BYTES))
        .with_state(state)
}

pub async fn start_scene_bridge(
    scene_cache: Arc<RwLock<SceneCache>>,
    config: &BridgeConfig,
) -> Result<SceneBridgeHandle, String> {
    let bridge_token = generate_bridge_token();
    write_bridge_token(&bridge_token)?;

    let state = BridgeState {
        scene_cache,
        bridge_token,
    };
    let app = build_router(state);

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
    #[cfg(test)]
    let local_addr = listener
        .local_addr()
        .map_err(|error| format!("Failed to resolve the Blender bridge address: {error}"))?;

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
        #[cfg(test)]
        local_addr,
        shutdown: Some(shutdown_tx),
        task,
    })
}

async fn health_handler(
    AxumState(state): AxumState<BridgeState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let snapshot = state.scene_cache.read().await.snapshot();

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
    state.scene_cache.write().await.update(request.scene_data);

    Ok(Json(json!({
        "status": "ok",
        "message": "Scene data updated"
    })))
}

async fn scene_current_handler(
    AxumState(state): AxumState<BridgeState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let snapshot = state.scene_cache.read().await.snapshot();

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
    use super::{
        build_router, start_scene_bridge, tokens_match, write_bridge_token_in_dir, BridgeConfig,
        BridgeState, BRIDGE_BODY_LIMIT_BYTES,
    };
    use crate::modes::blender::state::{shared_scene_cache, SceneData};
    use axum::body::{to_bytes, Body};
    use axum::http::{header, Request, StatusCode};
    use tempfile::tempdir;
    use tokio::net::TcpListener;
    use tower::util::ServiceExt;

    fn state_with_token(token: &str) -> BridgeState {
        BridgeState {
            scene_cache: shared_scene_cache(),
            bridge_token: token.to_string(),
        }
    }

    #[test]
    fn token_compare_requires_exact_match() {
        assert!(tokens_match("secret-token", "secret-token"));
        assert!(!tokens_match("secret-token", "wrong-token"));
        assert!(!tokens_match("secret-token", "secret-token-2"));
    }

    #[test]
    fn bridge_token_round_trips_on_disk() {
        let temp_dir = tempdir().expect("temp dir");
        let token = "bridge-secret-token";

        write_bridge_token_in_dir(temp_dir.path(), token).expect("write token");
        let token_path = temp_dir.path().join("bridge-token.txt");
        let written = std::fs::read_to_string(&token_path).expect("read token");

        assert_eq!(written, token);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = std::fs::metadata(&token_path)
                .expect("token metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[tokio::test]
    async fn health_route_is_public() {
        let response = build_router(state_with_token("bridge-token"))
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("health request"),
            )
            .await
            .expect("health response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn scene_current_requires_valid_bearer_token() {
        let response = build_router(state_with_token("bridge-token"))
            .oneshot(
                Request::builder()
                    .uri("/scene/current")
                    .body(Body::empty())
                    .expect("scene request"),
            )
            .await
            .expect("missing auth response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = build_router(state_with_token("bridge-token"))
            .oneshot(
                Request::builder()
                    .uri("/scene/current")
                    .header(header::AUTHORIZATION, "Bearer wrong-token")
                    .body(Body::empty())
                    .expect("scene request"),
            )
            .await
            .expect("wrong auth response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn scene_current_returns_snapshot_payload_with_valid_token() {
        let cache = shared_scene_cache();
        cache.write().await.update(SceneData {
            object_count: 1,
            active_object: Some("Cube".to_string()),
            mode: "OBJECT".to_string(),
            render_engine: Some("BLENDER_EEVEE".to_string()),
            objects: Vec::new(),
        });

        let response = build_router(BridgeState {
            scene_cache: cache,
            bridge_token: "bridge-token".to_string(),
        })
        .oneshot(
            Request::builder()
                .uri("/scene/current")
                .header(header::AUTHORIZATION, "Bearer bridge-token")
                .body(Body::empty())
                .expect("scene request"),
        )
        .await
        .expect("scene response");

        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("scene payload");
        let payload: serde_json::Value = serde_json::from_slice(&bytes).expect("scene json");
        assert_eq!(payload["scene_data"]["active_object"], "Cube");
    }

    #[tokio::test]
    async fn scene_update_mutates_shared_cache() {
        let cache = shared_scene_cache();
        let response = build_router(BridgeState {
            scene_cache: cache.clone(),
            bridge_token: "bridge-token".to_string(),
        })
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/scene/update")
                .header(header::AUTHORIZATION, "Bearer bridge-token")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "scene_data": {
                            "object_count": 2,
                            "active_object": "Suzanne",
                            "mode": "OBJECT",
                            "render_engine": "BLENDER_EEVEE",
                            "objects": []
                        }
                    })
                    .to_string(),
                ))
                .expect("update request"),
        )
        .await
        .expect("update response");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            cache
                .read()
                .await
                .snapshot()
                .scene_data
                .expect("scene data")
                .active_object,
            Some("Suzanne".to_string())
        );
    }

    #[tokio::test]
    async fn scene_update_rejects_oversized_bodies() {
        let response = build_router(state_with_token("bridge-token"))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/scene/update")
                    .header(header::AUTHORIZATION, "Bearer bridge-token")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(vec![b'x'; BRIDGE_BODY_LIMIT_BYTES + 1]))
                    .expect("oversized request"),
            )
            .await
            .expect("oversized response");

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

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

    #[tokio::test]
    async fn bridge_stop_shuts_down_background_task() {
        let mut handle = start_scene_bridge(
            shared_scene_cache(),
            &BridgeConfig {
                host: "127.0.0.1".to_string(),
                port: 0,
            },
        )
        .await
        .expect("bridge handle");

        let health_url = format!("http://{}/health", handle.local_addr());
        let response = reqwest::get(&health_url).await.expect("health request");
        assert_eq!(response.status(), StatusCode::OK);

        handle.stop();
        for _ in 0..20 {
            match reqwest::get(&health_url).await {
                Err(error) if error.is_connect() => return,
                Err(error) => panic!("unexpected shutdown error: {error}"),
                Ok(_) => tokio::time::sleep(std::time::Duration::from_millis(25)).await,
            }
        }

        panic!("bridge health endpoint still responded after shutdown");
    }
}
