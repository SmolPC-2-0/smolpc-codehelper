use axum::extract::State as AxumState;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::routing::{get, post};
use axum::{Json, Router};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::oneshot;

use crate::commands::assistant::{
    assistant_status_internal, ask_internal, analyze_scene_internal, retrieve_contexts, AskRequest,
    SceneAnalysisRequest,
};
use crate::commands::generation::{
    get_generation_backend_internal, set_generation_backend_internal, GenerationState,
};
use crate::commands::scene::current_scene_for_bridge;
use crate::state::{BackendState, SceneData};

#[derive(Clone)]
struct BridgeState {
    backend: BackendState,
    generation: GenerationState,
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

#[derive(Debug, Deserialize)]
struct RagRetrieveRequest {
    query: String,
    n_results: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct SceneAnalysisBridgeRequest {
    goal: Option<String>,
    #[serde(default)]
    scene_context: Option<SceneData>,
    #[serde(default)]
    scene_data: Option<SceneData>,
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BackendSetRequest {
    backend: String,
}

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

fn generate_bridge_token() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .map(char::from)
        .take(48)
        .collect()
}

fn write_bridge_token(token: &str) -> Result<(), String> {
    let local_app_data = std::env::var("LOCALAPPDATA").map_err(|_| {
        "LOCALAPPDATA is not set; cannot write bridge token".to_string()
    })?;
    let dir = std::path::PathBuf::from(local_app_data)
        .join("SmolPC")
        .join("engine-runtime");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create bridge token dir: {}", e))?;
    std::fs::write(dir.join("bridge-token.txt"), token)
        .map_err(|e| format!("Failed to write bridge token: {}", e))?;
    Ok(())
}

async fn auth_middleware(
    AxumState(state): AxumState<BridgeState>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    // Allow health and test endpoints without auth
    let path = request.uri().path();
    if path == "/health" || path == "/test" {
        return Ok(next.run(request).await);
    }

    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = auth_header.strip_prefix("Bearer ").unwrap_or("");
    if token == state.bridge_token {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

pub async fn start_scene_bridge(
    backend: BackendState,
    generation: GenerationState,
) -> Result<SceneBridgeHandle, String> {
    let bridge_token = generate_bridge_token();
    write_bridge_token(&bridge_token)?;

    let state = BridgeState {
        backend,
        generation,
        bridge_token,
    };
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/scene/update", post(scene_update_handler))
        .route("/scene/current", get(scene_current_handler))
        .route("/rag/retrieve", post(rag_retrieve_handler))
        .route("/ask", post(ask_handler))
        .route("/scene_analysis", post(scene_analysis_handler))
        .route("/backend/get", get(backend_get_handler))
        .route("/backend/set", post(backend_set_handler))
        .route("/test", get(test_handler))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:5179")
        .await
        .map_err(|e| {
            if e.to_string().contains("address already in use") || e.to_string().contains("Address already in use") {
                "Port 5179 is already in use. Another instance of Blender Learning Assistant may be running. Close it and try again.".to_string()
            } else {
                format!("Failed to bind scene bridge on 127.0.0.1:5179: {}", e)
            }
        })?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let task = tauri::async_runtime::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        });

        if let Err(e) = server.await {
            log::error!("[SceneBridge] Server error: {}", e);
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
    let status = assistant_status_internal(&state.backend, state.generation.is_generating()).await;
    Ok(Json(json!({
        "status": if status.connected { "ok" } else { "error" },
        "connected": status.connected,
        "backend": status.backend,
        "model": status.model,
        "generating": status.generating,
        "rag_enabled": status.rag_enabled,
        "rag_docs": status.rag_docs,
        "rag_error": status.rag_error
    })))
}

async fn scene_update_handler(
    AxumState(state): AxumState<BridgeState>,
    Json(request): Json<SceneUpdateRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match state.backend.scene_cache.lock() {
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
    let snapshot = current_scene_for_bridge(&state.backend);
    serde_json::to_value(snapshot)
        .map(Json)
        .map_err(internal_error)
}

async fn rag_retrieve_handler(
    AxumState(state): AxumState<BridgeState>,
    Json(request): Json<RagRetrieveRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let query = request.query.trim();
    if query.is_empty() {
        return Err(bad_request("Query must be a non-empty string"));
    }

    let response = retrieve_contexts(&state.backend, query, request.n_results.unwrap_or(3))
        .map_err(internal_error)?;

    serde_json::to_value(response)
        .map(Json)
        .map_err(internal_error)
}

async fn ask_handler(
    AxumState(state): AxumState<BridgeState>,
    Json(request): Json<AskRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let response = ask_internal(&state.backend, request)
        .await
        .map_err(internal_error)?;
    serde_json::to_value(response)
        .map(Json)
        .map_err(internal_error)
}

async fn scene_analysis_handler(
    AxumState(state): AxumState<BridgeState>,
    Json(request): Json<SceneAnalysisBridgeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let normalized = SceneAnalysisRequest {
        goal: request.goal,
        scene_context: request.scene_context.or(request.scene_data),
        model: request.model,
    };

    let response = analyze_scene_internal(&state.backend, normalized)
        .await
        .map_err(internal_error)?;
    serde_json::to_value(response)
        .map(Json)
        .map_err(internal_error)
}

async fn backend_get_handler(
    AxumState(state): AxumState<BridgeState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let response = get_generation_backend_internal(&state.backend);
    serde_json::to_value(response)
        .map(Json)
        .map_err(internal_error)
}

async fn backend_set_handler(
    AxumState(state): AxumState<BridgeState>,
    Json(request): Json<BackendSetRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let response = set_generation_backend_internal(&request.backend, &state.backend)
        .await
        .map_err(internal_error)?;
    serde_json::to_value(response)
        .map(Json)
        .map_err(internal_error)
}

async fn test_handler() -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    Ok(Json(json!({
        "message": "Rust Scene Bridge is running",
        "endpoints": ["/health", "/scene/update", "/scene/current", "/rag/retrieve", "/ask", "/scene_analysis", "/test"]
    })))
}

fn internal_error<E: ToString>(error: E) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

fn bad_request(message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
}
