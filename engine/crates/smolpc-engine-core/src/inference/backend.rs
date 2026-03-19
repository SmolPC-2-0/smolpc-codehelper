use chrono::Utc;
use serde::{Deserialize, Serialize};

pub const BENCHMARK_SELECTION_BUDGET_MS: u64 = 2_000;
pub const DIRECTML_MIN_DECODE_SPEEDUP_RATIO: f64 = 1.30;
pub const DIRECTML_MAX_TTFT_REGRESSION_RATIO: f64 = 1.15;
pub const DIRECTML_DEMOTION_THRESHOLD: u32 = 3;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum InferenceBackend {
    Cpu,
    #[serde(rename = "directml")]
    DirectML,
    #[serde(rename = "openvino_npu")]
    OpenVinoNpu,
}

impl InferenceBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::DirectML => "directml",
            Self::OpenVinoNpu => "openvino_npu",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionReason {
    DefaultCpu,
    #[serde(rename = "default_openvino_candidate")]
    DefaultOpenVinoCandidate,
    #[serde(
        rename = "default_directml_candidate",
        alias = "default_direct_m_l_candidate"
    )]
    DefaultDirectMLCandidate,
    ForcedOverride,
    PersistedDecision,
    BenchmarkPassed,
    #[serde(
        rename = "benchmark_directml_decode_too_slow",
        alias = "benchmark_decode_too_slow"
    )]
    BenchmarkDecodeTooSlow,
    BenchmarkTtftTooHigh,
    BenchmarkBudgetExceeded,
    #[serde(rename = "no_directml_candidate", alias = "no_direct_m_l_candidate")]
    NoDirectMLCandidate,
    #[serde(
        rename = "directml_initialization_failed",
        alias = "direct_m_l_initialization_failed"
    )]
    DirectMLInitializationFailed,
    #[serde(
        rename = "directml_preflight_failed",
        alias = "direct_m_l_preflight_failed"
    )]
    DirectMLPreflightFailed,
    #[serde(rename = "no_openvino_candidate")]
    NoOpenVinoCandidate,
    #[serde(rename = "openvino_startup_probe_pending")]
    OpenVinoStartupProbePending,
    #[serde(rename = "openvino_preflight_failed")]
    OpenVinoPreflightFailed,
    #[serde(rename = "openvino_preflight_timeout")]
    OpenVinoPreflightTimeout,
    #[serde(rename = "openvino_runtime_unavailable")]
    OpenVinoRuntimeUnavailable,
    RuntimeFailureFallback,
    DemotedAfterFailures,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendDecisionKey {
    pub model_id: String,
    #[serde(default)]
    pub model_artifact_fingerprint: Option<String>,
    pub app_version: String,
    #[serde(default)]
    pub selector_engine_id: String,
    #[serde(default)]
    pub ort_runtime_version: Option<String>,
    #[serde(default)]
    pub ort_bundle_fingerprint: Option<String>,
    #[serde(default)]
    pub openvino_runtime_version: Option<String>,
    #[serde(default)]
    pub openvino_genai_version: Option<String>,
    #[serde(default)]
    pub openvino_tokenizers_version: Option<String>,
    #[serde(default)]
    pub openvino_bundle_fingerprint: Option<String>,
    #[serde(default)]
    pub gpu_adapter_identity: Option<String>,
    #[serde(default)]
    pub gpu_driver_version: Option<String>,
    #[serde(default)]
    pub gpu_device_id: Option<i32>,
    #[serde(default)]
    pub npu_adapter_identity: Option<String>,
    #[serde(default)]
    pub npu_driver_version: Option<String>,
    #[serde(default)]
    pub openvino_npu_max_prompt_len: Option<usize>,
    #[serde(default)]
    pub openvino_npu_min_response_len: Option<usize>,
    #[serde(default)]
    pub openvino_message_mode: Option<String>,
    #[serde(default)]
    pub selection_profile: Option<String>,
}

