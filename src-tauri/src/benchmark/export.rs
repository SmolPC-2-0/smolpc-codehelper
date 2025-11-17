use super::metrics::BenchmarkResults;
use csv::Writer;
use std::fs;
use std::path::PathBuf;

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
"#;

        fs::write(&readme_path, readme_content)
            .map_err(|e| format!("Failed to create README: {}", e))?;
    }

    Ok(())
}
