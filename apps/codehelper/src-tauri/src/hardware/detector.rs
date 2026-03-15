use crate::hardware::errors::HardwareError;
use crate::hardware::types::{
    CpuInfo, GpuInfo, GpuVendor, HardwareInfo, MemoryInfo, NpuConfidence, NpuInfo, StorageInfo,
};
use std::time::{Duration, Instant};

const HARDWARE_DETECTION_TIMEOUT: Duration = Duration::from_secs(10);

/// Helper Functions
/// Convert non-zero u64 to Option (0 = detection failed)
fn non_zero_u64(val: u64) -> Option<u64> {
    (val > 0).then_some(val)
}

fn non_zero_u32(val: u32) -> Option<u32> {
    (val > 0).then_some(val)
}

fn non_empty_string(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        (!trimmed.is_empty()).then_some(trimmed.to_string())
    })
}

fn normalize_whitespace(value: &str) -> Option<String> {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    (!collapsed.is_empty()).then_some(collapsed)
}

fn normalize_identifier_component(value: &str) -> Option<String> {
    let normalized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, ':' | '.' | '-' | '_') {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    (!normalized.is_empty()).then_some(normalized)
}

fn confidence_rank(confidence: &NpuConfidence) -> u32 {
    match confidence {
        NpuConfidence::High => 3,
        NpuConfidence::Medium => 2,
        NpuConfidence::Low => 1,
    }
}

fn build_npu_candidate(npu: &hardware_query::NPUInfo) -> (NpuInfo, u32) {
    let vendor = normalize_whitespace(&npu.vendor().to_string()).unwrap_or("Unknown".to_string());
    let model = normalize_whitespace(npu.model_name()).unwrap_or("NPU".to_string());
    let driver_version = non_empty_string(npu.driver_version.clone());
    let pci_device_id = non_empty_string(npu.pci_device_id.clone());
    let usb_device_id = non_empty_string(npu.usb_device_id.clone());
    let tops = npu
        .tops_performance()
        .filter(|value| value.is_finite() && *value > 0.0);

    let display_name = if vendor.eq_ignore_ascii_case("unknown")
        || model
            .to_ascii_lowercase()
            .starts_with(&vendor.to_ascii_lowercase())
    {
        model.clone()
    } else {
        format!("{vendor} {model}")
    };

    let identifier = if let Some(pci) = pci_device_id
        .as_deref()
        .and_then(normalize_identifier_component)
    {
        format!("pci:{pci}")
    } else if let Some(usb) = usb_device_id
        .as_deref()
        .and_then(normalize_identifier_component)
    {
        format!("usb:{usb}")
    } else {
        let vendor_component =
            normalize_identifier_component(&vendor).unwrap_or_else(|| "unknown".to_string());
        let model_component =
            normalize_identifier_component(&model).unwrap_or_else(|| "npu".to_string());
        format!("{vendor_component}:{model_component}")
    };

    let confidence = if pci_device_id.is_some() || usb_device_id.is_some() {
        NpuConfidence::High
    } else if driver_version.is_some() || tops.is_some() {
        NpuConfidence::Medium
    } else {
        NpuConfidence::Low
    };

    let mut summary_parts = vec![display_name];
    if let Some(value) = tops {
        summary_parts.push(format!("{value:.1} TOPS"));
    }
    if let Some(driver) = driver_version.as_ref() {
        summary_parts.push(format!("Driver {driver}"));
    }
    if let Some(pci) = pci_device_id.as_ref() {
        summary_parts.push(format!("PCI {pci}"));
    }
    if let Some(usb) = usb_device_id.as_ref() {
        summary_parts.push(format!("USB {usb}"));
    }
    let details = summary_parts.join(" | ");

    let method = match confidence {
        NpuConfidence::High => "hardware-query offline detection (device-id confirmed)",
        NpuConfidence::Medium => "hardware-query offline detection (driver/perf corroborated)",
        NpuConfidence::Low => "hardware-query offline detection (heuristic)",
    }
    .to_string();

    let score = confidence_rank(&confidence) * 100
        + u32::from(driver_version.is_some()) * 20
        + u32::from(tops.is_some()) * 10
        + tops.map(|value| value.clamp(0.0, 99.0) as u32).unwrap_or(0);

    (
        NpuInfo {
            detected: true,
            confidence,
            identifier,
            driver_version,
            details,
            method,
        },
        score,
    )
}

