//! Engine supervisor — the single-owner background task that manages the engine lifecycle.
//!
//! The supervisor receives commands via an `mpsc` channel, broadcasts state via a
//! `watch` channel, and emits Tauri events for frontend reactivity. It owns the
//! `EngineClient`, runtime configuration, and restart policy — no other code
//! touches the engine process directly.

use super::{EngineCommand, EngineLifecycleState, StartupConfig};
use smolpc_engine_client::{EngineClient, RuntimeModePreference};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::{mpsc, watch};

/// Health check interval (seconds).
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(10);

/// Maximum number of automatic restarts within the restart window.
const MAX_RESTARTS: u32 = 3;

/// Duration of the restart counting window.
const RESTART_WINDOW: Duration = Duration::from_secs(300); // 5 minutes

/// Exponential backoff delays for consecutive restarts.
const BACKOFF_DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

/// The engine supervisor background task.
///
/// Created via [`EngineSupervisor::new`] and run via [`EngineSupervisor::run`].
/// The `run` method consumes the supervisor and loops until all command senders
/// are dropped (app shutdown).
pub struct EngineSupervisor<R: Runtime> {
    // --- Channels ---
    cmd_rx: mpsc::Receiver<EngineCommand>,
    state_tx: watch::Sender<EngineLifecycleState>,
    client_tx: watch::Sender<Option<EngineClient>>,
    app_handle: AppHandle<R>,

    // --- Owned state ---
    state: EngineLifecycleState,
    client: Option<EngineClient>,
    engine_pid: Option<u32>,
    runtime_config: RuntimeConfig,
    desired_model: Option<String>,

    // --- Restart policy ---
    restart_count: u32,
    last_restart_window_start: Option<Instant>,
    restart_pending: bool,
    restart_delay: Duration,
}

/// Runtime configuration held by the supervisor.
#[derive(Debug, Clone, Copy)]
struct RuntimeConfig {
    runtime_mode: RuntimeModePreference,
    dml_device_id: Option<i32>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            runtime_mode: RuntimeModePreference::Auto,
            dml_device_id: None,
        }
    }
}

impl<R: Runtime> EngineSupervisor<R> {
    /// Create a new supervisor. Does not start the loop — call [`run`] for that.
    pub fn new(
        cmd_rx: mpsc::Receiver<EngineCommand>,
        state_tx: watch::Sender<EngineLifecycleState>,
        client_tx: watch::Sender<Option<EngineClient>>,
        app_handle: AppHandle<R>,
    ) -> Self {
        Self {
            cmd_rx,
            state_tx,
            client_tx,
            app_handle,
            state: EngineLifecycleState::Idle,
            client: None,
            engine_pid: None,
            runtime_config: RuntimeConfig::default(),
            desired_model: None,
            restart_count: 0,
            last_restart_window_start: None,
            restart_pending: false,
            restart_delay: BACKOFF_DELAYS[0],
        }
    }

