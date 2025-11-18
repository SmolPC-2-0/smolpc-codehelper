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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test metric with minimal required fields
    fn create_test_metric(
        category: &str,
        first_token: f64,
        tokens_per_sec: f64,
        total_time: f64,
        peak_memory: f64,
        cpu: f64,
    ) -> BenchmarkMetrics {
        BenchmarkMetrics {
            first_token_latency_ms: first_token,
            total_response_time_ms: total_time,
            tokens_per_second: tokens_per_sec,
            avg_token_latency_ms: total_time / 100.0, // Simplified
            memory_before_mb: 1000.0,
            memory_during_mb: 1100.0,
            memory_after_mb: 1000.0,
            peak_memory_mb: peak_memory,
            cpu_usage_percent: cpu,
            model_name: "test-model".to_string(),
            prompt_type: category.to_string(),
            prompt: "test prompt".to_string(),
            response_tokens: 100,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            iteration: 1,
        }
    }

    #[test]
    fn test_calculate_summary_single_category() {
        let metrics = vec![
            create_test_metric("short", 100.0, 10.0, 1000.0, 500.0, 50.0),
            create_test_metric("short", 200.0, 20.0, 2000.0, 600.0, 60.0),
        ];

        let summary = calculate_summary(&metrics);

        assert_eq!(summary.len(), 1, "Should have exactly one category");
        assert_eq!(summary[0].category, "short");
        assert_eq!(summary[0].avg_first_token_ms, 150.0);
        assert_eq!(summary[0].avg_tokens_per_sec, 15.0);
        assert_eq!(summary[0].avg_total_time_ms, 1500.0);
        assert_eq!(summary[0].avg_memory_mb, 550.0);
        assert_eq!(summary[0].avg_cpu_percent, 55.0);
        assert_eq!(summary[0].test_count, 2);
    }

    #[test]
    fn test_calculate_summary_multiple_categories() {
        let metrics = vec![
            create_test_metric("short", 100.0, 10.0, 1000.0, 500.0, 50.0),
            create_test_metric("short", 200.0, 20.0, 2000.0, 600.0, 60.0),
            create_test_metric("medium", 300.0, 15.0, 3000.0, 700.0, 70.0),
            create_test_metric("long", 400.0, 25.0, 4000.0, 800.0, 80.0),
        ];

        let summary = calculate_summary(&metrics);

        assert_eq!(summary.len(), 3, "Should have three categories");

        // Check short category
        let short = summary.iter().find(|s| s.category == "short").unwrap();
        assert_eq!(short.test_count, 2);
        assert_eq!(short.avg_first_token_ms, 150.0);

        // Check medium category
        let medium = summary.iter().find(|s| s.category == "medium").unwrap();
        assert_eq!(medium.test_count, 1);
        assert_eq!(medium.avg_first_token_ms, 300.0);

        // Check long category
        let long = summary.iter().find(|s| s.category == "long").unwrap();
        assert_eq!(long.test_count, 1);
        assert_eq!(long.avg_first_token_ms, 400.0);
    }

    #[test]
    fn test_calculate_summary_empty_input() {
        let metrics: Vec<BenchmarkMetrics> = vec![];
        let summary = calculate_summary(&metrics);
        assert_eq!(summary.len(), 0, "Empty input should produce empty summary");
    }

    #[test]
    fn test_calculate_summary_skips_missing_categories() {
        let metrics = vec![
            create_test_metric("short", 100.0, 10.0, 1000.0, 500.0, 50.0),
            // No medium or long or follow-up tests
        ];

        let summary = calculate_summary(&metrics);

        assert_eq!(summary.len(), 1, "Should only include categories with data");
        assert_eq!(summary[0].category, "short");
    }

    #[test]
    fn test_calculate_summary_precision() {
        let metrics = vec![
            create_test_metric("short", 100.5, 10.3, 1000.7, 500.2, 50.1),
            create_test_metric("short", 200.5, 20.7, 2000.3, 600.8, 60.9),
        ];

        let summary = calculate_summary(&metrics);

        assert_eq!(summary.len(), 1);
        // Check that floating point averages are calculated correctly
        assert!((summary[0].avg_first_token_ms - 150.5).abs() < 0.01);
        assert!((summary[0].avg_tokens_per_sec - 15.5).abs() < 0.01);
        assert!((summary[0].avg_cpu_percent - 55.5).abs() < 0.01);
    }

    #[test]
    fn test_get_timestamp_format() {
        let timestamp = get_timestamp();

        // Should be a valid RFC3339 timestamp
        assert!(timestamp.contains('T'), "Timestamp should contain 'T' separator");
        assert!(timestamp.len() > 20, "Timestamp should be a reasonable length");

        // Should parse as chrono DateTime
        let parsed = chrono::DateTime::parse_from_rfc3339(&timestamp);
        assert!(parsed.is_ok(), "Timestamp should be valid RFC3339 format");
    }

    #[test]
    fn test_benchmark_metrics_serialization() {
        let metric = create_test_metric("short", 100.0, 10.0, 1000.0, 500.0, 50.0);

        // Test that serialization works
        let serialized = serde_json::to_string(&metric);
        assert!(serialized.is_ok(), "Should serialize to JSON");

        // Test deserialization
        let json = serialized.unwrap();
        let deserialized: Result<BenchmarkMetrics, _> = serde_json::from_str(&json);
        assert!(deserialized.is_ok(), "Should deserialize from JSON");

        let metric2 = deserialized.unwrap();
        assert_eq!(metric.prompt_type, metric2.prompt_type);
        assert_eq!(metric.first_token_latency_ms, metric2.first_token_latency_ms);
    }
}
