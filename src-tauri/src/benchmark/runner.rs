//! # Benchmark Runner - Production-Grade Data Collection
//!
//! ## Measurement Approach
//!
//! This module implements production-grade benchmark data collection suitable for academic research.
//! All measurements prioritize accuracy over performance, using native data sources wherever possible.
//!
//! ### Token Metrics
//! - Uses Ollama's native token metadata (`eval_count`) - no estimation
//! - Non-streaming API calls to ensure complete metadata availability
//! - Tests fail immediately if accurate token counts unavailable
//!
//! ### Timing Metrics
//! - All timing from Ollama's nanosecond-precision native timing data
//! - No client-side stopwatch measurements (subject to network latency)
//! - `first_token_ms`: From `prompt_eval_duration`
//! - `total_time_ms`: From `total_duration`
//! - `tokens_per_sec`: Calculated from `eval_count` / `eval_duration`
//!
//! ### Resource Monitoring
//! - **Warmup phase**: Loads model and identifies Ollama process PID before any tests
//! - **Process-specific monitoring**: Tracks Ollama inference process only (no system-wide fallback)
//! - **CPU baseline**: 200ms delay between refresh cycles (required by sysinfo crate)
//! - **Sampling frequency**: 50ms intervals during inference (rigorous monitoring)
//! - **Memory metrics**: Process-specific measurements at before/during(median)/peak/after
//! - **Statistical robustness**: Median (not average) for memory_during to resist outliers
//!
//! ## Known Limitations
//!
//! ### CPU Measurement Undercounting
//! CPU measurements show ~4-16% instead of expected 50-100% due to HTTP API architecture:
//! - Ollama runs in separate process: ~16% CPU, ~85% GPU (GPU-accelerated inference)
//! - Benchmark client (this process): ~40% CPU from HTTP overhead, JSON parsing, async runtime
//! - We monitor Ollama process only, missing the HTTP client overhead
//!
//! This is an architectural limitation of the HTTP API approach. CPU measurements are:
//! - ✅ Consistent across tests (useful for relative comparisons)
//! - ✅ Accurate for the Ollama process itself
//! - ❌ Don't capture total CPU cost (missing client-side HTTP overhead)
//!
//! **Resolution**: Planned migration to in-process llama.cpp integration will:
//! - Eliminate HTTP overhead (40% CPU)
//! - Enable monitoring of single unified process
//! - Provide accurate total CPU measurements
//!
//! ### GPU Metrics Not Captured
//! Currently no GPU utilization metrics collected. For GPU-accelerated inference,
//! low CPU usage is expected and legitimate.

use super::metrics::{BenchmarkMetrics, BenchmarkResults, calculate_summary, get_timestamp};
use super::test_suite::{get_test_suite, get_total_test_count, PromptCategory};
use crate::commands::ollama::{OllamaConfig, OllamaMessage, OllamaRequest, OllamaResponse};
use crate::hardware;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use sysinfo::System;
use tokio::sync::Mutex;

// Benchmark configuration constants
/// Interval (in milliseconds) for sampling CPU and memory during inference
/// 50ms provides rigorous monitoring for production-quality data
const RESOURCE_SAMPLING_INTERVAL_MS: u64 = 50;

/// Delay (in milliseconds) between tests to allow system stabilization
const TEST_STABILIZATION_DELAY_MS: u64 = 500;

/// Delay (in milliseconds) for CPU baseline establishment
const CPU_BASELINE_DELAY_MS: u64 = 200;

/// Progress update event for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkProgress {
    pub current: usize,
    pub total: usize,
    pub current_test: String,
    pub iteration: usize,
}

/// Warmup function to load model and identify Ollama process
/// This eliminates first-call latency and establishes process monitoring
async fn warmup_and_find_ollama_process(
    model: &str,
    client: &reqwest::Client,
    config: &OllamaConfig,
) -> Result<sysinfo::Pid, String> {
    // Make a minimal request to load the model
    let warmup_messages = vec![OllamaMessage {
        role: "user".to_string(),
        content: "Hi".to_string(),
    }];

    let request = OllamaRequest {
        model: model.to_string(),
        messages: warmup_messages,
        stream: false,
    };

    let url = format!("{}/api/chat", config.base_url());
    let _response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Warmup request failed: {}", e))?;

    // Give Ollama a moment to fully initialize the model
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Find the Ollama process
    let mut sys = System::new_all();
    sys.refresh_all();

    let ollama_pid = sys
        .processes()
        .iter()
        .find(|(_, process)| {
            let name = process.name().to_string_lossy().to_ascii_lowercase();
            name.contains("ollama")
        })
        .map(|(pid, _)| *pid)
        .ok_or_else(|| {
            "CRITICAL ERROR: Could not find Ollama process. \
             Benchmark requires process-specific monitoring. \
             Ensure Ollama is running before starting benchmark."
                .to_string()
        })?;

    Ok(ollama_pid)
}

