//! Benchmark suite execution.
//!
//! Runs inference tests against Ollama and collects timing/resource metrics.
//! Uses Ollama's native nanosecond-precision timing data when available.

use super::metrics::{BenchmarkMetrics, BenchmarkResults, TimingSource, calculate_summary, get_timestamp};
use super::process::{HardwareSnapshot, warmup_and_find_process};
use super::sampling::{SamplingState, collect_sampling_results, spawn_resource_sampler, calculate_average, calculate_median};
use super::test_suite::{get_test_suite, get_total_test_count, PromptCategory, SHORT_PROMPTS};
use crate::commands::ollama::{OllamaConfig, OllamaMessage, OllamaRequest, OllamaResponse};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use sysinfo::System;

// =============================================================================
// Configuration
// =============================================================================

const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
const TEST_STABILIZATION_DELAY: Duration = Duration::from_millis(500);
const BYTES_PER_MB: f64 = 1024.0 * 1024.0;

/// ID of the test used for follow-up context.
const CONTEXT_SOURCE_TEST_ID: &str = "short_1";

/// System prompt for the coding assistant benchmark.
const SYSTEM_PROMPT: &str = r"You are a helpful coding assistant designed for secondary school students (ages 11-18).
Your goal is to explain programming concepts clearly and provide well-commented code examples.

Guidelines:
- Use simple, encouraging language
- Break down complex concepts into steps
- Always include helpful comments in code
- Be patient and supportive
- Adapt explanations to the student's level
- Encourage learning and experimentation";

// =============================================================================
// Public Types
// =============================================================================

/// Progress update event for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkProgress {
    pub current: usize,
    pub total: usize,
    pub current_test: String,
    pub iteration: usize,
}

// =============================================================================
// Timing Calculation
// =============================================================================

struct TimingMetrics {
    first_token_latency_ms: f64,
    total_response_time_ms: f64,
    tokens_per_second: f64,
    avg_token_latency_ms: f64,
    response_tokens: usize,
    timing_source: TimingSource,
}

/// Extract timing metrics from Ollama's native data, falling back to client-side if unavailable.
fn calculate_timing_metrics(response: &OllamaResponse, client_elapsed_ms: f64) -> TimingMetrics {
    // Prefer native Ollama timing (nanosecond precision)
    if let (Some(eval_count), Some(eval_duration_ns), Some(total_duration_ns)) = (
        response.eval_count,
        response.eval_duration,
        response.total_duration,
    ) {
        let eval_duration_ms = (eval_duration_ns as f64) / 1_000_000.0;
        let prompt_eval_ms = response.prompt_eval_duration
            .map_or(0.0, |ns| (ns as f64) / 1_000_000.0);
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
            first_token_latency_ms: prompt_eval_ms,
            total_response_time_ms: total_duration_ms,
            tokens_per_second,
            avg_token_latency_ms,
            response_tokens: eval_count,
            timing_source: TimingSource::Native,
        };
    }

    // Fallback: estimate from client-side measurements
    log::warn!("Ollama did not provide native timing data, using client-side estimates");

    let response_content = response.message.as_ref().map_or("", |m| m.content.as_str());
    let estimated_tokens = (response_content.len() / 4).max(1); // ~4 chars per token

    let tokens_per_second = if client_elapsed_ms > 0.0 {
        (estimated_tokens as f64) / (client_elapsed_ms / 1000.0)
    } else {
        0.0
    };

    TimingMetrics {
        first_token_latency_ms: 0.0,
        total_response_time_ms: client_elapsed_ms,
        tokens_per_second,
        avg_token_latency_ms: if estimated_tokens > 0 { client_elapsed_ms / estimated_tokens as f64 } else { 0.0 },
        response_tokens: estimated_tokens,
        timing_source: TimingSource::Client,
    }
}

// =============================================================================
// Request Building
// =============================================================================

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

fn build_followup_context(previous_response: &str) -> Vec<OllamaMessage> {
    let base_prompt = SHORT_PROMPTS.first().copied().unwrap_or("What is a variable in Python?");

    vec![
        OllamaMessage { role: "user".to_string(), content: base_prompt.to_string() },
        OllamaMessage { role: "assistant".to_string(), content: previous_response.to_string() },
    ]
}

