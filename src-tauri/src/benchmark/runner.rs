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
//! ### Comprehensive CPU Measurement (v2.2.1+)
//!
//! To enable accurate comparison between Ollama (HTTP-based) and future llama.cpp
//! (in-process) implementations, we now collect multiple CPU metrics:
//!
//! - **`cpu_ollama_percent`**: Ollama inference process CPU usage
//! - **`cpu_tauri_percent`**: This process (HTTP overhead, JSON parsing, async runtime)
//! - **`cpu_system_percent`**: System-wide CPU load (context for overall system activity)
//! - **`cpu_total_percent`**: Sum of ollama + tauri (true total cost of current architecture)
//!
//! **Why this matters for llama.cpp migration:**
//! - Current Ollama architecture splits CPU across two processes
//! - Ollama process: ~16% CPU (inference, mostly GPU-bound)
//! - Tauri process: ~40% CPU (HTTP overhead, JSON serialization)
//! - Total actual cost: ~56% CPU
//!
//! After llama.cpp migration (in-process):
//! - Single process handles everything
//! - HTTP overhead eliminated (cpu_tauri_percent → ~0%)
//! - cpu_total_percent will drop significantly
//!
//! By measuring both processes now, benchmarks will accurately show the
//! performance improvement when llama.cpp is integrated.
//!
//! ### GPU Metrics Not Captured
//! Currently no GPU utilization metrics collected. For GPU-accelerated inference,
//! low CPU usage is expected and legitimate.

use super::metrics::{BenchmarkMetrics, BenchmarkResults, TimingSource, calculate_summary, get_timestamp};
use super::test_suite::{get_test_suite, get_total_test_count, PromptCategory, SHORT_PROMPTS};
use crate::commands::ollama::{OllamaConfig, OllamaMessage, OllamaRequest, OllamaResponse};
use crate::hardware;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use std::time::Duration;
use sysinfo::System;
use std::sync::{Arc, Mutex};

// =============================================================================
// BENCHMARK CONFIGURATION CONSTANTS
// =============================================================================

/// Interval (in milliseconds) for sampling CPU and memory during inference.
/// 50ms provides rigorous monitoring for production-quality data.
const RESOURCE_SAMPLING_INTERVAL_MS: Duration = Duration::from_millis(50);

/// Delay (in milliseconds) between tests to allow system stabilization.
const TEST_STABILIZATION_DELAY_MS: Duration = Duration::from_millis(500);

/// Delay (in milliseconds) for CPU baseline establishment.
/// Required by sysinfo crate: needs two refresh cycles with delay between them.
const CPU_BASELINE_DELAY_MS: Duration = Duration::from_millis(200);

/// Timeout (in seconds) for Ollama HTTP requests.
/// 5 minutes allows for long responses on slower hardware.
const REQUEST_TIMEOUT_SECS: Duration = Duration::from_secs(300);

/// Delay (in milliseconds) after warmup to allow model initialization.
const WARMUP_STABILIZATION_DELAY_MS: Duration = Duration::from_millis(500);

/// Conversion factor from bytes to megabytes.
const BYTES_PER_MB: f64 = 1024.0 * 1024.0;

// =============================================================================
// HELPER TYPES
// =============================================================================

/// Snapshot of hardware information for benchmark metadata.
/// Encapsulates hardware detection with graceful fallback handling.
#[derive(Debug, Clone)]
struct HardwareSnapshot {
    cpu_model: String,
    gpu_name: String,
    avx2_supported: bool,
    npu_detected: bool,
    detection_failed: bool,
}

