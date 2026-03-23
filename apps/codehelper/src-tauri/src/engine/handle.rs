//! Engine supervisor handle — the Clone-able interface for Tauri commands.
//!
//! All Tauri command handlers interact with the supervisor exclusively through
//! this handle. It communicates via channels (no Mutex contention).

use super::{EngineCommand, EngineLifecycleState, StartupConfig};
use smolpc_engine_client::{EngineClient, RuntimeModePreference};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, watch};

/// Clone-able handle to the engine supervisor task.
///
/// Tauri managed state already wraps this in `Arc`, so the handle itself
/// does NOT use an outer `Arc`. The inner fields are all cheaply cloneable:
/// - `mpsc::Sender` is `Clone`
/// - `watch::Receiver` is `Clone`
/// - `Arc<std::sync::Mutex<..>>` is `Clone`
#[derive(Clone)]
pub struct EngineSupervisorHandle {
    cmd_tx: mpsc::Sender<EngineCommand>,
    state_rx: watch::Receiver<EngineLifecycleState>,
    /// Cached clone of the current EngineClient, updated by a background
    /// watcher task whenever the supervisor broadcasts a new Running state.
    /// This allows get_client_if_ready() to be synchronous.
    cached_client: Arc<std::sync::Mutex<Option<EngineClient>>>,
}

impl EngineSupervisorHandle {
    /// Create a new handle from the supervisor's channel endpoints.
    ///
    /// The caller must also spawn the background client-cache watcher via
    /// [`Self::spawn_client_cache_watcher`].
    pub fn new(
        cmd_tx: mpsc::Sender<EngineCommand>,
        state_rx: watch::Receiver<EngineLifecycleState>,
    ) -> Self {
        Self {
            cmd_tx,
            state_rx,
            cached_client: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Spawn a background task that watches the state channel and updates
    /// `cached_client` when the supervisor broadcasts a Running state with
    /// a live EngineClient.
    ///
    /// This task exits when the watch sender is dropped (supervisor shutdown).
    /// The `client_source` watch receiver carries the EngineClient that the
    /// supervisor publishes alongside Running state transitions.
    pub fn spawn_client_cache_watcher(
        &self,
        mut client_rx: watch::Receiver<Option<EngineClient>>,
    ) {
        let cached = Arc::clone(&self.cached_client);
        tauri::async_runtime::spawn(async move {
            loop {
                if client_rx.changed().await.is_err() {
                    // Sender dropped — supervisor shut down.
                    break;
                }
                let new_client = client_rx.borrow().clone();
                if let Ok(mut guard) = cached.lock() {
                    *guard = new_client;
                }
            }
        });
    }

    // --- Public API ---

    /// Instant snapshot of engine state. No network, no lock.
    pub fn current_state(&self) -> EngineLifecycleState {
        self.state_rx.borrow().clone()
    }

    /// Wait until engine reaches Running state or timeout.
    /// Returns a clone of EngineClient (Arc-based, cheap to clone).
    pub async fn get_client(&self, timeout: Duration) -> Result<EngineClient, String> {
        // Fast path: already running and client cached.
        if let Some(client) = self.get_client_if_ready() {
            return Ok(client);
        }

        let mut rx = self.state_rx.clone();
        let result = tokio::time::timeout(timeout, async {
            loop {
                // Check current state first.
                {
                    let state = rx.borrow().clone();
                    if state.is_running() {
                        if let Some(client) = self.get_client_if_ready() {
                            return Ok(client);
                        }
                    }
                    if state.is_terminal() {
                        return Err(format!("Engine is in terminal state: {state:?}"));
                    }
                }
                // Wait for the next state change.
                if rx.changed().await.is_err() {
                    return Err("Supervisor shut down while waiting for engine".to_string());
                }
            }
        })
        .await;

        match result {
            Ok(inner) => inner,
            Err(_) => Err(format!(
                "Timed out after {timeout:?} waiting for engine to reach Running state"
            )),
        }
    }

    /// Non-blocking: return client clone only if engine is currently Running.
    pub fn get_client_if_ready(&self) -> Option<EngineClient> {
        self.cached_client.lock().ok()?.clone()
    }

    /// Request engine startup with config. Waits for the supervisor's response.
    pub async fn ensure_started(&self, config: StartupConfig) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(EngineCommand::Start {
                config,
                respond_to: tx,
            })
            .await
            .map_err(|_| "Supervisor is not running".to_string())?;
        rx.await
            .map_err(|_| "Supervisor dropped the response channel".to_string())?
    }

    /// Request runtime mode change. Supervisor handles restart internally.
    pub async fn set_runtime_mode(&self, mode: RuntimeModePreference) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(EngineCommand::SetRuntimeMode {
                mode,
                respond_to: tx,
            })
            .await
            .map_err(|_| "Supervisor is not running".to_string())?;
        rx.await
            .map_err(|_| "Supervisor dropped the response channel".to_string())?
    }

