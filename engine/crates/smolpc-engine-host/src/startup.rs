use chrono::Utc;
use smolpc_engine_core::inference::backend::{BackendSelectionState, InferenceBackend};
use smolpc_engine_core::models::ModelRegistry;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use crate::artifacts::{
    apply_directml_startup_probe_status, apply_openvino_startup_probe_status,
    apply_runtime_bundle_status,
};
use crate::config::{classify_startup_model_error, resolve_default_model_id};
use crate::openvino::{probe_openvino_startup, OpenVinoStartupProbeResult};
use crate::probe::{probe_backend_capabilities, BackendProbeResult};
use crate::state::EngineState;
use crate::types::{
    EnsureStartedOutcome, LastStartupError, ReadinessPayload, ReadinessState, StartupError,
    StartupMode, StartupPolicy, ENGINE_API_VERSION, OPENVINO_STARTUP_PROBE_WAIT,
    STARTUP_DEFAULT_MODEL_INVALID, STARTUP_DML_REQUIRED_UNAVAILABLE,
    STARTUP_PROBE_RECOVERY_WAIT_MS, STARTUP_PROBE_TOTAL_WAIT_MS, STARTUP_PROBE_WAIT_MS,
};

impl EngineState {
    pub(crate) fn launch_startup_probe(self: &Arc<Self>) {
        let engine = Arc::clone(self);
        tokio::spawn(async move {
            let probe_budget = Duration::from_millis(STARTUP_PROBE_TOTAL_WAIT_MS);
            let probed = match timeout(
                probe_budget,
                tokio::task::spawn_blocking(probe_backend_capabilities),
            )
            .await
            {
                Ok(Ok(probed)) => probed,
                Ok(Err(error)) => {
                    log::warn!("Backend startup probe task failed: {error}");
                    BackendProbeResult::directml_probe_failure(
                        "directml_startup_probe_failed",
                        format!("DirectML startup probe task failed: {error}"),
                    )
                }
                Err(_) => {
                    log::warn!(
                        "Backend startup probe timed out after {} ms",
                        probe_budget.as_millis()
                    );
                    BackendProbeResult::directml_probe_failure(
                        "directml_startup_probe_timeout",
                        format!(
                            "DirectML hardware probe did not complete within {} ms",
                            probe_budget.as_millis()
                        ),
                    )
                }
            };

            {
                let mut probe_guard = engine.startup_probe.lock().await;
                *probe_guard = Some(probed.clone());
            }

            {
                let mut status = engine.backend_status.lock().await;
                apply_runtime_bundle_status(engine.runtime_bundles(), &mut status);
                apply_directml_startup_probe_status(&mut status, &probed);
                if status.selection_state == Some(BackendSelectionState::Pending) {
                    status.selection_state = Some(BackendSelectionState::Ready);
                    status.selection_reason = Some(if probed.directml_candidate.is_some() {
                        "startup_probe_ready".to_string()
                    } else if let Some(class) = probed.directml_probe_failure_class.as_deref() {
                        class.to_string()
                    } else {
                        "startup_probe_cpu_only".to_string()
                    });
                }
            }

            engine.startup_probe_ready.notify_waiters();

            let openvino_bundle = engine.runtime_bundles().openvino.clone();
            let openvino_probe_task = tokio::task::spawn_blocking(move || {
                probe_openvino_startup(&openvino_bundle)
            });
            let openvino_probe =
                match timeout(OPENVINO_STARTUP_PROBE_WAIT, openvino_probe_task).await {
                    Ok(Ok(result)) => result,
                    Ok(Err(error)) => {
                        log::warn!("OpenVINO startup probe task failed: {error}");
                        // Timeout/error means unknown — probe didn't complete.
                        OpenVinoStartupProbeResult {
                            failure_class: Some("openvino_npu_plugin_unavailable".to_string()),
                            failure_message: Some(format!(
                                "OpenVINO startup probe task failed: {error}"
                            )),
                            ..Default::default()
                        }
                    }
                    Err(_) => {
                        log::warn!(
                            "OpenVINO startup probe timed out after {} ms",
                            OPENVINO_STARTUP_PROBE_WAIT.as_millis()
                        );
                        // Timeout means unknown — probe didn't complete.
                        OpenVinoStartupProbeResult {
                            failure_class: Some("openvino_startup_probe_timeout".to_string()),
                            failure_message: Some(format!(
                                "OpenVINO startup probe did not complete within {} ms",
                                OPENVINO_STARTUP_PROBE_WAIT.as_millis()
                            )),
                            ..Default::default()
                        }
                    }
                };

            {
                let mut probe_guard = engine.openvino_startup_probe.lock().await;
                *probe_guard = Some(openvino_probe.clone());
            }

            {
                let mut status = engine.backend_status.lock().await;
                apply_runtime_bundle_status(engine.runtime_bundles(), &mut status);
                apply_directml_startup_probe_status(&mut status, &probed);
                apply_openvino_startup_probe_status(&mut status, Some(&openvino_probe));
            }

            engine.openvino_startup_probe_ready.notify_waiters();
        });
    }