impl HardwareSnapshot {
    /// Create a hardware snapshot by detecting system hardware.
    /// Falls back to defaults if detection fails, setting `detection_failed` flag.
    async fn detect() -> Self {
        match hardware::detect_all().await {
            Ok(info) => {
                let gpu_name = info.gpus.iter()
                    .find(|g| g.device_type.eq_ignore_ascii_case("discrete"))
                    .or_else(|| info.gpus.first())
                    .map_or_else(|| "No GPU".to_string(), |g| g.name.clone());

                Self {
                    cpu_model: info.cpu.brand.clone(),
                    gpu_name,
                    avx2_supported: info.cpu.features.avx2,
                    npu_detected: info.npu.as_ref().is_some_and(|n| n.detected),
                    detection_failed:    false,
                }
            }
            Err(e) => {
                log::warn!("Hardware detection failed: {e}. Using defaults - benchmark metadata may be unreliable.");
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

/// Progress update event for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkProgress {
    pub current: usize,
    pub total: usize,
    pub current_test: String,
    pub iteration: usize,
}

/// Minimum memory threshold (in bytes) for the inference process.
/// The process running the model will typically use 500MB+ (usually GBs).
/// Server/CLI processes typically use <100MB.
const INFERENCE_PROCESS_MIN_MEMORY_BYTES: u64 = 500 * 1024 * 1024; // 500MB

/// Warmup function to load model and identify Ollama process.
/// This eliminates first-call latency and establishes process monitoring.
/// Uses a realistic prompt from the test suite for proper GPU/cache warming.
///
/// ## Process Identification Strategy
///
/// After warmup completes, the inference process is identified by:
/// 1. **Memory (primary)**: The process holding the loaded model has dramatically higher
///    memory usage (typically GBs) compared to server/CLI processes (~50-100MB)
/// 2. **Memory threshold**: Must exceed 500MB to be considered an inference process
/// 3. **Process name**: Must contain "ollama" (covers ollama, ollama.exe, ollama_llama_server)
///
/// This approach is more reliable than CPU-based detection because:
/// - Memory remains high even when idle (model stays loaded)
/// - CPU varies wildly based on timing of the sample
/// - Memory difference is orders of magnitude (GBs vs MBs)
async fn warmup_and_find_ollama_process(
    model: &str,
    client: &reqwest::Client,
    config: &OllamaConfig,
) -> Result<sysinfo::Pid, String> {
    // Use a realistic prompt from the test suite for proper warming
    // This ensures GPU memory and caches are properly initialized
    let warmup_prompt = SHORT_PROMPTS.first()
        .map_or_else(|| "What is a variable in Python?".to_string(), |s| (*s).to_string());

    let warmup_messages = vec![OllamaMessage {
        role: "user".to_string(),
        content: warmup_prompt,
    }];

    let request = OllamaRequest {
        model: model.to_string(),
        messages: warmup_messages,
        stream: false,
    };

    let url = format!("{}/api/chat", config.base_url());

    // Execute warmup request - this loads the model into memory
    let response = client
        .post(&url)
        .json(&request)
        .timeout(REQUEST_TIMEOUT_SECS)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!("Warmup request timed out after {REQUEST_TIMEOUT_SECS:?}. The model may be too large for this system or Ollama is unresponsive.")
            } else {
                format!("Warmup request failed: {e}")
            }
        })?;

    // Check for HTTP errors
    if !response.status().is_success() {
        return Err(format!(
            "Warmup request failed with status {}: model '{}' may not be available",
            response.status(),
            model
        ));
    }

    // Wait for response body to ensure model is fully loaded
    let _ = response.bytes().await.map_err(|e| format!("Failed to read warmup response: {e}"))?;

    // Allow brief stabilization after warmup
    tokio::time::sleep(WARMUP_STABILIZATION_DELAY_MS).await;

    // Now identify the inference process by memory usage
    // The model is loaded, so the inference process will have dramatically higher memory
    let ollama_pid = find_ollama_inference_process()?;

    Ok(ollama_pid)
}

