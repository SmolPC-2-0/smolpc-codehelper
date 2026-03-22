use smolpc_engine_core::inference::backend::{
    BackendDecision, BackendRuntimeBundleStatus, BackendSelectedDevice, BackendStatus,
    CheckModelResponse, DecisionReason, InferenceBackend, LaneStartupProbeState,
    ModelLaneReadiness, ModelLaneReadinessByBackend,
};
use smolpc_engine_core::inference::RuntimeVersionMetadata;
use smolpc_engine_core::models::{ModelArtifactBackend, ModelLoader, ModelRegistry};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use crate::openvino::{
    inspect_openvino_artifact, is_blocking_openvino_probe_failure, OpenVinoStartupProbeResult,
};
use crate::probe::BackendProbeResult;
use crate::runtime_bundles::ResolvedRuntimeBundles;

#[derive(Debug, Clone, Default)]
pub(crate) struct ModelLaneArtifacts {
    pub(crate) cpu_ready: bool,
    pub(crate) directml_ready: bool,
    pub(crate) openvino_artifact: Option<crate::openvino::OpenVinoReadyArtifact>,
    pub(crate) openvino_reason: Option<String>,
    pub(crate) openvino_message: Option<String>,
    pub(crate) fingerprint: Option<String>,
}

impl ModelLaneArtifacts {
    pub(crate) fn openvino_npu_ready(&self) -> bool {
        self.openvino_artifact.is_some()
    }
}

pub(crate) fn compute_artifact_fingerprint(paths: &[PathBuf]) -> Option<String> {
    let existing = paths
        .iter()
        .filter(|path| path.exists())
        .cloned()
        .collect::<Vec<_>>();
    if existing.is_empty() {
        return None;
    }

    let mut sorted = existing;
    sorted.sort_by(|a, b| {
        a.display()
            .to_string()
            .to_ascii_lowercase()
            .cmp(&b.display().to_string().to_ascii_lowercase())
    });

    let mut hasher = DefaultHasher::new();
    for path in &sorted {
        path.display()
            .to_string()
            .to_ascii_lowercase()
            .hash(&mut hasher);
        if let Ok(metadata) = std::fs::metadata(path) {
            metadata.len().hash(&mut hasher);
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                    duration.as_secs().hash(&mut hasher);
                    duration.subsec_nanos().hash(&mut hasher);
                }
            }
        }
    }

    Some(format!("{:016x}", hasher.finish()))
}

pub(crate) fn sanitize_cache_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

pub(crate) fn resolve_model_lane_artifacts(model_dir: &str) -> ModelLaneArtifacts {
    let (directml_model_exists, directml_tokenizer_exists) =
        ModelLoader::check_model_files_for_backend(model_dir, ModelArtifactBackend::DirectML);
    let openvino_manifest = ModelLoader::openvino_manifest_file(model_dir);
    let openvino_artifact = inspect_openvino_artifact(&openvino_manifest);

    let directml_root =
        ModelLoader::model_path(model_dir).join(ModelArtifactBackend::DirectML.as_dir());
    let mut artifact_paths = vec![
        directml_root.join("model.onnx"),
        directml_root.join("genai_config.json"),
        directml_root.join("tokenizer.json"),
    ];
    artifact_paths.extend(openvino_artifact.fingerprint_paths());

    ModelLaneArtifacts {
        cpu_ready: openvino_artifact.ready_artifact().is_some(),
        directml_ready: directml_model_exists && directml_tokenizer_exists,
        openvino_artifact: openvino_artifact.ready_artifact().cloned(),
        openvino_reason: Some(openvino_artifact.reason_code().to_string()),
        openvino_message: openvino_artifact.message(),
        fingerprint: compute_artifact_fingerprint(&artifact_paths),
    }
}

pub(crate) fn runtime_version_value(
    version_metadata: &[RuntimeVersionMetadata],
    component: &str,
) -> Option<String> {
    version_metadata
        .iter()
        .find(|entry| entry.component == component)
        .map(|entry| entry.version.clone())
}

pub(crate) fn runtime_version_summary(
    version_metadata: &[RuntimeVersionMetadata],
) -> Option<String> {
    (!version_metadata.is_empty()).then(|| {
        version_metadata
            .iter()
            .map(|entry| format!("{}={}", entry.component, entry.version))
            .collect::<Vec<_>>()
            .join("; ")
    })
}