/// Main entry point for hardware detection using hardware-query crate
/// Detects CPU, GPU, and NPU information offline
pub async fn detect_all() -> Result<HardwareInfo, HardwareError> {
    let started = Instant::now();
    log::info!(
        "Starting hardware detection using hardware-query (timeout={}s)",
        HARDWARE_DETECTION_TIMEOUT.as_secs()
    );

    let query_task = tokio::task::spawn_blocking(hardware_query::HardwareInfo::query);
    let hw_info = match tokio::time::timeout(HARDWARE_DETECTION_TIMEOUT, query_task).await {
        Ok(Ok(Ok(info))) => info,
        Ok(Ok(Err(error))) => {
            return Err(HardwareError::QueryFailed(error.to_string()));
        }
        Ok(Err(error)) => {
            return Err(HardwareError::QueryFailed(format!(
                "Hardware detection worker task failed: {error}"
            )));
        }
        Err(_) => {
            return Err(HardwareError::QueryFailed(format!(
                "Hardware detection timed out after {} seconds",
                HARDWARE_DETECTION_TIMEOUT.as_secs()
            )));
        }
    };

    // Convert CPU info
    let cpu_info = convert_cpu_info(&hw_info);

    // Convert GPU info
    let gpu_info = convert_gpu_info(&hw_info);

    // Convert NPU info
    let npu_info = convert_npu_info(&hw_info);

    // Convert memory info
    let memory_info = convert_memory_info(&hw_info);

    // Convert storage info
    let storage_info = convert_storage_info(&hw_info);

    let info = HardwareInfo {
        cpu: cpu_info,
        gpus: gpu_info,
        npu: npu_info,
        memory: memory_info,
        storage: storage_info,
        detected_at: chrono::Utc::now().to_rfc3339(),
    };

    log::info!(
        "Hardware detection completed in {} ms",
        started.elapsed().as_millis()
    );
    Ok(info)
}

/// Convert hardware-query CPU info to our CpuInfo format
fn convert_cpu_info(hw_info: &hardware_query::HardwareInfo) -> CpuInfo {
    let cpu = hw_info.cpu();

    // Validate and log frequency detection
    let frequency_raw = cpu.max_frequency();
    let frequency_mhz = non_zero_u32(frequency_raw);
    if frequency_mhz.is_none() {
        log::warn!("CPU frequency detection failed (returned 0)");
    } else {
        log::debug!("CPU frequency detected: {frequency_raw} MHz");
    }

    // Validate and log cache detection
    let cache_l1_kb = non_zero_u32(cpu.l1_cache_kb());
    let cache_l2_kb = non_zero_u32(cpu.l2_cache_kb());
    let cache_l3_kb = non_zero_u32(cpu.l3_cache_kb());

    if cache_l1_kb.is_none() {
        log::warn!("CPU L1 cache detection failed (returned 0)");
    }
    if cache_l2_kb.is_none() {
        log::warn!("CPU L2 cache detection failed (returned 0)");
    }
    if cache_l3_kb.is_none() {
        log::warn!("CPU L3 cache detection failed (returned 0)");
    }

    if cache_l1_kb.is_some() || cache_l2_kb.is_some() || cache_l3_kb.is_some() {
        log::debug!(
            "CPU cache detected: L1={cache_l1_kb:?} KB, L2={cache_l2_kb:?} KB, L3={cache_l3_kb:?} KB");
    }

    CpuInfo {
        vendor: cpu.vendor().to_string(),
        brand: cpu.model_name().to_string(),
        architecture: std::env::consts::ARCH.to_string(),
        cores_physical: cpu.physical_cores() as usize,
        cores_logical: cpu.logical_cores() as usize,
        frequency_mhz,
        features: cpu.features().to_vec(),
        cache_l1_kb,
        cache_l2_kb,
        cache_l3_kb,
    }
}