/// Find the Ollama inference process by memory usage.
///
/// This function identifies the correct process to monitor by:
/// 1. Finding all processes with "ollama" in the name
/// 2. Filtering to those exceeding the memory threshold (500MB)
/// 3. Selecting the one with highest memory (the loaded model)
///
/// Returns an error if no suitable process is found.
fn find_ollama_inference_process() -> Result<sysinfo::Pid, String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    // Collect all Ollama-related processes with their memory usage
    let mut candidates: Vec<(sysinfo::Pid, u64, String)> = sys
        .processes()
        .iter()
        .filter_map(|(pid, proc)| {
            let name = proc.name().to_string_lossy();
            let name_lower = name.to_ascii_lowercase();

            // Match ollama processes (covers ollama, ollama.exe, ollama_llama_server, etc.)
            if name_lower.contains("ollama") {
                Some((*pid, proc.memory(), name.to_string()))
            } else {
                None
            }
        })
        .collect();

    if candidates.is_empty() {
        return Err(
            "Could not find any Ollama process. Benchmark requires process-specific monitoring. \
             Ensure Ollama is running (try 'ollama serve' in terminal)."
                .to_string(),
        );
    }

    // Log all found processes for debugging
    log::debug!("Found {} Ollama process(es):", candidates.len());
    for (pid, mem, name) in &candidates {
        log::debug!("  PID {}: {} ({:.1} MB)", pid, name, *mem as f64 / BYTES_PER_MB);
    }

    // Sort by memory descending
    candidates.sort_by(|a, b| b.1.cmp(&a.1));

    // The inference process should have significantly more memory than the threshold
    let (pid, mem, name) = candidates.first().unwrap();

    if *mem < INFERENCE_PROCESS_MIN_MEMORY_BYTES {
        // No process exceeds threshold - model may not be loaded
        let threshold_mb = INFERENCE_PROCESS_MIN_MEMORY_BYTES as f64 / BYTES_PER_MB;
        let found_mb = *mem as f64 / BYTES_PER_MB;

        return Err(format!(
            "No Ollama inference process found with loaded model. \
             Highest memory process '{name}' (PID {pid}) has only {found_mb:.1} MB, \
             but inference process should have >{threshold_mb:.0} MB. \
             The model may have failed to load or Ollama may have unloaded it."   
        ));
    }

    log::info!(
        "Selected Ollama inference process: '{}' (PID {}, {:.1} MB)",
        name,
        pid,
        *mem as f64 / BYTES_PER_MB
    );

    // Validate selection if there are multiple processes
    if candidates.len() > 1 {
        let (_, second_mem, second_name) = &candidates[1];
        let memory_ratio = *mem as f64 / (*second_mem).max(1) as f64;

        if memory_ratio < 2.0 && *second_mem >= INFERENCE_PROCESS_MIN_MEMORY_BYTES {
            // Two processes with similar high memory - unusual, log warning
            log::warn!(
                "Multiple Ollama processes with high memory detected. \
                 Selected '{}' ({:.1} MB) over '{}' ({:.1} MB). \
                 If benchmarks show unexpected results, ensure only one model is loaded.",
                name,
                *mem as f64 / BYTES_PER_MB,
                second_name,
                *second_mem as f64 / BYTES_PER_MB
            );
        }
    }

    Ok(*pid)
}
// =============================================================================
// RESOURCE SAMPLING
// =============================================================================

/// Results from resource sampling during inference.
#[derive(Debug)]
struct SamplingResults {
    /// CPU samples from the Ollama/inference process
    cpu_ollama_samples: Vec<f64>,
    /// CPU samples from the Tauri (this) process
    cpu_tauri_samples: Vec<f64>,
    /// System-wide CPU samples (all cores averaged)
    cpu_system_samples: Vec<f64>,
    /// Memory samples from the Ollama process
    memory_samples: Vec<f64>,
    /// Peak memory observed during sampling
    peak_memory_mb: f64,
}

/// Internal data for resource sampling, protected by a single mutex.
///
/// We use `std::sync::Mutex` rather than `tokio::sync::Mutex` because:
/// 1. Lock operations are trivial (nanoseconds) - just pushing to a Vec or updating a f64
/// 2. Locks are NOT held across `.await` points
/// 3. `std::sync::Mutex` has lower overhead than async mutexes
///
/// Per Tokio docs: "Contrary to popular belief, it is ok and often preferred to use
/// the ordinary Mutex from the standard library in asynchronous code."
/// See: https://tokio.rs/tokio/tutorial/shared-state
#[derive(Debug)]
struct SamplingData {
    cpu_ollama_samples: Vec<f64>,
    cpu_tauri_samples: Vec<f64>,
    cpu_system_samples: Vec<f64>,
    memory_samples: Vec<f64>,
    peak_memory: f64,
    sampling_active: bool,
}

/// Shared state for the resource sampling task.
///
/// Wraps all sampling data in a single `Arc<Mutex<>>` for thread-safe access
/// between the sampling task and the main benchmark runner.
#[derive(Clone)]
struct SamplingState {
    inner: Arc<Mutex<SamplingData>>,
}

