# Benchmarks

The `smolpc-benchmark` crate is a standalone CLI tool for measuring inference performance across backends. It uses fixed prompts, greedy decoding, and process-specific resource monitoring to produce reproducible, comparable results.

**Source:** `engine/crates/smolpc-benchmark/src/`

## Quick Start

```bash
cargo run -p smolpc-benchmark -- run \
  --machine "core-ultra-16gb" \
  --backends cpu,openvino_npu \
  --runs 5 \
  --warmup 3 \
  --cooldown 10
```

This runs all 10 prompts on each backend, 5 times per prompt, with 3 warmup iterations and 10 seconds cooldown between backend switches.

## CLI

```
smolpc-benchmark run [OPTIONS]
  --machine <LABEL>           Human label for this machine (used in output filename)
  --backends <LIST>           Comma-separated: cpu,directml,openvino_npu (default: auto-detect)
  --models <LIST>             Comma-separated model IDs (default: all registered)
  --runs <N>                  Measurement iterations per prompt (default: 5)
  --warmup <N>                Warmup iterations before measurement (default: 2, NPU min: 3)
  --cooldown <SECS>           Seconds to wait between backend switches
  --output-dir <PATH>         Output directory (default: current directory)
  --port <PORT>               Engine port (default: 19432)
  --resource-dir <PATH>       Engine resource directory
  --dry-run                   Print plan without running

smolpc-benchmark compare <FILE_A> <FILE_B>
  Compare two benchmark reports side-by-side
```

## Measurement Methodology

### Generation Config

All benchmarks use deterministic settings for reproducibility:

- `temperature = 0.0` (greedy decoding, no sampling)
- `repetition_penalty = 1.0` (disabled)
- `top_k = None`, `top_p = None` (disabled)

This ensures identical output across runs, making timing measurements meaningful.

### Prompt Corpus

10 fixed coding prompts across 3 tiers:

| Tier | Count | Input Size | max_tokens | Purpose |
|---|---|---|---|---|
| Short | 4 | ~20 tokens | 128 | Quick completions (palindrome check, CSS centering, syntax fix, map explanation) |
| Medium | 4 | ~150 tokens | 256 | Multi-step tasks (bubble sort, statistics function, todo list app, JS scoping) |
| Long | 2 | ~500 tokens | 512 | Complex tasks (code review + rewrite, REST API design) |

The corpus is hardcoded in `prompts.rs`. A SHA-256 hash of the concatenated prompt contents is stored in the report for comparison validity — if two reports have different hashes, the prompts changed and results are not comparable.

### Execution Flow

For each backend × model combination:

1. **Load model** — send `POST /engine/load` and wait for ready
2. **Verify backend** — confirm the engine reports the expected backend
3. **Capture idle baseline** — sample RSS and CPU% for 2 seconds with no inference running
4. **Warmup** — run the first prompt N times (NPU always >= 3 warmups to ensure compilation cache is populated). The first warmup captures cold-start TTFT.
5. **Measured runs** — for each of the 10 prompts, run N iterations:
   a. Start resource sampler (background task, 200ms interval)
   b. Call `POST /v1/chat/completions` with streaming
   c. Stop sampler, record metrics
   d. On failure: increment consecutive failure counter; bail after 3 consecutive failures
6. **Compute statistics** — mean, median, p90, p95, std_dev for each metric
7. **Aggregate reliability** — success rate, truncation rate, natural EOS rate, error breakdown

### Warmup Strategy

Warmup is critical for the NPU backend: the first generation after model load triggers a compilation pass that can take 3-5 minutes. Without warmup, TTFT measurements would be dominated by compilation time rather than inference latency.

NPU warmup is forced to at least 3 iterations to ensure:
1. First run: triggers compilation (or cache hit)
2. Second run: primes the pipeline
3. Third run: stabilizes timing

CPU and DirectML backends use the configured warmup count (default 2).

### Cooldown Between Backends

When switching backends (e.g., CPU → DirectML → NPU), a configurable cooldown period allows:
- GPU memory to be released
- Thermal throttling to dissipate
- OS resource cleanup to complete

Without cooldown, the second backend may show degraded performance due to residual resource pressure from the first.

## Metrics Collected

### Per-Run Metrics

| Metric | Source | Unit |
|---|---|---|
| `ttft_ms` | Engine's `GenerationMetrics.time_to_first_token_ms` | Milliseconds |
| `tokens_per_second` | Engine's `GenerationMetrics.tokens_per_second` | Tokens/sec |
| `tpot_ms` | Derived: `(total_time - ttft) / (tokens - 1)` | Milliseconds |
| `total_time_ms` | Engine's `GenerationMetrics.total_time_ms` | Milliseconds |
| `total_tokens` | Engine's `GenerationMetrics.total_tokens` | Count |
| `peak_memory_mb` | Resource sampler: peak RSS of engine process | MB |
| `mean_cpu_percent` | Resource sampler: mean CPU% during generation | Percent |
| `peak_cpu_percent` | Resource sampler: peak CPU% during generation | Percent |
| `stop_reason` | Derived from metrics | Enum (see below) |
| `truncated` | Engine's `GenerationMetrics.truncated` | Boolean |

**TPOT (Time Per Output Token)** is derived from the other metrics: `(total_time_ms - ttft_ms) / (total_tokens - 1)`. This isolates decode speed from prefill latency.

**Stop reasons:**
- `natural_eos` — model emitted EOS before reaching max_tokens (ideal)
- `max_tokens` — output was truncated at the limit (generation was still going)
- `max_tokens_exact` — output was exactly max_tokens (ambiguous — could be natural or truncated)
- `error` — generation failed

### Resource Sampling

The `ResourceSampler` runs as a background Tokio task that polls the engine process every 200ms via `sysinfo`:

