use smolpc_engine_core::inference::backend::{
    BackendDecision, BackendSelectionState, BackendStatus, DecisionPersistenceState,
    DecisionReason, DirectMLFailureStage, InferenceBackend, LanePreflightState,
};
use smolpc_engine_core::inference::backend_store::{backend_store_path, BackendStore};
use smolpc_engine_core::inference::types::InferenceChatMessage;
use smolpc_engine_core::inference::InferenceRuntimeAdapter;
use smolpc_engine_core::models::{ModelLoader, ModelRegistry};
use smolpc_engine_core::{GenerationConfig, GenerationMetrics};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::{Mutex, Notify, Semaphore};

use crate::adapters::build_openvino_cpu_runtime_adapter;
use crate::artifacts::{
    apply_model_lane_artifacts, apply_persisted_eligibility, apply_runtime_bundle_status,
    decision_reason_code, resolve_model_lane_artifacts,
};
use crate::config::{epoch_ms, parse_force_override, with_memory_pressure_hint};
use crate::openvino::OpenVinoStartupProbeResult;
use crate::probe::{current_openvino_tuning_status, BackendProbeResult};
use crate::runtime_bundles::{resolve_runtime_bundles, ResolvedRuntimeBundles};
use crate::types::{
    lock_cancel, GenerationPermit, ParsedArgs, StartupReadiness, OPENVINO_CHAT_MODE_STRUCTURED,
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
                let current_model = self.current_model.lock().await.clone();
                let hinted_error = with_memory_pressure_hint(&error, current_model.as_deref());
                let recovered = self
                    .try_runtime_fallback_after_directml_failure(&error)
                    .await;
                if recovered {
                    return Err(format!(
                        "{hinted_error} [DirectML failed; backend switched to CPU — retry your request]"
                    ));
                }
                return Err(hinted_error);
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
                let current_model = self.current_model.lock().await.clone();
                let hinted_error = with_memory_pressure_hint(&error, current_model.as_deref());
                let recovered = self
                    .try_runtime_fallback_after_directml_failure(&error)
                    .await;
                if recovered {
                    return Err(format!(
                        "{hinted_error} [DirectML failed; backend switched to CPU — retry your request]"
                    ));
                }
                return Err(hinted_error);
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