impl SamplingState {
    /// Create a new sampling state with pre-allocated vectors.
    ///
    /// Pre-allocates capacity for ~100 samples (5 seconds at 50ms intervals),
    /// which covers most inference runs without reallocation.
    fn new(initial_memory: f64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SamplingData {
                cpu_ollama_samples: Vec::with_capacity(100),
                cpu_tauri_samples: Vec::with_capacity(100),
                cpu_system_samples: Vec::with_capacity(100),
                memory_samples: Vec::with_capacity(100),
                peak_memory: initial_memory,
                sampling_active: true,
            })),
        }
    }

    /// Record a single sample of CPU and memory metrics.
    ///
    /// This acquires the lock once and updates all fields atomically.
    fn record_sample(&self, ollama_cpu: f64, tauri_cpu: f64, system_cpu: f64, memory: f64) {
        let mut data = self.inner.lock().expect("SamplingState mutex poisoned");
        data.cpu_ollama_samples.push(ollama_cpu);
        data.cpu_tauri_samples.push(tauri_cpu);
        data.cpu_system_samples.push(system_cpu);
        data.memory_samples.push(memory);
        if memory > data.peak_memory {
            data.peak_memory = memory;
        }
    }

    /// Check if sampling should continue.
    fn is_active(&self) -> bool {
        self.inner.lock().expect("SamplingState mutex poisoned").sampling_active
    }

    /// Signal the sampler to stop.
    fn stop(&self) {
        self.inner.lock().expect("SamplingState mutex poisoned").sampling_active = false;
    }

    /// Extract the final sampling results, consuming the collected data.
    ///
    /// Returns `None` if no samples were collected.
    fn into_results(self) -> Option<SamplingResults> {
        let mut data = self.inner.lock().expect("SamplingState mutex poisoned");

        if data.cpu_ollama_samples.is_empty() || data.memory_samples.is_empty() {
            return None;
        }


        Some(SamplingResults {
            cpu_ollama_samples: std::mem::take(&mut data.cpu_ollama_samples),
            cpu_tauri_samples: std::mem::take(&mut data.cpu_tauri_samples),
            cpu_system_samples: std::mem::take(&mut data.cpu_system_samples),
            memory_samples: std::mem::take(&mut data.memory_samples),
            peak_memory_mb: data.peak_memory,
        })
    }
}

/// Spawn a background task that samples CPU and memory at regular intervals.
/// Returns a oneshot receiver that signals when sampling is complete.
///
/// ## CPU Measurements
///
/// This function collects three distinct CPU metrics:
/// 1. **Ollama CPU**: The inference process (ollama_llama_server or similar)
/// 2. **Tauri CPU**: This process (HTTP client, JSON parsing, async runtime overhead)
/// 3. **System CPU**: Overall system load (all processes, all cores averaged)
///
/// These separate measurements enable accurate comparison when migrating from
/// Ollama (HTTP-based, multi-process) to llama.cpp (in-process, single executable).
///
/// ## Threading Model
///
/// Uses `std::sync::Mutex` (not `tokio::sync::Mutex`) because:
/// - Lock operations are trivial (nanoseconds)
/// - Locks are released before any `.await` points
/// - Lower overhead than async mutex for this use case
fn spawn_resource_sampler(
    ollama_pid: sysinfo::Pid,
    state: SamplingState,
) -> tokio::sync::oneshot::Receiver<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    // Get our own process ID for Tauri CPU measurement
    let tauri_pid = sysinfo::Pid::from_u32(std::process::id());

    tokio::spawn(async move {
        let mut sys = System::new_all();
        sys.refresh_all();

        // Establish CPU baseline - sysinfo requires two refresh cycles with delay
        sys.refresh_cpu_all();
        tokio::time::sleep(CPU_BASELINE_DELAY_MS).await;
        sys.refresh_cpu_all();

        // Main sampling loop - check active flag, then sample, then sleep
        while state.is_active() {
            sys.refresh_all();
            sys.refresh_cpu_all();

            // Sample Ollama process CPU and memory
            let ollama_data = sys.process(ollama_pid).map(|process| {
                let memory = (process.memory() as f64) / BYTES_PER_MB;
                let cpu = f64::from(process.cpu_usage());
                (cpu, memory)
            });

            // Sample Tauri (this) process CPU
            let tauri_cpu = sys
                .process(tauri_pid)
                .map(|process| f64::from(process.cpu_usage()))
                .unwrap_or(0.0);

            // Sample system-wide CPU (average across all cores)
            let system_cpu = {
                let cpus = sys.cpus();
                if cpus.is_empty() {
                    0.0
                } else {
                    let total: f32 = cpus.iter().map(|cpu| cpu.cpu_usage()).sum();
                    f64::from(total / cpus.len() as f32)
                }
            };

            if let Some((ollama_cpu, memory)) = ollama_data {
                // Record all metrics with a single lock acquisition
                state.record_sample(ollama_cpu, tauri_cpu, system_cpu, memory);
            } else {
                // Ollama process disappeared - stop sampling
                log::warn!("Ollama process (PID {ollama_pid}) disappeared during sampling");
                break;
            }

            tokio::time::sleep(RESOURCE_SAMPLING_INTERVAL_MS).await;
        }

        let _ = tx.send(());
    });

    rx
}