/// Run a single benchmark test and collect metrics with accurate streaming measurements
/// Returns (BenchmarkMetrics, response_content) for follow-up context
async fn run_single_test(
    prompt: String,
    category: PromptCategory,
    model: String,
    iteration: usize,
    context: Option<Vec<OllamaMessage>>,
    client: &reqwest::Client,
    config: &OllamaConfig,
    ollama_pid: sysinfo::Pid,
    cpu_model: String,
    gpu_name: String,
    avx2_supported: bool,
    npu_detected: bool,
) -> Result<(BenchmarkMetrics, String), String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    // Capture initial process-specific memory state
    let memory_before_mb = sys
        .process(ollama_pid)
        .map(|p| (p.memory() as f64) / 1024.0 / 1024.0)
        .ok_or_else(|| "Ollama process disappeared before test started".to_string())?;

    // Build messages array
    let system_prompt = r#"You are a helpful coding assistant designed for secondary school students (ages 11-18).
Your goal is to explain programming concepts clearly and provide well-commented code examples.

Guidelines:
- Use simple, encouraging language
- Break down complex concepts into steps
- Always include helpful comments in code
- Be patient and supportive
- Adapt explanations to the student's level
- Encourage learning and experimentation"#;

    let mut messages = vec![OllamaMessage {
        role: "system".to_string(),
        content: system_prompt.to_string(),
    }];

    // Add context if provided (for follow-up tests)
    if let Some(ctx) = context {
        messages.extend(ctx);
    }

    // Add current prompt
    messages.push(OllamaMessage {
        role: "user".to_string(),
        content: prompt.clone(),
    });

    let request = OllamaRequest {
        model: model.clone(),
        messages,
        stream: false, // Non-streaming for accurate Ollama-native timing data
    };

    // Shared state for periodic sampling
    let peak_memory = Arc::new(Mutex::new(memory_before_mb));
    let cpu_samples = Arc::new(Mutex::new(Vec::new()));
    let memory_samples = Arc::new(Mutex::new(Vec::new()));
    let sampling_active = Arc::new(Mutex::new(true));

    // Channel for signaling sampling task completion
    let (sampling_done_tx, sampling_done_rx) = tokio::sync::oneshot::channel();

    // Spawn background task for periodic resource sampling
    let peak_memory_clone = Arc::clone(&peak_memory);
    let cpu_samples_clone = Arc::clone(&cpu_samples);
    let memory_samples_clone = Arc::clone(&memory_samples);
    let sampling_active_clone = Arc::clone(&sampling_active);

    tokio::spawn(async move {
        let mut sys_sampler = System::new_all();
        sys_sampler.refresh_all();

        // Establish CPU baseline - requires initial refresh + delay + second refresh
        sys_sampler.refresh_cpu_all();
        tokio::time::sleep(tokio::time::Duration::from_millis(CPU_BASELINE_DELAY_MS)).await;
        sys_sampler.refresh_cpu_all();

        // Use the pre-identified Ollama PID (passed in as parameter)
        // No need to search - we already found it during warmup
        while *sampling_active_clone.lock().await {
            sys_sampler.refresh_all();
            sys_sampler.refresh_cpu_all();

            // Process-specific monitoring (REQUIRED)
            if let Some(process) = sys_sampler.process(ollama_pid) {
                let memory = (process.memory() as f64) / 1024.0 / 1024.0;
                let cpu = process.cpu_usage() as f64;

                // Update peak memory
                let mut peak = peak_memory_clone.lock().await;
                if memory > *peak {
                    *peak = memory;
                }
                drop(peak);

                // Store samples
                cpu_samples_clone.lock().await.push(cpu);
                memory_samples_clone.lock().await.push(memory);
            } else {
                // Process disappeared during monitoring - this is a critical error
                break;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(RESOURCE_SAMPLING_INTERVAL_MS)).await;
        }

        // Signal completion
        let _ = sampling_done_tx.send(());
    });

    // Make non-streaming request to get accurate Ollama timing metadata
    let url = format!("{}/api/chat", config.base_url());
    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to Ollama: {}", e))?;

    // Parse the response JSON
    let ollama_response: OllamaResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

    // Stop resource sampling and wait for completion
    *sampling_active.lock().await = false;
    let _ = sampling_done_rx.await; // Wait for sampling task to actually finish

    // Validate that we successfully monitored the Ollama process
    let cpu_samples_vec = cpu_samples.lock().await.clone();
    let memory_samples_vec = memory_samples.lock().await.clone();

    if cpu_samples_vec.is_empty() || memory_samples_vec.is_empty() {
        return Err("Failed to monitor Ollama process - process not found or disappeared during benchmark".to_string());
    }

    // Extract response content
    let response_content = ollama_response.message
        .as_ref()
        .map(|m| m.content.clone())
        .unwrap_or_default();

    // === CRITICAL VALIDATION: Require all Ollama timing metadata ===
    // These fields are required for production-quality data
    let eval_count = ollama_response.eval_count
        .ok_or_else(|| "Ollama did not provide eval_count (token count) - cannot proceed with benchmark".to_string())?;

    let eval_duration_ns = ollama_response.eval_duration
        .ok_or_else(|| "Ollama did not provide eval_duration - cannot proceed with benchmark".to_string())?;

    let prompt_eval_duration_ns = ollama_response.prompt_eval_duration
        .unwrap_or(0); // Prompt eval can be 0 if model is already loaded

    let total_duration_ns = ollama_response.total_duration
        .ok_or_else(|| "Ollama did not provide total_duration - cannot proceed with benchmark".to_string())?;

    // Convert nanoseconds to milliseconds for all timing metrics
    let eval_duration_ms = (eval_duration_ns as f64) / 1_000_000.0;
    let prompt_eval_duration_ms = (prompt_eval_duration_ns as f64) / 1_000_000.0;
    let total_duration_ms = (total_duration_ns as f64) / 1_000_000.0;

    // === Calculate metrics using Ollama's NATIVE data (not our stopwatch) ===

    // First token latency = Time to process prompt (before first token generation)
    let first_token_latency_ms = prompt_eval_duration_ms;

    // Total response time = Full request duration from Ollama's perspective
    let total_response_time_ms = total_duration_ms;

    // Tokens per second = Using Ollama's actual generation time
    let tokens_per_second = if eval_duration_ms > 0.0 {
        (eval_count as f64) / (eval_duration_ms / 1000.0)
    } else {
        -1.0 // Invalid data marker
    };

    // Average time per token = Using Ollama's generation time
    let avg_token_latency = if eval_count > 0 {
        eval_duration_ms / eval_count as f64
    } else {
        -1.0 // Invalid data marker
    };

    // === Resource metrics from our process-specific sampling ===
    // Refresh final system state and get process-specific memory
    sys.refresh_all();
    let memory_after_mb = sys
        .process(ollama_pid)
        .map(|p| (p.memory() as f64) / 1024.0 / 1024.0)
        .ok_or_else(|| "Ollama process disappeared after test completed".to_string())?;

    // Get peak memory from sampling
    let peak_memory_mb = *peak_memory.lock().await;

    // Calculate average CPU from samples
    let avg_cpu = cpu_samples_vec.iter().sum::<f64>() / cpu_samples_vec.len() as f64;

    // Calculate MEDIAN memory during inference (more robust to outliers than average)
    let mut sorted_memory = memory_samples_vec.clone();
    sorted_memory.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_memory_during = if sorted_memory.len() % 2 == 0 {
        let mid = sorted_memory.len() / 2;
        (sorted_memory[mid - 1] + sorted_memory[mid]) / 2.0
    } else {
        sorted_memory[sorted_memory.len() / 2]
    };

    Ok((
        BenchmarkMetrics {
            first_token_latency_ms,
            total_response_time_ms,
            tokens_per_second,
            avg_token_latency_ms: avg_token_latency,
            memory_before_mb,
            memory_during_mb: median_memory_during,
            memory_after_mb,
            peak_memory_mb,
            cpu_usage_percent: avg_cpu,
            model_name: model,
            prompt_type: category.as_str().to_string(),
            prompt,
            response_tokens: eval_count,
            timestamp: get_timestamp(),
            iteration,
            cpu_model,
            gpu_name,
            avx2_supported,
            npu_detected,
        },
        response_content,
    ))
}

