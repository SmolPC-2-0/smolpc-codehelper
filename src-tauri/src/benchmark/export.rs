use super::metrics::{BenchmarkMetrics, BenchmarkResults};
use csv::Writer;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;


const FLUSH_INTERVAL: usize = 10;


/// CSV-specific format for benchmark metrics with proper column names and ordering
/// This ensures automatic column ordering via serde and prevents manual column mismatches
#[derive(Debug, Serialize)]
struct CsvMetricRow {
    timestamp: String,
    iteration: usize,
    category: String,
    model: String,
    first_token_ms: String,
    total_time_ms: String,
    tokens_per_sec: String,
    avg_token_ms: String,
    timing_source: String,
    memory_before_mb: String,
    memory_peak_mb: String,
    // Comprehensive CPU metrics for Ollama vs llama.cpp comparison
    cpu_ollama_percent: String,
    cpu_tauri_percent: String,
    cpu_system_percent: String,
    cpu_total_percent: String,
    response_tokens: usize,
    cpu_model: String,
    gpu_name: String,
    avx2_supported: bool,
    npu_detected: bool,
    hardware_detection_failed: bool,
    prompt: String,
}

impl From<&BenchmarkMetrics> for CsvMetricRow {
    fn from(metric: &BenchmarkMetrics) -> Self {
        Self {
            timestamp: metric.timestamp.clone(),
            iteration: metric.iteration,
            category: metric.prompt_type.clone(),
            model: metric.model_name.clone(),
            first_token_ms: format!("{:.2}", metric.first_token_latency_ms),
            total_time_ms: format!("{:.2}", metric.total_response_time_ms),
            tokens_per_sec: format!("{:.2}", metric.tokens_per_second),
            avg_token_ms: format!("{:.2}", metric.avg_token_latency_ms),
            timing_source: metric.timing_source.as_str().to_string(),
            memory_before_mb: format!("{:.2}", metric.memory_before_mb),
            memory_peak_mb: format!("{:.2}", metric.peak_memory_mb),
            // Comprehensive CPU metrics for Ollama vs llama.cpp comparison
            cpu_ollama_percent: format!("{:.2}", metric.cpu_ollama_percent),
            cpu_tauri_percent: format!("{:.2}", metric.cpu_tauri_percent),
            cpu_system_percent: format!("{:.2}", metric.cpu_system_percent),
            cpu_total_percent: format!("{:.2}", metric.cpu_total_percent),
            response_tokens: metric.response_tokens,
            cpu_model: metric.cpu_model.clone(),
            gpu_name: metric.gpu_name.clone(),
            avx2_supported: metric.avx2_supported,
            npu_detected: metric.npu_detected,
            hardware_detection_failed: metric.hardware_detection_failed,
            prompt: metric.prompt.clone(),
        }
    }
}

/// Get the benchmarks directory path (creates if doesn't exist)
pub fn get_benchmarks_dir() -> Result<PathBuf, String> {
    let current_dir = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {e}"))?;

    let benchmarks_dir = current_dir.join("benchmarks");

    // Create directory if it doesn't exist
    if !benchmarks_dir.exists() {
        fs::create_dir_all(&benchmarks_dir)
            .map_err(|e| format!("Failed to create benchmarks directory: {e}"))?;
    }

    Ok(benchmarks_dir)
}

/// Generate filename with timestamp
pub fn generate_filename(prefix: &str) -> String {
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    format!("{prefix}-{timestamp}.csv")
}

/// Export benchmark results to CSV using serde for automatic column management
pub fn export_to_csv(results: &BenchmarkResults, prefix: &str) -> Result<PathBuf, String> {
    let benchmarks_dir = get_benchmarks_dir()?;
    let filename = generate_filename(prefix);
    let filepath = benchmarks_dir.join(&filename);

    // Create CSV writer
    let mut wtr = Writer::from_path(&filepath)
        .map_err(|e| format!("Failed to create CSV file: {e}"))?;

    // Write all metrics using serde serialization (automatic headers and column ordering)
    // Flush periodically for crash safety

    for (index, metric) in results.metrics.iter().enumerate() {
        let csv_row = CsvMetricRow::from(metric);
        wtr.serialize(csv_row)
            .map_err(|e| format!("Failed to serialize metric row: {e}"))?;

        // Periodic flush every 10 rows to prevent data loss on crash
        if (index + 1) % FLUSH_INTERVAL == 0 {
            wtr.flush()
                .map_err(|e| format!("Failed to flush CSV writer during periodic flush: {e}"))?;
        }
    }

    // Final flush to ensure all data is written
    wtr.flush()
        .map_err(|e| format!("Failed to flush CSV writer: {e}"))?;

    Ok(filepath)
}

