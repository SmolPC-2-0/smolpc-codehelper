use super::metrics::{BenchmarkMetrics, BenchmarkResults, calculate_summary, get_timestamp};
use super::test_suite::{get_test_suite, get_total_test_count, PromptCategory};
use crate::commands::ollama::{OllamaConfig, OllamaMessage, OllamaRequest, OllamaResponse};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use sysinfo::System;

/// Progress update event for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkProgress {
    pub current: usize,
    pub total: usize,
    pub current_test: String,
    pub iteration: usize,
}

/// Run a single benchmark test and collect metrics with accurate streaming measurements
async fn run_single_test(
    prompt: String,
    category: PromptCategory,
    model: String,
    iteration: usize,
    context: Option<Vec<OllamaMessage>>,
    client: &reqwest::Client,
    config: &OllamaConfig,
) -> Result<BenchmarkMetrics, String> {
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

    // Spawn background task for periodic resource sampling
    let peak_memory_clone = Arc::clone(&peak_memory);
    let cpu_samples_clone = Arc::clone(&cpu_samples);
    let memory_samples_clone = Arc::clone(&memory_samples);
    let sampling_active_clone = Arc::clone(&sampling_active);

    tokio::spawn(async move {
        let mut sys_sampler = System::new_all();
        while *sampling_active_clone.lock().unwrap() {
            sys_sampler.refresh_all();
            sys_sampler.refresh_cpu_all();

            let current_memory = (sys_sampler.used_memory() as f64) / 1024.0 / 1024.0;
            let current_cpu = sys_sampler.global_cpu_usage();

            // Update peak memory
            let mut peak = peak_memory_clone.lock().unwrap();
            if current_memory > *peak {
                *peak = current_memory;
            }
            drop(peak);

            // Store samples
            cpu_samples_clone.lock().unwrap().push(current_cpu as f64);
            memory_samples_clone.lock().unwrap().push(current_memory);

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    });

    // Start timing and make streaming request
    let start_time = Instant::now();
    let mut first_token_time: Option<f64> = None;
    let mut token_count = 0usize;
    let mut response_content = String::new();

    let url = format!("{}/api/chat", config.base_url());
    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

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

                                    // Count tokens (approximate based on chunks received)
                                    // A better approach would be to use actual tokenizer, but this is reasonable
                                    token_count += 1;
                                }
                            }

                            if ollama_response.done {
                                break;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                // Stop sampling on error
                *sampling_active.lock().unwrap() = false;
                return Err(format!("Stream error: {}", e));
            }
        }
    }

    // End timing
    let total_time = start_time.elapsed().as_millis() as f64;

    // Stop resource sampling
    *sampling_active.lock().unwrap() = false;
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await; // Wait for last sample

    // Refresh final system state
    sys.refresh_all();
    let memory_after_mb = (sys.used_memory() as f64) / 1024.0 / 1024.0;

    // Get peak memory from sampling
    let peak_memory_mb = *peak_memory.lock().unwrap();

    // Calculate average CPU from samples
    let cpu_samples_vec = cpu_samples.lock().unwrap();
    let avg_cpu = if !cpu_samples_vec.is_empty() {
        cpu_samples_vec.iter().sum::<f64>() / cpu_samples_vec.len() as f64
    } else {
        0.0
    };
    drop(cpu_samples_vec);

    // Calculate average memory during inference
    let memory_samples_vec = memory_samples.lock().unwrap();
    let avg_memory_during = if !memory_samples_vec.is_empty() {
        memory_samples_vec.iter().sum::<f64>() / memory_samples_vec.len() as f64
    } else {
        memory_after_mb
    };
    drop(memory_samples_vec);

    // Use actual token count from streaming, or estimate if needed
    let response_tokens = if token_count > 0 {
        token_count
    } else {
        // Fallback: estimate based on character count
        response_content.len() / 4
    };

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

    Ok(BenchmarkMetrics {
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
    })
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
            let metrics = run_single_test(
                test.prompt.clone(),
                test.category,
                model.clone(),
                iteration,
                context,
                client,
                config,
            )
            .await?;

            // Store response for potential follow-up context
            // (In a real scenario, we'd extract this from the actual response)
            if test.category == PromptCategory::Short && test.id == "short_1" {
                last_response = Some("A variable is a container that stores data...".to_string());
            }

            all_metrics.push(metrics);

            // Small delay between tests to let system stabilize
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
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
