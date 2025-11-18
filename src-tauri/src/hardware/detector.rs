use crate::hardware::{cpu, gpu, npu, types::HardwareInfo};

/// Main entry point for hardware detection
/// Detects CPU, GPU, and NPU information
pub async fn detect_all() -> Result<HardwareInfo, String> {
    // Detect CPU (synchronous)
    let cpu_info = cpu::detect()?;

    // Detect GPUs (async - wgpu initialization)
    let gpu_info = gpu::detect().await?;

    // Detect NPU (uses CPU info for heuristics)
    let npu_info = npu::detect(&cpu_info)?;

    Ok(HardwareInfo {
        cpu: cpu_info,
        gpus: gpu_info,
        npu: npu_info,
        detected_at: chrono::Utc::now().to_rfc3339(),
    })
}
