use super::metrics::{BenchmarkMetrics, BenchmarkResults, BenchmarkSummary};
use csv::Writer;
use std::fs;
use std::path::{Path, PathBuf};

/// Get the benchmarks directory path (creates if doesn't exist)
pub fn get_benchmarks_dir() -> Result<PathBuf, String> {
    let current_dir = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;

    let benchmarks_dir = current_dir.join("benchmarks");

    // Create directory if it doesn't exist
    if !benchmarks_dir.exists() {
        fs::create_dir_all(&benchmarks_dir)
            .map_err(|e| format!("Failed to create benchmarks directory: {}", e))?;
    }

    Ok(benchmarks_dir)
}

/// Generate filename with timestamp
pub fn generate_filename(prefix: &str) -> String {
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    format!("{}-{}.csv", prefix, timestamp)
}

/// Export benchmark results to CSV
pub fn export_to_csv(results: &BenchmarkResults, prefix: &str) -> Result<PathBuf, String> {
    let benchmarks_dir = get_benchmarks_dir()?;
    let filename = generate_filename(prefix);
    let filepath = benchmarks_dir.join(&filename);

    // Create CSV writer
    let mut wtr = Writer::from_path(&filepath)
        .map_err(|e| format!("Failed to create CSV file: {}", e))?;

    // Write header
    wtr.write_record(&[
        "timestamp",
        "iteration",
        "category",
        "model",
        "first_token_ms",
        "total_time_ms",
        "tokens_per_sec",
        "avg_token_ms",
        "memory_before_mb",
        "memory_peak_mb",
        "cpu_percent",
        "response_tokens",
        "prompt",
    ])
    .map_err(|e| format!("Failed to write CSV header: {}", e))?;

    // Write metrics data
    for metric in &results.metrics {
        wtr.write_record(&[
            &metric.timestamp,
            &metric.iteration.to_string(),
            &metric.prompt_type,
            &metric.model_name,
            &format!("{:.2}", metric.first_token_latency_ms),
            &format!("{:.2}", metric.total_response_time_ms),
            &format!("{:.2}", metric.tokens_per_second),
            &format!("{:.2}", metric.avg_token_latency_ms),
            &format!("{:.2}", metric.memory_before_mb),
            &format!("{:.2}", metric.peak_memory_mb),
            &format!("{:.2}", metric.cpu_usage_percent),
            &metric.response_tokens.to_string(),
            &metric.prompt,
        ])
        .map_err(|e| format!("Failed to write metric row: {}", e))?;
    }

    // Write empty row separator
    wtr.write_record(&[""; 13])
        .map_err(|e| format!("Failed to write separator: {}", e))?;

    // Write summary section header
    wtr.write_record(&["SUMMARY"])
        .map_err(|e| format!("Failed to write summary header: {}", e))?;

    wtr.write_record(&[
        "category",
        "avg_first_token_ms",
        "avg_tokens_per_sec",
        "avg_total_time_ms",
        "avg_memory_mb",
        "avg_cpu_percent",
        "test_count",
    ])
    .map_err(|e| format!("Failed to write summary column headers: {}", e))?;

    // Write summary data
    for summary in &results.summary {
        wtr.write_record(&[
            &summary.category,
            &format!("{:.2}", summary.avg_first_token_ms),
            &format!("{:.2}", summary.avg_tokens_per_sec),
            &format!("{:.2}", summary.avg_total_time_ms),
            &format!("{:.2}", summary.avg_memory_mb),
            &format!("{:.2}", summary.avg_cpu_percent),
            &summary.test_count.to_string(),
        ])
        .map_err(|e| format!("Failed to write summary row: {}", e))?;
    }

    // Write metadata
    wtr.write_record(&[""; 13])
        .map_err(|e| format!("Failed to write separator: {}", e))?;

    wtr.write_record(&["METADATA"])
        .map_err(|e| format!("Failed to write metadata header: {}", e))?;

    wtr.write_record(&["Total Duration (seconds)", &format!("{:.2}", results.total_duration_seconds)])
        .map_err(|e| format!("Failed to write total duration: {}", e))?;

    wtr.write_record(&["Benchmark Timestamp", &results.timestamp])
        .map_err(|e| format!("Failed to write timestamp: {}", e))?;

    wtr.write_record(&["Total Tests", &results.metrics.len().to_string()])
        .map_err(|e| format!("Failed to write total tests: {}", e))?;

    // Flush the writer
    wtr.flush()
        .map_err(|e| format!("Failed to flush CSV writer: {}", e))?;

    Ok(filepath)
}

/// Create a README.md in the benchmarks directory explaining the CSV format
pub fn create_readme() -> Result<(), String> {
    let benchmarks_dir = get_benchmarks_dir()?;
    let readme_path = benchmarks_dir.join("README.md");

    // Only create if doesn't exist
    if !readme_path.exists() {
        let readme_content = r#"# Benchmark Results

This directory contains benchmark test results for SmolPC Code Helper.

## File Naming Convention

Files are named: `{prefix}-{timestamp}.csv`
- `baseline`: Initial performance before optimizations
- `phase1`: After Phase 1 optimizations
- `phase2`: After Phase 2 optimizations
- etc.

## CSV Format

### Main Data Section
- **timestamp**: ISO 8601 timestamp of test execution
- **iteration**: Test iteration number (1-3 typically)
- **category**: Prompt category (short, medium, long, follow-up)
- **model**: AI model name used
- **first_token_ms**: Latency to first token (ms)
- **total_time_ms**: Total response generation time (ms)
- **tokens_per_sec**: Throughput (tokens/second)
- **avg_token_ms**: Average time per token (ms)
- **memory_before_mb**: RAM before inference (MB)
- **memory_peak_mb**: Peak RAM during inference (MB)
- **cpu_percent**: Average CPU utilization (%)
- **response_tokens**: Number of tokens in response
- **prompt**: The test prompt text

### Summary Section
Aggregated statistics by category:
- **avg_first_token_ms**: Average first token latency
- **avg_tokens_per_sec**: Average throughput
- **avg_total_time_ms**: Average total time
- **avg_memory_mb**: Average peak memory
- **avg_cpu_percent**: Average CPU usage
- **test_count**: Number of tests in category

### Metadata Section
- **Total Duration**: Total benchmark execution time
- **Benchmark Timestamp**: When benchmark was run
- **Total Tests**: Number of individual tests

## Usage

Import CSV files into Excel, Google Sheets, or data analysis tools for visualization and comparison across optimization phases.
"#;

        fs::write(&readme_path, readme_content)
            .map_err(|e| format!("Failed to create README: {}", e))?;
    }

    Ok(())
}
