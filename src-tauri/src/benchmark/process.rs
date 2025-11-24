//! Ollama process detection and model warmup.
//!
//! Identifies the inference process by memory usage after loading a model.

use crate::commands::ollama::{OllamaConfig, OllamaMessage, OllamaRequest};
use crate::hardware;
use super::test_suite::SHORT_PROMPTS;
use sysinfo::System;
use std::time::Duration;

/// Minimum memory (bytes) for inference process detection.
/// Inference processes typically use 500MB+ (usually GBs) vs ~50-100MB for server/CLI.
const INFERENCE_PROCESS_MIN_MEMORY: u64 = 500 * 1024 * 1024;

const BYTES_PER_MB: f64 = 1024.0 * 1024.0;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
const WARMUP_STABILIZATION_DELAY: Duration = Duration::from_millis(500);

/// Hardware metadata snapshot for benchmark results.
#[derive(Debug, Clone)]
pub struct HardwareSnapshot {
    pub cpu_model: String,
    pub gpu_name: String,
    pub avx2_supported: bool,
    pub npu_detected: bool,
    pub detection_failed: bool,
}

impl HardwareSnapshot {
    /// Detect hardware, falling back to defaults on failure.
    pub async fn detect() -> Self {
        match hardware::detect_all().await {
            Ok(info) => {
                let gpu_name = info.gpus.iter()
                    .find(|g| g.device_type.eq_ignore_ascii_case("discrete"))
                    .or(info.gpus.first())
                    .map(|g| g.name.clone())
                    .unwrap_or_else(|| "No GPU".to_string());

                Self {
                    cpu_model: info.cpu.brand.clone(),
                    gpu_name,
                    avx2_supported: info.cpu.features.avx2,
                    npu_detected: info.npu.as_ref().is_some_and(|n| n.detected),
                    detection_failed: false,
                }
            }
            Err(e) => {
                log::warn!("Hardware detection failed: {e}");
                Self::default()
            }
        }
    }
}

impl Default for HardwareSnapshot {
    fn default() -> Self {
        Self {
            cpu_model: "Unknown CPU".to_string(),
            gpu_name: "Unknown GPU".to_string(),
            avx2_supported: false,
            npu_detected: false,
            detection_failed: true,
        }
    }
}

/// Load model via warmup request and identify the Ollama inference process.
///
/// The inference process is identified by memory usage (the loaded model uses GBs).
pub async fn warmup_and_find_process(
    model: &str,
    client: &reqwest::Client,
    config: &OllamaConfig,
) -> Result<sysinfo::Pid, String> {
    let warmup_prompt = SHORT_PROMPTS.first().copied().unwrap_or("What is a variable in Python?");

    let request = OllamaRequest {
        model: model.to_string(),
        messages: vec![OllamaMessage {
            role: "user".to_string(),
            content: warmup_prompt.to_string(),
        }],
        stream: false,
    };

    let url = format!("{}/api/chat", config.base_url());

    let response = client
        .post(&url)
        .json(&request)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!("Warmup timed out after {REQUEST_TIMEOUT:?} - model may be too large")
            } else {
                format!("Warmup request failed: {e}")
            }
        })?;

    if !response.status().is_success() {
        return Err(format!(
            "Warmup failed with status {}: model '{}' may not be available",
            response.status(), model
        ));
    }

    // Consume response body to ensure model is fully loaded
    let _ = response.bytes().await.map_err(|e| format!("Failed to read warmup response: {e}"))?;

    tokio::time::sleep(WARMUP_STABILIZATION_DELAY).await;

    find_inference_process()
}

/// Find the Ollama inference process by memory usage.
fn find_inference_process() -> Result<sysinfo::Pid, String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut candidates: Vec<(sysinfo::Pid, u64, String)> = sys
        .processes()
        .iter()
        .filter_map(|(pid, proc)| {
            let name = proc.name().to_string_lossy();
            if name.to_ascii_lowercase().contains("ollama") {
                Some((*pid, proc.memory(), name.to_string()))
            } else {
                None
            }
        })
        .collect();

    if candidates.is_empty() {
        return Err("No Ollama process found - ensure Ollama is running".to_string());
    }

    log::debug!("Found {} Ollama process(es):", candidates.len());
    for (pid, mem, name) in &candidates {
        log::debug!("  PID {}: {} ({:.1} MB)", pid, name, *mem as f64 / BYTES_PER_MB);
    }

    // Sort by memory descending, select highest
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    let (pid, mem, name) = candidates.first().unwrap();

    if *mem < INFERENCE_PROCESS_MIN_MEMORY {
        let threshold_mb = INFERENCE_PROCESS_MIN_MEMORY as f64 / BYTES_PER_MB;
        let found_mb = *mem as f64 / BYTES_PER_MB;
        return Err(format!(
            "No inference process found with loaded model. \
             Highest memory process '{name}' has {found_mb:.1} MB, need >{threshold_mb:.0} MB"
        ));
    }

    log::info!("Selected Ollama process: '{}' (PID {}, {:.1} MB)", name, pid, *mem as f64 / BYTES_PER_MB);

    // Warn if multiple high-memory processes detected
    if candidates.len() > 1 {
        let (_, second_mem, second_name) = &candidates[1];
        let memory_ratio = *mem as f64 / (*second_mem).max(1) as f64;
        if memory_ratio < 2.0 && *second_mem >= INFERENCE_PROCESS_MIN_MEMORY {
            log::warn!(
                "Multiple high-memory Ollama processes detected: '{}' ({:.1} MB) vs '{}' ({:.1} MB)",
                name, *mem as f64 / BYTES_PER_MB, second_name, *second_mem as f64 / BYTES_PER_MB
            );
        }
    }

    Ok(*pid)
}