pub(crate) fn apply_model_lane_artifacts(
    status: &mut BackendStatus,
    artifacts: &ModelLaneArtifacts,
) {
    status.lanes.cpu.artifact_ready = artifacts.cpu_ready;
    status.lanes.directml.artifact_ready = artifacts.directml_ready;
    status.lanes.openvino_npu.artifact_ready = artifacts.openvino_npu_ready();
}

pub(crate) fn apply_persisted_eligibility(
    status: &mut BackendStatus,
    persisted_decision: Option<&BackendDecision>,
) {
    status.lanes.cpu.persisted_eligibility = false;
    status.lanes.directml.persisted_eligibility = false;
    status.lanes.openvino_npu.persisted_eligibility = false;

    if let Some(decision) = persisted_decision {
        match decision.backend {
            InferenceBackend::Cpu => status.lanes.cpu.persisted_eligibility = true,
            InferenceBackend::DirectML => status.lanes.directml.persisted_eligibility = true,
            InferenceBackend::OpenVinoNpu => {
                status.lanes.openvino_npu.persisted_eligibility = true
            }
        }
    }
}

pub(crate) fn apply_directml_device(
    status: &mut BackendStatus,
    device_id: Option<i32>,
    device_name: Option<String>,
    vram_mb: Option<u64>,
) {
    status.lanes.directml.device_id = device_id;
    status.lanes.directml.device_name = device_name.clone();
    status.selected_device = if device_id.is_some() || device_name.is_some() {
        Some(BackendSelectedDevice {
            backend: InferenceBackend::DirectML,
            device_id,
            device_name,
            vram_mb,
        })
    } else {
        None
    };
}

pub(crate) fn rebuild_available_backends(status: &mut BackendStatus) {
    status.available_backends = vec![InferenceBackend::Cpu];
    if status.lanes.directml.detected {
        status.available_backends.push(InferenceBackend::DirectML);
    }
    if status.lanes.openvino_npu.startup_probe_state == LaneStartupProbeState::Ready {
        status
            .available_backends
            .push(InferenceBackend::OpenVinoNpu);
    }
}

pub(crate) fn apply_directml_startup_probe_status(
    status: &mut BackendStatus,
    probe: &BackendProbeResult,
) {
    status.lanes.directml.detected = probe.directml_candidate.is_some();
    if let Some(candidate) = probe.directml_candidate.as_ref() {
        status.lanes.directml.startup_probe_state = LaneStartupProbeState::Ready;
        status.lanes.directml.driver_version = Some(candidate.driver_version.clone());
        status.lanes.directml.last_failure_class = None;
        status.lanes.directml.last_failure_message = None;
        apply_directml_device(
            status,
            Some(candidate.device_id),
            Some(candidate.device_name.clone()),
            Some(candidate.vram_mb),
        );
    } else {
        status.lanes.directml.startup_probe_state = LaneStartupProbeState::Error;
        status.lanes.directml.last_failure_class = probe
            .directml_probe_failure_class
            .clone()
            .or_else(|| Some("directml_candidate_missing".to_string()));
        status.lanes.directml.last_failure_message = probe
            .directml_probe_failure_message
            .clone()
            .or_else(|| Some("No DirectML-capable adapter detected".to_string()));
        apply_directml_device(status, None, None, None);
    }
    rebuild_available_backends(status);
}

pub(crate) fn apply_openvino_startup_probe_status(
    status: &mut BackendStatus,
    probe: Option<&OpenVinoStartupProbeResult>,
) {
    let Some(probe) = probe else {
        rebuild_available_backends(status);
        return;
    };

    status.lanes.openvino_npu.detected = probe.device_visible;
    status.lanes.openvino_npu.device_name = probe.device_name.clone();
    status.lanes.openvino_npu.driver_version = probe.driver_version.clone();
    status.lanes.openvino_npu.device_id = None;
    status.lanes.openvino_npu.startup_probe_state = if probe.startup_ready {
        LaneStartupProbeState::Ready
    } else {
        LaneStartupProbeState::Error
    };
    status.lanes.openvino_npu.last_failure_class = probe.failure_class.clone();
    status.lanes.openvino_npu.last_failure_message = probe.failure_message.clone();
    rebuild_available_backends(status);
}

