use super::metrics::{BenchmarkMetrics, BenchmarkResults, calculate_summary, get_timestamp};
use super::test_suite::{get_test_suite, get_total_test_count, PromptCategory};
use crate::commands::ollama::{OllamaConfig, OllamaMessage, OllamaRequest, OllamaResponse};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use sysinfo::System;
use tokio::sync::Mutex;

// Benchmark configuration constants
/// Interval (in milliseconds) for sampling CPU and memory during inference
const RESOURCE_SAMPLING_INTERVAL_MS: u64 = 100;

/// Delay (in milliseconds) between tests to allow system stabilization
const TEST_STABILIZATION_DELAY_MS: u64 = 500;

/// Estimated average characters per token for fallback token counting
/// Used only when Ollama metadata is unavailable
const CHARS_PER_TOKEN_ESTIMATE: usize = 4;

/// Progress update event for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkProgress {
    pub current: usize,
    pub total: usize,
    pub current_test: String,
    pub iteration: usize,
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
) -> Result<(BenchmarkMetrics, String), String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    // Capture initial memory state
    let memory_before_mb = (sys.used_memory() as f64) / 1024.0 / 1024.0;

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
        stream: true, // Use streaming for accurate token timing
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

        // Find Ollama process by name for process-specific monitoring
        let ollama_pid = sys_sampler.processes().iter()
            .find(|(_, process)| {
                let name = process.name().to_string_lossy().to_ascii_lowercase();
                name.contains("ollama")
            })
            .map(|(pid, _)| *pid);

        while *sampling_active_clone.lock().await {
            sys_sampler.refresh_all();
            sys_sampler.refresh_cpu_all();

            let (current_memory, current_cpu) = if let Some(pid) = ollama_pid {
                // Process-specific monitoring (preferred)
                if let Some(process) = sys_sampler.process(pid) {
                    let memory = (process.memory() as f64) / 1024.0 / 1024.0;
                    let cpu = process.cpu_usage() as f64;
                    (memory, cpu)
                } else {
                    // Fallback to system-wide if process disappeared
                    let memory = (sys_sampler.used_memory() as f64) / 1024.0 / 1024.0;
                    let cpu = sys_sampler.global_cpu_usage() as f64;
                    (memory, cpu)
                }
            } else {
                // Fallback to system-wide if Ollama process not found
                let memory = (sys_sampler.used_memory() as f64) / 1024.0 / 1024.0;
                let cpu = sys_sampler.global_cpu_usage() as f64;
                (memory, cpu)
            };

            // Update peak memory
            let mut peak = peak_memory_clone.lock().await;
            if current_memory > *peak {
                *peak = current_memory;
            }
            drop(peak);

            // Store samples
            cpu_samples_clone.lock().await.push(current_cpu);
            memory_samples_clone.lock().await.push(current_memory);

            tokio::time::sleep(tokio::time::Duration::from_millis(RESOURCE_SAMPLING_INTERVAL_MS)).await;
        }
        // Signal completion when loop exits
        let _ = sampling_done_tx.send(());
    });

    // Start timing and make streaming request
    let start_time = Instant::now();
    let mut first_token_time: Option<f64> = None;
    let mut actual_token_count: Option<usize> = None;
    let mut response_content = String::new();

    let url = format!("{}/api/chat", config.base_url());
    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to Ollama: {}", e))?;

    let mut stream = response.bytes_stream();

    // Process streaming response
    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(bytes) => {
                if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                    for line in text.lines() {
                        if let Ok(ollama_response) = serde_json::from_str::<OllamaResponse>(line) {
                            if let Some(message) = ollama_response.message {
                                if !message.content.is_empty() {
                                    // Capture first token timing
                                    if first_token_time.is_none() {
                                        first_token_time = Some(start_time.elapsed().as_millis() as f64);
                                    }

                                    // Accumulate content
                                    response_content.push_str(&message.content);
                                }
                            }

                            if ollama_response.done {
                                // Capture actual token count from Ollama metadata
                                actual_token_count = ollama_response.eval_count;
                                break;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                // Stop sampling on error and wait for cleanup
                *sampling_active.lock().await = false;
                let _ = sampling_done_rx.await;
                return Err(format!("Stream error while reading from Ollama: {}", e));
            }
        }
    }

    // End timing
    let total_time = start_time.elapsed().as_millis() as f64;

    // Stop resource sampling and wait for completion
    *sampling_active.lock().await = false;
    let _ = sampling_done_rx.await; // Wait for sampling task to actually finish

    // Refresh final system state
    sys.refresh_all();
    let memory_after_mb = (sys.used_memory() as f64) / 1024.0 / 1024.0;

    // Get peak memory from sampling
    let peak_memory_mb = *peak_memory.lock().await;

    // Calculate average CPU from samples
    let cpu_samples_vec = cpu_samples.lock().await;
    let avg_cpu = if !cpu_samples_vec.is_empty() {
        cpu_samples_vec.iter().sum::<f64>() / cpu_samples_vec.len() as f64
    } else {
        0.0
    };
    drop(cpu_samples_vec);

    // Calculate average memory during inference
    let memory_samples_vec = memory_samples.lock().await;
    let avg_memory_during = if !memory_samples_vec.is_empty() {
        memory_samples_vec.iter().sum::<f64>() / memory_samples_vec.len() as f64
    } else {
        memory_after_mb
    };
    drop(memory_samples_vec);

    // Use actual token count from Ollama metadata (most accurate)
    // Fallback to character-based estimation only if metadata unavailable
    let response_tokens = actual_token_count.unwrap_or_else(|| {
        // Estimate using configured chars-per-token ratio
        (response_content.len() / CHARS_PER_TOKEN_ESTIMATE).max(1)
    });

    // Calculate metrics
    let first_token_latency_ms = first_token_time.unwrap_or(total_time);

    let tokens_per_second = if total_time > 0.0 {
        (response_tokens as f64) / (total_time / 1000.0)
    } else {
        0.0
    };

    let avg_token_latency = if response_tokens > 0 {
        total_time / response_tokens as f64
    } else {
        0.0
    };

    Ok((
        BenchmarkMetrics {
            first_token_latency_ms,
            total_response_time_ms: total_time,
            tokens_per_second,
            avg_token_latency_ms: avg_token_latency,
            memory_before_mb,
            memory_during_mb: avg_memory_during,
            memory_after_mb,
            peak_memory_mb,
            cpu_usage_percent: avg_cpu,
            model_name: model,
            prompt_type: category.as_str().to_string(),
            prompt,
            response_tokens,
            timestamp: get_timestamp(),
            iteration,
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