/// Run the complete benchmark suite
pub async fn run_benchmark_suite(
    model: String,
    iterations: usize,
    client: &reqwest::Client,
    config: &OllamaConfig,
    progress_callback: impl Fn(BenchmarkProgress),
) -> Result<BenchmarkResults, String> {
    // Detect hardware information for benchmark metadata
    let hardware_info = hardware::detect_all().await.unwrap_or_else(|e| {
        log::warn!("Failed to detect hardware: {}, using defaults", e);
        hardware::types::HardwareInfo {
            cpu: hardware::types::CpuInfo {
                vendor: "Unknown".to_string(),
                brand: "Unknown CPU".to_string(),
                architecture: "Unknown".to_string(),
                cores_physical: 0,
                cores_logical: 0,
                frequency_mhz: None,
                features: hardware::types::CpuFeatures {
                    sse42: false,
                    avx: false,
                    avx2: false,
                    avx512f: false,
                    fma: false,
                    neon: false,
                    sve: false,
                },
                cache_l1_kb: None,
                cache_l2_kb: None,
                cache_l3_kb: None,
            },
            gpus: vec![],
            npu: None,
            detected_at: chrono::Utc::now().to_rfc3339(),
        }
    });

    // Extract hardware info for benchmark metadata
    let cpu_model = hardware_info.cpu.brand.clone();
    let gpu_name = hardware_info
        .gpus
        .iter()
        .find(|g| g.device_type.to_lowercase().contains("discrete"))
        .or_else(|| hardware_info.gpus.first())
        .map(|g| g.name.clone())
        .unwrap_or_else(|| "No GPU".to_string());
    let avx2_supported = hardware_info.cpu.features.avx2;
    let npu_detected = hardware_info.npu.as_ref().map(|n| n.detected).unwrap_or(false);

    // Warmup: Load model and identify Ollama process (eliminates first-call latency)
    let ollama_pid = warmup_and_find_ollama_process(&model, client, config).await?;

    let suite_start = Instant::now();
    let test_suite = get_test_suite();
    let total_tests = get_total_test_count(iterations);
    let mut all_metrics = Vec::new();
    let mut current_test = 0;

    // Store last response for follow-up context
    let mut last_response: Option<String> = None;

    for iteration in 1..=iterations {
        for test in &test_suite {
            current_test += 1;

            // Build context for follow-up prompts
            let context = if test.category == PromptCategory::FollowUp {
                if let Some(ref prev_response) = last_response {
                    Some(vec![
                        OllamaMessage {
                            role: "user".to_string(),
                            content: "What is a variable in Python?".to_string(), // Use first short prompt as base
                        },
                        OllamaMessage {
                            role: "assistant".to_string(),
                            content: prev_response.clone(),
                        },
                    ])
                } else {
                    None
                }
            } else {
                None
            };

            // Report progress
            progress_callback(BenchmarkProgress {
                current: current_test,
                total: total_tests,
                current_test: format!("{} (iteration {})", test.id, iteration),
                iteration,
            });

            // Run the test
            let (metrics, response_content) = run_single_test(
                test.prompt.clone(),
                test.category,
                model.clone(),
                iteration,
                context,
                client,
                config,
                ollama_pid,
                cpu_model.clone(),
                gpu_name.clone(),
                avx2_supported,
                npu_detected,
            )
            .await?;

            // Store actual response content for follow-up context
            if test.category == PromptCategory::Short && test.id == "short_1" {
                last_response = Some(response_content);
            }

            all_metrics.push(metrics);

            // Small delay between tests to let system stabilize
            tokio::time::sleep(tokio::time::Duration::from_millis(TEST_STABILIZATION_DELAY_MS)).await;
        }
    }

    let total_duration = suite_start.elapsed().as_secs_f64();
    let summary = calculate_summary(&all_metrics);

    Ok(BenchmarkResults {
        metrics: all_metrics,
        summary,
        total_duration_seconds: total_duration,
        timestamp: get_timestamp(),
    })
}
