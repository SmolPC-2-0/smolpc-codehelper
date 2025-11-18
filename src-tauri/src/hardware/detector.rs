use crate::hardware::types::{
    CpuFeatures, CpuInfo, GpuInfo, GpuVendor, HardwareInfo, NpuConfidence, NpuInfo,
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

    Ok(HardwareInfo {
        cpu: cpu_info,
        gpus: gpu_info,
        npu: npu_info,
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
        cores_physical: cpu.physical_cores(),
        cores_logical: cpu.logical_cores(),
        frequency_mhz: cpu.max_frequency_mhz(),
        features: CpuFeatures {
            sse42: cpu.has_feature("sse4.2") || cpu.has_feature("sse42"),
            avx: cpu.has_feature("avx"),
            avx2: cpu.has_feature("avx2"),
            avx512f: cpu.has_feature("avx512f"),
            fma: cpu.has_feature("fma"),
            neon: cpu.has_feature("neon"),
            sve: cpu.has_feature("sve"),
        },
        cache_l1_kb: cpu.l1_cache_kb(),
        cache_l2_kb: cpu.l2_cache_kb(),
        cache_l3_kb: cpu.l3_cache_kb(),
    }
}

/// Convert hardware-query GPU info to our GpuInfo format
fn convert_gpu_info(hw_info: &hardware_query::HardwareInfo) -> Vec<GpuInfo> {
    hw_info
        .gpus()
        .iter()
        .map(|gpu| {
            let vendor = match gpu.vendor().to_lowercase().as_str() {
                v if v.contains("nvidia") => GpuVendor::Nvidia,
                v if v.contains("amd") || v.contains("ati") => GpuVendor::Amd,
                v if v.contains("intel") => GpuVendor::Intel,
                v if v.contains("apple") => GpuVendor::Apple,
                v if v.contains("qualcomm") => GpuVendor::Qualcomm,
                _ => GpuVendor::Unknown,
            };

            GpuInfo {
                name: gpu.model_name().to_string(),
                vendor,
                backend: gpu.api().unwrap_or("Unknown").to_string(),
                device_type: gpu.device_type().to_string(),
                vram_mb: gpu.memory_mb(),
                temperature_c: gpu.temperature_c(),
                utilization_percent: gpu.utilization_percent(),
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

    Some(NpuInfo {
        detected: true,
        confidence: NpuConfidence::High, // hardware-query provides actual detection
        details: format!("{} - {}", npu.vendor(), npu.model_name()),
        method: "hardware-query offline detection".to_string(),
    })
}