impl BackendDecisionKey {
    pub fn fingerprint(&self) -> String {
        let gpu_device_id = self
            .gpu_device_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "none".to_string());
        let openvino_npu_max_prompt_len = self
            .openvino_npu_max_prompt_len
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string());
        let openvino_npu_min_response_len = self
            .openvino_npu_min_response_len
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string());
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.model_id,
            self.model_artifact_fingerprint.as_deref().unwrap_or("none"),
            self.app_version,
            self.selector_engine_id,
            self.ort_runtime_version.as_deref().unwrap_or("none"),
            self.ort_bundle_fingerprint.as_deref().unwrap_or("none"),
            self.openvino_runtime_version.as_deref().unwrap_or("none"),
            self.openvino_genai_version.as_deref().unwrap_or("none"),
            self.openvino_tokenizers_version
                .as_deref()
                .unwrap_or("none"),
            self.openvino_bundle_fingerprint
                .as_deref()
                .unwrap_or("none"),
            self.gpu_adapter_identity.as_deref().unwrap_or("none"),
            self.gpu_driver_version.as_deref().unwrap_or("none"),
            gpu_device_id,
            self.npu_adapter_identity.as_deref().unwrap_or("none"),
            self.npu_driver_version.as_deref().unwrap_or("none"),
            openvino_npu_max_prompt_len,
            openvino_npu_min_response_len,
            self.openvino_message_mode.as_deref().unwrap_or("none"),
            self.selection_profile.as_deref().unwrap_or("none")
        )
        .to_ascii_lowercase()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BackendOpenVinoTuningStatus {
    pub max_prompt_len: Option<usize>,
    pub min_response_len: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackendBenchmark {
    pub backend: InferenceBackend,
    pub sample_tokens: usize,
    pub total_time_ms: u64,
    pub time_to_first_token_ms: u64,
    pub overall_tokens_per_second: f64,
    pub decode_tokens_per_second: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackendBenchmarkComparison {
    pub cpu: BackendBenchmark,
    pub directml: BackendBenchmark,
    pub elapsed_ms: u64,
    pub budget_ms: u64,
}

impl BackendBenchmarkComparison {
    pub fn directml_decode_speedup_ratio(&self) -> f64 {
        if self.cpu.decode_tokens_per_second <= f64::EPSILON {
            0.0
        } else {
            self.directml.decode_tokens_per_second / self.cpu.decode_tokens_per_second
        }
    }

    pub fn directml_ttft_ratio(&self) -> f64 {
        if self.cpu.time_to_first_token_ms == 0 {
            if self.directml.time_to_first_token_ms == 0 {
                1.0
            } else {
                f64::INFINITY
            }
        } else {
            self.directml.time_to_first_token_ms as f64 / self.cpu.time_to_first_token_ms as f64
        }
    }

    pub fn directml_passes_gate(&self) -> bool {
        self.directml_decode_speedup_ratio() >= DIRECTML_MIN_DECODE_SPEEDUP_RATIO
            && self.directml_ttft_ratio() <= DIRECTML_MAX_TTFT_REGRESSION_RATIO
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DirectMLFailureStage {
    Init,
    Runtime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FailureCounters {
    pub directml_init_failures: u32,
    pub directml_runtime_failures: u32,
    pub directml_total_failures: u32,
    pub directml_consecutive_failures: u32,
    pub demotions: u32,
    pub last_failure_stage: Option<DirectMLFailureStage>,
    pub last_failure_reason: Option<String>,
    pub last_failure_at: Option<String>,
}

impl FailureCounters {
    pub fn record_directml_failure(
        &mut self,
        stage: DirectMLFailureStage,
        reason: impl Into<String>,
    ) {
        match stage {
            DirectMLFailureStage::Init => self.directml_init_failures += 1,
            DirectMLFailureStage::Runtime => self.directml_runtime_failures += 1,
        }
        self.directml_total_failures += 1;
        self.directml_consecutive_failures += 1;
        self.last_failure_stage = Some(stage);
        self.last_failure_reason = Some(reason.into());
        self.last_failure_at = Some(Utc::now().to_rfc3339());
    }

    pub fn record_directml_success(&mut self) {
        self.directml_consecutive_failures = 0;
    }

    pub fn should_demote_directml(&self) -> bool {
        self.directml_consecutive_failures >= DIRECTML_DEMOTION_THRESHOLD
    }

    pub fn mark_demotion(&mut self) {
        self.demotions += 1;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackendDecision {
    pub backend: InferenceBackend,
    pub reason: DecisionReason,
    pub benchmark: Option<BackendBenchmarkComparison>,
    pub decided_at: String,
}

impl BackendDecision {
    pub fn new(
        backend: InferenceBackend,
        reason: DecisionReason,
        benchmark: Option<BackendBenchmarkComparison>,
    ) -> Self {
        Self {
            backend,
            reason,
            benchmark,
            decided_at: Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackendSelectionState {
    Pending,
    Ready,
    Fallback,
    Error,
}

impl Default for BackendSelectionState {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionPersistenceState {
    None,
    Persisted,
    TemporaryFallback,
}

impl Default for DecisionPersistenceState {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LaneStartupProbeState {
    NotStarted,
    Ready,
    Error,
}

impl Default for LaneStartupProbeState {
    fn default() -> Self {
        Self::NotStarted
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LanePreflightState {
    NotStarted,
    Pending,
    Ready,
    Timeout,
    Error,
}

impl Default for LanePreflightState {
    fn default() -> Self {
        Self::NotStarted
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LaneCacheState {
    Unknown,
    Cold,
    Warm,
}

impl Default for LaneCacheState {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendSelectedDevice {
    pub backend: InferenceBackend,
    pub device_id: Option<i32>,
    pub device_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BackendRuntimeBundleStatus {
    pub root: Option<String>,
    pub fingerprint: Option<String>,
    pub validated: bool,
    pub failure: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BackendRuntimeBundlesStatus {
    pub load_mode: Option<String>,
    pub ort: BackendRuntimeBundleStatus,
    pub directml: BackendRuntimeBundleStatus,
    pub openvino: BackendRuntimeBundleStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BackendLaneStatus {
    pub detected: bool,
    pub bundle_ready: bool,
    pub artifact_ready: bool,
    pub startup_probe_state: LaneStartupProbeState,
    pub preflight_state: LanePreflightState,
    pub persisted_eligibility: bool,
    pub last_failure_class: Option<String>,
    pub last_failure_message: Option<String>,
    pub driver_version: Option<String>,
    pub runtime_version: Option<String>,
    pub cache_state: LaneCacheState,
    pub device_id: Option<i32>,
    pub device_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BackendLaneStatuses {
    pub openvino_npu: BackendLaneStatus,
    pub directml: BackendLaneStatus,
    pub cpu: BackendLaneStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct BackendStatus {
    pub active_backend: Option<InferenceBackend>,
    pub active_model_path: Option<String>,
    pub active_artifact_backend: Option<InferenceBackend>,
    pub runtime_engine: Option<String>,
    pub available_backends: Vec<InferenceBackend>,
    pub selected_device: Option<BackendSelectedDevice>,
    pub selection_state: Option<BackendSelectionState>,
    pub selection_reason: Option<String>,
    pub decision_persistence_state: DecisionPersistenceState,
    pub selection_fingerprint: Option<String>,
    pub decision_key: Option<BackendDecisionKey>,
    pub last_decision: Option<BackendDecision>,
    pub runtime_bundles: BackendRuntimeBundlesStatus,
    pub lanes: BackendLaneStatuses,
    pub openvino_message_mode: Option<String>,
    pub openvino_tuning: Option<BackendOpenVinoTuningStatus>,
    pub failure_counters: FailureCounters,
    pub force_override: Option<InferenceBackend>,
    pub store_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelLaneReadiness {
    pub artifact_ready: bool,
    pub bundle_ready: bool,
    pub ready: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ModelLaneReadinessByBackend {
    pub openvino_npu: ModelLaneReadiness,
    pub directml: ModelLaneReadiness,
    pub cpu: ModelLaneReadiness,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckModelResponse {
    pub model_id: String,
    pub lanes: ModelLaneReadinessByBackend,
}

impl CheckModelResponse {
    pub fn any_ready(&self) -> bool {
        self.lanes.openvino_npu.ready || self.lanes.directml.ready || self.lanes.cpu.ready
    }

    pub fn any_artifact_ready(&self) -> bool {
        self.lanes.openvino_npu.artifact_ready
            || self.lanes.directml.artifact_ready
            || self.lanes.cpu.artifact_ready
    }
}

impl Default for ModelLaneReadiness {
    fn default() -> Self {
        Self {
            artifact_ready: false,
            bundle_ready: false,
            ready: false,
            reason: "not_ready".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn benchmark(
        backend: InferenceBackend,
        decode_tps: f64,
        ttft_ms: u64,
        total_ms: u64,
    ) -> BackendBenchmark {
        BackendBenchmark {
            backend,
            sample_tokens: 8,
            total_time_ms: total_ms,
            time_to_first_token_ms: ttft_ms,
            overall_tokens_per_second: decode_tps,
            decode_tokens_per_second: decode_tps,
        }
    }

    fn decision_key() -> BackendDecisionKey {
        BackendDecisionKey {
            model_id: "qwen2.5-1.5b-instruct".to_string(),
            model_artifact_fingerprint: Some("artifact-v1".to_string()),
            app_version: "2.2.0".to_string(),
            selector_engine_id: "engine_host".to_string(),
            ort_runtime_version: Some("2.0.0-rc.11".to_string()),
            ort_bundle_fingerprint: Some("ort-bundle-v1".to_string()),
            openvino_runtime_version: Some("2026.0.0".to_string()),
            openvino_genai_version: Some("2026.0.0".to_string()),
            openvino_tokenizers_version: Some("2026.0.0".to_string()),
            openvino_bundle_fingerprint: Some("openvino-bundle-v1".to_string()),
            gpu_adapter_identity: Some("intel:arc:a370m".to_string()),
            gpu_driver_version: Some("31.0.101.5522".to_string()),
            gpu_device_id: Some(0),
            npu_adapter_identity: Some("intel:npu".to_string()),
            npu_driver_version: Some("32.0.100.3104".to_string()),
            openvino_npu_max_prompt_len: Some(256),
            openvino_npu_min_response_len: Some(8),
            openvino_message_mode: Some("structured_messages".to_string()),
            selection_profile: Some("default".to_string()),
        }
    }

    #[test]
    fn backend_decision_key_keeps_legacy_payload_compatible() {
        let payload = serde_json::json!({
            "model_id": "qwen3-4b-instruct-2507",
            "app_version": "1.0.0"
        });

        let key: BackendDecisionKey =
            serde_json::from_value(payload).expect("legacy decision key should deserialize");
        assert_eq!(key.selector_engine_id, "");
        assert_eq!(key.model_id, "qwen3-4b-instruct-2507");
    }

    #[test]
    fn decision_key_fingerprint_changes_when_gpu_driver_changes() {
        let key_a = decision_key();
        let mut key_b = key_a.clone();
        key_b.gpu_driver_version = Some("31.0.101.5590".to_string());

        assert_ne!(key_a.fingerprint(), key_b.fingerprint());
    }

    #[test]
    fn decision_key_fingerprint_changes_when_openvino_bundle_changes() {
        let key_a = decision_key();
        let mut key_b = key_a.clone();
        key_b.openvino_bundle_fingerprint = Some("openvino-bundle-v2".to_string());

        assert_ne!(key_a.fingerprint(), key_b.fingerprint());
    }

    #[test]
    fn decision_key_fingerprint_changes_when_openvino_npu_tuning_changes() {
        let key_a = decision_key();
        let mut key_b = key_a.clone();
        key_b.openvino_npu_max_prompt_len = Some(512);

        assert_ne!(key_a.fingerprint(), key_b.fingerprint());
    }

    #[test]
    fn benchmark_gate_requires_speedup_and_ttft_guardrail() {
        let pass = BackendBenchmarkComparison {
            cpu: benchmark(InferenceBackend::Cpu, 10.0, 100, 900),
            directml: benchmark(InferenceBackend::DirectML, 13.1, 114, 800),
            elapsed_ms: 1_100,
            budget_ms: BENCHMARK_SELECTION_BUDGET_MS,
        };
        assert!(pass.directml_passes_gate());

        let fail_speed = BackendBenchmarkComparison {
            cpu: benchmark(InferenceBackend::Cpu, 10.0, 100, 900),
            directml: benchmark(InferenceBackend::DirectML, 12.9, 110, 840),
            elapsed_ms: 1_000,
            budget_ms: BENCHMARK_SELECTION_BUDGET_MS,
        };
        assert!(!fail_speed.directml_passes_gate());

        let fail_ttft = BackendBenchmarkComparison {
            cpu: benchmark(InferenceBackend::Cpu, 10.0, 100, 900),
            directml: benchmark(InferenceBackend::DirectML, 14.0, 116, 850),
            elapsed_ms: 1_200,
            budget_ms: BENCHMARK_SELECTION_BUDGET_MS,
        };
        assert!(!fail_ttft.directml_passes_gate());
    }

    #[test]
    fn directml_is_demoted_after_three_consecutive_failures() {
        let mut counters = FailureCounters::default();
        for _ in 0..2 {
            counters.record_directml_failure(DirectMLFailureStage::Runtime, "runtime error");
        }
        assert!(!counters.should_demote_directml());

        counters.record_directml_failure(DirectMLFailureStage::Init, "init error");
        assert!(counters.should_demote_directml());
    }

    #[test]
    fn ttft_ratio_is_infinite_when_cpu_ttft_is_zero_and_directml_non_zero() {
        let comparison = BackendBenchmarkComparison {
            cpu: benchmark(InferenceBackend::Cpu, 10.0, 0, 900),
            directml: benchmark(InferenceBackend::DirectML, 13.5, 8, 800),
            elapsed_ms: 1_000,
            budget_ms: BENCHMARK_SELECTION_BUDGET_MS,
        };

        assert_eq!(comparison.directml_ttft_ratio(), f64::INFINITY);
        assert!(!comparison.directml_passes_gate());
    }

    #[test]
    fn ttft_ratio_is_one_when_both_ttft_values_are_zero() {
        let comparison = BackendBenchmarkComparison {
            cpu: benchmark(InferenceBackend::Cpu, 10.0, 0, 900),
            directml: benchmark(InferenceBackend::DirectML, 13.5, 0, 800),
            elapsed_ms: 1_000,
            budget_ms: BENCHMARK_SELECTION_BUDGET_MS,
        };

        assert_eq!(comparison.directml_ttft_ratio(), 1.0);
    }

    #[test]
    fn directml_backend_serializes_without_split_initialism() {
        let serialized = serde_json::to_string(&InferenceBackend::DirectML)
            .expect("directml backend should serialize");
        assert_eq!(serialized, "\"directml\"");
    }

    #[test]
    fn openvino_backend_serializes_with_lane_name() {
        let serialized = serde_json::to_string(&InferenceBackend::OpenVinoNpu)
            .expect("openvino_npu backend should serialize");
        assert_eq!(serialized, "\"openvino_npu\"");
    }

    #[test]
    fn backend_status_serializes_lane_based_surface() {
        let status = BackendStatus {
            selection_state: Some(BackendSelectionState::Fallback),
            decision_persistence_state: DecisionPersistenceState::TemporaryFallback,
            lanes: BackendLaneStatuses {
                directml: BackendLaneStatus {
                    startup_probe_state: LaneStartupProbeState::Error,
                    preflight_state: LanePreflightState::Error,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let value = serde_json::to_value(&status).expect("status should serialize");
        assert_eq!(value["decision_persistence_state"], "temporary_fallback");
        assert!(value["lanes"]["directml"].is_object());
        assert!(value.get("dml_gate_state").is_none());
        assert!(value.get("selected_device_id").is_none());
    }

    #[test]
    fn check_model_response_any_ready_requires_a_ready_lane() {
        let response = CheckModelResponse {
            model_id: "qwen2.5-1.5b-instruct".to_string(),
            lanes: ModelLaneReadinessByBackend {
                openvino_npu: ModelLaneReadiness {
                    artifact_ready: true,
                    bundle_ready: true,
                    ready: false,
                    reason: "startup_probe_not_run".to_string(),
                },
                directml: ModelLaneReadiness {
                    artifact_ready: true,
                    bundle_ready: true,
                    ready: true,
                    reason: "ready".to_string(),
                },
                cpu: ModelLaneReadiness::default(),
            },
        };

        assert!(response.any_ready());
        assert!(response.any_artifact_ready());
    }

    #[test]
    fn check_model_response_any_ready_is_false_when_only_artifacts_exist() {
        let response = CheckModelResponse {
            model_id: "qwen2.5-1.5b-instruct".to_string(),
            lanes: ModelLaneReadinessByBackend {
                openvino_npu: ModelLaneReadiness {
                    artifact_ready: true,
                    bundle_ready: true,
                    ready: false,
                    reason: "startup_probe_not_run".to_string(),
                },
                directml: ModelLaneReadiness {
                    artifact_ready: true,
                    bundle_ready: false,
                    ready: false,
                    reason: "directml_missing".to_string(),
                },
                cpu: ModelLaneReadiness {
                    artifact_ready: true,
                    bundle_ready: false,
                    ready: false,
                    reason: "ort_core_missing".to_string(),
                },
            },
        };

        assert!(!response.any_ready());
        assert!(response.any_artifact_ready());
    }
}