// =============================================================================
// Single Test Execution
// =============================================================================

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
    let mut sys = System::new_all();
    sys.refresh_all();

    let memory_before_mb = sys
        .process(ollama_pid)
        .map(|p| (p.memory() as f64) / BYTES_PER_MB)
        .ok_or_else(|| format!("Ollama process (PID {ollama_pid}) not found"))?;

    let request = OllamaRequest {
        model: model.clone(),
        messages: build_request_messages(&prompt, context),
        stream: false,
    };

    // Start background resource sampling
    let sampling_state = SamplingState::new(memory_before_mb);
    let sampling_done = spawn_resource_sampler(ollama_pid, sampling_state.clone());

    // Execute request
    let url = format!("{}/api/chat", config.base_url());
    let request_start = Instant::now();

    let response = client
        .post(&url)
        .json(&request)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!("Request timed out after {REQUEST_TIMEOUT:?}")
            } else if e.is_connect() {
                "Failed to connect to Ollama - ensure it's running".to_string()
            } else {
                format!("HTTP request failed: {e}")
            }
        })?;

    if !response.status().is_success() {
        return Err(format!(
            "Ollama returned error status {}: {}",
            response.status(),
            response.status().canonical_reason().unwrap_or("Unknown error")
        ));
    }

    let ollama_response: OllamaResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))?;

    let client_elapsed_ms = request_start.elapsed().as_secs_f64() * 1000.0;

    // Collect sampling results
    let sampling_results = collect_sampling_results(sampling_state, sampling_done, ollama_pid).await?;

    let timing = calculate_timing_metrics(&ollama_response, client_elapsed_ms);

    let response_content = ollama_response.message
        .as_ref()
        .map(|m| m.content.clone())
        .unwrap_or_default();

    // Get final memory state
    sys.refresh_all();
    let memory_after_mb = sys
        .process(ollama_pid)
        .map(|p| (p.memory() as f64) / BYTES_PER_MB)
        .ok_or_else(|| format!("Ollama process (PID {ollama_pid}) disappeared"))?;

    // Calculate resource metrics
    let avg_cpu_ollama = calculate_average(&sampling_results.cpu_ollama_samples);
    let avg_cpu_tauri = calculate_average(&sampling_results.cpu_tauri_samples);
    let avg_cpu_system = calculate_average(&sampling_results.cpu_system_samples);
    let median_memory_during = calculate_median(&sampling_results.memory_samples);

    #[allow(deprecated)]
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
            cpu_ollama_percent: avg_cpu_ollama,
            cpu_tauri_percent: avg_cpu_tauri,
            cpu_system_percent: avg_cpu_system,
            cpu_total_percent: avg_cpu_ollama + avg_cpu_tauri,
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
// Benchmark Suite Execution
// =============================================================================

/// Run the complete benchmark suite against a model.
pub async fn run_benchmark_suite(
    model: String,
    iterations: usize,
    client: &reqwest::Client,
    config: &OllamaConfig,
    progress_callback: impl Fn(BenchmarkProgress),
) -> Result<BenchmarkResults, String> {
    let hardware = HardwareSnapshot::detect().await;

    if hardware.detection_failed {
        log::warn!("Hardware detection failed - benchmark metadata may be incomplete");
    }

    let ollama_pid = warmup_and_find_process(&model, client, config).await?;

    let suite_start = Instant::now();
    let test_suite = get_test_suite();
    let total_tests = get_total_test_count(iterations);
    let mut all_metrics = Vec::new();
    let mut current_test = 0;
    let mut last_response: Option<String> = None;

    for iteration in 1..=iterations {
        for test in &test_suite {
            current_test += 1;

            let context = if test.category == PromptCategory::FollowUp {
                last_response.as_ref().map(|r| build_followup_context(r))
            } else {
                None
            };

            progress_callback(BenchmarkProgress {
                current: current_test,
                total: total_tests,
                current_test: format!("{} (iteration {})", test.id, iteration),
                iteration,
            });

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
            ).await?;

            // Store first short test response for follow-up context
            if test.category == PromptCategory::Short && test.id == CONTEXT_SOURCE_TEST_ID {
                last_response = Some(response_content);
            }

            all_metrics.push(metrics);

            tokio::time::sleep(TEST_STABILIZATION_DELAY).await;
        }
    }

    let summary = calculate_summary(&all_metrics);

    Ok(BenchmarkResults {
        metrics: all_metrics,
        summary,
        total_duration_seconds: suite_start.elapsed().as_secs_f64(),
        timestamp: get_timestamp(),
    })
}
