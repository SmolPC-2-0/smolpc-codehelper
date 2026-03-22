use crate::hardware::errors::HardwareError;
use crate::hardware::types::{CpuInfo, GpuInfo, HardwareInfo, MemoryInfo, NpuInfo, StorageInfo};
use std::ffi::OsStr;
use sysinfo::{CpuRefreshKind, DiskKind, Disks, MemoryRefreshKind, RefreshKind, System};

/// Helper Functions
/// Convert non-zero u32 to Option (0 = detection failed)
fn non_zero_u32(val: u32) -> Option<u32> {
    (val > 0).then_some(val)
}

fn non_empty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed.to_string())
}

fn non_empty_os_string(value: &OsStr) -> Option<String> {
    let rendered = value.to_string_lossy();
    non_empty_string(rendered.as_ref())
}

fn push_feature(features: &mut Vec<String>, enabled: bool, name: &str) {
    if enabled {
        features.push(name.to_string());
    }
}

fn detect_cpu_features() -> Vec<String> {
    let mut features = Vec::new();

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("sse"),
            "SSE",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("sse2"),
            "SSE2",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("sse3"),
            "SSE3",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("sse4.1"),
            "SSE41",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("sse4.2"),
            "SSE42",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("avx"),
            "AVX",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("avx2"),
            "AVX2",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("avx512f"),
            "AVX512",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("fma"),
            "FMA",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("aes"),
            "AES",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("sha"),
            "SHA",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("bmi1"),
            "BMI1",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("bmi2"),
            "BMI2",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("rdrand"),
            "RDRAND",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("rdseed"),
            "RDSEED",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("popcnt"),
            "POPCNT",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("lzcnt"),
            "LZCNT",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("movbe"),
            "MOVBE",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("xsave"),
            "XSAVE",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("xsaveopt"),
            "XSAVEOPT",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("xsavec"),
            "XSAVEC",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("xsaves"),
            "XSAVES",
        );
        push_feature(
            &mut features,
            std::arch::is_x86_feature_detected!("f16c"),
            "F16C",
        );
    }

    #[cfg(target_arch = "aarch64")]
    {
        push_feature(
            &mut features,
            std::arch::is_aarch64_feature_detected!("neon"),
            "NEON",
        );
        push_feature(
            &mut features,
            std::arch::is_aarch64_feature_detected!("sve"),
            "SVE",
        );
    }

    features
}

fn sysinfo_memory_values_are_bytes(total_raw: u64) -> bool {
    // sysinfo may expose bytes (newer releases) or KiB (older releases).
    // Cargo.toml pins sysinfo to 0.32.1; this check guards us if that contract
    // changes during dependency updates.
    // This heuristic is intentionally scoped to supported SmolPC targets where
    // model minimum RAM starts at 8 GB. In that range, KiB totals remain far
    // below 1e9 while byte totals are above 1e9.
    // Sub-1 GB machines are unsupported and may be misclassified.
    total_raw > 1_000_000_000
}

fn raw_memory_to_gb(raw: u64, values_are_bytes: bool) -> f64 {
    if values_are_bytes {
        raw as f64 / (1024.0 * 1024.0 * 1024.0)
    } else {
        raw as f64 / (1024.0 * 1024.0)
    }
}

fn bytes_to_gb(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0 * 1024.0)
}

/// Main entry point for hardware detection using sysinfo crate
/// Detects CPU, memory, and storage information offline without WMI/COM.
pub async fn detect_all() -> Result<HardwareInfo, HardwareError> {
    log::info!("Starting hardware detection using sysinfo");

    let mut system = System::new_with_specifics(
        RefreshKind::new()
            .with_memory(MemoryRefreshKind::everything())
            .with_cpu(CpuRefreshKind::everything()),
    );

    // Keep refresh explicit so we don't depend on constructor-side behavior.
    system.refresh_memory();
    system.refresh_cpu_specifics(CpuRefreshKind::everything());

    let info = HardwareInfo {
        cpu: convert_cpu_info(&system),
        gpus: convert_gpu_info(),
        npu: convert_npu_info(),
        memory: convert_memory_info(&system),
        storage: convert_storage_info(),
        detected_at: chrono::Utc::now().to_rfc3339(),
    };

    log::info!("Hardware detection completed via sysinfo");
    Ok(info)
}

