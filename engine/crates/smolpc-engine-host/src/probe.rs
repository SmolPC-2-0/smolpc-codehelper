use smolpc_engine_core::inference::backend::{BackendOpenVinoTuningStatus, InferenceBackend};
use std::cmp::Ordering as CmpOrdering;

use crate::openvino::resolve_openvino_npu_tuning;
use crate::runtime_bundles::ResolvedRuntimeBundles;

#[derive(Debug, Clone)]
pub(crate) struct DirectMlCandidate {
    pub(crate) device_id: i32,
    pub(crate) device_name: String,
    pub(crate) adapter_identity: String,
    pub(crate) driver_version: String,
    pub(crate) vram_mb: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct BackendProbeResult {
    pub(crate) available_backends: Vec<InferenceBackend>,
    pub(crate) directml_device_count: usize,
    pub(crate) directml_candidate: Option<DirectMlCandidate>,
    pub(crate) directml_probe_failure_class: Option<String>,
    pub(crate) directml_probe_failure_message: Option<String>,
}

impl Default for BackendProbeResult {
    fn default() -> Self {
        Self {
            available_backends: vec![InferenceBackend::Cpu],
            directml_device_count: 0,
            directml_candidate: None,
            directml_probe_failure_class: None,
            directml_probe_failure_message: None,
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
    format!("Model '{model_id}' currently requires OpenVINO NPU backend in shared engine: {reason}")
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

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Minimum dedicated VRAM (MB) to classify an adapter as discrete.
/// Integrated GPUs report 0–128 MB of reserved system RAM as DedicatedVideoMemory.
/// Discrete GPUs start at 1–2 GB. 512 MB is a conservative midpoint.
const MIN_DISCRETE_VRAM_MB: u64 = 512;

// ---------------------------------------------------------------------------
// DXGI GPU enumeration (replaces WMI-based hardware_query, sub-5ms)
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
struct DxgiGpuInfo {
    name: String,
    vendor_id: u32,
    device_id: u32,
    vram_mb: u64,
    is_software: bool,
    is_discrete: bool,
    _perf_rank: u32,
}

#[cfg(target_os = "windows")]
fn enumerate_dxgi_gpus() -> Vec<DxgiGpuInfo> {
    use windows::core::Interface;
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIFactory6,
        DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
    };

    let factory: IDXGIFactory1 = match unsafe { CreateDXGIFactory1() } {
        Ok(f) => f,
        Err(e) => {
            log::warn!("CreateDXGIFactory1 failed: {e}");
            return Vec::new();
        }
    };

    let factory6 = factory.cast::<IDXGIFactory6>().ok();
    let mut gpus = Vec::new();

    for i in 0u32.. {
        let adapter: Result<IDXGIAdapter1, _> = if let Some(ref f6) = factory6 {
            unsafe { f6.EnumAdapterByGpuPreference(i, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE) }
        } else {
            unsafe { factory.EnumAdapters1(i) }
        };

        let adapter = match adapter {
            Ok(a) => a,
            Err(_) => break, // end of adapter list
        };

        let desc = match unsafe { adapter.GetDesc1() } {
            Ok(d) => d,
            Err(_) => continue,
        };

        let name = String::from_utf16_lossy(
            &desc.Description[..desc
                .Description
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(desc.Description.len())],
        );

        let is_software = (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0;
        let vram_mb = desc.DedicatedVideoMemory as u64 / (1024 * 1024);
        // Discrete GPUs have dedicated VRAM > 512 MB and are not software adapters
        let is_discrete = !is_software && vram_mb > MIN_DISCRETE_VRAM_MB;

        gpus.push(DxgiGpuInfo {
            name,
            vendor_id: desc.VendorId,
            device_id: desc.DeviceId,
            vram_mb,
            is_software,
            is_discrete,
            _perf_rank: i,
        });
    }

    gpus
}

#[cfg(target_os = "windows")]
fn pick_best_dml_candidate(gpus: &[DxgiGpuInfo]) -> Option<DirectMlCandidate> {
    // Filter to non-software adapters, sort by discrete > VRAM > name
    let mut candidates: Vec<(usize, &DxgiGpuInfo)> = gpus
        .iter()
        .enumerate()
        .filter(|(_, gpu)| !gpu.is_software)
        .collect();

    candidates.sort_by(|a, b| {
        let ka = (a.1.is_discrete, a.1.vram_mb);
        let kb = (b.1.is_discrete, b.1.vram_mb);
        match kb.0.cmp(&ka.0) {
            CmpOrdering::Equal => kb.1.cmp(&ka.1),
            other => other,
        }
    });

    let (device_index, gpu) = candidates.first()?;

    // Reject integrated GPUs for DirectML — ORT+DirectML inference quality
    // on Intel (and other) integrated GPUs is unreliable for LLM workloads
    // (produces runaway generation / no EOS detection). Only discrete GPUs
    // are viable DirectML targets. Integrated-only machines fall through to
    // OpenVINO CPU, which works correctly.
    if !gpu.is_discrete {
        log::info!(
            "DirectML candidate '{}' rejected: not a discrete GPU (vram={}MB). \
             Machines without a discrete GPU will use OpenVINO CPU.",
            gpu.name,
            gpu.vram_mb
        );
        return None;
    }

    // adapter_identity format: "0xVENDOR:0xDEVICE" (PCI IDs from DXGI).
    // This differs from the old hardware_query format ("vendor:model:pci").
    // The identity is used for backend decision caching — format changes
    // invalidate cached decisions, which is the desired behavior.
    let vendor = format!("0x{:04x}", gpu.vendor_id);
    let device = format!("0x{:04x}", gpu.device_id);

    Some(DirectMlCandidate {
        device_id: *device_index as i32,
        device_name: gpu.name.clone(),
        adapter_identity: format!("{vendor}:{device}"),
        driver_version: String::new(), // DXGI doesn't provide driver version directly
        vram_mb: gpu.vram_mb,
    })
}

#[cfg(target_os = "windows")]
pub(crate) fn probe_backend_capabilities() -> BackendProbeResult {
    let gpus = enumerate_dxgi_gpus();
    let directml_device_count = gpus.iter().filter(|gpu| !gpu.is_software).count();

    // directml_device_count = non-software DXGI adapters. All WDDM hardware
    // adapters support DirectML, so this is equivalent to the old
    // hardware_query supports_directml() filter.
    //
    // NPU detection is handled entirely by the OpenVINO startup probe
    // (probe_openvino_startup in openvino.rs) — no hint needed here.
    let mut result = BackendProbeResult {
        directml_device_count,
        ..Default::default()
    };

    if let Some(candidate) = pick_best_dml_candidate(&gpus) {
        result.available_backends.push(InferenceBackend::DirectML);
        result.directml_candidate = Some(candidate);
    } else {
        result.directml_probe_failure_class = Some("directml_candidate_missing".to_string());
        result.directml_probe_failure_message =
            Some("No DirectML-capable discrete GPU detected".to_string());
    }
    result
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn probe_backend_capabilities() -> BackendProbeResult {
    BackendProbeResult::default()
}

// ---------------------------------------------------------------------------
// System RAM query (for model auto-selection, replaces hardware_query)
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
pub(crate) fn total_system_ram_gb() -> Option<f64> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    if ok.is_ok() {
        Some(status.ullTotalPhys as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn total_system_ram_gb() -> Option<f64> {
    None
}
