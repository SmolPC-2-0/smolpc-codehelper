//! Engine supervisor types and state machine.
//!
//! This module defines the lifecycle state machine, command types, and startup
//! configuration for the engine supervisor actor pattern.

use smolpc_engine_client::{RuntimeModePreference, StartupMode};
use tokio::sync::oneshot;

// --- Lifecycle State Machine ---

/// Represents the current lifecycle state of the engine process.
///
/// Broadcast via `tokio::sync::watch` channel to all consumers.
/// Serialized as a tagged enum for Tauri event emission.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum EngineLifecycleState {
    /// No engine process. Initial state.
    Idle,
    /// Engine binary is being spawned.
    Starting,
    /// Process spawned, waiting for HTTP health endpoint to respond.
    WaitingForHealth,
    /// Health passed, engine is running. Model may or may not be loaded.
    Running {
        backend: Option<String>,
        model_id: Option<String>,
    },
    /// Engine process died unexpectedly. Will auto-restart if under limit.
    Crashed {
        message: String,
        restart_count: u32,
    },
    /// Too many crashes or unrecoverable error. User must click Retry.
    Failed { message: String },
}

impl EngineLifecycleState {
    /// Returns `true` if the engine is in the `Running` state.
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    /// Returns `true` if the state is terminal (requires user intervention).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Returns `true` if transitioning from `self` to `next` is valid.
    pub fn can_transition_to(&self, next: &Self) -> bool {
        matches!(
            (self, next),
            (Self::Idle, Self::Starting)
                | (Self::Starting, Self::WaitingForHealth)
                | (Self::Starting, Self::Failed { .. })
                | (Self::WaitingForHealth, Self::Running { .. })
                | (Self::WaitingForHealth, Self::Crashed { .. })
                | (Self::Running { .. }, Self::Running { .. })
                | (Self::Running { .. }, Self::Crashed { .. })
                | (Self::Crashed { .. }, Self::Starting)
                | (Self::Crashed { .. }, Self::Failed { .. })
                | (Self::Failed { .. }, Self::Starting)
        )
    }
}

// --- Command Types ---

/// Commands sent from the handle to the supervisor task via `mpsc` channel.
pub enum EngineCommand {
    /// Request engine startup with the given configuration.
    Start {
        config: StartupConfig,
        respond_to: oneshot::Sender<Result<(), String>>,
    },
    /// Request a runtime mode change (may trigger engine restart).
    SetRuntimeMode {
        mode: RuntimeModePreference,
        respond_to: oneshot::Sender<Result<(), String>>,
    },
    /// Update the desired model to restore after restarts.
    SetDesiredModel { model_id: Option<String> },
    /// Re-poll engine status and broadcast updated state.
    RefreshStatus,
    /// Request graceful engine shutdown.
    Shutdown {
        respond_to: oneshot::Sender<Result<(), String>>,
    },
}

// --- Startup Configuration ---

/// Configuration for starting the engine process.
#[derive(Debug, Clone)]
pub struct StartupConfig {
    /// The runtime mode preference (Auto, Cpu, Dml, Npu).
    pub runtime_mode: RuntimeModePreference,
    /// DirectML device ID override, if any.
    pub dml_device_id: Option<i32>,
    /// Default model to load after engine reaches Running state.
    pub default_model_id: Option<String>,
    /// Startup mode (normal, DirectML required, etc.).
    pub startup_mode: StartupMode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_running_returns_true_for_running_state() {
        let state = EngineLifecycleState::Running {
            backend: Some("cpu".to_string()),
            model_id: None,
        };
        assert!(state.is_running());
    }

    #[test]
    fn is_running_returns_false_for_non_running_states() {
        assert!(!EngineLifecycleState::Idle.is_running());
        assert!(!EngineLifecycleState::Starting.is_running());
        assert!(!EngineLifecycleState::WaitingForHealth.is_running());
        assert!(
            !EngineLifecycleState::Crashed {
                message: "oops".into(),
                restart_count: 1,
            }
            .is_running()
        );
        assert!(
            !EngineLifecycleState::Failed {
                message: "done".into(),
            }
            .is_running()
        );
    }

    #[test]
    fn is_terminal_only_for_failed() {
        assert!(
            EngineLifecycleState::Failed {
                message: "fatal".into(),
            }
            .is_terminal()
        );
        assert!(!EngineLifecycleState::Idle.is_terminal());
        assert!(
            !EngineLifecycleState::Crashed {
                message: "oops".into(),
                restart_count: 1,
            }
            .is_terminal()
        );
    }

    #[test]
    fn valid_transitions_are_accepted() {
        let idle = EngineLifecycleState::Idle;
        let starting = EngineLifecycleState::Starting;
        let waiting = EngineLifecycleState::WaitingForHealth;
        let running = EngineLifecycleState::Running {
            backend: None,
            model_id: None,
        };
        let crashed = EngineLifecycleState::Crashed {
            message: "oops".into(),
            restart_count: 1,
        };
        let failed = EngineLifecycleState::Failed {
            message: "fatal".into(),
        };

        assert!(idle.can_transition_to(&starting));
        assert!(starting.can_transition_to(&waiting));
        assert!(starting.can_transition_to(&failed));
        assert!(waiting.can_transition_to(&running));
        assert!(waiting.can_transition_to(&crashed));
        assert!(running.can_transition_to(&running));
        assert!(running.can_transition_to(&crashed));
        assert!(crashed.can_transition_to(&starting));
        assert!(crashed.can_transition_to(&failed));
        assert!(failed.can_transition_to(&starting));
    }

    #[test]
    fn invalid_transitions_are_rejected() {
        let idle = EngineLifecycleState::Idle;
        let running = EngineLifecycleState::Running {
            backend: None,
            model_id: None,
        };
        let failed = EngineLifecycleState::Failed {
            message: "fatal".into(),
        };

        // Can't go from Idle to Running directly
        assert!(!idle.can_transition_to(&running));
        // Can't go from Running to Idle
        assert!(!running.can_transition_to(&idle));
        // Can't go from Failed to Idle
        assert!(!failed.can_transition_to(&idle));
    }

    #[test]
    fn lifecycle_state_serializes_with_tag() {
        let idle_json = serde_json::to_string(&EngineLifecycleState::Idle).unwrap();
        assert_eq!(idle_json, r#"{"state":"idle"}"#);

        let running_json = serde_json::to_string(&EngineLifecycleState::Running {
            backend: Some("openvino_npu".to_string()),
            model_id: Some("qwen2.5-1.5b".to_string()),
        })
        .unwrap();
        assert!(running_json.contains(r#""state":"running""#));
        assert!(running_json.contains(r#""backend":"openvino_npu""#));
        assert!(running_json.contains(r#""model_id":"qwen2.5-1.5b""#));

        let crashed_json = serde_json::to_string(&EngineLifecycleState::Crashed {
            message: "health check failed".to_string(),
            restart_count: 2,
        })
        .unwrap();
        assert!(crashed_json.contains(r#""state":"crashed""#));
        assert!(crashed_json.contains(r#""restart_count":2"#));

        let failed_json = serde_json::to_string(&EngineLifecycleState::Failed {
            message: "too many restarts".to_string(),
        })
        .unwrap();
        assert!(failed_json.contains(r#""state":"failed""#));
    }
}
