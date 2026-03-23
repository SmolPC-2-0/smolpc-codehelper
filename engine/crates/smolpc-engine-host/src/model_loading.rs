use chrono::Utc;
use smolpc_engine_core::inference::backend::{
    BackendDecision, BackendDecisionKey, BackendSelectionState, BackendStatus,
    DecisionPersistenceState, DecisionReason, DirectMLFailureStage, FailureCounters,
    InferenceBackend, LanePreflightState,
};
use smolpc_engine_core::inference::backend_store::BackendDecisionRecord;
use smolpc_engine_core::inference::InferenceRuntimeAdapter;
use smolpc_engine_core::models::{ModelArtifactBackend, ModelLoader, ModelRegistry};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use crate::adapters::{build_directml_runtime_adapter, build_openvino_cpu_runtime_adapter};
use crate::artifacts::{
    apply_directml_device, apply_directml_startup_probe_status, apply_model_lane_artifacts,
    apply_openvino_startup_probe_status, apply_persisted_eligibility, apply_runtime_bundle_status,
    bundle_reason, decision_reason_code, resolve_model_lane_artifacts, runtime_version_value,
    sanitize_cache_component, ModelLaneArtifacts,
};
use crate::config::{parse_dml_device_id_env, parse_force_override};
use crate::openvino::{
    is_blocking_openvino_probe_failure, resolve_openvino_npu_tuning, run_openvino_preflight,
    OpenVinoPreflightReady, OpenVinoPreflightResult, OpenVinoStartupProbeResult,
};
use crate::probe::{
    current_openvino_tuning_status, directml_required_error, directml_unavailable_reason,
    model_requires_directml, model_requires_openvino, openvino_required_error, BackendProbeResult,
};
use crate::selection::{choose_preferred_backend, should_release_current_adapter_for_load};
use crate::state::EngineState;
use crate::types::{
    StartupMode, TransitionGuard, CPU_PREFLIGHT_BUDGET, DIRECTML_PREFLIGHT_BUDGET,
    OPENVINO_CHAT_MODE_STRUCTURED, OPENVINO_PREFLIGHT_BUDGET, OPENVINO_SELECTION_PROFILE,
    OPENVINO_STARTUP_PROBE_WAIT, STARTUP_PROBE_WAIT_MS,
};

/// Outcome of evaluating the OpenVINO NPU lane for model loading.
/// Captures all state that load_model needs from the evaluation.
pub(crate) struct OpenVinoLaneOutcome {
    pub preflight_state: LanePreflightState,
    pub failure_class: Option<String>,
    pub failure_message: Option<String>,
    pub ready: Option<OpenVinoPreflightReady>,
    pub reason_override: Option<DecisionReason>,
    /// If true, a timeout or pending probe means we should NOT persist
    /// this as a negative decision — it's a temporary fallback.
    pub suppress_store_update: bool,
    /// Updated persistence state if the evaluation changed it
    /// (e.g., TemporaryFallback on timeout).
    pub persistence_state_override: Option<DecisionPersistenceState>,
    /// Updated selection state if the evaluation changed it
    /// (e.g., Fallback on timeout or failure).
    pub selection_state_override: Option<BackendSelectionState>,
}

/// Outcome of running the DirectML preflight (just the preflight, not fallback logic).
/// Callers must check for artifact existence before calling `run_directml_preflight`.
pub(crate) enum DirectMLPreflightOutcome {
    /// Preflight succeeded — adapter is ready.
    Success { adapter: InferenceRuntimeAdapter },
    /// Preflight failed or timed out.
    Failed { error: String },
}

