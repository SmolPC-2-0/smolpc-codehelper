use crate::hardware::types::{
    CpuFeatures, CpuInfo, GpuInfo, GpuVendor, HardwareInfo, MemoryInfo, NpuConfidence, NpuInfo,
    StorageInfo,
};

/// Main entry point for hardware detection using hardware-query crate
/// Detects CPU, GPU, and NPU information offline
pub async fn detect_all() -> Result<HardwareInfo, String> {
    // Query all hardware information using hardware-query
    let hw_info = hardware_query::HardwareInfo::query()
        .map_err(|e| format!("Hardware query failed: {}", e))?;

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

    CpuInfo {
        vendor: cpu.vendor().to_string(),
        brand: cpu.model_name().to_string(),
        architecture: std::env::consts::ARCH.to_string(),
        cores_physical: cpu.physical_cores() as usize,
        cores_logical: cpu.logical_cores() as usize,
        frequency_mhz: Some(cpu.max_frequency() as u64),
        features: CpuFeatures {
            sse42: cpu.has_feature("sse4.2") || cpu.has_feature("sse42"),
            avx: cpu.has_feature("avx"),
            avx2: cpu.has_feature("avx2"),
            avx512f: cpu.has_feature("avx512f"),
            fma: cpu.has_feature("fma"),
            neon: cpu.has_feature("neon"),
            sve: cpu.has_feature("sve"),
        },
        cache_l1_kb: Some(cpu.l1_cache_kb() as usize),
        cache_l2_kb: Some(cpu.l2_cache_kb() as usize),
        cache_l3_kb: Some(cpu.l3_cache_kb() as usize),
    }
}

/// Convert hardware-query GPU info to our GpuInfo format
fn convert_gpu_info(hw_info: &hardware_query::HardwareInfo) -> Vec<GpuInfo> {
    hw_info
        .gpus()
        .iter()
        .map(|gpu| {
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

            GpuInfo {
                name: gpu.model_name().to_string(),
                vendor,
                backend: backend.to_string(),
                device_type,
                vram_mb: Some(gpu.memory_mb()),
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
            .partial_cmp(&b.capacity_gb())
            .unwrap_or(std::cmp::Ordering::Equal)
    }) {
        let is_ssd = matches!(
            primary.drive_type().to_string().to_lowercase().as_str(),
            s if s.contains("ssd") || s.contains("nvme")
        );

        StorageInfo {
            total_gb: primary.capacity_gb(),
            available_gb: primary.available_gb(),
            is_ssd,
            device_name: primary.model().to_string(),
        }
    } else {
        // Fallback if no storage detected
        StorageInfo {
            total_gb: 0.0,
            available_gb: 0.0,
            is_ssd: false,
            device_name: "Unknown".to_string(),
        }
    }
}