/// Collect and validate sampling results after inference completes.
///
/// This function:
/// 1. Signals the sampler to stop
/// 2. Waits for the sampling task to complete
/// 3. Extracts and validates the collected results
async fn collect_sampling_results(
    state: SamplingState,
    sampling_done: tokio::sync::oneshot::Receiver<()>,
    ollama_pid: sysinfo::Pid,
) -> Result<SamplingResults, String> {
    // Signal sampling to stop
    state.stop();

    // Wait for the sampling task to complete
    let _ = sampling_done.await;

    // Extract results - this consumes the state
    state.into_results().ok_or_else(|| {
        format!(
            "Resource sampling failed: no samples collected for Ollama process (PID {ollama_pid}). \
             The process may have crashed or become unresponsive."
        )
    })
}

// =============================================================================
// TIMING METRICS CALCULATION
// =============================================================================

/// Timing metrics extracted from Ollama response or calculated from client measurements.
struct TimingMetrics {
    first_token_latency_ms: f64,
    total_response_time_ms: f64,
    tokens_per_second: f64,
    avg_token_latency_ms: f64,
    response_tokens: usize,
    timing_source: TimingSource,
}

/// Calculate timing metrics from Ollama's native timing data.
/// Falls back to client-side measurements if native data is unavailable.
fn calculate_timing_metrics(
    response: &OllamaResponse,
    client_elapsed_ms: f64,
) -> TimingMetrics {
    // Try to use native Ollama timing data (preferred)
    if let (Some(eval_count), Some(eval_duration_ns), Some(total_duration_ns)) = (
        response.eval_count,
        response.eval_duration,
        response.total_duration,
    ) {
        let eval_duration_ms = (eval_duration_ns as f64) / 1_000_000.0;
        let prompt_eval_duration_ms = response.prompt_eval_duration
            .map_or_else(|| 0.0, |ns| (ns as f64) / 1_000_000.0);
        let total_duration_ms = (total_duration_ns as f64) / 1_000_000.0;

        let tokens_per_second = if eval_duration_ms > 0.0 {
            (eval_count as f64) / (eval_duration_ms / 1000.0)
        } else {
            0.0
        };

        let avg_token_latency_ms = if eval_count > 0 {
            eval_duration_ms / eval_count as f64
        } else {
            0.0
        };

        return TimingMetrics {
            first_token_latency_ms: prompt_eval_duration_ms,
            total_response_time_ms: total_duration_ms,
            tokens_per_second,
            avg_token_latency_ms,
            response_tokens: eval_count,
            timing_source: TimingSource::Native,
        };
    }

    // Fallback to client-side measurements
    log::warn!(
        "Ollama did not provide native timing data. Using client-side measurements (less accurate)."
    );

    // Estimate token count from response content
    let response_content = response.message
        .as_ref()
        .map_or("", |m| m.content.as_str());

    // Rough estimation: ~4 characters per token for English text
    let estimated_tokens = (response_content.len() / 4).max(1);

    let tokens_per_second = if client_elapsed_ms > 0.0 {
        (estimated_tokens as f64) / (client_elapsed_ms / 1000.0)
    } else {
        0.0
    };

    let avg_token_latency_ms = if estimated_tokens > 0 {
        client_elapsed_ms / estimated_tokens as f64
    } else {
        0.0
    };

    TimingMetrics {
        first_token_latency_ms: 0.0, // Cannot determine without native data
        total_response_time_ms: client_elapsed_ms,
        tokens_per_second,
        avg_token_latency_ms,
        response_tokens: estimated_tokens,
        timing_source: TimingSource::Client,
    }
}

