use chrono::Utc;
use smolpc_engine_core::inference::backend::{
    BackendDecision, BackendDecisionKey, BackendSelectionState, BackendStatus,
    DecisionPersistenceState, DecisionReason, DirectMLFailureStage, FailureCounters,
    InferenceBackend, LanePreflightState,
};
use smolpc_engine_core::inference::backend_store::{
    backend_store_path, BackendDecisionRecord, BackendStore,
};
#[cfg(target_os = "windows")]
use smolpc_engine_core::inference::genai::GenAiDirectMlGenerator;
use smolpc_engine_core::inference::types::InferenceChatMessage;
use smolpc_engine_core::inference::{
    InferenceRuntimeAdapter, OpenVinoPipelineConfig, OpenVinoRuntimeBundle, OrtRuntimeBundle,
    OrtRuntimeLoader,
};
use smolpc_engine_core::models::{ModelArtifactBackend, ModelLoader, ModelRegistry};
use smolpc_engine_core::{GenerationConfig, GenerationMetrics};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::{Mutex, Notify, Semaphore};
use tokio::time::timeout;

use crate::artifacts::{
    apply_directml_device, apply_directml_startup_probe_status, apply_model_lane_artifacts,
    apply_openvino_startup_probe_status, apply_persisted_eligibility, apply_runtime_bundle_status,
    bundle_reason, decision_reason_code, resolve_model_lane_artifacts, runtime_version_value,
    sanitize_cache_component, ModelLaneArtifacts,
};
use crate::config::{
    classify_startup_model_error, epoch_ms, parse_dml_device_id_env, parse_force_override,
    resolve_default_model_id,
};
use crate::openvino::{
    ensure_qwen3_nothink_template, is_blocking_openvino_probe_failure,
    openvino_generation_controls_for_model, openvino_model_tuning_for_model,
    probe_openvino_startup, resolve_openvino_npu_tuning, run_openvino_preflight,
    OpenVinoPreflightResult, OpenVinoStartupProbeResult,
};
use crate::probe::{
    current_openvino_tuning_status, directml_required_error, directml_unavailable_reason,
    model_requires_directml, model_requires_openvino, openvino_required_error,
    probe_backend_capabilities, BackendProbeResult,
};
use crate::runtime_bundles::{resolve_runtime_bundles, ResolvedRuntimeBundles};
use crate::selection::{choose_preferred_backend, should_release_current_adapter_for_load};
use crate::types::{
    EnsureStartedOutcome, GenerationPermit, LastStartupError, ParsedArgs, ReadinessPayload,
    ReadinessState, StartupError, StartupMode, StartupPolicy, StartupReadiness, TransitionGuard,
    lock_cancel, CPU_PREFLIGHT_BUDGET, DIRECTML_PREFLIGHT_BUDGET, ENGINE_API_VERSION,
    OPENVINO_CHAT_MODE_STRUCTURED, OPENVINO_PREFLIGHT_BUDGET, OPENVINO_SELECTION_PROFILE,
    OPENVINO_STARTUP_PROBE_WAIT, STARTUP_PROBE_RECOVERY_WAIT_MS, STARTUP_PROBE_TOTAL_WAIT_MS,
    STARTUP_PROBE_WAIT_MS,
};

pub(crate) struct EngineState {
    pub(crate) runtime_adapter: Arc<Mutex<Option<InferenceRuntimeAdapter>>>,
    pub(crate) current_model: Arc<Mutex<Option<String>>>,
    pub(crate) backend_status: Arc<Mutex<BackendStatus>>,
    pub(crate) runtime_bundles: ResolvedRuntimeBundles,
    pub(crate) data_dir: PathBuf,
    pub(crate) active_cancel: Arc<StdMutex<Option<Arc<AtomicBool>>>>,
    pub(crate) generating: Arc<AtomicBool>,
    pub(crate) model_transition_in_progress: Arc<AtomicBool>,
    pub(crate) app_version: String,
    pub(crate) store_path: Option<PathBuf>,
    pub(crate) backend_store: Arc<Mutex<Option<BackendStore>>>,
    pub(crate) startup_probe: Arc<Mutex<Option<BackendProbeResult>>>,
    pub(crate) startup_probe_ready: Arc<Notify>,
    pub(crate) readiness: Arc<Mutex<StartupReadiness>>,
    pub(crate) startup_terminal: Arc<Notify>,
    pub(crate) startup_attempt_seq: AtomicU64,
    pub(crate) openvino_startup_probe: Arc<Mutex<Option<OpenVinoStartupProbeResult>>>,
    pub(crate) openvino_startup_probe_ready: Arc<Notify>,
}

impl EngineState {
    pub(crate) fn next_attempt_id(&self) -> String {
        let attempt = self.startup_attempt_seq.fetch_add(1, Ordering::SeqCst) + 1;
        format!("startup-{}-{attempt}", epoch_ms())
    }

    pub(crate) fn new(args: &ParsedArgs) -> Self {
        let runtime_bundles = resolve_runtime_bundles(args.resource_dir.as_deref());
        Self::new_with_runtime_bundles(args, runtime_bundles)
    }

