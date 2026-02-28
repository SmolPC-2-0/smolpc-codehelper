use chrono::Utc;
use serde::{Deserialize, Serialize};

pub const BENCHMARK_SELECTION_BUDGET_MS: u64 = 2_000;
pub const DIRECTML_MIN_DECODE_SPEEDUP_RATIO: f64 = 1.30;
pub const DIRECTML_MAX_TTFT_REGRESSION_RATIO: f64 = 1.15;
pub const DIRECTML_DEMOTION_THRESHOLD: u32 = 3;
pub const ORT_CRATE_VERSION: &str = "2.0.0-rc.11";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InferenceBackend {
    Cpu,
    #[serde(rename = "directml")]
    DirectML,
}

impl InferenceBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::DirectML => "directml",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionReason {
    DefaultCpu,
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
    RuntimeFailureFallback,
    DemotedAfterFailures,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendDecisionKey {
    pub model_id: String,
    pub adapter_identity: String,
    pub driver_version: String,
    pub app_version: String,
    pub ort_version: String,
    #[serde(default)]
    pub directml_device_id: Option<i32>,
}

impl BackendDecisionKey {
    pub fn fingerprint(&self) -> String {
        let directml_device_id = self
            .directml_device_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "auto".to_string());
        format!(
            "{}|{}|{}|{}|{}|{}",
            self.model_id,
            self.adapter_identity,
            self.driver_version,
            self.app_version,
            self.ort_version,
            directml_device_id
        )
        .to_ascii_lowercase()
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct BackendStatus {
    pub active_backend: Option<InferenceBackend>,
    pub active_model_path: Option<String>,
    pub active_artifact_backend: Option<InferenceBackend>,
    pub runtime_engine: Option<String>,
    pub available_backends: Vec<InferenceBackend>,
    pub selection_state: Option<String>,
    pub selection_reason: Option<String>,
    pub selected_device_id: Option<i32>,
    pub selected_device_name: Option<String>,
    pub dml_gate_state: Option<String>,
    pub dml_gate_reason: Option<String>,
    pub decision_key: Option<BackendDecisionKey>,
    pub last_decision: Option<BackendDecision>,
    pub directml_probe_passed: Option<bool>,
    pub directml_probe_error: Option<String>,
    pub directml_probe_at: Option<String>,
    pub failure_counters: FailureCounters,
    pub force_override: Option<InferenceBackend>,
    pub store_path: Option<String>,
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

    #[test]
    fn decision_key_fingerprint_changes_when_driver_changes() {
        let key_a = BackendDecisionKey {
            model_id: "qwen2.5-coder-1.5b".to_string(),
            adapter_identity: "intel:arc".to_string(),
            driver_version: "31.0.101.5522".to_string(),
            app_version: "2.2.0".to_string(),
            ort_version: "1.23".to_string(),
            directml_device_id: None,
        };
        let mut key_b = key_a.clone();
        key_b.driver_version = "31.0.101.5590".to_string();

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
}