// =============================================================================
// REQUEST BUILDING
// =============================================================================

/// System prompt for the coding assistant.
const SYSTEM_PROMPT: &str = r"You are a helpful coding assistant designed for secondary school students (ages 11-18).
Your goal is to explain programming concepts clearly and provide well-commented code examples.

Guidelines:
- Use simple, encouraging language
- Break down complex concepts into steps
- Always include helpful comments in code
- Be patient and supportive
- Adapt explanations to the student's level
- Encourage learning and experimentation";

/// Build the messages array for an Ollama request.
fn build_request_messages(prompt: &str, context: Option<Vec<OllamaMessage>>) -> Vec<OllamaMessage> {
    let mut messages = vec![OllamaMessage {
        role: "system".to_string(),
        content: SYSTEM_PROMPT.to_string(),
    }];

    if let Some(ctx) = context {
        messages.extend(ctx);
    }

    messages.push(OllamaMessage {
        role: "user".to_string(),
        content: prompt.to_string(),
    });

    messages
}

// =============================================================================
// STATISTICAL HELPERS
// =============================================================================

/// Calculate the median - note that full sorting is slower but acceptable for benchmark sizes. 
fn calculate_median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec(); // clones input (must clone if you can't mutate caller)
    // Use total_cmp for a deterministic, NaN-handling comparison
    sorted.sort_by(f64::total_cmp);

    let len = sorted.len();
    if len % 2 == 0 {
        (sorted[len / 2 - 1] + sorted[len / 2]) / 2.0
    } else {
        sorted[len / 2]
    }
}