    pub(crate) fn new_with_runtime_bundles(
        args: &ParsedArgs,
        runtime_bundles: ResolvedRuntimeBundles,
    ) -> Self {
        let store_path = match backend_store_path(&args.data_dir) {
            Ok(path) => Some(path),
            Err(error) => {
                log::warn!("Failed to resolve backend decision store path: {error}");
                None
            }
        };

        let backend_store = match store_path.as_ref() {
            Some(path) => match BackendStore::load(path) {
                Ok(store) => Some(store),
                Err(error) => {
                    log::warn!(
                        "Failed to load backend decision store {}: {error}",
                        path.display()
                    );
                    None
                }
            },
            None => None,
        };

        let mut status = BackendStatus {
            available_backends: vec![InferenceBackend::Cpu],
            selection_state: Some(BackendSelectionState::Pending),
            selection_reason: Some("startup_probe_pending".to_string()),
            openvino_message_mode: Some(OPENVINO_CHAT_MODE_STRUCTURED.to_string()),
            openvino_tuning: current_openvino_tuning_status(),
            store_path: store_path.as_ref().map(|path| path.display().to_string()),
            ..Default::default()
        };
        apply_runtime_bundle_status(&runtime_bundles, &mut status);

        Self {
            runtime_adapter: Arc::new(Mutex::new(None)),
            current_model: Arc::new(Mutex::new(None)),
            backend_status: Arc::new(Mutex::new(status)),
            runtime_bundles,
            data_dir: args.data_dir.clone(),
            active_cancel: Arc::new(StdMutex::new(None)),
            generating: Arc::new(AtomicBool::new(false)),
            model_transition_in_progress: Arc::new(AtomicBool::new(false)),
            app_version: args.app_version.clone(),
            store_path,
            backend_store: Arc::new(Mutex::new(backend_store)),
            startup_probe: Arc::new(Mutex::new(None)),
            startup_probe_ready: Arc::new(Notify::new()),
            readiness: Arc::new(Mutex::new(StartupReadiness::default())),
            startup_terminal: Arc::new(Notify::new()),
            startup_attempt_seq: AtomicU64::new(0),
            openvino_startup_probe: Arc::new(Mutex::new(None)),
            openvino_startup_probe_ready: Arc::new(Notify::new()),
        }
    }

    pub(crate) fn runtime_bundles(&self) -> &ResolvedRuntimeBundles {
        &self.runtime_bundles
    }

    pub(crate) async fn active_backend(&self) -> Option<InferenceBackend> {
        self.backend_status.lock().await.active_backend
    }

    pub(crate) async fn uses_openvino_genai_runtime(&self) -> bool {
        self.runtime_adapter
            .lock()
            .await
            .as_ref()
            .is_some_and(InferenceRuntimeAdapter::is_openvino_genai)
    }

    pub(crate) fn openvino_cache_dir(&self, model_id: &str, artifacts: &ModelLaneArtifacts) -> PathBuf {
        let model_key = sanitize_cache_component(model_id);
        let artifact_key = artifacts
            .fingerprint
            .as_deref()
            .map(sanitize_cache_component)
            .unwrap_or_else(|| "artifact-unknown".to_string());

        self.data_dir
            .join("inference")
            .join("openvino-cache")
            .join(model_key)
            .join(artifact_key)
    }