pub(crate) fn apply_runtime_bundle_status(
    runtime_bundles: &ResolvedRuntimeBundles,
    status: &mut BackendStatus,
) {
    status.runtime_bundles.load_mode = Some(runtime_bundles.mode.as_str().to_string());
    status.runtime_bundles.ort = BackendRuntimeBundleStatus {
        root: Some(runtime_bundles.ort.display_root().display().to_string()),
        fingerprint: Some(runtime_bundles.ort.fingerprint.value.clone()),
        validated: runtime_bundles.ort.ort_validated(),
        failure: runtime_bundles
            .ort
            .ort_failure_code()
            .map(ToString::to_string),
    };
    status.runtime_bundles.directml = BackendRuntimeBundleStatus {
        root: Some(runtime_bundles.ort.display_root().display().to_string()),
        fingerprint: Some(runtime_bundles.ort.fingerprint.value.clone()),
        validated: runtime_bundles.ort.directml_validated(),
        failure: runtime_bundles
            .ort
            .directml_failure_code()
            .map(ToString::to_string),
    };
    status.runtime_bundles.openvino = BackendRuntimeBundleStatus {
        root: Some(
            runtime_bundles
                .openvino
                .display_root()
                .display()
                .to_string(),
        ),
        fingerprint: Some(runtime_bundles.openvino.fingerprint.value.clone()),
        validated: runtime_bundles.openvino.cpu_validated(),
        failure: runtime_bundles
            .openvino
            .failure_code()
            .map(ToString::to_string),
    };

    status.lanes.cpu.detected = true;
    status.lanes.cpu.bundle_ready = runtime_bundles.openvino.cpu_validated();
    status.lanes.cpu.runtime_version =
        runtime_version_summary(&runtime_bundles.openvino.version_metadata);
    status.lanes.cpu.startup_probe_state = LaneStartupProbeState::Ready;
    if !status.lanes.cpu.bundle_ready && status.lanes.cpu.last_failure_class.is_none() {
        status.lanes.cpu.last_failure_class = runtime_bundles
            .openvino
            .cpu_failure_code()
            .map(ToString::to_string);
    }

    status.lanes.directml.bundle_ready = runtime_bundles.ort.directml_validated();
    status.lanes.directml.runtime_version =
        runtime_version_summary(&runtime_bundles.ort.version_metadata);
    if !status.lanes.directml.bundle_ready && status.lanes.directml.last_failure_class.is_none() {
        status.lanes.directml.last_failure_class = runtime_bundles
            .ort
            .directml_failure_code()
            .map(ToString::to_string);
    }

    status.lanes.openvino_npu.bundle_ready = runtime_bundles.openvino.npu_validated();
    status.lanes.openvino_npu.runtime_version =
        runtime_version_summary(&runtime_bundles.openvino.version_metadata);
    if !status.lanes.openvino_npu.bundle_ready
        && status.lanes.openvino_npu.last_failure_class.is_none()
    {
        status.lanes.openvino_npu.last_failure_class = runtime_bundles
            .openvino
            .npu_failure_code()
            .map(ToString::to_string);
    }
}

pub(crate) fn bundle_reason(code: Option<&str>) -> String {
    code.unwrap_or("bundle_missing").to_string()
}

pub(crate) fn decision_reason_code(reason: &DecisionReason) -> &'static str {
    match reason {
        DecisionReason::DefaultCpu => "default_cpu",
        DecisionReason::DefaultOpenVinoCandidate => "default_openvino_candidate",
        DecisionReason::DefaultDirectMLCandidate => "default_directml_candidate",
        DecisionReason::ForcedOverride => "forced_override",
        DecisionReason::PersistedDecision => "persisted_decision",
        DecisionReason::BenchmarkPassed => "benchmark_passed",
        DecisionReason::BenchmarkDecodeTooSlow => "benchmark_directml_decode_too_slow",
        DecisionReason::BenchmarkTtftTooHigh => "benchmark_ttft_too_high",
        DecisionReason::BenchmarkBudgetExceeded => "benchmark_budget_exceeded",
        DecisionReason::NoDirectMLCandidate => "no_directml_candidate",
        DecisionReason::DirectMLInitializationFailed => "directml_initialization_failed",
        DecisionReason::DirectMLPreflightFailed => "directml_preflight_failed",
        DecisionReason::NoOpenVinoCandidate => "no_openvino_candidate",
        DecisionReason::OpenVinoStartupProbePending => "openvino_startup_probe_pending",
        DecisionReason::OpenVinoPreflightFailed => "openvino_preflight_failed",
        DecisionReason::OpenVinoPreflightTimeout => "openvino_preflight_timeout",
        DecisionReason::OpenVinoRuntimeUnavailable => "runtime_unavailable",
        DecisionReason::RuntimeFailureFallback => "runtime_failure_fallback",
        DecisionReason::DemotedAfterFailures => "demoted_after_failures",
    }
}

