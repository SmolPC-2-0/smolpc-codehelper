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
    /// Watch receiver for the current EngineClient, broadcast by the supervisor.
    /// Wrapped in a sync Mutex so `get_client_if_ready()` can borrow it from `&self`.
    /// The supervisor always sends the client via `client_tx` BEFORE broadcasting
    /// the Running state via `state_tx`, so this receiver is guaranteed to have the
    /// client available by the time `get_client()` observes Running.
    client_rx: Arc<std::sync::Mutex<watch::Receiver<Option<EngineClient>>>>,
    /// Watch receiver for the current engine PID. Used by the shutdown path to
    /// force-kill the correct process without relying on a potentially stale PID file.
    pid_rx: watch::Receiver<Option<u32>>,
}

impl EngineSupervisorHandle {
    /// Create a new handle from the supervisor's channel endpoints.
    pub fn new(
        cmd_tx: mpsc::Sender<EngineCommand>,
        state_rx: watch::Receiver<EngineLifecycleState>,
        client_rx: watch::Receiver<Option<EngineClient>>,
        pid_rx: watch::Receiver<Option<u32>>,
    ) -> Self {
        Self {
            cmd_tx,
            state_rx,
            client_rx: Arc::new(std::sync::Mutex::new(client_rx)),
            pid_rx,
        }
    }

    // --- Public API ---

    /// Wait until engine reaches Running state or timeout.
    /// Returns a clone of EngineClient (Arc-based, cheap to clone).
    ///
    /// Watches both the state channel and the client channel via `tokio::select!`
    /// to avoid a race where Running is observed before the client cache is populated.
    pub async fn get_client(&self, timeout: Duration) -> Result<EngineClient, String> {
        // Fast path: already running and client cached.
        if let Some(client) = self.get_client_if_ready() {
            return Ok(client);
        }

        let mut state_rx = self.state_rx.clone();
        let mut client_rx = self
            .client_rx
            .lock()
            .map_err(|_| "Client receiver lock poisoned".to_string())?
            .clone();

        let result = tokio::time::timeout(timeout, async {
            loop {
                // Check current state and client.
                {
                    let state = state_rx.borrow().clone();
                    if state.is_running() {
                        if let Some(client) = client_rx.borrow().clone() {
                            return Ok(client);
                        }
                    }
                    if state.is_terminal() {
                        return Err(format!("Engine is in terminal state: {state:?}"));
                    }
                }
                // Wait for either a state change or a client broadcast.
                tokio::select! {
                    result = state_rx.changed() => {
                        if result.is_err() {
                            return Err("Supervisor shut down while waiting for engine".to_string());
                        }
                    }
                    result = client_rx.changed() => {
                        if result.is_err() {
                            return Err("Supervisor shut down while waiting for engine".to_string());
                        }
                    }
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

    /// Non-blocking: return client clone only if the supervisor has broadcast one.
    pub fn get_client_if_ready(&self) -> Option<EngineClient> {
        self.client_rx.lock().ok()?.borrow().clone()
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

    /// Return the last known engine PID, if any.
    /// Used by the shutdown fallback to force-kill the correct process.
    pub fn last_engine_pid(&self) -> Option<u32> {
        *self.pid_rx.borrow()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a handle with default channels for testing.
    fn make_handle(
        initial_state: EngineLifecycleState,
    ) -> (
        EngineSupervisorHandle,
        mpsc::Receiver<EngineCommand>,
        watch::Sender<EngineLifecycleState>,
        watch::Sender<Option<EngineClient>>,
    ) {
        let (cmd_tx, cmd_rx) = mpsc::channel(16);
        let (state_tx, state_rx) = watch::channel(initial_state);
        let (client_tx, client_rx) = watch::channel::<Option<EngineClient>>(None);
        let (_pid_tx, pid_rx) = watch::channel::<Option<u32>>(None);
        let handle = EngineSupervisorHandle::new(cmd_tx, state_rx, client_rx, pid_rx);
        (handle, cmd_rx, state_tx, client_tx)
    }

    #[test]
    fn handle_is_clone() {
        let (handle, ..) = make_handle(EngineLifecycleState::Idle);
        let _cloned = handle.clone();
    }

    #[test]
    fn get_client_if_ready_returns_none_when_no_client() {
        let (handle, ..) = make_handle(EngineLifecycleState::Idle);
        assert!(handle.get_client_if_ready().is_none());
    }

    #[test]
    fn get_client_if_ready_returns_client_after_broadcast() {
        let (handle, _cmd_rx, _state_tx, client_tx) = make_handle(EngineLifecycleState::Idle);
        let client = EngineClient::new("http://127.0.0.1:19432".to_string(), "tok".to_string());
        client_tx.send(Some(client)).expect("send client");
        assert!(handle.get_client_if_ready().is_some());
    }

    #[tokio::test]
    async fn get_client_times_out_when_not_running() {
        let (handle, ..) = make_handle(EngineLifecycleState::Idle);

        let result = handle.get_client(Duration::from_millis(50)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Timed out"));
    }

    #[tokio::test]
    async fn get_client_returns_error_on_terminal_state() {
        let (handle, ..) = make_handle(EngineLifecycleState::Failed {
            message: "fatal".into(),
        });

        let result = handle.get_client(Duration::from_secs(5)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("terminal state"));
    }

    #[tokio::test]
    async fn get_client_returns_client_when_broadcast_before_state() {
        let (handle, _cmd_rx, state_tx, client_tx) = make_handle(EngineLifecycleState::Idle);

        // Simulate the supervisor: broadcast client BEFORE Running state.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let client = EngineClient::new("http://127.0.0.1:19432".to_string(), "tok".to_string());
            client_tx.send(Some(client)).expect("send client");
            state_tx
                .send(EngineLifecycleState::Running {
                    backend: None,
                    model_id: None,
                })
                .expect("send state");
        });

        let result = handle.get_client(Duration::from_secs(5)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn ensure_started_sends_command() {
        let (handle, mut cmd_rx, _state_tx, _client_tx) = make_handle(EngineLifecycleState::Idle);

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
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn shutdown_sends_command() {
        let (handle, mut cmd_rx, _state_tx, _client_tx) = make_handle(EngineLifecycleState::Idle);

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
        let (handle, mut cmd_rx, _state_tx, _client_tx) = make_handle(EngineLifecycleState::Idle);

        handle.refresh_status().await;

        let cmd = cmd_rx.try_recv().expect("should receive command");
        assert!(matches!(cmd, EngineCommand::RefreshStatus));
    }

    #[tokio::test]
    async fn set_runtime_mode_sends_command() {
        let (handle, mut cmd_rx, _state_tx, _client_tx) = make_handle(EngineLifecycleState::Idle);

        tokio::spawn(async move {
            if let Some(EngineCommand::SetRuntimeMode { respond_to, .. }) = cmd_rx.recv().await {
                let _ = respond_to.send(Ok(()));
            }
        });

        let result = handle.set_runtime_mode(RuntimeModePreference::Cpu).await;
        assert!(result.is_ok());
    }
}