/// Calculate the average of a slice of f64 values.
fn calculate_average(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

// =============================================================================
// SINGLE TEST EXECUTION
// =============================================================================

/// Run a single benchmark test and collect metrics.
/// Returns (`BenchmarkMetrics`, `response_content`) for follow-up context.
async fn run_single_test(
    prompt: String,
    category: PromptCategory,
    model: String,
    iteration: usize,
    context: Option<Vec<OllamaMessage>>,
    client: &reqwest::Client,
    config: &OllamaConfig,
    ollama_pid: sysinfo::Pid,
    hardware: &HardwareSnapshot,
) -> Result<(BenchmarkMetrics, String), String> {
    // Capture initial memory state
    let mut sys = System::new_all();
    sys.refresh_all();

    let memory_before_mb = sys
        .process(ollama_pid)
        .map(|p| (p.memory() as f64) / BYTES_PER_MB)
        .ok_or_else(|| format!(
            "Ollama process (PID {ollama_pid}) not found before test. Ensure Ollama is still running.",
        ))?;

    // Build request
    let messages = build_request_messages(&prompt, context);
    let request = OllamaRequest {
        model: model.clone(),
        messages,
        stream: false, // Non-streaming for accurate native timing data
    };

    // Start resource sampling
    // Clone the state for the spawned sampler task - both share the same Arc<Mutex<>>
    let sampling_state = SamplingState::new(memory_before_mb);
    let sampling_done = spawn_resource_sampler(ollama_pid, sampling_state.clone());

    // Make request with timeout
    let url = format!("{}/api/chat", config.base_url());
    let request_start = Instant::now();

    let response = client
        .post(&url)
        .json(&request)
        .timeout(REQUEST_TIMEOUT_SECS)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!(
                    "Request timed out after {:?} for prompt '{}...'. \
                     Consider using a smaller model or shorter prompts.",
                    REQUEST_TIMEOUT_SECS,
                    prompt.chars().take(50).collect::<String>()
                )
            } else if e.is_connect() {
                "Failed to connect to Ollama. Ensure Ollama is running (try 'ollama serve').".to_string()
            } else {
                format!("HTTP request failed: {e}")
            }
        })?;

    // Check HTTP status
    if !response.status().is_success() {
        return Err(format!(
            "Ollama returned error status {}: {}",
            response.status(),
            response.status().canonical_reason().unwrap_or("Unknown error")
        ));
    }

    // Parse response
    let ollama_response: OllamaResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response JSON: {e}"))?;

    let client_elapsed_ms = request_start.elapsed().as_secs_f64() * 1000.0;

    // Collect sampling results (consumes the state)
    let sampling_results = collect_sampling_results(sampling_state, sampling_done, ollama_pid).await?;

    // Calculate timing metrics
    let timing = calculate_timing_metrics(&ollama_response, client_elapsed_ms);

    // Extract response content
    let response_content = ollama_response.message
        .as_ref()
        .map(|m| m.content.clone())
        .unwrap_or_default();

    // Get final memory state
    sys.refresh_all();
    let memory_after_mb = sys
        .process(ollama_pid)
        .map(|p| (p.memory() as f64) / BYTES_PER_MB)
        .ok_or_else(|| format!(
            "Ollama process (PID {ollama_pid}) disappeared after test completed.",    
        ))?;

    // Calculate resource metrics
    let avg_cpu_ollama = calculate_average(&sampling_results.cpu_ollama_samples);
    let avg_cpu_tauri = calculate_average(&sampling_results.cpu_tauri_samples);
    let avg_cpu_system = calculate_average(&sampling_results.cpu_system_samples);
    let avg_cpu_total = avg_cpu_ollama + avg_cpu_tauri; // Combined process CPU
    let median_memory_during = calculate_median(&sampling_results.memory_samples);

    #[allow(deprecated)] // We need to set the legacy cpu_usage_percent field
    Ok((
        BenchmarkMetrics {
            first_token_latency_ms: timing.first_token_latency_ms,
            total_response_time_ms: timing.total_response_time_ms,
            tokens_per_second: timing.tokens_per_second,
            avg_token_latency_ms: timing.avg_token_latency_ms,
            timing_source: timing.timing_source,
            memory_before_mb,
            memory_during_mb: median_memory_during,
            memory_after_mb,
            peak_memory_mb: sampling_results.peak_memory_mb,
            // New CPU metrics for accurate Ollama vs llama.cpp comparison
            cpu_ollama_percent: avg_cpu_ollama,
            cpu_tauri_percent: avg_cpu_tauri,
            cpu_system_percent: avg_cpu_system,
            cpu_total_percent: avg_cpu_total,
            cpu_usage_percent: avg_cpu_ollama, // Legacy: same as ollama for backwards compat
            model_name: model,
            prompt_type: category.as_str().to_string(),
            prompt,
            response_tokens: timing.response_tokens,
            timestamp: get_timestamp(),
            iteration,
            cpu_model: hardware.cpu_model.clone(),
            gpu_name: hardware.gpu_name.clone(),
            avx2_supported: hardware.avx2_supported,
            npu_detected: hardware.npu_detected,
            hardware_detection_failed: hardware.detection_failed,
        },
        response_content,
    ))
}

// =============================================================================
// BENCHMARK SUITE EXECUTION
// =============================================================================

/// Build follow-up context from a previous response.
fn build_followup_context(previous_response: &str) -> Vec<OllamaMessage> {
    let base_prompt = SHORT_PROMPTS.first()
        .map_or_else(|| "What is a variable in Python?".to_string(), |s| s.to_string());

    vec![
        OllamaMessage {
            role: "user".to_string(),
            content: base_prompt,
        },
        OllamaMessage {
            role: "assistant".to_string(),
            content: previous_response.to_string(),
        },
    ]
}

/// Run the complete benchmark suite.
pub async fn run_benchmark_suite(
    model: String,
    iterations: usize,
    client: &reqwest::Client,
    config: &OllamaConfig,
    progress_callback: impl Fn(BenchmarkProgress),
) -> Result<BenchmarkResults, String> {
    // Detect hardware information for benchmark metadata
    let hardware = HardwareSnapshot::detect().await;

    if hardware.detection_failed {
        log::warn!(
            "Hardware detection failed. Benchmark will continue with default metadata values. \
             Results may be less useful for cross-system comparisons."
        );
    }

    // Warmup: Load model and identify Ollama process
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
                last_response.as_ref().map(|r| build_followup_context(r))
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
                &hardware,
            )
            .await?;

            // Store actual response content for follow-up context
            if test.category == PromptCategory::Short && test.id == "short_1" {
                last_response = Some(response_content);
            }

            all_metrics.push(metrics);

            // Delay between tests to let system stabilize
            tokio::time::sleep(TEST_STABILIZATION_DELAY_MS).await;
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
