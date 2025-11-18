use crate::hardware::types::{CpuFeatures, CpuInfo};
use sysinfo::System;

/// Detect CPU information (cross-platform)
pub fn detect() -> Result<CpuInfo, String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    // Core counts (works on all platforms via sysinfo)
    let cores_logical = sys.cpus().len();
    let cores_physical = sys.physical_core_count().unwrap_or(cores_logical);

    // CPU frequency (from first CPU)
    let frequency_mhz = sys.cpus().first().map(|cpu| cpu.frequency());

    // Platform-specific detection
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        detect_x86(sys, cores_physical, cores_logical, frequency_mhz)
    }

    #[cfg(target_arch = "aarch64")]
    {
        detect_arm64(sys, cores_physical, cores_logical, frequency_mhz)
    }

    #[cfg(not(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "aarch64"
    )))]
    {
        detect_generic(sys, cores_physical, cores_logical, frequency_mhz)
    }
}

/// x86/x86_64 CPU detection using CPUID
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86(
    sys: System,
    cores_physical: usize,
    cores_logical: usize,
    frequency_mhz: Option<u64>,
) -> Result<CpuInfo, String> {
    use raw_cpuid::CpuId;

    // Create CpuId instance using the native CPUID reader
    let cpuid = CpuId::new();

    // Vendor information
    let vendor = cpuid
        .get_vendor_info()
        .map(|v| v.as_str().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    // Brand string (e.g., "Intel(R) Core(TM) i7-12700K")
    let brand = cpuid
        .get_processor_brand_string()
        .map(|b| b.as_str().trim().to_string())
        .unwrap_or_else(|| {
            // Fallback to sysinfo brand
            sys.cpus()
                .first()
                .map(|cpu| cpu.brand().to_string())
                .unwrap_or_else(|| "Unknown CPU".to_string())
        });

    // Feature detection using runtime detection
    let features = CpuFeatures {
        // x86 features
        sse42: is_x86_feature_detected!("sse4.2"),
        avx: is_x86_feature_detected!("avx"),
        avx2: is_x86_feature_detected!("avx2"),
        avx512f: is_x86_feature_detected!("avx512f"),
        fma: is_x86_feature_detected!("fma"),
        // ARM features (not available on x86)
        neon: false,
        sve: false,
    };

    // Cache information from CPUID
    let (cache_l1_kb, cache_l2_kb, cache_l3_kb) = detect_cache_info_x86(&cpuid);

    Ok(CpuInfo {
        vendor,
        brand,
        architecture: std::env::consts::ARCH.to_string(),
        cores_physical,
        cores_logical,
        frequency_mhz,
        features,
        cache_l1_kb,
        cache_l2_kb,
        cache_l3_kb,
    })
}

/// x86 cache detection via CPUID
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_cache_info_x86(cpuid: &raw_cpuid::CpuId) -> (Option<usize>, Option<usize>, Option<usize>) {
    let mut l1_data = None;
    let mut l2 = None;
    let mut l3 = None;

    // Try Intel cache info (leaf 0x04)
    if let Some(mut cache_params) = cpuid.get_cache_parameters() {
        for cache in &mut cache_params {
            let cache_size_kb = (cache.associativity() as usize
                * cache.physical_line_partitions() as usize
                * cache.coherency_line_size() as usize
                * cache.sets() as usize)
                / 1024;

            match cache.level() {
                1 if cache.cache_type() == raw_cpuid::CacheType::Data => {
                    l1_data = Some(cache_size_kb);
                }
                2 => l2 = Some(cache_size_kb),
                3 => l3 = Some(cache_size_kb),
                _ => {}
            }
        }
    }

    // Fallback: Try AMD cache info (leaf 0x8000_0006)
    if l2.is_none() || l3.is_none() {
        if let Some(l2_l3_info) = cpuid.get_l2_l3_cache_and_tlb_info() {
            if l2.is_none() {
                l2 = Some(l2_l3_info.l2cache_size() as usize);
            }
            if l3.is_none() {
                let l3_size = l2_l3_info.l3cache_size() as usize;
                if l3_size > 0 {
                    l3 = Some(l3_size * 512); // AMD reports in 512KB units
                }
            }
        }
    }

    (l1_data, l2, l3)
}

/// ARM64/AArch64 CPU detection
#[cfg(target_arch = "aarch64")]
fn detect_arm64(
    sys: System,
    cores_physical: usize,
    cores_logical: usize,
    frequency_mhz: Option<u64>,
) -> Result<CpuInfo, String> {
    // Vendor detection from CPU brand
    let brand = sys
        .cpus()
        .first()
        .map(|cpu| cpu.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    let vendor = if brand.contains("Apple") {
        "Apple"
    } else if brand.contains("Qualcomm") || brand.contains("Snapdragon") {
        "Qualcomm"
    } else if brand.contains("ARM") {
        "ARM"
    } else {
        "Unknown"
    }
    .to_string();

    // ARM feature detection using runtime detection
    #[cfg(target_feature = "neon")]
    let neon = true;
    #[cfg(not(target_feature = "neon"))]
    let neon = std::arch::is_aarch64_feature_detected!("neon");

    // SVE detection (Scalable Vector Extension - ARM's equivalent to AVX-512)
    #[cfg(target_feature = "sve")]
    let sve = true;
    #[cfg(not(target_feature = "sve"))]
    let sve = std::arch::is_aarch64_feature_detected!("sve");

    // ARM has different features than x86
    let features = CpuFeatures {
        // x86 features (not available on ARM)
        sse42: false,
        avx: false,
        avx2: false,
        avx512f: false,
        fma: false,
        // ARM features
        neon,
        sve,
    };

    // Cache detection not available via standard APIs on ARM
    // Could potentially read from /sys/devices/system/cpu on Linux
    let (cache_l1_kb, cache_l2_kb, cache_l3_kb) = (None, None, None);

    Ok(CpuInfo {
        vendor,
        brand,
        architecture: std::env::consts::ARCH.to_string(),
        cores_physical,
        cores_logical,
        frequency_mhz,
        features,
        cache_l1_kb,
        cache_l2_kb,
        cache_l3_kb,
    })
}

/// Generic fallback for other architectures
#[cfg(not(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64"
)))]
fn detect_generic(
    sys: System,
    cores_physical: usize,
    cores_logical: usize,
    frequency_mhz: Option<u64>,
) -> Result<CpuInfo, String> {
    let brand = sys
        .cpus()
        .first()
        .map(|cpu| cpu.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    let vendor = "Unknown".to_string();

    // No feature detection on unknown architectures
    let features = CpuFeatures {
        // x86 features
        sse42: false,
        avx: false,
        avx2: false,
        avx512f: false,
        fma: false,
        // ARM features
        neon: false,
        sve: false,
    };

    Ok(CpuInfo {
        vendor,
        brand,
        architecture: std::env::consts::ARCH.to_string(),
        cores_physical,
        cores_logical,
        frequency_mhz,
        features,
        cache_l1_kb: None,
        cache_l2_kb: None,
        cache_l3_kb: None,
    })
}