    /// Run the supervisor loop. Blocks until all command senders are dropped.
    pub async fn run(mut self) {
        let mut health_interval = tokio::time::interval(HEALTH_CHECK_INTERVAL);
        // The first tick completes immediately — consume it so we don't
        // fire a spurious health check before the engine is even started.
        health_interval.tick().await;

        loop {
            let is_running = self.state.is_running();
            let restart_pending = self.restart_pending;
            let restart_delay = self.restart_delay;

            tokio::select! {
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(cmd) => self.handle_command(cmd).await,
                        None => {
                            // All senders dropped — app is shutting down.
                            log::info!("Supervisor: all command senders dropped, shutting down");
                            let _ = self.do_shutdown().await;
                            break;
                        }
                    }
                },
                _ = health_interval.tick(), if is_running => {
                    self.do_health_check().await;
                },
                _ = tokio::time::sleep(restart_delay), if restart_pending => {
                    self.restart_pending = false;
                    self.do_spawn_sequence().await;
                },
            }
        }
    }

    // --- Command handling ---

    async fn handle_command(&mut self, cmd: EngineCommand) {
        match cmd {
            EngineCommand::Start {
                config,
                respond_to,
            } => {
                let result = self.handle_start(config).await;
                let _ = respond_to.send(result);
            }
            EngineCommand::SetRuntimeMode {
                mode,
                respond_to,
            } => {
                let result = self.handle_set_runtime_mode(mode).await;
                let _ = respond_to.send(result);
            }
            EngineCommand::SetDesiredModel { model_id } => {
                self.desired_model = model_id;
            }
            EngineCommand::RefreshStatus => {
                self.refresh_running_status().await;
            }
            EngineCommand::Shutdown { respond_to } => {
                let result = self.do_shutdown().await;
                let _ = respond_to.send(result);
            }
        }
    }

    async fn handle_start(&mut self, config: StartupConfig) -> Result<(), String> {
        // Update runtime configuration.
        self.runtime_config = RuntimeConfig {
            runtime_mode: config.runtime_mode,
            dml_device_id: config.dml_device_id,
        };
        if let Some(model_id) = config.default_model_id {
            self.desired_model = Some(model_id);
        }

        // If already running, just return success.
        if self.state.is_running() {
            return Ok(());
        }

        // If in a terminal state, reset for retry.
        if self.state.is_terminal() {
            self.restart_count = 0;
            self.last_restart_window_start = None;
        }

        // Transition to Starting and begin the spawn sequence.
        self.transition(EngineLifecycleState::Starting);
        self.do_spawn_sequence().await;
        Ok(())
    }

    async fn handle_set_runtime_mode(
        &mut self,
        mode: RuntimeModePreference,
    ) -> Result<(), String> {
        let old_mode = self.runtime_config.runtime_mode;
        self.runtime_config.runtime_mode = mode;

        if old_mode == mode && self.state.is_running() {
            return Ok(());
        }

        // Need to restart the engine with the new runtime mode.
        if self.state.is_running() {
            // Gracefully shut down first.
            if let Some(client) = &self.client {
                let _ = smolpc_engine_client::shutdown_and_wait(
                    client,
                    Duration::from_secs(5),
                )
                .await;
            }
        }

        // Reset restart count for a user-initiated mode change.
        self.restart_count = 0;
        self.last_restart_window_start = None;
        self.transition(EngineLifecycleState::Starting);
        self.do_spawn_sequence().await;
        Ok(())
    }

    // --- Health monitoring ---

    async fn do_health_check(&mut self) {
        // Check PID is alive (fast OS-level check).
        if let Some(pid) = self.engine_pid {
            if !is_pid_alive(pid) {
                log::warn!("Supervisor: engine PID {pid} is no longer alive");
                self.transition_to_crashed("Engine process exited unexpectedly");
                self.schedule_restart();
                return;
            }
        }

        // Check HTTP health.
        let healthy = if let Some(client) = &self.client {
            client.health().await.unwrap_or(false)
        } else {
            false
        };

        if !healthy {
            log::warn!("Supervisor: engine health check failed");
            self.transition_to_crashed("Engine health check failed");
            self.schedule_restart();
            return;
        }

        // Health passed — refresh status fields (backend, model_id).
        self.refresh_running_status().await;
    }

    async fn refresh_running_status(&mut self) {
        if !self.state.is_running() {
            return;
        }
        let Some(client) = &self.client else {
            return;
        };

        match client.status().await {
            Ok(status) => {
                let new_state = EngineLifecycleState::Running {
                    backend: status.active_backend.clone(),
                    model_id: status.active_model_id.clone(),
                };
                // Only broadcast if state actually changed.
                if self.state != new_state {
                    self.transition(new_state);
                }
            }
            Err(e) => {
                log::warn!("Supervisor: failed to poll engine status: {e}");
                // Don't crash on a single status poll failure — health check
                // is the authoritative liveness signal.
            }
        }
    }

    // --- Spawn sequence (stub — filled in US-005) ---

    async fn do_spawn_sequence(&mut self) {
        // Ensure we're in Starting state.
        if !matches!(self.state, EngineLifecycleState::Starting) {
            self.transition(EngineLifecycleState::Starting);
        }

        // US-005 will implement the full spawn sequence here:
        //   1. Resolve binary path
        //   2. Delete old token, regenerate
        //   3. Construct new EngineClient
        //   4. Kill stale processes
        //   5. Call spawn_engine + wait_for_healthy
        //   6. Transition to Running
        //   7. Restore desired_model if set

        // For now, log a placeholder. The supervisor compiles and runs
        // its core loop; the actual engine spawn is wired in US-005.
        log::info!(
            "Supervisor: spawn sequence requested (runtime_mode={:?}) — \
             full implementation in US-005",
            self.runtime_config.runtime_mode
        );

        // Transition to WaitingForHealth then to a stub Running state
        // so the rest of the command flow can be tested once wired.
        // US-005 replaces this with real spawn + health wait.
        self.transition(EngineLifecycleState::WaitingForHealth);

        // The actual spawn and health wait will go here.
        // For now, we remain in WaitingForHealth until the real
        // implementation is added. We do NOT fake a Running transition.
    }

    // --- Shutdown ---

    async fn do_shutdown(&mut self) -> Result<(), String> {
        if let Some(client) = &self.client {
            log::info!("Supervisor: sending shutdown to engine");
            match smolpc_engine_client::shutdown_and_wait(client, Duration::from_secs(5)).await {
                Ok(()) => log::info!("Supervisor: engine shut down gracefully"),
                Err(e) => log::warn!("Supervisor: engine shutdown error: {e}"),
            }
        }

        self.client = None;
        self.engine_pid = None;
        self.broadcast_client(None);
        Ok(())
    }

    // --- Restart scheduling ---

    fn schedule_restart(&mut self) {
        // Check if we're within the restart window.
        let now = Instant::now();
        let window_start = self
            .last_restart_window_start
            .get_or_insert(now);

        if now.duration_since(*window_start) > RESTART_WINDOW {
            // Reset the window.
            self.restart_count = 0;
            self.last_restart_window_start = Some(now);
        }

        if self.restart_count >= MAX_RESTARTS {
            log::error!(
                "Supervisor: restart limit ({MAX_RESTARTS}) exceeded within {RESTART_WINDOW:?}"
            );
            self.transition(EngineLifecycleState::Failed {
                message: format!(
                    "Engine crashed {MAX_RESTARTS} times within {} minutes. Click Retry to try again.",
                    RESTART_WINDOW.as_secs() / 60
                ),
            });
            return;
        }

        // Calculate backoff delay.
        let delay_index = self.restart_count.min(BACKOFF_DELAYS.len() as u32 - 1) as usize;
        self.restart_delay = BACKOFF_DELAYS[delay_index];
        self.restart_count += 1;
        self.restart_pending = true;

        log::info!(
            "Supervisor: scheduling restart #{} in {:?}",
            self.restart_count,
            self.restart_delay
        );
    }

    // --- State transitions ---

    fn transition_to_crashed(&mut self, message: &str) {
        self.client = None;
        self.engine_pid = None;
        self.broadcast_client(None);

        self.transition(EngineLifecycleState::Crashed {
            message: message.to_string(),
            restart_count: self.restart_count,
        });
    }

    fn transition(&mut self, new_state: EngineLifecycleState) {
        if self.state == new_state {
            return;
        }
        log::info!(
            "Supervisor: state transition {:?} -> {:?}",
            self.state,
            new_state
        );
        self.state = new_state.clone();

        // Broadcast via watch channel.
        let _ = self.state_tx.send(new_state.clone());

        // Emit Tauri event for frontend.
        if let Err(e) = self.app_handle.emit("engine-state-changed", &new_state) {
            log::warn!("Supervisor: failed to emit engine-state-changed event: {e}");
        }
    }

    fn broadcast_client(&self, client: Option<EngineClient>) {
        let _ = self.client_tx.send(client);
    }
}

// --- OS-level PID check ---

/// Check if a process with the given PID is still alive.
#[cfg(target_os = "windows")]
fn is_pid_alive(pid: u32) -> bool {
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;

    // SAFETY: OpenProcess / GetExitCodeProcess are safe Windows API calls.
    // We immediately close the handle after use.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut exit_code: u32 = 0;
        let ok = GetExitCodeProcess(handle, &mut exit_code);
        CloseHandle(handle);
        ok != 0 && exit_code == STILL_ACTIVE
    }
}

#[cfg(target_os = "windows")]
#[allow(dead_code)] // Used by is_pid_alive; warning is a cfg artifact.
extern "system" {
    fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> *mut std::ffi::c_void;
    fn GetExitCodeProcess(process: *mut std::ffi::c_void, exit_code: *mut u32) -> i32;
    fn CloseHandle(object: *mut std::ffi::c_void) -> i32;
}

#[cfg(unix)]
fn is_pid_alive(pid: u32) -> bool {
    // kill(pid, 0) checks existence without sending a signal.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(all(not(windows), not(unix)))]
fn is_pid_alive(_pid: u32) -> bool {
    // Fallback: assume alive (health HTTP check is the backup).
    true
}
