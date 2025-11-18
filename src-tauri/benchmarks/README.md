# Benchmark Results

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

## Measurement Methodology

### Token Counting
Token counts use Ollama's actual token metadata (`eval_count`) for maximum accuracy. If metadata is unavailable, estimation fallback uses ~4 characters per token.

### Resource Monitoring
**CPU and Memory measurements are system-wide, not process-specific.** Measurements capture total system resource usage during inference via periodic sampling (100ms intervals).

**For accurate benchmarks:**
- Close unnecessary background applications
- Run benchmarks when system is relatively idle
- Use consistent system conditions across test runs

**Future enhancement:** Process-specific monitoring of the Ollama server process.

## Usage

Import CSV files into Excel, Google Sheets, or data analysis tools for visualization and comparison across optimization phases.
