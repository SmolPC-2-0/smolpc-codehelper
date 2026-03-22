mod adapters;
mod artifacts;
mod auth;
mod chat;
mod config;
mod model_loading;
mod openvino;
mod probe;
mod routes;
mod runtime_bundles;
mod selection;
mod startup;
mod state;
mod types;

use axum::routing::{get, post};
use axum::Router;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{Notify, Semaphore};
use tokio::time::sleep;

use crate::config::{epoch_ms, parse_args};
use crate::state::{AppState, EngineState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = env_logger::try_init();
    let args = parse_args();
    std::fs::create_dir_all(&args.data_dir)?;

    let token =
        std::env::var("SMOLPC_ENGINE_TOKEN").map_err(|_| "SMOLPC_ENGINE_TOKEN is required")?;

    let state = AppState {
        token: Arc::new(token),
        engine: Arc::new(EngineState::new(&args)),
        generation_semaphore: Arc::new(Semaphore::new(1)),
        queue_semaphore: Arc::new(Semaphore::new(args.queue_size)),
        queue_timeout: args.queue_timeout,
        shutdown: Arc::new(Notify::new()),
        last_activity_ms: Arc::new(AtomicU64::new(epoch_ms())),
    };
    state.engine.launch_startup_probe();

    let idle_state = state.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(30)).await;
            let idle_ms =
                epoch_ms().saturating_sub(idle_state.last_activity_ms.load(Ordering::SeqCst));
            if let Some(model_idle_unload) = args.model_idle_unload {
                if idle_ms >= model_idle_unload.as_millis() as u64
                    && !idle_state.engine.generating.load(Ordering::SeqCst)
                    && !idle_state
                        .engine
                        .model_transition_in_progress
                        .load(Ordering::SeqCst)
                    && idle_state.engine.current_model.lock().await.is_some()
                {
                    let _ = idle_state.engine.unload_model(false).await;
                }
            }
            if let Some(process_idle_exit) = args.process_idle_exit {
                if idle_ms >= process_idle_exit.as_millis() as u64
                    && !idle_state.engine.generating.load(Ordering::SeqCst)
                {
                    idle_state.shutdown.notify_waiters();
                    break;
                }
            }
        }
    });

    let app = Router::new()
        .route("/engine/health", get(routes::health))
        .route("/engine/meta", get(routes::meta))
        .route("/engine/status", get(routes::status))
        .route("/engine/ensure-started", post(routes::ensure_started))
        .route("/engine/load", post(routes::load))
        .route("/engine/unload", post(routes::unload))
        .route("/engine/cancel", post(routes::cancel))
        .route("/engine/shutdown", post(routes::shutdown))
        .route("/engine/check-model", post(routes::check_model))
        .route("/v1/models", get(routes::v1_models))
        .route("/v1/chat/completions", post(routes::v1_chat_completions))
        .with_state(state.clone());

    let listener = TcpListener::bind(("127.0.0.1", args.port)).await?;
    println!(
        "smolpc-engine-host listening on http://127.0.0.1:{}",
        args.port
    );

    let shutdown_signal = async move {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = state.shutdown.notified() => {},
        }
        log::info!("Shutdown signal received, cancelling active generation");
        state.engine.cancel();
        sleep(Duration::from_secs(2)).await;
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests;
