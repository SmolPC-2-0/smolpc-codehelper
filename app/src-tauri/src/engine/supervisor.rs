//! Engine supervisor — the single-owner background task that manages the engine lifecycle.
//!
//! The supervisor receives commands via an `mpsc` channel, broadcasts state via a
//! `watch` channel, and emits Tauri events for frontend reactivity. It owns the
//! `EngineClient`, runtime configuration, and restart policy — no other code
//! touches the engine process directly.

use super::{EngineCommand, EngineLifecycleState, StartupConfig};
use crate::app_paths::{
    bundled_resource_dir_path, default_dev_bundled_resource_dir,
    select_bundled_resource_dir_resolution,
};
use smolpc_engine_client::{
    kill_stale_processes, load_or_create_token, spawn_engine, wait_for_healthy, EngineClient,
    EngineConnectOptions, RuntimeModePreference,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::sync::{mpsc, watch};

/// Health check interval (seconds).
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(10);

/// Number of consecutive HTTP health check failures before transitioning to Crashed.
/// PID-dead checks still trigger an immediate crash (process exit is definitive).
const HEALTH_FAILURE_THRESHOLD: u32 = 3;

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
    pid_tx: watch::Sender<Option<u32>>,
    app_handle: AppHandle<R>,

    // --- Owned state ---
    state: EngineLifecycleState,
    client: Option<EngineClient>,
    engine_pid: Option<u32>,
    runtime_config: RuntimeConfig,
    desired_model: Option<String>,

    // --- Health tracking ---
    health_failure_count: u32,

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
        pid_tx: watch::Sender<Option<u32>>,
        app_handle: AppHandle<R>,
    ) -> Self {
        Self {
            cmd_rx,
            state_tx,
            client_tx,
            pid_tx,
            app_handle,
            state: EngineLifecycleState::Idle,
            client: None,
            engine_pid: None,
            runtime_config: RuntimeConfig::default(),
            desired_model: None,
            health_failure_count: 0,
            restart_count: 0,
            last_restart_window_start: None,
            restart_pending: false,
            restart_delay: BACKOFF_DELAYS[0],
        }
    }

    /// Run the supervisor loop. Blocks until all command senders are dropped.
    pub async fn run(mut self) {
        let mut health_interval = tokio::time::interval(HEALTH_CHECK_INTERVAL);
        // Skip missed ticks rather than firing them in a burst. This prevents
        // a spurious health check from firing immediately after do_spawn_sequence()
        // blocks the select loop — the first real check will be a full interval
        // after the engine enters Running.
        health_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
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
                        Some(cmd) => {
                            let spawned = self.handle_command(cmd).await;
                            if spawned {
                                health_interval.reset();
                            }
                        }
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
                    // Reset so the first health check fires a full interval after
                    // the engine enters Running, not immediately.
                    health_interval.reset();
                },
            }
        }
    }

    // --- Command handling ---

    /// Handle a command. Returns `true` if a spawn sequence was executed (callers
    /// should reset the health interval).
    async fn handle_command(&mut self, cmd: EngineCommand) -> bool {
        let state_before = self.state.clone();
        match cmd {
            EngineCommand::Start { config, respond_to } => {
                let result = self.handle_start(config).await;
                let _ = respond_to.send(result);
            }
            EngineCommand::SetRuntimeMode { mode, respond_to } => {
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
        // A spawn occurred if we transitioned into Running from a non-Running state.
        !state_before.is_running() && self.state.is_running()
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

    async fn handle_set_runtime_mode(&mut self, mode: RuntimeModePreference) -> Result<(), String> {
        let old_mode = self.runtime_config.runtime_mode;
        self.runtime_config.runtime_mode = mode;

        if old_mode == mode && self.state.is_running() {
            return Ok(());
        }

        // Need to restart the engine with the new runtime mode.
        if self.state.is_running() {
            // Gracefully shut down first.
            if let Some(client) = &self.client {
                let _ =
                    smolpc_engine_client::shutdown_and_wait(client, Duration::from_secs(5)).await;
            }
        }

        // Reset restart count for a user-initiated mode change.
        self.restart_count = 0;
        self.last_restart_window_start = None;
        self.transition(EngineLifecycleState::Starting);
        self.do_spawn_sequence().await;

        // If the spawn failed (state is Crashed or Failed), revert to the previous
        // runtime mode so auto-restart doesn't keep trying the broken mode.
        if !self.state.is_running() {
            log::warn!("Supervisor: mode switch to {mode:?} failed, reverting to {old_mode:?}");
            self.runtime_config.runtime_mode = old_mode;
            return Err(format!(
                "Failed to switch runtime mode to {mode:?} — engine did not reach Running state"
            ));
        }

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
            self.health_failure_count += 1;
            if self.health_failure_count >= HEALTH_FAILURE_THRESHOLD {
                log::warn!(
                    "Supervisor: engine health check failed {} consecutive times, transitioning to Crashed",
                    self.health_failure_count
                );
                self.transition_to_crashed("Engine health check failed");
                self.schedule_restart();
            } else {
                log::warn!(
                    "Supervisor: engine health check failed ({}/{} before restart)",
                    self.health_failure_count,
                    HEALTH_FAILURE_THRESHOLD
                );
            }
            return;
        }

        // Health passed — reset failure counter and refresh status fields.
        self.health_failure_count = 0;
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

    // --- Spawn sequence ---

    async fn do_spawn_sequence(&mut self) {
        // Ensure we're in Starting state.
        if !matches!(self.state, EngineLifecycleState::Starting) {
            self.transition(EngineLifecycleState::Starting);
        }

        // 1. Resolve paths.
        let paths = match self.resolve_paths() {
            Ok(p) => p,
            Err(e) => {
                log::error!("Supervisor: path resolution failed: {e}");
                self.transition(EngineLifecycleState::Failed { message: e });
                return;
            }
        };

        // 2. Delete old token and regenerate a fresh one.
        let token_path = paths.shared_runtime_dir.join("engine-token.txt");
        let _ = std::fs::remove_file(&token_path);
        let token = match load_or_create_token(&token_path) {
            Ok(t) => t,
            Err(e) => {
                log::error!("Supervisor: token regeneration failed: {e}");
                self.transition(EngineLifecycleState::Failed {
                    message: format!("Token regeneration failed: {e}"),
                });
                return;
            }
        };

        // 3. Construct new EngineClient with the fresh token.
        let base_url = format!("http://127.0.0.1:{}", paths.port);
        let client = EngineClient::new(base_url, token.clone());

        // 4. Kill stale engine processes.
        kill_stale_processes();

        // 5. Build connect options and spawn.
        let options = EngineConnectOptions {
            port: paths.port,
            app_version: paths.app_version,
            shared_runtime_dir: paths.shared_runtime_dir.clone(),
            data_dir: paths.data_dir,
            resource_dir: paths.resource_dir,
            models_dir: paths.models_dir,
            host_binary: paths.host_binary,
            runtime_mode: self.runtime_config.runtime_mode,
            dml_device_id: self.runtime_config.dml_device_id,
            force_respawn: false,
        };

        // Create runtime dirs if needed.
        let _ = std::fs::create_dir_all(&options.shared_runtime_dir);
        let _ = std::fs::create_dir_all(&options.data_dir);

        let pid = match spawn_engine(&options, &token) {
            Ok(pid) => {
                log::info!("Supervisor: engine spawned with PID {pid}");
                pid
            }
            Err(e) => {
                log::error!("Supervisor: spawn failed: {e}");
                self.transition(EngineLifecycleState::Failed {
                    message: format!("Engine spawn failed: {e}"),
                });
                return;
            }
        };

        self.engine_pid = Some(pid);
        self.broadcast_pid(Some(pid));
        self.transition(EngineLifecycleState::WaitingForHealth);

        // 6. Wait for the engine to become healthy.
        match wait_for_healthy(&client, Duration::from_secs(30)).await {
            Ok(()) => {
                log::info!("Supervisor: engine is healthy");
            }
            Err(e) => {
                log::error!("Supervisor: health wait failed: {e}");
                let spawn_log = paths.shared_runtime_dir.join("engine-spawn.log");
                let log_hint = if spawn_log.exists() {
                    format!(" Check spawn log: {}", spawn_log.display())
                } else {
                    String::new()
                };
                self.transition_to_crashed(&format!(
                    "Engine failed to become healthy: {e}{log_hint}"
                ));
                self.schedule_restart();
                return;
            }
        }

        // 7. Transition to Running and broadcast the client.
        self.health_failure_count = 0;
        self.client = Some(client.clone());
        self.broadcast_client(Some(client.clone()));
        self.transition(EngineLifecycleState::Running {
            backend: None,
            model_id: None,
        });

        // 8. Refresh status to get backend/model info.
        self.refresh_running_status().await;

        // 9. Restore desired model if set (retry once after 2s on failure).
        if let Some(model_id) = self.desired_model.clone() {
            log::info!("Supervisor: restoring desired model: {model_id}");
            match client.load_model(&model_id).await {
                Ok(_) => {
                    log::info!("Supervisor: model {model_id} loaded successfully");
                    self.refresh_running_status().await;
                }
                Err(e) => {
                    log::warn!(
                        "Supervisor: first attempt to restore model {model_id} failed: {e} \
                         — retrying in 2 seconds"
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    match client.load_model(&model_id).await {
                        Ok(_) => {
                            log::info!("Supervisor: model {model_id} loaded successfully on retry");
                            self.refresh_running_status().await;
                        }
                        Err(e2) => {
                            log::error!(
                                "Supervisor: retry also failed for model {model_id}: {e2} \
                                 — remaining in Running state without model"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Resolve all paths needed for spawning the engine.
    fn resolve_paths(&self) -> Result<SpawnPaths, String> {
        let app_data_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to resolve app data directory: {e}"))?;

        let shared_runtime_dir = if let Some(base) = dirs::data_local_dir() {
            base.join("SmolPC").join("engine-runtime")
        } else {
            app_data_dir.join("engine-runtime")
        };
        let data_dir = shared_runtime_dir.join("host-data");

        let resource_dir = self
            .app_handle
            .path()
            .resource_dir()
            .ok()
            .or_else(|| Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))));

        let bundled_resource_dir = select_bundled_resource_dir_resolution(
            self.app_handle
                .path()
                .resource_dir()
                .map_err(|e| e.to_string()),
            cfg!(debug_assertions),
            Some(default_dev_bundled_resource_dir()),
        )
        .map(|resolution| bundled_resource_dir_path(&resolution).to_path_buf());

        let models_dir = resolve_models_dir(bundled_resource_dir.as_deref());
        let host_binary = resolve_host_binary_path();

        let port = std::env::var("SMOLPC_ENGINE_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(DEFAULT_ENGINE_PORT);

        let app_version = self.app_handle.package_info().version.to_string();

        Ok(SpawnPaths {
            port,
            app_version,
            shared_runtime_dir,
            data_dir,
            resource_dir,
            models_dir,
            host_binary,
        })
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
        self.broadcast_pid(None);
        Ok(())
    }

    // --- Restart scheduling ---

    fn schedule_restart(&mut self) {
        // Check if we're within the restart window.
        let now = Instant::now();
        let window_start = self.last_restart_window_start.get_or_insert(now);

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
        self.health_failure_count = 0;
        self.broadcast_client(None);
        self.broadcast_pid(None);

        self.transition(EngineLifecycleState::Crashed {
            message: message.to_string(),
            restart_count: self.restart_count,
        });
    }

    fn transition(&mut self, new_state: EngineLifecycleState) {
        if self.state == new_state {
            return;
        }
        if !self.state.can_transition_to(&new_state) {
            log::error!(
                "Supervisor: invalid state transition {:?} -> {:?}, ignoring",
                self.state,
                new_state
            );
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

    fn broadcast_pid(&self, pid: Option<u32>) {
        let _ = self.pid_tx.send(pid);
    }
}

// --- Path resolution helpers ---

const DEFAULT_ENGINE_PORT: u16 = 19432;
const SHARED_MODELS_VENDOR_DIR: &str = "SmolPC";
const SHARED_MODELS_DIR: &str = "models";

/// Resolved paths for engine spawning.
struct SpawnPaths {
    port: u16,
    app_version: String,
    shared_runtime_dir: PathBuf,
    data_dir: PathBuf,
    resource_dir: Option<PathBuf>,
    models_dir: Option<PathBuf>,
    host_binary: Option<PathBuf>,
}

fn resolve_models_dir(resource_dir: Option<&std::path::Path>) -> Option<PathBuf> {
    let override_dir = std::env::var("SMOLPC_MODELS_DIR").ok().map(PathBuf::from);
    let shared_dir = dirs::data_local_dir()
        .map(|base| base.join(SHARED_MODELS_VENDOR_DIR).join(SHARED_MODELS_DIR));
    let bundled_dir = resource_dir
        .map(|res_dir| res_dir.join("models"))
        .filter(|path| path.exists());

    // Priority: env override → bundled (fresh from installer) → shared local.
    // Bundled before shared prevents a stale prior install from shadowing
    // fresh models shipped with the current version.
    override_dir
        .filter(|path| path.exists())
        .or(bundled_dir)
        .or_else(|| shared_dir.filter(|path| path.exists()))
}

fn resolve_host_binary_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("SMOLPC_ENGINE_HOST_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    let workspace_target = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join(if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        })
        .join(format!(
            "smolpc-engine-host{}",
            std::env::consts::EXE_SUFFIX
        ));
    if workspace_target.exists() {
        return Some(workspace_target);
    }

    None
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
    fn OpenProcess(
        desired_access: u32,
        inherit_handle: i32,
        process_id: u32,
    ) -> *mut std::ffi::c_void;
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
