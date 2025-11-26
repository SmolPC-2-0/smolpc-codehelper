use serde::{Deserialize, Serialize};

// Re-export hardware-query's CPUFeature for direct use
pub use hardware_query::CPUFeature;

/// Complete hardware information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub cpu: CpuInfo,
    pub gpus: Vec<GpuInfo>,
    pub npu: Option<NpuInfo>,
    pub memory: MemoryInfo,
    pub storage: StorageInfo,
    pub detected_at: String,
}

/// CPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub vendor: String,
    pub brand: String,
    pub architecture: String,  // "x86_64", "aarch64", etc.
    pub cores_physical: usize,
    pub cores_logical: usize,
    pub frequency_mhz: Option<u32>,
    pub features: Vec<CPUFeature>,
    pub cache_l1_kb: Option<u32>,
    pub cache_l2_kb: Option<u32>,
    pub cache_l3_kb: Option<u32>,
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
    pub cuda_compute_capability: Option<String>,
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

/// System memory information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total_gb: f64,
    pub available_gb: f64,
}

/// Primary storage device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    pub total_gb: f64,
    pub available_gb: f64,
    pub is_ssd: bool,
    pub device_name: String,
}
