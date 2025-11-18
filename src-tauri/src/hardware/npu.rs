use crate::hardware::types::{CpuInfo, NpuConfidence, NpuInfo};

/// Detect NPU availability
pub fn detect(cpu: &CpuInfo) -> Result<Option<NpuInfo>, String> {
    // Try Windows DXCore detection first (highest confidence)
    #[cfg(target_os = "windows")]
    {
        if let Some(npu) = detect_dxcore_npu()? {
            return Ok(Some(npu));
        }
    }

    // Fallback: CPU model heuristic detection
    detect_cpu_model_heuristic(cpu)
}

/// Detect NPU via CPU model name heuristics
fn detect_cpu_model_heuristic(cpu: &CpuInfo) -> Result<Option<NpuInfo>, String> {
    let brand_lower = cpu.brand.to_lowercase();

    // Intel Core Ultra (Intel AI Boost NPU)
    if brand_lower.contains("core ultra") {
        return Ok(Some(NpuInfo {
            detected: true,
            confidence: NpuConfidence::Medium,
            details: "Intel AI Boost NPU detected (Intel Core Ultra series)".to_string(),
            method: "CPU model heuristic".to_string(),
        }));
    }

    // Qualcomm Snapdragon X Elite/Plus (Hexagon NPU)
    if brand_lower.contains("snapdragon x elite") || brand_lower.contains("snapdragon x plus") {
        return Ok(Some(NpuInfo {
            detected: true,
            confidence: NpuConfidence::Medium,
            details: "Qualcomm Hexagon NPU detected (Snapdragon X Elite/Plus)".to_string(),
            method: "CPU model heuristic".to_string(),
        }));
    }

    // AMD Ryzen AI (future support)
    if brand_lower.contains("ryzen ai") {
        return Ok(Some(NpuInfo {
            detected: true,
            confidence: NpuConfidence::Medium,
            details: "AMD Ryzen AI NPU detected".to_string(),
            method: "CPU model heuristic".to_string(),
        }));
    }

    Ok(None)
}

/// Detect NPU via Windows DXCore API
#[cfg(target_os = "windows")]
fn detect_dxcore_npu() -> Result<Option<NpuInfo>, String> {
    use windows::Win32::Graphics::Direct3D12::*;
    use windows::Win32::Graphics::Dxgi::*;

    unsafe {
        // Try to create DXCore adapter factory
        let factory_result: Result<IDXGIFactory4, _> = CreateDXGIFactory2(0);

        if let Ok(factory) = factory_result {
            // Enumerate adapters
            for i in 0.. {
                let adapter_result: Result<IDXGIAdapter1, _> = factory.EnumAdapters1(i);

                match adapter_result {
                    Ok(adapter) => {
                        // Get adapter description
                        if let Ok(desc) = adapter.GetDesc1() {
                            let device_name = String::from_utf16_lossy(&desc.Description);

                            // Check for NPU indicators in device description
                            let device_lower = device_name.to_lowercase();

                            // Known NPU identifiers
                            if device_lower.contains("npu")
                                || device_lower.contains("neural processor")
                                || device_lower.contains("ai boost")
                                || device_lower.contains("hexagon")
                            {
                                return Ok(Some(NpuInfo {
                                    detected: true,
                                    confidence: NpuConfidence::High,
                                    details: format!("NPU detected via DXCore: {}", device_name.trim_end_matches('\0')),
                                    method: "Windows DXCore API".to_string(),
                                }));
                            }
                        }
                    }
                    Err(_) => break, // No more adapters
                }
            }
        }
    }

    Ok(None)
}

/// Non-Windows platforms don't support DXCore
#[cfg(not(target_os = "windows"))]
fn detect_dxcore_npu() -> Result<Option<NpuInfo>, String> {
    Ok(None)
}
