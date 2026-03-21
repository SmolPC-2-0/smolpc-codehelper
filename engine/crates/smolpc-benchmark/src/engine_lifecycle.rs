use anyhow::{Context, Result};
use smolpc_engine_client::{
    connect_or_spawn, EngineClient, EngineConnectOptions, StartupMode, StartupPolicy,
    WaitReadyOptions,
};
use std::path::PathBuf;
use std::time::Duration;

use crate::config::BenchmarkBackend;

const SHARED_RUNTIME_VENDOR_DIR: &str = "SmolPC";
const SHARED_RUNTIME_DIR: &str = "engine-runtime";
const HOST_DATA_DIR: &str = "host-data";
const MODEL_LOAD_TIMEOUT: Duration = Duration::from_secs(600);

fn shared_runtime_dir() -> PathBuf {
    dirs::data_local_dir()
        .expect("data_local_dir must exist")
        .join(SHARED_RUNTIME_VENDOR_DIR)
        .join(SHARED_RUNTIME_DIR)
}

/// Spawn (or restart) the engine forced to the given backend.
pub async fn spawn_engine(
    backend: BenchmarkBackend,
    port: u16,
    resource_dir: Option<PathBuf>,
) -> Result<EngineClient> {
    let runtime_dir = shared_runtime_dir();
    let data_dir = runtime_dir.join(HOST_DATA_DIR);

    let options = EngineConnectOptions {
        port,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        shared_runtime_dir: runtime_dir,
        data_dir,
        resource_dir,
        models_dir: None,
        host_binary: None,
        runtime_mode: backend.to_runtime_mode(),
        dml_device_id: None,
        force_respawn: true,
    };

    log::info!("Spawning engine for backend={backend} on port={port}");
    let client = connect_or_spawn(options)
        .await
        .context("failed to spawn engine")?;
    Ok(client)
}

/// Load a model and wait until the engine reports ready.
pub async fn load_and_wait(client: &EngineClient, model_id: &str) -> Result<()> {
    log::info!("Loading model: {model_id}");

    client
        .ensure_started(
            StartupMode::Auto,
            StartupPolicy {
                default_model_id: Some(model_id.to_string()),
            },
        )
        .await
        .context("ensure_started failed")?;

    client
        .wait_ready(WaitReadyOptions {
            timeout: MODEL_LOAD_TIMEOUT,
            ..Default::default()
        })
        .await
        .context("engine did not become ready")?;

    // Verify the right model is loaded
    let status = client.status().await.context("status check failed")?;
    if !status.is_ready() {
        if let Some(msg) = status.failure_message() {
            anyhow::bail!("Engine failed to start: {msg}");
        }
        anyhow::bail!("Engine is not ready after wait");
    }

    log::info!(
        "Model loaded: active_backend={:?} model={:?}",
        status.active_backend,
        status.current_model
    );
    Ok(())
}

/// Verify the engine's active backend matches what we expect.
pub async fn verify_backend(
    client: &EngineClient,
    expected: BenchmarkBackend,
) -> Result<()> {
    let status = client.status().await?;
    let active = status.active_backend.as_deref().unwrap_or("none");
    let expected_label = expected.engine_label();

    if !active.eq_ignore_ascii_case(expected_label) {
        anyhow::bail!(
            "Backend mismatch: expected '{expected_label}', engine reports '{active}'"
        );
    }
    Ok(())
}

/// Shut down the engine process by PID.
pub async fn shutdown_engine(client: &EngineClient) -> Result<()> {
    let meta = match client.meta().await {
        Ok(m) => m,
        Err(_) => {
            log::warn!("Could not reach engine for shutdown — may already be stopped");
            return Ok(());
        }
    };

    log::info!("Shutting down engine (PID {})", meta.pid);

    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &meta.pid.to_string(), "/F"])
            .output();
    }
    #[cfg(not(windows))]
    {
        unsafe {
            libc::kill(meta.pid as i32, libc::SIGTERM);
        }
    }

    // Give the process a moment to exit
    tokio::time::sleep(Duration::from_secs(2)).await;
    Ok(())
}

/// Get the engine process PID (for memory sampling).
pub async fn engine_pid(client: &EngineClient) -> Result<u32> {
    let meta = client.meta().await.context("failed to get engine meta")?;
    Ok(meta.pid)
}
