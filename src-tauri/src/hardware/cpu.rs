use crate::hardware::types::{CpuFeatures, CpuInfo};
use raw_cpuid::CpuId;
use sysinfo::System;

/// Detect CPU information
pub fn detect() -> Result<CpuInfo, String> {
    let cpuid = CpuId::new();
    let mut sys = System::new_all();
    sys.refresh_all();

    // Vendor information
    let vendor = cpuid
        .get_vendor_info()
        .map(|v| v.as_str().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    // Brand string (e.g., "Intel(R) Core(TM) i7-12700K")
    let brand = cpuid
        .get_processor_brand_string()
        .map(|b| b.as_str().trim().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    // Core counts
    let cores_logical = sys.cpus().len();
    let cores_physical = sys.physical_core_count().unwrap_or(cores_logical);

    // CPU frequency (from first CPU)
    let frequency_mhz = sys.cpus().first().map(|cpu| cpu.frequency());

    // Feature detection using runtime detection
    let features = CpuFeatures {
        sse42: is_x86_feature_detected!("sse4.2"),
        avx: is_x86_feature_detected!("avx"),
        avx2: is_x86_feature_detected!("avx2"),
        avx512f: is_x86_feature_detected!("avx512f"),
        fma: is_x86_feature_detected!("fma"),
    };

    // Cache information from CPUID
    let (cache_l1_kb, cache_l2_kb, cache_l3_kb) = detect_cache_info(&cpuid);

    Ok(CpuInfo {
        vendor,
        brand,
        cores_physical,
        cores_logical,
        frequency_mhz,
        features,
        cache_l1_kb,
        cache_l2_kb,
        cache_l3_kb,
    })
}

/// Detect cache hierarchy information
fn detect_cache_info(cpuid: &CpuId) -> (Option<usize>, Option<usize>, Option<usize>) {
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