/// Convert sysinfo CPU information to our CpuInfo format.
fn convert_cpu_info(system: &System) -> CpuInfo {
    let primary_cpu = system.cpus().first();
    let vendor = primary_cpu
        .and_then(|cpu| non_empty_string(cpu.vendor_id()))
        .unwrap_or_else(|| "Unknown".to_string());
    let brand = primary_cpu
        .and_then(|cpu| non_empty_string(cpu.brand()))
        .unwrap_or_else(|| "Unknown CPU".to_string());
    let architecture = System::cpu_arch().unwrap_or_else(|| std::env::consts::ARCH.to_string());
    let cores_logical = std::cmp::max(
        system.cpus().len(),
        std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(0),
    )
    .max(1);
    let cores_physical = system.physical_core_count().unwrap_or(cores_logical).max(1);
    let frequency_mhz = primary_cpu
        .and_then(|cpu| u32::try_from(cpu.frequency()).ok())
        .and_then(non_zero_u32);
    let features = detect_cpu_features();

    if frequency_mhz.is_none() {
        log::warn!("CPU frequency detection unavailable via sysinfo");
    }
    if features.is_empty() {
        log::warn!("CPU feature detection unavailable for current architecture/runtime");
    }

    CpuInfo {
        vendor,
        brand,
        architecture,
        cores_physical,
        cores_logical,
        frequency_mhz,
        features,
        cache_l1_kb: None,
        cache_l2_kb: None,
        cache_l3_kb: None,
    }
}

/// GPU detection is intentionally empty for now.
/// The pinned sysinfo version used by this app does not expose GPU inventory.
fn convert_gpu_info() -> Vec<GpuInfo> {
    log::info!("GPU detection unavailable via sysinfo=0.32.1; returning empty list");
    Vec::new()
}

/// NPU detection is intentionally empty for now.
/// WMI-backed probing was removed from this path to prevent startup hangs.
fn convert_npu_info() -> Option<NpuInfo> {
    None
}

/// Convert sysinfo memory information to our MemoryInfo format.
fn convert_memory_info(system: &System) -> MemoryInfo {
    let total_raw = system.total_memory();
    let available_raw = system.available_memory();
    let values_are_bytes = sysinfo_memory_values_are_bytes(total_raw);
    let total_gb = raw_memory_to_gb(total_raw, values_are_bytes);
    let available_gb = raw_memory_to_gb(available_raw, values_are_bytes);

    if total_gb <= 0.0 {
        log::warn!("System total memory detection failed via sysinfo");
    }
    if available_gb <= 0.0 {
        log::warn!("System available memory detection failed via sysinfo");
    }

    MemoryInfo {
        total_gb,
        available_gb,
    }
}

/// Convert sysinfo storage information to our StorageInfo format.
fn convert_storage_info() -> StorageInfo {
    let disks = Disks::new_with_refreshed_list();
    let primary_disk = disks
        .list()
        .iter()
        .filter(|disk| !disk.is_removable())
        .max_by_key(|disk| disk.total_space())
        .or_else(|| disks.list().iter().max_by_key(|disk| disk.total_space()));

    if let Some(primary) = primary_disk {
        let total_gb = bytes_to_gb(primary.total_space());
        let available_gb = bytes_to_gb(primary.available_space());
        let mount_point = primary.mount_point().display().to_string();
        let device_name = non_empty_os_string(primary.name())
            .or_else(|| non_empty_string(&mount_point))
            .unwrap_or_else(|| "Unknown".to_string());
        let is_ssd = matches!(primary.kind(), DiskKind::SSD);

        if total_gb <= 0.0 {
            log::warn!("Primary storage capacity detection failed via sysinfo");
        }

        StorageInfo {
            total_gb,
            available_gb,
            is_ssd,
            device_name,
        }
    } else {
        log::warn!("No storage devices detected by sysinfo");
        StorageInfo {
            total_gb: 0.0,
            available_gb: 0.0,
            is_ssd: false,
            device_name: "Unknown".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{raw_memory_to_gb, sysinfo_memory_values_are_bytes};

    #[test]
    fn sysinfo_memory_unit_heuristic_targets_supported_ram_range() {
        assert!(!sysinfo_memory_values_are_bytes(8 * 1024 * 1024));
        assert!(sysinfo_memory_values_are_bytes(8 * 1024 * 1024 * 1024));
        assert!(!sysinfo_memory_values_are_bytes(999_999_488));
    }

    #[test]
    fn raw_memory_to_gb_handles_bytes_and_kib_contracts() {
        let gib_in_bytes = 1024 * 1024 * 1024;
        let gib_in_kib = 1024 * 1024;

        assert!((raw_memory_to_gb(gib_in_bytes, true) - 1.0).abs() < f64::EPSILON);
        assert!((raw_memory_to_gb(gib_in_kib, false) - 1.0).abs() < f64::EPSILON);
    }
}
