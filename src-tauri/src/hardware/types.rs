use serde::{Deserialize, Serialize};

/// Complete hardware information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub cpu: CpuInfo,
    pub gpus: Vec<GpuInfo>,
    pub npu: Option<NpuInfo>,
    pub detected_at: String,
}

/// CPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub vendor: String,
    pub brand: String,
    pub cores_physical: usize,
    pub cores_logical: usize,
    pub frequency_mhz: Option<u64>,
    pub features: CpuFeatures,
    pub cache_l1_kb: Option<usize>,
    pub cache_l2_kb: Option<usize>,
    pub cache_l3_kb: Option<usize>,
}

/// CPU feature flags (SIMD instruction sets)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuFeatures {
    pub sse42: bool,
    pub avx: bool,
    pub avx2: bool,
    pub avx512f: bool,
    pub fma: bool,
}

/// GPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: GpuVendor,
    pub backend: String,
    pub device_type: String,
    pub vram_mb: Option<u64>,
    pub temperature_c: Option<u32>,
    pub utilization_percent: Option<u32>,
}

/// GPU vendor enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
    Qualcomm,
    Unknown,
}

impl GpuVendor {
    /// Parse vendor from PCI vendor ID
    pub fn from_pci_id(vendor_id: u32) -> Self {
        match vendor_id {
            0x10DE => GpuVendor::Nvidia,
            0x1002 | 0x1022 => GpuVendor::Amd,
            0x8086 => GpuVendor::Intel,
            0x106B => GpuVendor::Apple,
            0x5143 | 0x4D4F | 0x17CB => GpuVendor::Qualcomm, // Qualcomm IDs
            _ => GpuVendor::Unknown,
        }
    }
}

/// NPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpuInfo {
    pub detected: bool,
    pub confidence: NpuConfidence,
    pub details: String,
    pub method: String,
}

/// NPU detection confidence level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NpuConfidence {
    High,   // DirectML/DXCore confirmed
    Medium, // CPU model match
    Low,    // Generic heuristic
}
