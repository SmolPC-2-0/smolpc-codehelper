use smolpc_engine_core::inference::backend::{
    BackendOpenVinoTuningStatus, InferenceBackend,
};
use std::cmp::Ordering as CmpOrdering;

use crate::openvino::resolve_openvino_npu_tuning;
use crate::runtime_bundles::ResolvedRuntimeBundles;

#[derive(Debug, Clone)]
pub(crate) struct DirectMlCandidate {
    pub(crate) device_id: i32,
    pub(crate) device_name: String,
    pub(crate) adapter_identity: String,
    pub(crate) driver_version: String,
}

#[derive(Debug, Clone)]
pub(crate) struct BackendProbeResult {
    pub(crate) available_backends: Vec<InferenceBackend>,
    pub(crate) directml_device_count: usize,
    pub(crate) directml_candidate: Option<DirectMlCandidate>,
    pub(crate) directml_probe_failure_class: Option<String>,
    pub(crate) directml_probe_failure_message: Option<String>,
    pub(crate) npu_hardware_detected: bool,
}

impl Default for BackendProbeResult {
    fn default() -> Self {
        Self {
            available_backends: vec![InferenceBackend::Cpu],
            directml_device_count: 0,
            directml_candidate: None,
            directml_probe_failure_class: None,
            directml_probe_failure_message: None,
            npu_hardware_detected: false,
        }
    }
}

impl BackendProbeResult {
    pub(crate) fn directml_probe_failure(
        class: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            directml_probe_failure_class: Some(class.into()),
            directml_probe_failure_message: Some(message.into()),
            ..Self::default()
        }
    }

    pub(crate) fn directml_ready_gate_failure(&self) -> Option<String> {
        if self.directml_candidate.is_some() {
            return None;
        }

        match self.directml_probe_failure_class.as_deref() {
            Some("directml_candidate_missing") => Some("directml_candidate_missing".to_string()),
            _ => None,
        }
    }
}

pub(crate) fn current_openvino_tuning_status() -> Option<BackendOpenVinoTuningStatus> {
    resolve_openvino_npu_tuning()
        .ok()
        .map(|tuning| BackendOpenVinoTuningStatus {
            max_prompt_len: Some(tuning.max_prompt_len),
            min_response_len: Some(tuning.min_response_len),
        })
}

pub(crate) fn model_requires_directml(model_id: &str) -> bool {
    let _ = model_id;
    false
}

pub(crate) fn model_requires_openvino(model_id: &str) -> bool {
    let _ = model_id;
    false
}

pub(crate) fn directml_required_error(model_id: &str, reason: &str) -> String {
    format!("Model '{model_id}' currently requires DirectML backend in shared engine: {reason}")
}

pub(crate) fn openvino_required_error(model_id: &str, reason: &str) -> String {
    format!(
        "Model '{model_id}' currently requires OpenVINO NPU backend in shared engine: {reason}"
    )
}

pub(crate) fn directml_unavailable_reason(
    directml_detected: bool,
    directml_artifact_available: bool,
    runtime_bundles: &ResolvedRuntimeBundles,
) -> String {
    if !directml_detected {
        "no DirectML-capable adapter was detected".to_string()
    } else if !directml_artifact_available {
        "the DirectML model artifact is missing".to_string()
    } else if let Some(code) = runtime_bundles.ort.directml_failure_code() {
        format!("the DirectML runtime bundle is unavailable ({code})")
    } else {
        "the DirectML runtime bundle is unavailable".to_string()
    }
}

#[cfg(target_os = "windows")]
fn gpu_rank_key(gpu: &hardware_query::GPUInfo) -> (bool, u64, String) {
    let device_type = gpu.gpu_type().to_string().to_ascii_lowercase();
    let is_discrete = device_type.contains("discrete");
    let vram_mb = gpu.memory_mb();
    let name = gpu.model_name().to_ascii_lowercase();
    (is_discrete, vram_mb, name)
}

#[cfg(target_os = "windows")]
pub(crate) fn pick_best_dml_candidate(
    gpus: &[hardware_query::GPUInfo],
) -> Option<DirectMlCandidate> {
    let mut candidates = gpus
        .iter()
        .enumerate()
        .filter(|(_, gpu)| gpu.supports_directml())
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| {
        let ka = gpu_rank_key(a.1);
        let kb = gpu_rank_key(b.1);
        match kb.0.cmp(&ka.0) {
            CmpOrdering::Equal => match kb.1.cmp(&ka.1) {
                CmpOrdering::Equal => ka.2.cmp(&kb.2),
                other => other,
            },
            other => other,
        }
    });

    let (device_index, gpu) = candidates.first()?;

    // Reject integrated GPUs for DirectML — ORT+DirectML inference quality
    // on Intel (and other) integrated GPUs is unreliable for LLM workloads
    // (produces runaway generation / no EOS detection). Only discrete GPUs
    // are viable DirectML targets. Integrated-only machines fall through to
    // OpenVINO CPU, which works correctly.
    //
    // Accept GPUs whose type string contains "discrete", "dedicated", or
    // "external" to cover different hardware_query reporting styles.
    let device_type = gpu.gpu_type().to_string().to_ascii_lowercase();
    let is_discrete = device_type.contains("discrete")
        || device_type.contains("dedicated")
        || device_type.contains("external");
    if !is_discrete {
        log::info!(
            "DirectML candidate '{}' rejected: not a discrete GPU (type='{}'). \
             Machines without a discrete GPU will use OpenVINO CPU.",
            gpu.model_name(),
            gpu.gpu_type()
        );
        return None;
    }
    let vendor = gpu.vendor().to_string().to_ascii_lowercase();
    let model = gpu.model_name().trim().to_ascii_lowercase();
    let pci = gpu
        .pci_device_id
        .as_deref()
        .unwrap_or("unknown")
        .trim()
        .to_ascii_lowercase();
    let driver = gpu
        .driver_version
        .as_deref()
        .unwrap_or("unknown")
        .trim()
        .to_string();
    Some(DirectMlCandidate {
        device_id: *device_index as i32,
        device_name: gpu.model_name().to_string(),
        adapter_identity: format!("{vendor}:{model}:{pci}"),
        driver_version: driver,
    })
}

#[cfg(target_os = "windows")]
pub(crate) fn probe_backend_capabilities() -> BackendProbeResult {
    let info = match hardware_query::HardwareInfo::query() {
        Ok(info) => info,
        Err(error) => {
            return BackendProbeResult::directml_probe_failure(
                "directml_startup_probe_failed",
                format!("DirectML hardware query failed: {error}"),
            );
        }
    };
    let directml_device_count = info
        .gpus()
        .iter()
        .filter(|gpu| gpu.supports_directml())
        .count();
    let mut result = BackendProbeResult {
        npu_hardware_detected: !info.npus().is_empty(),
        directml_device_count,
        ..Default::default()
    };
    if let Some(candidate) = pick_best_dml_candidate(info.gpus()) {
        result.available_backends.push(InferenceBackend::DirectML);
        result.directml_candidate = Some(candidate);
    } else {
        result.directml_probe_failure_class = Some("directml_candidate_missing".to_string());
        result.directml_probe_failure_message =
            Some("No DirectML-capable adapter detected".to_string());
    }
    result
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn probe_backend_capabilities() -> BackendProbeResult {
    BackendProbeResult::default()
}
