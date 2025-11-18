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
**Process-Specific Monitoring:** CPU and memory measurements track the Ollama server process specifically, not system-wide resources. This provides accurate measurements even with other applications running.

- Automatically detects the Ollama process by name at benchmark start
- Samples CPU and memory usage every 100ms during inference
- Tracks peak memory and average CPU for the Ollama process only
- Fallback to system-wide monitoring if Ollama process cannot be identified

**Benefits:**
- Accurate measurements regardless of background apps
- No need to close other applications before benchmarking
- Isolates Ollama performance from system noise

## Usage

Import CSV files into Excel, Google Sheets, or data analysis tools for visualization and comparison across optimization phases.
