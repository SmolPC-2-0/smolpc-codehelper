use crate::hardware::types::{GpuInfo, GpuVendor};
use wgpu::{Backends, Instance, InstanceDescriptor};

/// Detect all available GPUs
pub async fn detect() -> Result<Vec<GpuInfo>, String> {
    // Create wgpu instance with primary backends (Vulkan, Metal, DX12)
    let instance = Instance::new(InstanceDescriptor {
        backends: Backends::PRIMARY,
        ..Default::default()
    });

    let mut gpus = Vec::new();

    // Enumerate all adapters
    for adapter in instance.enumerate_adapters(Backends::PRIMARY) {
        let info = adapter.get_info();

        let vendor = GpuVendor::from_pci_id(info.vendor);

        gpus.push(GpuInfo {
            name: info.name.clone(),
            vendor,
            backend: format!("{:?}", info.backend),
            device_type: format!("{:?}", info.device_type),
            vram_mb: None, // wgpu doesn't expose VRAM
            temperature_c: None,
            utilization_percent: None,
        });
    }

    Ok(gpus)
}
