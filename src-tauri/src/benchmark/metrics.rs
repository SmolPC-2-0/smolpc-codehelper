use serde::{Deserialize, Serialize};

/// Performance metrics for a single benchmark test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetrics {
    // Timing metrics (PRIMARY)
    /// Time from request start to first token received (ms)
    pub first_token_latency_ms: f64,

    /// Total time to generate complete response (ms)
    pub total_response_time_ms: f64,

    /// Throughput: tokens generated per second
    pub tokens_per_second: f64,

    /// Average time per token (ms)
    pub avg_token_latency_ms: f64,

    // Memory metrics (SECONDARY)
    /// RAM usage before inference started (MB)
    pub memory_before_mb: f64,

    /// RAM usage during inference (MB)
    pub memory_during_mb: f64,

    /// RAM usage after inference completed (MB)
    pub memory_after_mb: f64,

    /// Peak RAM usage during test (MB)
    pub peak_memory_mb: f64,

    // CPU metrics (SECONDARY)
    /// Average CPU utilization during inference (%)
    pub cpu_usage_percent: f64,

    // Metadata
    /// Model name used for inference
    pub model_name: String,

    /// Test prompt category (short, medium, long, follow-up)
    pub prompt_type: String,

    /// The actual prompt sent
    pub prompt: String,

    /// Number of tokens in the response
    pub response_tokens: usize,

    /// ISO 8601 timestamp of test execution
    pub timestamp: String,

    /// Test iteration number
    pub iteration: usize,
}

/// Summary statistics across multiple benchmark runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub category: String,
    pub avg_first_token_ms: f64,
    pub avg_tokens_per_sec: f64,
    pub avg_total_time_ms: f64,
    pub avg_memory_mb: f64,
    pub avg_cpu_percent: f64,
    pub test_count: usize,
}

/// Complete benchmark results including all metrics and summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    pub metrics: Vec<BenchmarkMetrics>,
    pub summary: Vec<BenchmarkSummary>,
    pub total_duration_seconds: f64,
    pub timestamp: String,
}

/// Helper to get current timestamp in ISO 8601 format
pub fn get_timestamp() -> String {
    chrono::Local::now().to_rfc3339()
}

/// Calculate summary statistics from a collection of metrics
pub fn calculate_summary(metrics: &[BenchmarkMetrics]) -> Vec<BenchmarkSummary> {
    let categories = ["short", "medium", "long", "follow-up"];
    let mut summaries = Vec::new();

    for category in categories {
        let category_metrics: Vec<_> = metrics
            .iter()
            .filter(|m| m.prompt_type == category)
            .collect();

        if category_metrics.is_empty() {
            continue;
        }

        let count = category_metrics.len();
        let avg_first_token = category_metrics.iter().map(|m| m.first_token_latency_ms).sum::<f64>() / count as f64;
        let avg_tokens_per_sec = category_metrics.iter().map(|m| m.tokens_per_second).sum::<f64>() / count as f64;
        let avg_total_time = category_metrics.iter().map(|m| m.total_response_time_ms).sum::<f64>() / count as f64;
        let avg_memory = category_metrics.iter().map(|m| m.peak_memory_mb).sum::<f64>() / count as f64;
        let avg_cpu = category_metrics.iter().map(|m| m.cpu_usage_percent).sum::<f64>() / count as f64;

        summaries.push(BenchmarkSummary {
            category: category.to_string(),
            avg_first_token_ms: avg_first_token,
            avg_tokens_per_sec: avg_tokens_per_sec,
            avg_total_time_ms: avg_total_time,
            avg_memory_mb: avg_memory,
            avg_cpu_percent: avg_cpu,
            test_count: count,
        });
    }

    summaries
}