/// Create a README.md in the benchmarks directory explaining the CSV format
pub fn create_readme() -> Result<(), String> {
    let benchmarks_dir = get_benchmarks_dir()?;
    let readme_path = benchmarks_dir.join("README.md");

    // Only create if doesn't exist
    if !readme_path.exists() {
        let readme_content = r"# Benchmark Results

This directory contains benchmark test results for SmolPC Code Helper.

## File Naming Convention

Files are named: `{prefix}-{timestamp}.csv`
- `baseline`: Initial performance before optimizations
- `phase1`: After Phase 1 optimizations
- `phase2`: After Phase 2 optimizations
- etc.

## CSV Format

### Main Data Columns

**Timing Metrics (Streaming-based)**
- **first_token_ms**: Time to first actual token via streaming (ms) - accurate measurement
- **total_time_ms**: Total response generation time (ms)
- **tokens_per_sec**: Real throughput based on streaming chunks (tokens/second)
- **avg_token_ms**: Average time per token (ms)

**Resource Metrics (Sampled every 100ms during inference)**
- **memory_before_mb**: RAM before inference starts (MB)
- **memory_during_mb**: Average RAM during inference (MB) - sampled periodically
- **memory_peak_mb**: Peak RAM during inference (MB) - true peak from sampling
- **memory_after_mb**: RAM after inference completes (MB)
- **cpu_percent**: Average CPU utilization during inference (%) - sampled periodically

**Test Metadata**
- **timestamp**: ISO 8601 timestamp of test execution
- **iteration**: Test iteration number (1-3 typically)
- **category**: Prompt category (short, medium, long, follow-up)
- **model**: AI model name used
- **response_tokens**: Number of tokens in response (counted from streaming chunks)
- **prompt**: The test prompt text

## Measurement Methodology

### Streaming for Accurate Token Timing
The benchmark uses Ollama's streaming API (`stream: true`) to:
- Capture the exact moment the first token arrives
- Count actual tokens as they stream in
- Measure real throughput based on token arrival times

### Periodic Resource Sampling
A background task samples CPU and memory every 100ms during inference to:
- Track true peak memory usage (not just before/after snapshots)
- Calculate average CPU usage during the inference window
- Capture resource spikes that would be missed by endpoint measurements

### Token Counting
Tokens are counted from streaming chunks received from Ollama. This provides a more accurate count than character-based estimation, though it still approximates true tokenizer output.

## Usage

Import CSV files into Excel, Google Sheets, or data analysis tools for visualization and comparison across optimization phases.

## Limitations

- Token count is based on streaming chunks, not a true tokenizer (may vary slightly from actual token count)
- CPU/memory sampling is system-wide, not process-specific (future improvement: track Ollama process specifically)
- Sampling interval is 100ms (adequate for most measurements, but may miss very brief spikes)
";

        fs::write(&readme_path, readme_content)
            .map_err(|e| format!("Failed to create README: {e}"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::metrics::{BenchmarkMetrics, TimingSource};

    #[allow(deprecated)] // We need to set legacy cpu_usage_percent field
    fn create_test_metric() -> BenchmarkMetrics {
        BenchmarkMetrics {
            first_token_latency_ms: 100.0,
            total_response_time_ms: 1000.0,
            tokens_per_second: 10.0,
            avg_token_latency_ms: 100.0,
            timing_source: TimingSource::Native,
            memory_before_mb: 1000.0,
            memory_during_mb: 1100.0,
            memory_after_mb: 1000.0,
            peak_memory_mb: 1200.0,
            // New CPU metrics
            cpu_ollama_percent: 16.0,
            cpu_tauri_percent: 40.0,
            cpu_system_percent: 45.0,
            cpu_total_percent: 56.0,
            cpu_usage_percent: 16.0, // Legacy field
            model_name: "test-model".to_string(),
            prompt_type: "short".to_string(),
            prompt: "test prompt".to_string(),
            response_tokens: 100,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            iteration: 1,
            cpu_model: "Test CPU".to_string(),
            gpu_name: "Test GPU".to_string(),
            avx2_supported: true,
            npu_detected: false,
            hardware_detection_failed: false,
        }
    }

    #[test]
    fn test_csv_metric_row_conversion() {
        let metric = create_test_metric();
        let csv_row = CsvMetricRow::from(&metric);

        assert_eq!(csv_row.timestamp, "2025-01-01T00:00:00Z");
        assert_eq!(csv_row.iteration, 1);
        assert_eq!(csv_row.category, "short");
        assert_eq!(csv_row.model, "test-model");
        assert_eq!(csv_row.first_token_ms, "100.00");
        assert_eq!(csv_row.total_time_ms, "1000.00");
        assert_eq!(csv_row.tokens_per_sec, "10.00");
        assert_eq!(csv_row.response_tokens, 100);
        assert_eq!(csv_row.prompt, "test prompt");
    }

    #[test]
    #[allow(deprecated)] // We need to set legacy cpu_usage_percent field
    fn test_csv_metric_row_formatting() {
        let metric = BenchmarkMetrics {
            first_token_latency_ms: 123.456,
            total_response_time_ms: 1234.567,
            tokens_per_second: 12.345,
            avg_token_latency_ms: 98.765,
            memory_before_mb: 1000.123,
            memory_during_mb: 1100.456,
            memory_after_mb: 1000.789,
            peak_memory_mb: 1200.987,
            cpu_ollama_percent: 16.123,
            cpu_tauri_percent: 40.456,
            cpu_system_percent: 45.789,
            cpu_total_percent: 56.579,
            cpu_usage_percent: 16.123, // Legacy field
            model_name: "test".to_string(),
            prompt_type: "medium".to_string(),
            prompt: "prompt".to_string(),
            response_tokens: 200,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            iteration: 2,
            cpu_model: "Test CPU".to_string(),
            gpu_name: "Test GPU".to_string(),
            avx2_supported: true,
            npu_detected: false,
            hardware_detection_failed: false,
            timing_source: TimingSource::Native,
        };

        let csv_row = CsvMetricRow::from(&metric);

        // Verify 2 decimal place formatting
        assert_eq!(csv_row.first_token_ms, "123.46");
        assert_eq!(csv_row.total_time_ms, "1234.57");
        assert_eq!(csv_row.tokens_per_sec, "12.35");
        assert_eq!(csv_row.memory_peak_mb, "1200.99");
        // Verify CPU metrics formatting
        assert_eq!(csv_row.cpu_ollama_percent, "16.12");
        assert_eq!(csv_row.cpu_tauri_percent, "40.46");
        assert_eq!(csv_row.cpu_system_percent, "45.79");
        assert_eq!(csv_row.cpu_total_percent, "56.58");
    }

    #[test]
    fn test_generate_filename() {
        let filename = generate_filename("test-prefix");

        assert!(filename.starts_with("test-prefix-"));
        assert!(filename.ends_with(".csv"));
        assert!(filename.contains('-'));

        // Should contain timestamp components (year-month-day_hour-minute-second)
        let parts: Vec<&str> = filename.split('-').collect();
        assert!(parts.len() >= 6, "Filename should contain date/time components");
    }

    #[test]
    fn test_export_to_csv_creates_file() {
        let metric = create_test_metric();
        let results = BenchmarkResults {
            metrics: vec![metric],
            summary: vec![],
            total_duration_seconds: 10.0,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };

        let result = export_to_csv(&results, "test");

        assert!(result.is_ok(), "CSV export should succeed");

        let filepath = result.unwrap();
        assert!(filepath.exists(), "CSV file should be created");
        assert_eq!(filepath.extension().unwrap(), "csv");

        // Cleanup
        let _ = std::fs::remove_file(&filepath);
    }

    #[test]
    fn test_export_to_csv_content_validation() {
        let metric = create_test_metric();
        let results = BenchmarkResults {
            metrics: vec![metric],
            summary: vec![],
            total_duration_seconds: 10.0,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };

        // Use unique prefix to avoid test interference
        let prefix = format!("test-content-{}", std::process::id());
        let filepath = export_to_csv(&results, &prefix).unwrap();

        // Read the CSV file and verify content
        let content = std::fs::read_to_string(&filepath).unwrap();

        // Check for header row
        assert!(content.contains("timestamp,iteration,category,model"));
        assert!(content.contains("first_token_ms,total_time_ms,tokens_per_sec"));

        // Check for data row
        assert!(content.contains("2025-01-01T00:00:00Z"));
        assert!(content.contains("test-model"));
        assert!(content.contains("short"));
        assert!(content.contains("test prompt"));

        // Cleanup
        let _ = std::fs::remove_file(&filepath);
    }

    #[test]
    fn test_export_to_csv_multiple_rows() {
        let results = BenchmarkResults {
            metrics: vec![
                create_test_metric(),
                BenchmarkMetrics {
                    prompt_type: "medium".to_string(),
                    iteration: 2,
                    ..create_test_metric()
                },
            ],
            summary: vec![],
            total_duration_seconds: 20.0,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };

        // Use unique prefix to avoid test interference
        let prefix = format!("test-multi-{}", std::process::id());
        let filepath = export_to_csv(&results, &prefix).unwrap();
        let content = std::fs::read_to_string(&filepath).unwrap();

        // Should have header + 2 data rows
        let line_count = content.lines().count();
        assert_eq!(line_count, 3, "Should have header + 2 data rows");

        // Should contain both categories
        assert!(content.contains("short"));
        assert!(content.contains("medium"));

        // Cleanup
        let _ = std::fs::remove_file(&filepath);
    }

    #[test]
    fn test_get_benchmarks_dir_creates_directory() {
        let result = get_benchmarks_dir();
        assert!(result.is_ok(), "Should create benchmarks directory");

        let path = result.unwrap();
        assert!(path.exists(), "Benchmarks directory should exist");
        assert!(path.is_dir(), "Path should be a directory");
        assert_eq!(path.file_name().unwrap(), "benchmarks");
    }
}