/// Convert hardware-query GPU info to our GpuInfo format
fn convert_gpu_info(hw_info: &hardware_query::HardwareInfo) -> Vec<GpuInfo> {
    hw_info
        .gpus()
        .iter()
        .enumerate()
        .map(|(idx, gpu)| {
            let gpu_name = gpu.model_name().to_string();
            let vendor_str = gpu.vendor().to_string().to_lowercase();
            let vendor = match vendor_str.as_str() {
                v if v.contains("nvidia") => GpuVendor::Nvidia,
                v if v.contains("amd") || v.contains("ati") => GpuVendor::Amd,
                v if v.contains("intel") => GpuVendor::Intel,
                v if v.contains("apple") => GpuVendor::Apple,
                v if v.contains("qualcomm") => GpuVendor::Qualcomm,
                _ => GpuVendor::Unknown,
            };

            // Determine backend based on supported APIs
            let backend = if gpu.supports_metal() {
                "Metal"
            } else if gpu.supports_directml() {
                "DirectX 12"
            } else if gpu.supports_vulkan() {
                "Vulkan"
            } else if gpu.supports_cuda() {
                "CUDA"
            } else if gpu.supports_opencl() {
                "OpenCL"
            } else {
                "Unknown"
            };

            let device_type = gpu.gpu_type().to_string();

            // Validate and log VRAM detection
            let vram_raw = gpu.memory_mb();
            let vram_mb = non_zero_u64(vram_raw);
            if vram_mb.is_none() {
                log::warn!("GPU {idx} ({gpu_name}) VRAM detection failed (returned 0)");
            } else {
                log::debug!("GPU {idx} ({gpu_name}) VRAM detected: {vram_raw} MB");
            }

            GpuInfo {
                name: gpu_name,
                vendor,
                backend: backend.to_string(),
                device_type,
                driver_version: non_empty_string(gpu.driver_version.clone()),
                pci_device_id: non_empty_string(gpu.pci_device_id.clone()),
                vram_mb,
                temperature_c: gpu.temperature().map(|t| t as u32),
                utilization_percent: gpu.usage_percent().map(|u| u as u32),
                cuda_compute_capability: gpu
                    .cuda_capability()
                    .map(std::string::ToString::to_string),
            }
        })
        .collect()
}

/// Convert hardware-query NPU info to our NpuInfo format
fn convert_npu_info(hw_info: &hardware_query::HardwareInfo) -> Option<NpuInfo> {
    if hw_info.npus().is_empty() {
        return None;
    }

    let selected = hw_info
        .npus()
        .iter()
        .map(build_npu_candidate)
        .max_by(|(info_a, score_a), (info_b, score_b)| {
            score_a
                .cmp(score_b)
                .then_with(|| info_a.identifier.cmp(&info_b.identifier))
        })
        .map(|(info, _)| info);

    if hw_info.npus().len() > 1 {
        log::debug!("Detected {} NPUs; selected best identifier", hw_info.npus().len());
    }

    selected
}

/// Convert hardware-query memory info to our MemoryInfo format
fn convert_memory_info(hw_info: &hardware_query::HardwareInfo) -> MemoryInfo {
    let mem = hw_info.memory();

    MemoryInfo {
        total_gb: mem.total_gb(),
        available_gb: mem.available_gb(),
    }
}

/// Convert hardware-query storage info to our StorageInfo format
fn convert_storage_info(hw_info: &hardware_query::HardwareInfo) -> StorageInfo {
    let storage_devices = hw_info.storage_devices();

    // Find primary storage device (largest capacity or first device)
    if let Some(primary) = storage_devices
        .iter()
        .max_by(|a, b| a.capacity_gb().total_cmp(&b.capacity_gb()))
    {
        let total_gb = primary.capacity_gb();
        let available_gb = primary.available_gb();
        let device_name = primary.model().to_string();

        let is_ssd = matches!(
            primary.drive_type().to_string().to_lowercase().as_str(),
            s if s.contains("ssd") || s.contains("nvme")
        );

        // Log storage detection results
        if total_gb == 0.0 {
            log::warn!(
                "Storage capacity detection failed for device '{device_name}' (returned 0.0 GB)"
            );
        } else {
            log::debug!(
                "Storage detected: {device_name} - {total_gb:.2} GB total, {available_gb:.2} GB available, SSD: {is_ssd}");
        }

        if available_gb == 0.0 && total_gb > 0.0 {
            log::warn!("Storage available space detection failed for device '{device_name}' (returned 0.0 GB)");
        }

        StorageInfo {
            total_gb,
            available_gb,
            is_ssd,
            device_name,
        }
    } else {
        // Fallback if no storage detected
        log::error!("No storage devices detected by hardware-query");
        StorageInfo {
            total_gb: 0.0,
            available_gb: 0.0,
            is_ssd: false,
            device_name: "Unknown".to_string(),
        }
    }
}
