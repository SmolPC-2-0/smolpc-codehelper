use crate::hardware::errors::HardwareError;
use crate::hardware::types::{
    CpuFeatures, CpuInfo, GpuInfo, GpuVendor, HardwareInfo, MemoryInfo, NpuConfidence, NpuInfo,
    StorageInfo,
};

/// Helper Functions
/// Convert non-zero u64 to Option (0 = detection failed)
fn non_zero_u64(val: u64) -> Option<u64> {
    (val > 0).then_some(val)
}

fn non_zero_u32(val: u32) -> Option<u32> {
    (val > 0).then_some(val)
}

/// Convert non-zero usize to Option (0 = detection failed)
fn non_zero_usize(val: usize) -> Option<usize> {
    (val > 0).then_some(val)
}

/// Main entry point for hardware detection using hardware-query crate
/// Detects CPU, GPU, and NPU information offline
pub async fn detect_all() -> Result<HardwareInfo, HardwareError> {
    // Query all hardware information using hardware-query
    let hw_info = hardware_query::HardwareInfo::query()
        .map_err(|e| HardwareError::QueryFailed(e.to_string()))?;

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

    Ok(HardwareInfo {
        cpu: cpu_info,
        gpus: gpu_info,
        npu: npu_info,
        memory: memory_info,
        storage: storage_info,
        detected_at: chrono::Utc::now().to_rfc3339(),
    })
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
        features: CpuFeatures {
            sse42: cpu.has_feature("sse4.2") || cpu.has_feature("sse42"),
            avx: cpu.has_feature("avx"),
            avx2: cpu.has_feature("avx2"),
            avx512f: cpu.has_feature("avx512f"),
            fma: cpu.has_feature("fma"),
            neon: cpu.has_feature("neon"),
            sve: cpu.has_feature("sve"),
        },
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
                log::warn!("GPU {} ({}) VRAM detection failed (returned 0)", idx, gpu_name);
            } else {
                log::debug!("GPU {} ({}) VRAM detected: {} MB", idx, gpu_name, vram_raw);
            }

            GpuInfo {
                name: gpu_name,
                vendor,
                backend: backend.to_string(),
                device_type,
                vram_mb,
                temperature_c: gpu.temperature().map(|t| t as u32),
                utilization_percent: gpu.usage_percent().map(|u| u as u32),
                cuda_compute_capability: gpu.cuda_capability().map(|s| s.to_string()),
            }
        })
        .collect()
}

/// Convert hardware-query NPU info to our NpuInfo format
fn convert_npu_info(hw_info: &hardware_query::HardwareInfo) -> Option<NpuInfo> {
    if hw_info.npus().is_empty() {
        return None;
    }

    let npu = &hw_info.npus()[0]; // Use first NPU

    let details = if let Some(tops) = npu.tops_performance() {
        format!("{} {} - {:.1} TOPS", npu.vendor(), npu.model_name(), tops)
    } else {
        format!("{} {}", npu.vendor(), npu.model_name())
    };

    Some(NpuInfo {
        detected: true,
        confidence: NpuConfidence::High, // hardware-query provides actual detection
        details,
        method: "hardware-query offline detection".to_string(),
    })
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
    if let Some(primary) = storage_devices.iter().max_by(|a, b| {
        a.capacity_gb()
            .total_cmp(&b.capacity_gb())
    }) {
        let total_gb = primary.capacity_gb();
        let available_gb = primary.available_gb();
        let device_name = primary.model().to_string();

        let is_ssd = matches!(
            primary.drive_type().to_string().to_lowercase().as_str(),
            s if s.contains("ssd") || s.contains("nvme")
        );

        // Log storage detection results
        if total_gb == 0.0 {
            log::warn!("Storage capacity detection failed for device '{}' (returned 0.0 GB)", device_name);
        } else {
            log::debug!(
                "Storage detected: {} - {:.2} GB total, {:.2} GB available, SSD: {}",
                device_name, total_gb, available_gb, is_ssd
            );
        }

        if available_gb == 0.0 && total_gb > 0.0 {
            log::warn!("Storage available space detection failed for device '{}' (returned 0.0 GB)", device_name);
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