- **RSS (Resident Set Size)** — physical memory used by the engine process. Peak value tracked via `AtomicU64::fetch_max`.
- **CPU%** — CPU utilization of the engine process (0.0 to N×100.0 for N cores). Mean computed from all samples; peak tracked via `AtomicU32::fetch_max` with fixed-point encoding.

The sampler requires two consecutive `sysinfo` refreshes to compute a CPU delta (the first refresh only primes the counter), so it sleeps 100ms after the first refresh before entering the sampling loop.

### Idle Baseline

Before each backend × model combo, a 2-second idle baseline is captured with no inference running. This measures the engine's resting memory footprint and CPU usage, providing a reference point for computing inference-specific resource consumption.

### Per-Prompt Statistics

For each prompt, statistics are computed across all successful runs (failed runs are excluded):

```rust
pub struct PromptStats {
    pub ttft: Option<Stats>,
    pub tokens_per_second: Option<Stats>,
    pub tpot: Option<Stats>,
    pub total_time: Option<Stats>,
    pub peak_memory_mb: Option<Stats>,
    pub mean_cpu_percent: Option<Stats>,
    pub peak_cpu_percent: Option<Stats>,
}
```

Each `Stats` object contains: mean, median, p90, p95, std_dev, min, max. Percentiles use linear interpolation on sorted values.

### Reliability Metrics

Aggregated across all prompts for a backend × model combo:

| Metric | Description |
|---|---|
| `total_runs` | Total runs attempted |
| `successful_runs` | Runs that completed without error |
| `failed_runs` | Runs that produced errors |
| `truncation_rate` | Fraction of successful runs truncated at max_tokens (0.0-1.0) |
| `natural_eos_rate` | Fraction of successful runs that stopped at natural EOS (0.0-1.0) |
| `error_breakdown` | Error messages grouped by count (truncated to 100 chars for grouping) |

A high truncation rate suggests max_tokens is too low for the prompt. A low natural_eos_rate on short prompts may indicate the model is not generating stop tokens correctly.

## Output Format

### JSON Report

The primary output is a JSON report written to the output directory:

```
benchmark-<machine>-<date>.json
```

Schema version 2.0.0. Top-level structure:

```json
{
  "schema_version": "2.0.0",
  "generated_at": "2026-03-27T14:30:00Z",
  "tool_version": "0.1.0",
  "machine": { "label": "core-ultra-16gb", "os": "Windows 11", "hostname": "..." },
  "hardware": {
    "cpu_brand": "Intel Core Ultra 7 155H",
    "cpu_cores_physical": 16,
    "cpu_cores_logical": 22,
    "gpus": [{ "name": "Intel Arc Graphics", "vendor": "Intel", "vram_mb": null }],
    "npu": "Intel AI Boost",
    "memory_total_gb": 16.0
  },
  "test_config": {
    "backends": ["cpu", "openvino_npu"],
    "models": ["qwen2.5-1.5b-instruct"],
    "runs_per_prompt": 5,
    "warmup_runs": 3,
    "temperature": 0.0,
    "repetition_penalty": 1.0,
    "prompt_corpus_hash": "abc123..."
  },
  "results": [ ... ],
  "skipped": [ ... ]
}
```

Each entry in `results` contains per-prompt breakdowns with individual run metrics and aggregated statistics.

### Partial Results

A partial file (`benchmark-<machine>.partial.json`) is written after each completed backend × model combo. If the benchmark crashes mid-run, partial results are preserved. The partial file is deleted when the final report is written.

### Summary Table

After completion, a summary table is printed to stdout:

```
Backend         Model                        TTFT(ms)   Tok/s   TPOT(ms)   Mem(MB)  Trunc%  Errors
----------------------------------------------------------------------------------------------------
cpu             qwen2.5-1.5b-instruct           1234     5.2      192.3      1240      0%       0
openvino_npu    qwen2.5-1.5b-instruct            180    12.8       78.1       890      0%       0
```

## Comparison

The `compare` subcommand loads two JSON reports and highlights regressions:

```bash
cargo run -p smolpc-benchmark -- compare \
  benchmark-core-ultra-2026-03-20.json \
  benchmark-core-ultra-2026-03-27.json
```

The default regression threshold is 10% — if a metric degrades by more than 10%, it is flagged. Comparison requires matching prompt corpus hashes to ensure validity.

## Backend Comparison Methodology

To produce valid cross-backend comparisons:

1. **Same prompts** — all backends run the identical 10-prompt corpus with the same max_tokens
2. **Same model** — each comparison row uses the same model ID (quantization may differ by backend)
3. **Greedy decoding** — temperature=0.0 ensures deterministic output, eliminating sampling variance
4. **Warmup** — ensures all pipelines are primed before measurement
5. **Cooldown** — prevents thermal/resource carryover between backends
6. **Process-specific monitoring** — RSS and CPU% are measured for the engine process only, not system-wide
7. **Multiple runs** — statistical aggregation (mean, median, p90) smooths individual run variance

## Key Files

| File | Purpose |
|---|---|
| `main.rs` | CLI definition (clap), subcommands, orchestration |
| `config.rs` | `BenchmarkConfig`, backend enum, constants |
| `runner.rs` | Core measurement loop, per-prompt stats computation |
| `prompts.rs` | 10 fixed coding prompts across 3 tiers |
| `stats.rs` | Descriptive statistics (mean, median, p90, p95, std_dev) |
| `output.rs` | JSON report types, file I/O, summary table |
| `compare.rs` | Two-report comparison with regression detection |
| `resource_sampler.rs` | Background RSS + CPU% sampling via sysinfo |
| `engine_lifecycle.rs` | Engine spawn, health check, model load, shutdown |
| `reliability.rs` | Success/failure/truncation rate aggregation |