pub(crate) fn build_check_model_response(
    model_id: &str,
    runtime_bundles: &ResolvedRuntimeBundles,
    startup_probe: Option<&BackendProbeResult>,
    openvino_probe: Option<&OpenVinoStartupProbeResult>,
) -> CheckModelResponse {
    let Some(model_def) = ModelRegistry::get_model(model_id) else {
        let unknown = ModelLaneReadiness {
            artifact_ready: false,
            bundle_ready: false,
            ready: false,
            reason: "unknown_model".to_string(),
        };
        return CheckModelResponse {
            model_id: model_id.to_string(),
            lanes: ModelLaneReadinessByBackend {
                openvino_npu: unknown.clone(),
                directml: unknown.clone(),
                cpu: unknown,
            },
        };
    };

    let artifacts = resolve_model_lane_artifacts(&model_def.directory);
    let cpu_bundle_ready = runtime_bundles.openvino.cpu_validated();
    let directml_bundle_ready = runtime_bundles.ort.directml_validated();
    let openvino_bundle_ready = runtime_bundles.openvino.npu_validated();
    let directml_ready_gate_failure =
        startup_probe.and_then(BackendProbeResult::directml_ready_gate_failure);
    let openvino_probe_ready = openvino_probe.is_some_and(|probe| probe.startup_ready);
    let openvino_probe_failure = openvino_probe.and_then(|probe| {
        probe
            .failure_class
            .as_deref()
            .filter(|class| is_blocking_openvino_probe_failure(class))
            .map(ToString::to_string)
    });

    let cpu = ModelLaneReadiness {
        artifact_ready: artifacts.cpu_ready,
        bundle_ready: cpu_bundle_ready,
        ready: artifacts.cpu_ready && cpu_bundle_ready,
        reason: if artifacts.cpu_ready && cpu_bundle_ready {
            "ready".to_string()
        } else if !artifacts.cpu_ready {
            "artifact_missing".to_string()
        } else {
            bundle_reason(runtime_bundles.openvino.cpu_failure_code())
        },
    };
    let directml = ModelLaneReadiness {
        artifact_ready: artifacts.directml_ready,
        bundle_ready: directml_bundle_ready,
        ready: artifacts.directml_ready
            && directml_bundle_ready
            && directml_ready_gate_failure.is_none(),
        reason: if !artifacts.directml_ready {
            "artifact_missing".to_string()
        } else if !directml_bundle_ready {
            bundle_reason(runtime_bundles.ort.directml_failure_code())
        } else if let Some(failure) = directml_ready_gate_failure {
            failure
        } else {
            "ready".to_string()
        },
    };
    let openvino_npu = ModelLaneReadiness {
        artifact_ready: artifacts.openvino_npu_ready(),
        bundle_ready: openvino_bundle_ready,
        ready: artifacts.openvino_npu_ready()
            && openvino_bundle_ready
            && openvino_probe_ready
            && openvino_probe_failure.is_none(),
        reason: if !artifacts.openvino_npu_ready() {
            artifacts
                .openvino_reason
                .clone()
                .unwrap_or_else(|| "artifact_missing".to_string())
        } else if !openvino_bundle_ready {
            bundle_reason(runtime_bundles.openvino.npu_failure_code())
        } else if openvino_probe.is_none() {
            "startup_probe_pending".to_string()
        } else if let Some(failure) = openvino_probe_failure {
            failure
        } else if !openvino_probe_ready {
            "startup_probe_failed".to_string()
        } else {
            "ready".to_string()
        },
    };

    CheckModelResponse {
        model_id: model_id.to_string(),
        lanes: ModelLaneReadinessByBackend {
            openvino_npu,
            directml,
            cpu,
        },
    }
}