    /// Request graceful shutdown.
    pub async fn shutdown(&self) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(EngineCommand::Shutdown { respond_to: tx })
            .await
            .map_err(|_| "Supervisor is not running".to_string())?;
        rx.await
            .map_err(|_| "Supervisor dropped the response channel".to_string())?
    }

    /// Tell supervisor to re-poll engine status (after load_model, etc.).
    /// Fire-and-forget — no response needed.
    pub async fn refresh_status(&self) {
        let _ = self.cmd_tx.send(EngineCommand::RefreshStatus).await;
    }

    /// Update the desired model the supervisor should restore after restarts.
    /// Fire-and-forget — no response needed.
    pub async fn set_desired_model(&self, model_id: Option<String>) {
        let _ = self
            .cmd_tx
            .send(EngineCommand::SetDesiredModel { model_id })
            .await;
    }

    /// Subscribe to state changes. Returns a cloned watch receiver.
    pub fn subscribe(&self) -> watch::Receiver<EngineLifecycleState> {
        self.state_rx.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_is_clone() {
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let (_state_tx, state_rx) = watch::channel(EngineLifecycleState::Idle);
        let handle = EngineSupervisorHandle::new(cmd_tx, state_rx);
        let _cloned = handle.clone();
    }

    #[test]
    fn current_state_returns_initial_state() {
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let (_state_tx, state_rx) = watch::channel(EngineLifecycleState::Idle);
        let handle = EngineSupervisorHandle::new(cmd_tx, state_rx);
        assert_eq!(handle.current_state(), EngineLifecycleState::Idle);
    }

    #[test]
    fn current_state_reflects_broadcast() {
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let (state_tx, state_rx) = watch::channel(EngineLifecycleState::Idle);
        let handle = EngineSupervisorHandle::new(cmd_tx, state_rx);

        state_tx
            .send(EngineLifecycleState::Running {
                backend: Some("cpu".into()),
                model_id: None,
            })
            .expect("send state");

        assert!(handle.current_state().is_running());
    }

    #[test]
    fn get_client_if_ready_returns_none_when_no_client() {
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let (_state_tx, state_rx) = watch::channel(EngineLifecycleState::Idle);
        let handle = EngineSupervisorHandle::new(cmd_tx, state_rx);
        assert!(handle.get_client_if_ready().is_none());
    }

    #[tokio::test]
    async fn get_client_times_out_when_not_running() {
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let (_state_tx, state_rx) = watch::channel(EngineLifecycleState::Idle);
        let handle = EngineSupervisorHandle::new(cmd_tx, state_rx);

        let result = handle.get_client(Duration::from_millis(50)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Timed out"));
    }

    #[tokio::test]
    async fn get_client_returns_error_on_terminal_state() {
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let (_state_tx, state_rx) = watch::channel(EngineLifecycleState::Failed {
            message: "fatal".into(),
        });
        let handle = EngineSupervisorHandle::new(cmd_tx, state_rx);

        let result = handle.get_client(Duration::from_secs(5)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("terminal state"));
    }

    #[tokio::test]
    async fn ensure_started_sends_command() {
        let (cmd_tx, mut cmd_rx) = mpsc::channel(16);
        let (_state_tx, state_rx) = watch::channel(EngineLifecycleState::Idle);
        let handle = EngineSupervisorHandle::new(cmd_tx, state_rx);

        // Spawn a task to respond to the command.
        tokio::spawn(async move {
            if let Some(EngineCommand::Start { respond_to, .. }) = cmd_rx.recv().await {
                let _ = respond_to.send(Ok(()));
            }
        });

        let result = handle
            .ensure_started(StartupConfig {
                runtime_mode: RuntimeModePreference::Auto,
                dml_device_id: None,
                default_model_id: None,
                startup_mode: smolpc_engine_client::StartupMode::Auto,
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn shutdown_sends_command() {
        let (cmd_tx, mut cmd_rx) = mpsc::channel(16);
        let (_state_tx, state_rx) = watch::channel(EngineLifecycleState::Idle);
        let handle = EngineSupervisorHandle::new(cmd_tx, state_rx);

        tokio::spawn(async move {
            if let Some(EngineCommand::Shutdown { respond_to }) = cmd_rx.recv().await {
                let _ = respond_to.send(Ok(()));
            }
        });

        let result = handle.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn refresh_status_is_fire_and_forget() {
        let (cmd_tx, mut cmd_rx) = mpsc::channel(16);
        let (_state_tx, state_rx) = watch::channel(EngineLifecycleState::Idle);
        let handle = EngineSupervisorHandle::new(cmd_tx, state_rx);

        handle.refresh_status().await;

        let cmd = cmd_rx.try_recv().expect("should receive command");
        assert!(matches!(cmd, EngineCommand::RefreshStatus));
    }

    #[tokio::test]
    async fn set_runtime_mode_sends_command() {
        let (cmd_tx, mut cmd_rx) = mpsc::channel(16);
        let (_state_tx, state_rx) = watch::channel(EngineLifecycleState::Idle);
        let handle = EngineSupervisorHandle::new(cmd_tx, state_rx);

        tokio::spawn(async move {
            if let Some(EngineCommand::SetRuntimeMode { respond_to, .. }) = cmd_rx.recv().await {
                let _ = respond_to.send(Ok(()));
            }
        });

        let result = handle.set_runtime_mode(RuntimeModePreference::Cpu).await;
        assert!(result.is_ok());
    }
}