    pub(crate) async fn wait_for_startup_probe(&self, budget: Duration) -> BackendProbeResult {
        if let Some(existing) = self.startup_probe.lock().await.clone() {
            return existing;
        }

        let wait = self.startup_probe_ready.notified();
        let _ = timeout(budget, wait).await;
        self.startup_probe.lock().await.clone().unwrap_or_default()
    }

    pub(crate) async fn wait_for_startup_probe_with_recovery(
        &self,
        require_directml: bool,
    ) -> BackendProbeResult {
        let mut probe = self
            .wait_for_startup_probe(Duration::from_millis(STARTUP_PROBE_WAIT_MS))
            .await;
        if require_directml
            && !probe
                .available_backends
                .contains(&InferenceBackend::DirectML)
        {
            // DirectML enumeration may still be in flight after the initial budget.
            // If the probe has already settled, this returns immediately.
            probe = self
                .wait_for_startup_probe(Duration::from_millis(STARTUP_PROBE_RECOVERY_WAIT_MS))
                .await;
        }
        probe
    }

    pub(crate) async fn wait_for_openvino_startup_probe(
        &self,
        budget: Duration,
    ) -> Option<OpenVinoStartupProbeResult> {
        if let Some(existing) = self.openvino_startup_probe.lock().await.clone() {
            return Some(existing);
        }

        let wait = self.openvino_startup_probe_ready.notified();
        let _ = timeout(budget, wait).await;
        self.openvino_startup_probe.lock().await.clone()
    }

    pub(crate) async fn transition_readiness(&self, next: ReadinessState) {
        let mut readiness = self.readiness.lock().await;
        readiness.transition(next);
    }

    pub(crate) async fn mark_readiness_failed(&self, error: StartupError) {
        let mut readiness = self.readiness.lock().await;
        readiness.mark_failed(error.phase, error.code, error.message, error.retryable);
    }

    pub(crate) async fn mark_readiness_ready(&self) {
        let mut readiness = self.readiness.lock().await;
        readiness.mark_ready();
    }

    pub(crate) async fn mark_readiness_idle_after_unload(&self) {
        let mut readiness = self.readiness.lock().await;
        if readiness.state.is_starting() {
            return;
        }
        readiness.mark_idle();
    }

    pub(crate) async fn mark_ready_after_external_load(&self, model_id: String) {
        let mut readiness = self.readiness.lock().await;
        if readiness.state.is_starting() {
            return;
        }
        if readiness.state != ReadinessState::Ready {
            readiness.begin_attempt(
                self.next_attempt_id(),
                StartupMode::Auto,
                StartupPolicy {
                    default_model_id: Some(model_id.clone()),
                },
            );
        } else {
            readiness.effective_mode = StartupMode::Auto;
            readiness.effective_startup_policy = StartupPolicy {
                default_model_id: Some(model_id.clone()),
            };
        }
        readiness.mark_ready();
    }