impl EngineState {
    pub(crate) fn openvino_cache_dir(
        &self,
        model_id: &str,
        artifacts: &ModelLaneArtifacts,
    ) -> PathBuf {
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

    /// Build the comprehensive BackendStatus from all collected lane data.
    /// Called after the adapter has been selected and stored.
    #[allow(clippy::too_many_arguments)]
    fn assemble_backend_status(
        &self,
        active_backend: InferenceBackend,
        active_reason: DecisionReason,
        active_model_path: &str,
        runtime_engine: &str,
        selection_state: BackendSelectionState,
        decision_persistence_state: DecisionPersistenceState,
        decision_key: &BackendDecisionKey,
        force_override: Option<InferenceBackend>,
        failure_counters: &FailureCounters,
        probe: &BackendProbeResult,
        openvino_probe: Option<&OpenVinoStartupProbeResult>,
        artifacts: &ModelLaneArtifacts,
        selected_device_id: Option<i32>,
        selected_device_name: Option<String>,
        persisted_record_decision: Option<&BackendDecision>,
        directml_preflight_state: LanePreflightState,
        openvino_preflight_state: LanePreflightState,
        directml_failure_class: Option<String>,
        directml_failure_message: Option<String>,
        openvino_failure_class: Option<String>,
        openvino_failure_message: Option<String>,
        directml_detected: bool,
    ) -> BackendStatus {
        let mut status = BackendStatus {
            active_backend: Some(active_backend),
            active_model_path: Some(active_model_path.to_string()),
            active_artifact_backend: Some(active_backend),
            runtime_engine: Some(runtime_engine.to_string()),
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
        apply_directml_startup_probe_status(&mut status, probe);
        apply_openvino_startup_probe_status(&mut status, openvino_probe);
        apply_model_lane_artifacts(&mut status, artifacts);
        let vram = probe.directml_candidate.as_ref().map(|c| c.vram_mb);
        apply_directml_device(&mut status, selected_device_id, selected_device_name, vram);
        apply_persisted_eligibility(&mut status, persisted_record_decision);
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
        status
    }

    /// Run the DirectML preflight on a worker thread with timeout.
    /// Returns Success with the adapter, or Failed with error details.
    /// Does NOT handle fallback — caller decides what to do on failure.
    async fn run_directml_preflight(
        &self,
        dml_model_path: &Path,
        device_id: Option<i32>,
    ) -> DirectMLPreflightOutcome {
        let ort_bundle = self.runtime_bundles().ort.clone();
        let dml_path_owned = dml_model_path.to_path_buf();
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
            Ok(adapter) => DirectMLPreflightOutcome::Success { adapter },
            Err(error) => DirectMLPreflightOutcome::Failed { error },
        }
    }

    /// Evaluate the OpenVINO NPU lane: check artifacts, probe status, run preflight.
    /// Returns structured outcome without mutating any load_model locals.
    async fn evaluate_openvino_lane(
        &self,
        model_id: &str,
        artifacts: &ModelLaneArtifacts,
        openvino_probe: Option<&OpenVinoStartupProbeResult>,
        openvino_bundle_ready: bool,
    ) -> OpenVinoLaneOutcome {
        let mut preflight_state = LanePreflightState::NotStarted;
        let mut failure_class = None;
        let mut failure_message = None;
        let mut ready = None;
        let mut reason_override = None;
        let mut suppress_store_update = false;
        let mut persistence_state_override = None;
        let mut selection_state_override = None;

        if !artifacts.openvino_npu_ready() {
            failure_class = artifacts.openvino_reason.clone();
            failure_message = artifacts.openvino_message.clone();
            reason_override = Some(DecisionReason::NoOpenVinoCandidate);
        } else if !openvino_bundle_ready {
            failure_class = Some(bundle_reason(
                self.runtime_bundles().openvino.failure_code(),
            ));
            failure_message = Some(format!(
                "OpenVINO runtime bundle is unavailable at {}",
                self.runtime_bundles().openvino.display_root().display()
            ));
            reason_override = Some(DecisionReason::NoOpenVinoCandidate);
        } else if openvino_probe.is_none() {
            failure_class = Some("openvino_startup_probe_pending".to_string());
            failure_message = Some("OpenVINO startup probe is still running".to_string());
            suppress_store_update = true;
            persistence_state_override = Some(DecisionPersistenceState::TemporaryFallback);
            selection_state_override = Some(BackendSelectionState::Fallback);
            reason_override = Some(DecisionReason::OpenVinoStartupProbePending);
        } else if let Some(class) = openvino_probe
            .and_then(|probe| probe.failure_class.as_deref())
            .filter(|class| is_blocking_openvino_probe_failure(class))
        {
            failure_class = Some(class.to_string());
            failure_message = openvino_probe.and_then(|probe| probe.failure_message.clone());
            reason_override = Some(DecisionReason::NoOpenVinoCandidate);
        } else if let Some(probe) = openvino_probe {
            match self
                .run_openvino_preflight_with_timeout(model_id, artifacts, probe)
                .await
            {
                OpenVinoPreflightResult::Ready(r) => {
                    preflight_state = LanePreflightState::Ready;
                    ready = Some(r);
                }
                OpenVinoPreflightResult::Timeout => {
                    preflight_state = LanePreflightState::Timeout;
                    failure_class = Some("openvino_npu_preflight_timeout".to_string());
                    failure_message = Some(format!(
                        "OpenVINO preflight exceeded the {} second budget",
                        OPENVINO_PREFLIGHT_BUDGET.as_secs()
                    ));
                    suppress_store_update = true;
                    persistence_state_override = Some(DecisionPersistenceState::TemporaryFallback);
                    selection_state_override = Some(BackendSelectionState::Fallback);
                    reason_override = Some(DecisionReason::OpenVinoPreflightTimeout);
                }
                OpenVinoPreflightResult::Failed { class, message } => {
                    preflight_state = LanePreflightState::Error;
                    failure_class = Some(class);
                    failure_message = Some(message);
                    selection_state_override = Some(BackendSelectionState::Fallback);
                    reason_override = Some(DecisionReason::OpenVinoPreflightFailed);
                }
            }
        } else {
            log::error!("OpenVINO startup probe missing unexpectedly after readiness checks");
            failure_class = Some("openvino_startup_probe_missing".to_string());
            failure_message = Some("OpenVINO startup probe was not available".to_string());
            reason_override = Some(DecisionReason::NoOpenVinoCandidate);
        }

        OpenVinoLaneOutcome {
            preflight_state,
            failure_class,
            failure_message,
            ready,
            reason_override,
            suppress_store_update,
            persistence_state_override,
            selection_state_override,
        }
    }

    pub(crate) async fn load_model(
        &self,
        model_id: String,
        startup_mode: StartupMode,
    ) -> Result<(), String> {
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

        if !cpu_model_dir.exists() {
            return Err(format!(
                "Model directory not found for '{}': {}",
                model_id,
                cpu_model_dir.display()
            ));
        }

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
            let vram = probe.directml_candidate.as_ref().map(|c| c.vram_mb);
            apply_directml_device(&mut status, device_id, device_name, vram);
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

        let openvino_outcome = if should_attempt_openvino {
            self.evaluate_openvino_lane(
                &model_id,
                &artifacts,
                openvino_probe.as_ref(),
                openvino_bundle_ready,
            )
            .await
        } else {
            OpenVinoLaneOutcome {
                preflight_state: LanePreflightState::NotStarted,
                failure_class: openvino_failure_class.take(),
                failure_message: openvino_failure_message.take(),
                ready: None,
                reason_override: None,
                suppress_store_update: false,
                persistence_state_override: None,
                selection_state_override: None,
            }
        };
        // Unpack results back into load_model locals
        openvino_preflight_state = openvino_outcome.preflight_state;
        openvino_failure_class = openvino_outcome.failure_class;
        openvino_failure_message = openvino_outcome.failure_message;
        openvino_ready = openvino_outcome.ready;
        openvino_reason_override = openvino_outcome.reason_override;
        if openvino_outcome.suppress_store_update {
            suppress_store_update = true;
        }
        if let Some(ps) = openvino_outcome.persistence_state_override {
            decision_persistence_state = ps;
        }
        if let Some(ss) = openvino_outcome.selection_state_override {
            selection_state = ss;
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
                    let dml_outcome = self
                        .run_directml_preflight(dml_path, selected_device_id)
                        .await;
                    match dml_outcome {
                        DirectMLPreflightOutcome::Success { adapter } => {
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
                        DirectMLPreflightOutcome::Failed { error } => {
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
                                .await
                                .map_err(|cpu_err| {
                                    format!(
                                    "DirectML failed: {error}; CPU fallback also failed: {cpu_err}"
                                )
                                })?;
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
                        .await
                        .map_err(|cpu_err| {
                            format!(
                                "DirectML artifact missing; CPU fallback also failed: {cpu_err}"
                            )
                        })?;
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

        let status = self.assemble_backend_status(
            active_backend,
            active_reason,
            &active_model_path,
            &runtime_engine,
            selection_state,
            decision_persistence_state,
            &decision_key,
            force_override,
            &failure_counters,
            &probe,
            openvino_probe.as_ref(),
            &artifacts,
            selected_device_id,
            selected_device_name,
            persisted_record_decision.as_ref(),
            directml_preflight_state,
            openvino_preflight_state,
            directml_failure_class,
            directml_failure_message,
            openvino_failure_class,
            openvino_failure_message,
            directml_detected,
        );
        *self.backend_status.lock().await = status;

        if force_override.is_none() && !suppress_store_update {
            self.persist_backend_record(decision_key, persisted_record_decision, failure_counters)
                .await;
        }

        Ok(())
    }
}