    pub(crate) async fn run_openvino_preflight_with_timeout(
        &self,
        model_id: &str,
        artifacts: &ModelLaneArtifacts,
        probe: &OpenVinoStartupProbeResult,
    ) -> OpenVinoPreflightResult {
        let Some(artifact) = artifacts.openvino_artifact.clone() else {
            return OpenVinoPreflightResult::Failed {
                class: "openvino_npu_artifact_missing".to_string(),
                message: "OpenVINO lane artifact is missing".to_string(),
            };
        };

        let bundle = self.runtime_bundles().openvino.clone();
        let cache_dir = self.openvino_cache_dir(model_id, artifacts);
        let probe = probe.clone();
        let model_id = model_id.to_string();
        let task = tokio::task::spawn_blocking(move || {
            run_openvino_preflight(&bundle, &model_id, &artifact, &probe, &cache_dir)
        });

        match timeout(OPENVINO_PREFLIGHT_BUDGET, task).await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => OpenVinoPreflightResult::Failed {
                class: "openvino_npu_preflight_join_failed".to_string(),
                message: format!("OpenVINO preflight task failed: {error}"),
            },
            Err(_) => OpenVinoPreflightResult::Timeout,
        }
    }

    pub(crate) async fn run_cpu_preflight_with_timeout(
        &self,
        model_id: &str,
        model_dir: &Path,
    ) -> Result<InferenceRuntimeAdapter, String> {
        let ov_bundle = self.runtime_bundles().openvino.clone();
        let mid = model_id.to_string();
        let mdir = model_dir.to_path_buf();
        match timeout(
            CPU_PREFLIGHT_BUDGET,
            tokio::task::spawn_blocking(move || {
                build_openvino_cpu_runtime_adapter(&ov_bundle, &mid, &mdir)
            }),
        )
        .await
        {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => Err(format!("CPU preflight task panicked: {e}")),
            Err(_) => Err(format!(
                "CPU preflight timed out after {}s",
                CPU_PREFLIGHT_BUDGET.as_secs()
            )),
        }
    }

    pub(crate) fn build_decision_key(
        &self,
        model_id: &str,
        artifacts: &ModelLaneArtifacts,
        probe: &BackendProbeResult,
        openvino_probe: Option<&OpenVinoStartupProbeResult>,
        selected_device_id: Option<i32>,
    ) -> BackendDecisionKey {
        let directml_candidate = probe.directml_candidate.as_ref();
        let openvino_probe = openvino_probe.filter(|probe| probe.device_visible);
        let openvino_tuning = resolve_openvino_npu_tuning().ok();
        BackendDecisionKey {
            model_id: model_id.to_string(),
            model_artifact_fingerprint: artifacts.fingerprint.clone(),
            app_version: self.app_version.clone(),
            selector_engine_id: "engine_host".to_string(),
            ort_runtime_version: runtime_version_value(
                &self.runtime_bundles().ort.version_metadata,
                "onnxruntime",
            ),
            ort_bundle_fingerprint: Some(self.runtime_bundles().ort.fingerprint.value.clone()),
            openvino_runtime_version: runtime_version_value(
                &self.runtime_bundles().openvino.version_metadata,
                "openvino-runtime",
            ),
            openvino_genai_version: runtime_version_value(
                &self.runtime_bundles().openvino.version_metadata,
                "openvino-genai",
            ),
            openvino_tokenizers_version: runtime_version_value(
                &self.runtime_bundles().openvino.version_metadata,
                "openvino-tokenizers",
            ),
            openvino_bundle_fingerprint: Some(
                self.runtime_bundles().openvino.fingerprint.value.clone(),
            ),
            gpu_adapter_identity: directml_candidate
                .map(|candidate| candidate.adapter_identity.clone()),
            gpu_driver_version: directml_candidate
                .map(|candidate| candidate.driver_version.clone()),
            gpu_device_id: selected_device_id,
            npu_adapter_identity: openvino_probe.and_then(|probe| probe.adapter_identity.clone()),
            npu_driver_version: openvino_probe.and_then(|probe| probe.driver_version.clone()),
            openvino_npu_max_prompt_len: openvino_tuning
                .as_ref()
                .map(|tuning| tuning.max_prompt_len),
            openvino_npu_min_response_len: openvino_tuning
                .as_ref()
                .map(|tuning| tuning.min_response_len),
            openvino_message_mode: Some(OPENVINO_CHAT_MODE_STRUCTURED.to_string()),
            selection_profile: Some(OPENVINO_SELECTION_PROFILE.to_string()),
        }
    }

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
            let hardware_detected = probed.npu_hardware_detected;
            let openvino_probe_task = tokio::task::spawn_blocking(move || {
                probe_openvino_startup(&openvino_bundle, hardware_detected)
            });
            let openvino_probe =
                match timeout(OPENVINO_STARTUP_PROBE_WAIT, openvino_probe_task).await {
                    Ok(Ok(result)) => result,
                    Ok(Err(error)) => {
                        log::warn!("OpenVINO startup probe task failed: {error}");
                        OpenVinoStartupProbeResult {
                            hardware_detected,
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
                        OpenVinoStartupProbeResult {
                            hardware_detected,
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

    pub(crate) async fn lookup_backend_record(
        &self,
        key: &BackendDecisionKey,
    ) -> Option<BackendDecisionRecord> {
        let store_guard = self.backend_store.lock().await;
        let store = store_guard.as_ref()?;
        store.get(key).cloned()
    }

    pub(crate) async fn persist_backend_record(
        &self,
        key: BackendDecisionKey,
        persisted_decision: Option<BackendDecision>,
        failure_counters: FailureCounters,
    ) {
        let mut store_guard = self.backend_store.lock().await;
        let Some(store) = store_guard.as_mut() else {
            return;
        };

        let record = BackendDecisionRecord {
            key,
            persisted_decision,
            failure_counters,
            updated_at: Utc::now().to_rfc3339(),
        };

        store.upsert(record);
        if let Err(error) = store.persist() {
            log::warn!("Failed to persist backend decision store: {error}");
        }
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
                code: "STARTUP_DEFAULT_MODEL_INVALID",
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
                code: "STARTUP_DML_REQUIRED_UNAVAILABLE",
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

    pub(crate) fn begin_generation(&self) -> Result<(GenerationPermit, Arc<AtomicBool>), String> {
        if self
            .generating
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("Generation already in progress".to_string());
        }
        let cancel_token = Arc::new(AtomicBool::new(false));
        let mut active = lock_cancel(&self.active_cancel);
        *active = Some(cancel_token.clone());
        Ok((
            GenerationPermit {
                generating: self.generating.clone(),
                active_cancel: self.active_cancel.clone(),
            },
            cancel_token,
        ))
    }

    pub(crate) async fn load_model(&self, model_id: String, startup_mode: StartupMode) -> Result<(), String> {
        if self.generating.load(Ordering::SeqCst) {
            return Err("Cannot load or unload model while generation is in progress".to_string());
        }
        self.model_transition_in_progress
            .store(true, Ordering::SeqCst);
        let _transition_guard = TransitionGuard(Arc::clone(&self.model_transition_in_progress));
        let current_backend = self.active_backend().await;
        let has_loaded_model = self.current_model.lock().await.is_some();
        let directml_required =
            model_requires_directml(&model_id) || startup_mode.requires_directml();
        let openvino_required = model_requires_openvino(&model_id);
        let model_def = ModelRegistry::get_model(&model_id)
            .ok_or_else(|| format!("Unknown model ID: {model_id}"))?;
        let cpu_model_dir = ModelLoader::openvino_dir(&model_def.directory);
        let artifacts = resolve_model_lane_artifacts(&model_def.directory);
        let dml_model_path = ModelLoader::resolve_model_file_for_backend(
            &model_def.directory,
            ModelArtifactBackend::DirectML,
        );

        let force_override = parse_force_override();
        let forced_device_id = parse_dml_device_id_env();
        let probe = self
            .wait_for_startup_probe_with_recovery(directml_required)
            .await;
        let openvino_probe_budget =
            if openvino_required || force_override == Some(InferenceBackend::OpenVinoNpu) {
                OPENVINO_STARTUP_PROBE_WAIT
            } else {
                Duration::from_millis(STARTUP_PROBE_WAIT_MS)
            };
        let openvino_probe = self
            .wait_for_openvino_startup_probe(openvino_probe_budget)
            .await;

        let directml_detected = probe.directml_candidate.is_some();
        let directml_artifact_available = artifacts.directml_ready && dml_model_path.is_some();
        let has_dml_candidate = directml_detected
            && directml_artifact_available
            && self.runtime_bundles().ort.directml_validated();
        let openvino_bundle_ready = self.runtime_bundles().openvino.npu_validated();

        if directml_required && force_override == Some(InferenceBackend::Cpu) {
            return Err(directml_required_error(
                &model_id,
                "forced CPU mode is not supported for this model",
            ));
        }
        if openvino_required && force_override == Some(InferenceBackend::Cpu) {
            return Err(openvino_required_error(
                &model_id,
                "forced CPU mode is not supported for this model",
            ));
        }
        if openvino_required && force_override == Some(InferenceBackend::DirectML) {
            return Err(openvino_required_error(
                &model_id,
                "forced DirectML mode is not supported for this model",
            ));
        }

        let probe_device_id = probe
            .directml_candidate
            .as_ref()
            .map(|candidate| candidate.device_id);
        let mut selected_device_id = forced_device_id.or(probe_device_id);
        let mut selected_device_name = if selected_device_id == probe_device_id {
            probe
                .directml_candidate
                .as_ref()
                .map(|candidate| candidate.device_name.clone())
        } else {
            None
        };

        let make_status = |selection_state: BackendSelectionState,
                           selection_reason: String,
                           device_id: Option<i32>,
                           device_name: Option<String>| {
            let mut status = BackendStatus {
                selection_state: Some(selection_state),
                selection_reason: Some(selection_reason),
                openvino_message_mode: Some(OPENVINO_CHAT_MODE_STRUCTURED.to_string()),
                openvino_tuning: current_openvino_tuning_status(),
                force_override,
                store_path: self
                    .store_path
                    .as_ref()
                    .map(|path| path.display().to_string()),
                ..Default::default()
            };
            apply_runtime_bundle_status(self.runtime_bundles(), &mut status);
            apply_directml_startup_probe_status(&mut status, &probe);
            apply_openvino_startup_probe_status(&mut status, openvino_probe.as_ref());
            apply_model_lane_artifacts(&mut status, &artifacts);
            apply_directml_device(&mut status, device_id, device_name);
            status
        };

        if let Some(forced_id) = forced_device_id {
            let forced_out_of_range = forced_id < 0
                || (probe.directml_device_count > 0
                    && forced_id as usize >= probe.directml_device_count);
            if forced_out_of_range {
                let error = format!(
                    "Invalid SMOLPC_DML_DEVICE_ID={forced_id}; detected DirectML device count={}",
                    probe.directml_device_count
                );
                if force_override == Some(InferenceBackend::DirectML) {
                    let mut status = make_status(
                        BackendSelectionState::Error,
                        "invalid_directml_device_id".to_string(),
                        Some(forced_id),
                        selected_device_name.clone(),
                    );
                    status.lanes.directml.last_failure_class =
                        Some("invalid_directml_device_id".to_string());
                    status.lanes.directml.last_failure_message = Some(error.clone());
                    *self.backend_status.lock().await = status;
                    return Err(error);
                }
                selected_device_id = probe_device_id;
                selected_device_name = probe
                    .directml_candidate
                    .as_ref()
                    .map(|candidate| candidate.device_name.clone());
            }
        }

        let decision_key = self.build_decision_key(
            &model_id,
            &artifacts,
            &probe,
            openvino_probe.as_ref(),
            selected_device_id,
        );
        let stored = self.lookup_backend_record(&decision_key).await;
        let mut failure_counters = stored
            .as_ref()
            .map(|record| record.failure_counters.clone())
            .unwrap_or_default();
        let persisted_backend = stored
            .as_ref()
            .and_then(|record| record.persisted_decision.as_ref())
            .map(|decision| decision.backend);
        let stored_openvino = persisted_backend == Some(InferenceBackend::OpenVinoNpu);
        let should_attempt_openvino = match force_override {
            Some(InferenceBackend::OpenVinoNpu) => true,
            Some(_) => false,
            None => persisted_backend.is_none() || stored_openvino,
        };
        let mut persisted_record_decision = stored
            .as_ref()
            .and_then(|record| record.persisted_decision.clone());
        let mut directml_preflight_state = LanePreflightState::NotStarted;
        let mut openvino_preflight_state = LanePreflightState::NotStarted;
        let mut directml_failure_class = None;
        let mut directml_failure_message = None;
        let mut openvino_failure_class = openvino_probe
            .as_ref()
            .and_then(|probe| probe.failure_class.clone());
        let mut openvino_failure_message = openvino_probe
            .as_ref()
            .and_then(|probe| probe.failure_message.clone());
        let mut suppress_store_update = false;
        let mut decision_persistence_state = if force_override.is_some() {
            DecisionPersistenceState::None
        } else {
            DecisionPersistenceState::Persisted
        };
        let mut selection_state = BackendSelectionState::Ready;
        let mut openvino_reason_override = None;
        let mut openvino_ready = None;

        if should_attempt_openvino {
            if !artifacts.openvino_npu_ready() {
                openvino_failure_class = artifacts.openvino_reason.clone();
                openvino_failure_message = artifacts.openvino_message.clone();
                openvino_reason_override = Some(DecisionReason::NoOpenVinoCandidate);
            } else if !openvino_bundle_ready {
                openvino_failure_class = Some(bundle_reason(
                    self.runtime_bundles().openvino.failure_code(),
                ));
                openvino_failure_message = Some(format!(
                    "OpenVINO runtime bundle is unavailable at {}",
                    self.runtime_bundles().openvino.display_root().display()
                ));
                openvino_reason_override = Some(DecisionReason::NoOpenVinoCandidate);
            } else if openvino_probe.is_none() {
                openvino_failure_class = Some("openvino_startup_probe_pending".to_string());
                openvino_failure_message =
                    Some("OpenVINO startup probe is still running".to_string());
                suppress_store_update = true;
                decision_persistence_state = DecisionPersistenceState::TemporaryFallback;
                selection_state = BackendSelectionState::Fallback;
                openvino_reason_override = Some(DecisionReason::OpenVinoStartupProbePending);
            } else if let Some(class) = openvino_probe
                .as_ref()
                .and_then(|probe| probe.failure_class.as_deref())
                .filter(|class| is_blocking_openvino_probe_failure(class))
            {
                openvino_failure_class = Some(class.to_string());
                openvino_failure_message = openvino_probe
                    .as_ref()
                    .and_then(|probe| probe.failure_message.clone());
                openvino_reason_override = Some(DecisionReason::NoOpenVinoCandidate);
            } else if let Some(probe) = openvino_probe.as_ref() {
                match self
                    .run_openvino_preflight_with_timeout(&model_id, &artifacts, probe)
                    .await
                {
                    OpenVinoPreflightResult::Ready(ready) => {
                        openvino_preflight_state = LanePreflightState::Ready;
                        openvino_ready = Some(ready);
                    }
                    OpenVinoPreflightResult::Timeout => {
                        openvino_preflight_state = LanePreflightState::Timeout;
                        openvino_failure_class = Some("openvino_npu_preflight_timeout".to_string());
                        openvino_failure_message = Some(format!(
                            "OpenVINO preflight exceeded the {} second budget",
                            OPENVINO_PREFLIGHT_BUDGET.as_secs()
                        ));
                        suppress_store_update = true;
                        decision_persistence_state = DecisionPersistenceState::TemporaryFallback;
                        selection_state = BackendSelectionState::Fallback;
                        openvino_reason_override = Some(DecisionReason::OpenVinoPreflightTimeout);
                    }
                    OpenVinoPreflightResult::Failed { class, message } => {
                        openvino_preflight_state = LanePreflightState::Error;
                        openvino_failure_class = Some(class);
                        openvino_failure_message = Some(message);
                        selection_state = BackendSelectionState::Fallback;
                        openvino_reason_override = Some(DecisionReason::OpenVinoPreflightFailed);
                    }
                }
            } else {
                log::error!("OpenVINO startup probe missing unexpectedly after readiness checks");
                openvino_failure_class = Some("openvino_startup_probe_missing".to_string());
                openvino_failure_message =
                    Some("OpenVINO startup probe was not available".to_string());
                openvino_reason_override = Some(DecisionReason::NoOpenVinoCandidate);
            }
        }

        if (force_override == Some(InferenceBackend::OpenVinoNpu) || openvino_required)
            && openvino_ready.is_none()
        {
            let selection_reason = openvino_reason_override
                .as_ref()
                .map(decision_reason_code)
                .unwrap_or_else(|| {
                    openvino_failure_class
                        .as_deref()
                        .unwrap_or("openvino_npu_forced_activation_failed")
                })
                .to_string();
            let detail = openvino_failure_message
                .clone()
                .or_else(|| artifacts.openvino_message.clone())
                .unwrap_or_else(|| format!("OpenVINO lane is unavailable: {selection_reason}"));
            let error =
                if openvino_required && force_override != Some(InferenceBackend::OpenVinoNpu) {
                    openvino_required_error(&model_id, &detail)
                } else {
                    detail
                };

            let mut status = make_status(
                BackendSelectionState::Error,
                selection_reason,
                selected_device_id,
                selected_device_name.clone(),
            );
            status.decision_key = Some(decision_key.clone());
            status.selection_fingerprint = Some(decision_key.fingerprint());
            status.last_decision = None;
            status.lanes.openvino_npu.preflight_state = openvino_preflight_state;
            status.lanes.openvino_npu.last_failure_class = openvino_failure_class.clone();
            status.lanes.openvino_npu.last_failure_message = openvino_failure_message.clone();
            *self.backend_status.lock().await = status;
            return Err(error);
        }

        let has_openvino_candidate = openvino_ready.is_some();
        let (preferred_backend, decision_reason) = choose_preferred_backend(
            force_override,
            &failure_counters,
            stored.as_ref(),
            has_dml_candidate,
            has_openvino_candidate,
        );
        let release_current_adapter = should_release_current_adapter_for_load(
            current_backend,
            preferred_backend,
            has_loaded_model,
        );

        let mut active_backend = preferred_backend;
        let mut active_reason = decision_reason.clone();
        let mut runtime_engine = "ov_genai_cpu".to_string();
        if let Some(reason) = openvino_reason_override {
            active_reason = reason;
            selection_state = BackendSelectionState::Fallback;
        }
        let active_model_path: String;

        if release_current_adapter {
            log::info!(
                "Unloading current model before reload to avoid overlapping adapter residency: current_backend={} next_backend={}",
                current_backend.map(InferenceBackend::as_str).unwrap_or("none"),
                preferred_backend.as_str(),
            );
            self.unload_model(false).await?;
        }

        let adapter = if preferred_backend == InferenceBackend::OpenVinoNpu {
            let ready = openvino_ready.take().ok_or_else(|| {
                "OpenVINO selection expected a retained preflight generator".to_string()
            })?;
            if !suppress_store_update
                && force_override.is_none()
                && active_reason != DecisionReason::PersistedDecision
            {
                persisted_record_decision = Some(BackendDecision::new(
                    InferenceBackend::OpenVinoNpu,
                    active_reason.clone(),
                    None,
                ));
            }
            runtime_engine = "ov_genai_npu".to_string();
            active_model_path = artifacts
                .openvino_artifact
                .as_ref()
                .and_then(|artifact| artifact.manifest_path.parent())
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| {
                    ModelLoader::openvino_dir(&model_def.directory)
                        .display()
                        .to_string()
                });
            InferenceRuntimeAdapter::openvino_genai(ready.generator)
        } else if preferred_backend == InferenceBackend::DirectML {
            match dml_model_path.as_deref() {
                Some(dml_path) => {
                    let ort_bundle = self.runtime_bundles().ort.clone();
                    let dml_path_owned = dml_path.to_path_buf();
                    let device_id = selected_device_id;
                    let dml_build_result = match timeout(
                        DIRECTML_PREFLIGHT_BUDGET,
                        tokio::task::spawn_blocking(move || {
                            build_directml_runtime_adapter(&ort_bundle, &dml_path_owned, device_id)
                        }),
                    )
                    .await
                    {
                        Ok(Ok(result)) => result,
                        Ok(Err(e)) => Err(format!("DirectML preflight task panicked: {e}")),
                        Err(_) => Err(format!(
                            "DirectML preflight timed out after {}s",
                            DIRECTML_PREFLIGHT_BUDGET.as_secs()
                        )),
                    };
                    match dml_build_result {
                        Ok(adapter) => {
                            failure_counters.record_directml_success();
                            if !suppress_store_update
                                && force_override.is_none()
                                && active_reason != DecisionReason::PersistedDecision
                            {
                                persisted_record_decision = Some(BackendDecision::new(
                                    InferenceBackend::DirectML,
                                    active_reason.clone(),
                                    None,
                                ));
                            }
                            runtime_engine = "genai_dml".to_string();
                            directml_preflight_state = LanePreflightState::Ready;
                            active_model_path = dml_path.display().to_string();
                            adapter
                        }
                        Err(error) => {
                            if force_override == Some(InferenceBackend::DirectML)
                                || directml_required
                            {
                                failure_counters.record_directml_failure(
                                    DirectMLFailureStage::Init,
                                    error.clone(),
                                );
                                let mut status = make_status(
                                    BackendSelectionState::Error,
                                    decision_reason_code(
                                        &DecisionReason::DirectMLInitializationFailed,
                                    )
                                    .to_string(),
                                    selected_device_id,
                                    selected_device_name.clone(),
                                );
                                status.failure_counters = failure_counters.clone();
                                status.decision_key = Some(decision_key.clone());
                                status.selection_fingerprint = Some(decision_key.fingerprint());
                                status.lanes.directml.preflight_state = LanePreflightState::Error;
                                status.lanes.directml.last_failure_class =
                                    Some("directml_initialization_failed".to_string());
                                status.lanes.directml.last_failure_message = Some(error.clone());
                                *self.backend_status.lock().await = status;
                                return Err(error);
                            }

                            failure_counters
                                .record_directml_failure(DirectMLFailureStage::Init, error.clone());
                            selection_state = BackendSelectionState::Fallback;
                            active_backend = InferenceBackend::Cpu;
                            if !matches!(
                                active_reason,
                                DecisionReason::OpenVinoStartupProbePending
                                    | DecisionReason::OpenVinoPreflightTimeout
                                    | DecisionReason::OpenVinoRuntimeUnavailable
                            ) {
                                active_reason = DecisionReason::DirectMLInitializationFailed;
                            }
                            decision_persistence_state =
                                DecisionPersistenceState::TemporaryFallback;
                            directml_preflight_state = LanePreflightState::Error;
                            directml_failure_class =
                                Some("directml_initialization_failed".to_string());
                            directml_failure_message = Some(error.clone());

                            if failure_counters.should_demote_directml() {
                                failure_counters.mark_demotion();
                                active_reason = DecisionReason::DemotedAfterFailures;
                                decision_persistence_state = DecisionPersistenceState::Persisted;
                                persisted_record_decision = Some(BackendDecision::new(
                                    InferenceBackend::Cpu,
                                    DecisionReason::DemotedAfterFailures,
                                    None,
                                ));
                            }

                            let adapter = self
                                .run_cpu_preflight_with_timeout(&model_id, &cpu_model_dir)
                                .await?;
                            active_model_path = cpu_model_dir.display().to_string();
                            adapter
                        }
                    }
                }
                None => {
                    let error =
                        "DirectML model artifact missing (expected models/<model>/dml/model.onnx)"
                            .to_string();
                    if force_override == Some(InferenceBackend::DirectML) || directml_required {
                        failure_counters
                            .record_directml_failure(DirectMLFailureStage::Init, error.clone());
                        let mut status = make_status(
                            BackendSelectionState::Error,
                            "directml_artifact_missing".to_string(),
                            selected_device_id,
                            selected_device_name.clone(),
                        );
                        status.failure_counters = failure_counters.clone();
                        status.decision_key = Some(decision_key.clone());
                        status.selection_fingerprint = Some(decision_key.fingerprint());
                        status.lanes.directml.last_failure_class =
                            Some("directml_artifact_missing".to_string());
                        status.lanes.directml.last_failure_message = Some(error.clone());
                        *self.backend_status.lock().await = status;
                        return Err(error);
                    }
                    let adapter = self
                        .run_cpu_preflight_with_timeout(&model_id, &cpu_model_dir)
                        .await?;
                    active_backend = InferenceBackend::Cpu;
                    if !matches!(
                        active_reason,
                        DecisionReason::OpenVinoStartupProbePending
                            | DecisionReason::OpenVinoPreflightFailed
                            | DecisionReason::OpenVinoPreflightTimeout
                            | DecisionReason::OpenVinoRuntimeUnavailable
                            | DecisionReason::NoOpenVinoCandidate
                    ) {
                        active_reason = DecisionReason::NoDirectMLCandidate;
                    }
                    selection_state = BackendSelectionState::Fallback;
                    decision_persistence_state =
                        if suppress_store_update || force_override.is_some() {
                            DecisionPersistenceState::TemporaryFallback
                        } else {
                            DecisionPersistenceState::Persisted
                        };
                    if !suppress_store_update && force_override.is_none() {
                        persisted_record_decision = Some(BackendDecision::new(
                            InferenceBackend::Cpu,
                            active_reason.clone(),
                            None,
                        ));
                    }
                    directml_failure_class = Some("directml_artifact_missing".to_string());
                    directml_failure_message = Some(error.clone());
                    active_model_path = cpu_model_dir.display().to_string();
                    adapter
                }
            }
        } else {
            if directml_required {
                let reason = directml_unavailable_reason(
                    directml_detected,
                    directml_artifact_available,
                    self.runtime_bundles(),
                );
                return Err(directml_required_error(&model_id, &reason));
            }
            let adapter = self
                .run_cpu_preflight_with_timeout(&model_id, &cpu_model_dir)
                .await?;
            if !suppress_store_update
                && force_override.is_none()
                && active_reason != DecisionReason::PersistedDecision
            {
                persisted_record_decision = Some(BackendDecision::new(
                    InferenceBackend::Cpu,
                    active_reason.clone(),
                    None,
                ));
            }
            if should_attempt_openvino && active_reason != DecisionReason::PersistedDecision {
                selection_state = BackendSelectionState::Fallback;
            }
            active_model_path = cpu_model_dir.display().to_string();
            adapter
        };

        *self.runtime_adapter.lock().await = Some(adapter);
        *self.current_model.lock().await = Some(model_id.clone());
        let mut status = BackendStatus {
            active_backend: Some(active_backend),
            active_model_path: Some(active_model_path),
            active_artifact_backend: Some(active_backend),
            runtime_engine: Some(runtime_engine),
            selection_state: Some(selection_state),
            selection_reason: Some(decision_reason_code(&active_reason).to_string()),
            decision_persistence_state,
            selection_fingerprint: Some(decision_key.fingerprint()),
            decision_key: Some(decision_key.clone()),
            last_decision: Some(BackendDecision::new(active_backend, active_reason, None)),
            openvino_message_mode: Some(OPENVINO_CHAT_MODE_STRUCTURED.to_string()),
            openvino_tuning: current_openvino_tuning_status(),
            failure_counters: failure_counters.clone(),
            force_override,
            store_path: self
                .store_path
                .as_ref()
                .map(|path| path.display().to_string()),
            ..Default::default()
        };
        apply_runtime_bundle_status(self.runtime_bundles(), &mut status);
        apply_directml_startup_probe_status(&mut status, &probe);
        apply_openvino_startup_probe_status(&mut status, openvino_probe.as_ref());
        apply_model_lane_artifacts(&mut status, &artifacts);
        apply_directml_device(&mut status, selected_device_id, selected_device_name);
        apply_persisted_eligibility(&mut status, persisted_record_decision.as_ref());
        status.lanes.directml.preflight_state = directml_preflight_state;
        status.lanes.openvino_npu.preflight_state = openvino_preflight_state;
        status.lanes.openvino_npu.last_failure_class = openvino_failure_class;
        status.lanes.openvino_npu.last_failure_message = openvino_failure_message;
        if active_backend == InferenceBackend::DirectML {
            status.lanes.directml.last_failure_class = None;
            status.lanes.directml.last_failure_message = None;
        } else if let Some(class) = directml_failure_class {
            status.lanes.directml.last_failure_class = Some(class);
            status.lanes.directml.last_failure_message = directml_failure_message;
        } else if !directml_detected {
            status.lanes.directml.last_failure_class =
                Some("directml_candidate_missing".to_string());
            status.lanes.directml.last_failure_message =
                Some("No DirectML-capable adapter detected".to_string());
        }
        *self.backend_status.lock().await = status;

        if force_override.is_none() && !suppress_store_update {
            self.persist_backend_record(decision_key, persisted_record_decision, failure_counters)
                .await;
        }

        Ok(())
    }

    pub(crate) async fn unload_model(&self, force: bool) -> Result<(), String> {
        if self.generating.load(Ordering::SeqCst) {
            if force {
                self.cancel();
            } else {
                return Err("Cannot unload model while generation is in progress".to_string());
            }
        }
        *self.runtime_adapter.lock().await = None;
        *self.current_model.lock().await = None;
        let mut status = self.backend_status.lock().await;
        status.active_backend = None;
        status.active_model_path = None;
        status.active_artifact_backend = None;
        status.runtime_engine = None;
        status.selection_state = Some(BackendSelectionState::Ready);
        status.selection_reason = Some("model_unloaded".to_string());
        status.decision_persistence_state = DecisionPersistenceState::None;
        status.selection_fingerprint = None;
        status.decision_key = None;
        status.last_decision = None;
        status.lanes.cpu.artifact_ready = false;
        status.lanes.directml.artifact_ready = false;
        status.lanes.openvino_npu.artifact_ready = false;
        status.lanes.cpu.persisted_eligibility = false;
        status.lanes.directml.persisted_eligibility = false;
        status.lanes.openvino_npu.persisted_eligibility = false;
        status.lanes.directml.preflight_state = LanePreflightState::NotStarted;
        status.lanes.openvino_npu.preflight_state = LanePreflightState::NotStarted;
        apply_runtime_bundle_status(self.runtime_bundles(), &mut status);
        drop(status);
        self.mark_readiness_idle_after_unload().await;
        Ok(())
    }

    pub(crate) async fn try_runtime_fallback_after_directml_failure(&self, error: &str) -> bool {
        if error.contains("INFERENCE_GENERATION_CANCELLED") {
            return false;
        }
        if parse_force_override() == Some(InferenceBackend::DirectML) {
            return false;
        }

        let status_snapshot = self.backend_status.lock().await.clone();
        if status_snapshot.active_backend != Some(InferenceBackend::DirectML) {
            return false;
        }

        let Some(model_id) = self.current_model.lock().await.clone() else {
            return false;
        };
        let Some(model_def) = ModelRegistry::get_model(&model_id) else {
            return false;
        };
        let model_artifacts = resolve_model_lane_artifacts(&model_def.directory);
        let cpu_model_dir = ModelLoader::openvino_dir(&model_def.directory);
        let Ok(cpu_adapter) = build_openvino_cpu_runtime_adapter(
            &self.runtime_bundles().openvino,
            &model_id,
            &cpu_model_dir,
        ) else {
            return false;
        };

        *self.runtime_adapter.lock().await = Some(cpu_adapter);
        let mut counters = status_snapshot.failure_counters.clone();
        counters.record_directml_failure(DirectMLFailureStage::Runtime, error.to_string());
        let mut persisted_record_decision =
            if let Some(decision_key) = status_snapshot.decision_key.as_ref() {
                self.lookup_backend_record(decision_key)
                    .await
                    .and_then(|record| record.persisted_decision)
            } else {
                None
            };
        let mut decision_reason = DecisionReason::RuntimeFailureFallback;
        let mut decision_persistence_state = DecisionPersistenceState::TemporaryFallback;
        if counters.should_demote_directml() {
            counters.mark_demotion();
            decision_reason = DecisionReason::DemotedAfterFailures;
            decision_persistence_state = DecisionPersistenceState::Persisted;
            persisted_record_decision = Some(BackendDecision::new(
                InferenceBackend::Cpu,
                DecisionReason::DemotedAfterFailures,
                None,
            ));
        }

        let mut updated = status_snapshot.clone();
        updated.active_backend = Some(InferenceBackend::Cpu);
        updated.active_artifact_backend = Some(InferenceBackend::Cpu);
        updated.runtime_engine = Some("ov_genai_cpu".to_string());
        updated.active_model_path = Some(cpu_model_dir.display().to_string());
        updated.selection_state = Some(BackendSelectionState::Fallback);
        updated.selection_reason = Some(decision_reason_code(&decision_reason).to_string());
        updated.decision_persistence_state = decision_persistence_state;
        updated.last_decision = Some(BackendDecision::new(
            InferenceBackend::Cpu,
            decision_reason,
            None,
        ));
        updated.failure_counters = counters.clone();
        updated.force_override = parse_force_override();
        apply_runtime_bundle_status(self.runtime_bundles(), &mut updated);
        apply_model_lane_artifacts(&mut updated, &model_artifacts);
        apply_persisted_eligibility(&mut updated, persisted_record_decision.as_ref());
        updated.lanes.directml.preflight_state = LanePreflightState::Ready;
        updated.lanes.directml.last_failure_class = Some("directml_runtime_failed".to_string());
        updated.lanes.directml.last_failure_message = Some(error.to_string());
        *self.backend_status.lock().await = updated;

        if let Some(decision_key) = status_snapshot.decision_key {
            self.persist_backend_record(decision_key, persisted_record_decision, counters)
                .await;
        }
        true
    }

    pub(crate) async fn generate_stream<F>(
        &self,
        prompt: &str,
        config: Option<GenerationConfig>,
        on_token: F,
    ) -> Result<GenerationMetrics, String>
    where
        F: FnMut(String),
    {
        let (_permit, cancelled) = self.begin_generation()?;
        let result = {
            let adapter_guard = self.runtime_adapter.lock().await;
            let adapter = adapter_guard
                .as_ref()
                .ok_or_else(|| "No model loaded. Call /engine/load first.".to_string())?;
            adapter
                .generate_stream(prompt, config, cancelled.clone(), on_token)
                .await
        };
        let metrics = match result {
            Ok(metrics) => metrics,
            Err(error) => {
                let recovered = self
                    .try_runtime_fallback_after_directml_failure(&error)
                    .await;
                if recovered {
                    return Err(format!(
                        "{error} [DirectML failed; backend switched to CPU — retry your request]"
                    ));
                }
                return Err(error);
            }
        };
        if cancelled.load(Ordering::SeqCst) {
            return Err("INFERENCE_GENERATION_CANCELLED: Generation cancelled".to_string());
        }
        Ok(metrics)
    }

    pub(crate) async fn generate_stream_messages<F>(
        &self,
        messages: &[InferenceChatMessage],
        config: Option<GenerationConfig>,
        on_token: F,
    ) -> Result<GenerationMetrics, String>
    where
        F: FnMut(String),
    {
        let (_permit, cancelled) = self.begin_generation()?;
        let result = {
            let adapter_guard = self.runtime_adapter.lock().await;
            let adapter = adapter_guard
                .as_ref()
                .ok_or_else(|| "No model loaded. Call /engine/load first.".to_string())?;
            adapter
                .generate_stream_messages(messages, config, cancelled.clone(), on_token)
                .await
        };
        let metrics = match result {
            Ok(metrics) => metrics,
            Err(error) => {
                let recovered = self
                    .try_runtime_fallback_after_directml_failure(&error)
                    .await;
                if recovered {
                    return Err(format!(
                        "{error} [DirectML failed; backend switched to CPU — retry your request]"
                    ));
                }
                return Err(error);
            }
        };
        if cancelled.load(Ordering::SeqCst) {
            return Err("INFERENCE_GENERATION_CANCELLED: Generation cancelled".to_string());
        }
        Ok(metrics)
    }

    pub(crate) fn cancel(&self) {
        if let Some(token) = lock_cancel(&self.active_cancel).clone() {
            token.store(true, Ordering::SeqCst);
        }
    }
}

pub(crate) fn build_openvino_cpu_runtime_adapter(
    bundle: &OpenVinoRuntimeBundle,
    model_id: &str,
    model_dir: &Path,
) -> Result<InferenceRuntimeAdapter, String> {
    if model_id.starts_with("qwen3") {
        match ensure_qwen3_nothink_template(model_dir) {
            Ok(true) => log::info!("Patched Qwen3 chat template for CPU non-thinking default"),
            Ok(false) => {}
            Err(e) => {
                return Err(format!(
                    "Qwen3 CPU requires non-thinking template but patch failed: {e}"
                ));
            }
        }
    }

    let model_tuning = openvino_model_tuning_for_model(model_id);
    let pipeline_config = OpenVinoPipelineConfig::cpu()
        .with_generation_controls(openvino_generation_controls_for_model(model_id, model_dir))
        .with_disable_thinking(model_tuning.disable_thinking);
    let generator = smolpc_engine_core::inference::OpenVinoGenAiGenerator::new(
        bundle,
        model_dir,
        &pipeline_config,
    )?;
    generator.run_preflight("Warmup preflight")?;
    Ok(InferenceRuntimeAdapter::openvino_genai(generator))
}

#[cfg(target_os = "windows")]
pub(crate) fn build_directml_runtime_adapter(
    ort_bundle: &OrtRuntimeBundle,
    dml_model_path: &Path,
    directml_device_id: Option<i32>,
) -> Result<InferenceRuntimeAdapter, String> {
    let model_dir = dml_model_path
        .parent()
        .ok_or_else(|| format!("Invalid DirectML model path: {}", dml_model_path.display()))?;
    OrtRuntimeLoader::ensure_initialized(ort_bundle)?;
    let generator = GenAiDirectMlGenerator::new(ort_bundle, model_dir, directml_device_id)?;
    generator.run_preflight("Warmup preflight")?;
    Ok(InferenceRuntimeAdapter::genai_directml(generator))
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn build_directml_runtime_adapter(
    _ort_bundle: &OrtRuntimeBundle,
    _dml_model_path: &Path,
    _directml_device_id: Option<i32>,
) -> Result<InferenceRuntimeAdapter, String> {
    Err("DirectML is only supported on Windows".to_string())
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) token: Arc<String>,
    pub(crate) engine: Arc<EngineState>,
    pub(crate) generation_semaphore: Arc<Semaphore>,
    pub(crate) queue_semaphore: Arc<Semaphore>,
    pub(crate) queue_timeout: Duration,
    pub(crate) shutdown: Arc<Notify>,
    pub(crate) last_activity_ms: Arc<AtomicU64>,
}