    pub(crate) async fn current_readiness_payload(
        &self,
        ok: bool,
        override_error: Option<StartupError>,
    ) -> ReadinessPayload {
        let readiness = self.readiness.lock().await.clone();
        let current_model = self.current_model.lock().await.clone();
        let backend_status = self.backend_status.lock().await.clone();
        let active_backend = backend_status.active_backend;
        let active_model_id = current_model.clone();
        let ready = readiness.state == ReadinessState::Ready
            && active_backend.is_some()
            && active_model_id.is_some();

        let (error_code, error_message, retryable, last_error) = match override_error {
            Some(error) => {
                let stamped = LastStartupError {
                    attempt_id: readiness.attempt_id.clone(),
                    phase: error.phase,
                    code: error.code.to_string(),
                    message: error.message.clone(),
                    retryable: error.retryable,
                    at: Utc::now().to_rfc3339(),
                };
                (
                    Some(error.code.to_string()),
                    Some(error.message),
                    Some(error.retryable),
                    Some(stamped),
                )
            }
            None => (
                readiness.error_code.clone(),
                readiness.error_message.clone(),
                readiness.retryable,
                readiness.last_error.clone(),
            ),
        };

        ReadinessPayload {
            ok,
            ready,
            attempt_id: readiness.attempt_id,
            state: readiness.state,
            startup_phase: readiness.state,
            state_since: readiness.state_since,
            active_backend,
            active_model_id,
            error_code,
            error_message,
            retryable,
            last_error,
            engine_version: env!("CARGO_PKG_VERSION"),
            engine_api_version: ENGINE_API_VERSION,
            effective_mode: readiness.effective_mode,
            effective_startup_policy: readiness.effective_startup_policy,
            current_model,
            generating: self.generating.load(Ordering::SeqCst),
            backend_status,
        }
    }

    pub(crate) async fn run_startup_attempt(
        &self,
        mode: StartupMode,
        default_model_id: String,
    ) -> Result<(), StartupError> {
        self.transition_readiness(ReadinessState::ResolvingAssets)
            .await;
        if ModelRegistry::get_model(&default_model_id).is_none() {
            return Err(StartupError {
                phase: ReadinessState::LoadingModel,
                code: STARTUP_DEFAULT_MODEL_INVALID,
                message: format!("Unknown default model id '{default_model_id}'"),
                retryable: false,
            });
        }

        self.transition_readiness(ReadinessState::Probing).await;
        let probe = self
            .wait_for_startup_probe_with_recovery(mode.requires_directml())
            .await;
        let has_directml = probe
            .available_backends
            .contains(&InferenceBackend::DirectML);
        if mode.requires_directml() && !has_directml {
            return Err(StartupError {
                phase: ReadinessState::Probing,
                code: STARTUP_DML_REQUIRED_UNAVAILABLE,
                message: "DirectML adapter is required but unavailable.".to_string(),
                retryable: false,
            });
        }

        self.transition_readiness(ReadinessState::LoadingModel)
            .await;
        self.load_model(default_model_id, mode)
            .await
            .map_err(|error| classify_startup_model_error(&error))?;
        self.mark_readiness_ready().await;
        Ok(())
    }

    pub(crate) async fn ensure_started(
        &self,
        mode: StartupMode,
        startup_policy: StartupPolicy,
    ) -> EnsureStartedOutcome {
        let default_model_id = match resolve_default_model_id(&startup_policy) {
            Ok(model_id) => model_id,
            Err(error) => {
                let mut readiness = self.readiness.lock().await;
                readiness.begin_attempt(self.next_attempt_id(), mode, startup_policy);
                readiness.mark_failed(error.phase, error.code, error.message, error.retryable);
                drop(readiness);
                self.startup_terminal.notify_waiters();
                return EnsureStartedOutcome::Failed;
            }
        };
        let effective_startup_policy = StartupPolicy {
            default_model_id: Some(default_model_id.clone()),
        };

        let mut readiness = self.readiness.lock().await;
        match readiness.state {
            ReadinessState::Ready => {
                let matches_policy = readiness.effective_mode == mode
                    && readiness.effective_startup_policy == effective_startup_policy;
                return if matches_policy {
                    EnsureStartedOutcome::Ready
                } else {
                    EnsureStartedOutcome::Conflict
                };
            }
            state if state.is_starting() => {
                let joined = self.startup_terminal.notified();
                drop(readiness);
                joined.await;
                let terminal = self.readiness.lock().await.state;
                return if terminal == ReadinessState::Ready {
                    EnsureStartedOutcome::Ready
                } else {
                    EnsureStartedOutcome::Failed
                };
            }
            ReadinessState::Idle | ReadinessState::Failed => {
                readiness.begin_attempt(
                    self.next_attempt_id(),
                    mode,
                    effective_startup_policy.clone(),
                );
            }
            ReadinessState::Starting
            | ReadinessState::ResolvingAssets
            | ReadinessState::Probing
            | ReadinessState::LoadingModel => {}
        }
        drop(readiness);

        let outcome = match self.run_startup_attempt(mode, default_model_id).await {
            Ok(()) => EnsureStartedOutcome::Ready,
            Err(error) => {
                self.mark_readiness_failed(error).await;
                EnsureStartedOutcome::Failed
            }
        };
        self.startup_terminal.notify_waiters();
        outcome
    }
}
