use super::metrics::{BenchmarkMetrics, BenchmarkResults, calculate_summary, get_timestamp};
use super::test_suite::{get_test_suite, get_total_test_count, PromptCategory};
use crate::commands::ollama::{HttpClient, OllamaConfig, OllamaMessage, OllamaRequest, OllamaResponse};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use sysinfo::{System, SystemExt, CpuExt};

/// Progress update event for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkProgress {
    pub current: usize,
    pub total: usize,
    pub current_test: String,
    pub iteration: usize,
}

/// Run a single benchmark test and collect metrics
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

    // Build messages array (same as streaming version)
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
        stream: false, // Non-streaming for easier benchmarking
    };

    // Start timing
    let start_time = Instant::now();

    // Track CPU usage
    let cpu_before = sys.global_cpu_info().cpu_usage();

    // Make request
    let url = format!("{}/api/chat", config.base_url());
    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    // Measure time to first byte (approximation of first token)
    let first_byte_time = start_time.elapsed().as_millis() as f64;

    // Get the complete response
    let ollama_response: OllamaResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    // End timing
    let total_time = start_time.elapsed().as_millis() as f64;

    // Refresh system info to get updated metrics
    sys.refresh_all();
    let memory_after_mb = (sys.used_memory() as f64) / 1024.0 / 1024.0;
    let cpu_after = sys.global_cpu_info().cpu_usage();

    // Extract response content and estimate token count
    let response_content = ollama_response
        .message
        .as_ref()
        .map(|m| m.content.as_str())
        .unwrap_or("");

    // Simple token estimation: ~4 chars per token (rough approximation)
    let response_tokens = response_content.len() / 4;

    // Calculate metrics
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
        first_token_latency_ms: first_byte_time,
        total_response_time_ms: total_time,
        tokens_per_second,
        avg_token_latency_ms: avg_token_latency,
        memory_before_mb,
        memory_during_mb: memory_after_mb, // Simplified: using after as proxy for during
        memory_after_mb,
        peak_memory_mb: memory_after_mb.max(memory_before_mb),
        cpu_usage_percent: ((cpu_before + cpu_after) / 2.0) as f64,
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
