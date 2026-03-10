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
Token counts use Ollama's **native token metadata** (`eval_count`) for maximum accuracy. No estimation or approximation is used. Tests fail immediately if Ollama does not provide accurate token counts.

### Timing Metrics
All timing measurements use **Ollama's native nanosecond-precision timing data**:
- `first_token_ms`: From `prompt_eval_duration` (time to process prompt before generation)
- `total_time_ms`: From `total_duration` (full request duration)
- `tokens_per_sec`: Calculated from `eval_count` / `eval_duration`
- `avg_token_ms`: Calculated from `eval_duration` / `eval_count`

**Non-streaming mode** is used to ensure Ollama provides complete metadata. This sacrifices real-time streaming for production-grade measurement accuracy.

### Resource Monitoring

**Model Warmup:** Before any benchmarks run, a warmup request loads the model and identifies the Ollama process. This eliminates first-call latency from benchmark results and establishes reliable process monitoring.

**Process-Specific Monitoring:** CPU and memory measurements track the Ollama inference process specifically, not system-wide resources. All measurements are process-specific with **no fallback to system-wide monitoring** (tests fail if Ollama process cannot be found).

**Sampling methodology:**
- Pre-identifies Ollama process during warmup (fails immediately if not found)
- Establishes CPU baseline with 200ms delay (required by sysinfo crate)
- Samples CPU and memory every 50ms during inference (rigorous monitoring)
- Captures memory from Ollama process at: before, during (median), after, and peak
- Tracks peak memory by comparing all samples (not initialized to system memory)

**Memory metrics:**
- `memory_before_mb`: Process-specific memory before inference request
- `memory_peak_mb`: Maximum process memory observed during all samples
- `memory_during_mb`: **Median** of all memory samples (outlier-resistant)
- `memory_after_mb`: Process-specific memory after inference completes

**Known Limitations:**
- CPU measurements show lower values (~4-16%) than total system load due to HTTP API architecture
- The benchmark client (code helper) runs in a separate process, adding HTTP overhead
- GPU-accelerated inference legitimately shows low CPU usage (GPU does heavy lifting)
- This will be resolved in future versions using in-process llama.cpp integration

**Benefits:**
- Production-grade accuracy suitable for academic research reports
- Measurements isolated from background system noise
- Consistent methodology across all tests
- Clear failure modes (no silent degradation to inaccurate measurements)

## Usage

Import CSV files into Excel, Google Sheets, or data analysis tools for visualization and comparison across optimization phases.
