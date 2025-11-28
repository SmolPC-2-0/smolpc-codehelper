# SmolPC Code Helper - Benchmark Module Technical Documentation

**Version:** 2.2.0
**Last Updated:** January 2025
**Audience:** Developers/Engineers
**Purpose:** Comprehensive technical reference for presentation and deep system understanding

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Architecture Overview](#2-architecture-overview)
3. [Data Collection Deep Dive](#3-data-collection-deep-dive)
4. [Data Processing](#4-data-processing)
5. [Export System](#5-export-system)
6. [Rust Patterns & Mechanisms](#6-rust-patterns--mechanisms)
7. [IPC & Frontend Integration](#7-ipc--frontend-integration)
8. [Performance Characteristics](#8-performance-characteristics)
9. [Accuracy & Reliability](#9-accuracy--reliability)
10. [Future Architecture Support](#10-future-architecture-support)
11. [Testing Infrastructure](#11-testing-infrastructure)
12. [Critical Implementation Details](#12-critical-implementation-details)
13. [Dependency Reference](#13-dependency-reference)
14. [Complete API Reference](#14-complete-api-reference)
15. [Code References Index](#15-code-references-index)

---

## 1. Executive Summary

### Purpose and Goals

The benchmark module provides **rigorous, production-grade performance testing** for AI model inference in an educational coding assistant context. It measures real-world performance characteristics to:

1. **Compare models** - Evaluate different AI models (Qwen 2.5 Coder, DeepSeek Coder) for educational suitability
2. **Architecture comparison** - Measure HTTP-based Ollama vs planned llama.cpp in-process inference
3. **Hardware optimization** - Validate AVX2/AVX512/NEON optimizations and GPU offloading effectiveness
4. **Educational workload simulation** - Test realistic secondary school student coding scenarios

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         FRONTEND (Svelte 5)                     │
│  ┌────────────────┐         ┌──────────────────────┐           │
│  │ BenchmarkPanel │────────▶│  benchmarkStore      │           │
│  │   .svelte      │         │  (Svelte 5 Runes)    │           │
│  └────────────────┘         └──────────────────────┘           │
│         │                              │                         │
│         │ Tauri IPC                    │ listen('benchmark_     │
│         │ invoke()                     │ progress/complete')    │
└─────────┼──────────────────────────────┼─────────────────────────┘
          │                              │
          ▼                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    BACKEND (Rust/Tauri)                         │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  commands/benchmark.rs (IPC Layer)                       │  │
│  │  • run_benchmark(model, iterations)                      │  │
│  │  • get_benchmarks_directory()                            │  │
│  │  • open_benchmarks_folder()                              │  │
│  └────────────────┬─────────────────────────────────────────┘  │
│                   │                                             │
│                   ▼                                             │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  benchmark/runner.rs (Execution Engine)                  │  │
│  │  • run_benchmark_suite()                                 │  │
│  │  • run_single_test()                                     │  │
│  └────┬─────────────┬─────────────┬─────────────┬───────────┘  │
│       │             │             │             │               │
│       ▼             ▼             ▼             ▼               │
│  ┌─────────┐  ┌─────────┐  ┌──────────┐  ┌──────────┐         │
│  │ process │  │ sampling│  │ metrics  │  │ export   │         │
│  │   .rs   │  │   .rs   │  │   .rs    │  │   .rs    │         │
│  │         │  │         │  │          │  │          │         │
│  │ • HW    │  │ • CPU/  │  │ • Data   │  │ • CSV    │         │
│  │ detect  │  │   Memory│  │   structs│  │   write  │         │
│  │ • Ollama│  │   sample│  │ • Summary│  │ • README │         │
│  │ PID     │  │ • Stats │  │   calc   │  │   gen    │         │
│  └─────────┘  └─────────┘  └──────────┘  └──────────┘         │
│       │             │             │             │               │
└───────┼─────────────┼─────────────┼─────────────┼───────────────┘
        │             │             │             │
        ▼             ▼             ▼             ▼
┌─────────────────────────────────────────────────────────────────┐
│                    EXTERNAL DEPENDENCIES                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │
│  │ hardware-    │  │  sysinfo     │  │    Ollama    │         │
│  │ query 0.2.1  │  │    0.32      │  │  (localhost) │         │
│  │              │  │              │  │              │         │
│  │ CPU/GPU/NPU  │  │ Process info │  │ HTTP API     │         │
│  └──────────────┘  └──────────────┘  └──────────────┘         │
└─────────────────────────────────────────────────────────────────┘
```

### Key Metrics Tracked

**PRIMARY - Timing Metrics:**
- **First Token Latency** (ms) - Time to first response token (user-perceived responsiveness)
- **Total Response Time** (ms) - Complete generation duration
- **Tokens Per Second** - Throughput (critical for long responses)
- **Average Token Latency** (ms) - Per-token cost

**SECONDARY - Resource Metrics:**
- **Memory** (MB) - Before/During(median)/After/Peak
- **CPU** (%) - Ollama process, Tauri process, System-wide, Total (ollama+tauri)

**METADATA:**
- Hardware snapshot (CPU model, GPU, AVX2, NPU)
- Test category, iteration, timestamp
- Timing source (native Ollama vs client-side estimation)

### Design Philosophy

1. **Accuracy First** - Prefer Ollama's nanosecond-precision native timing over client estimates
2. **Architecture Comparison Ready** - Separate Ollama/Tauri CPU tracking for HTTP overhead analysis
3. **Crash Safety** - Periodic CSV flushes prevent data loss mid-benchmark
4. **Fallback Mechanisms** - Graceful degradation when hardware detection or native timing fails
5. **Cross-Platform** - sysinfo abstractions for Windows/macOS/Linux compatibility

---

## 2. Architecture Overview

### Complete File Inventory

| File Path | Lines | Purpose |
|-----------|-------|---------|
| `src-tauri/src/benchmark/mod.rs` | 9 | Module exports and public API surface |
| `src-tauri/src/benchmark/runner.rs` | 354 | Core benchmark execution engine, test orchestration |
| `src-tauri/src/benchmark/metrics.rs` | 349 | Data structures, summary calculation, tests |
| `src-tauri/src/benchmark/test_suite.rs` | 107 | 12-prompt test suite definitions |
| `src-tauri/src/benchmark/sampling.rs` | 225 | Background CPU/memory resource monitoring |
| `src-tauri/src/benchmark/process.rs` | 175 | Hardware detection, Ollama process identification |
| `src-tauri/src/benchmark/export.rs` | 460 | CSV generation, README creation, tests |
| `src-tauri/src/commands/benchmark.rs` | 85 | Tauri IPC command interface |
| **Backend Total** | **~1,764 lines** | **Core implementation** |
| `src/lib/stores/benchmark.svelte.ts` | 74 | Frontend state management (Svelte 5) |
| `src/lib/components/BenchmarkPanel.svelte` | ~150 | UI component (not analyzed in detail) |
| **Frontend Total** | **~224 lines** | **User interface** |

### Module Dependency Graph

```
benchmark/mod.rs (public exports)
    ├─► export.rs
    │      ├─► metrics.rs (BenchmarkResults)
    │      └─► tauri::AppHandle (directory resolution)
    │
    ├─► metrics.rs
    │      ├─► serde (serialization)
    │      └─► chrono (timestamps)
    │
    └─► runner.rs
           ├─► metrics.rs (BenchmarkMetrics, TimingSource)
           ├─► process.rs (HardwareSnapshot, warmup_and_find_process)
           ├─► sampling.rs (SamplingState, spawn_resource_sampler)
           ├─► test_suite.rs (get_test_suite, PromptCategory)
           ├─► commands/ollama.rs (OllamaConfig, OllamaRequest, OllamaResponse)
           ├─► reqwest::Client (HTTP requests)
           └─► sysinfo::System (memory snapshots)

process.rs
    ├─► hardware (detect_all from hardware-query crate)
    ├─► sysinfo (process enumeration)
    └─► commands/ollama.rs (warmup request)

sampling.rs
    ├─► sysinfo (CPU/memory monitoring)
    ├─► std::sync::{Arc, Mutex} (shared state)
    └─► tokio (async task spawning, oneshot channels)

commands/benchmark.rs
    ├─► benchmark::{run_benchmark_suite, export_to_csv, create_readme}
    ├─► commands/ollama::{HttpClient, OllamaConfig} (managed state)
    └─► tauri::{AppHandle, Emitter, State}
```

### Data Flow: Complete Request Lifecycle

```
1. USER ACTION
   └─► BenchmarkPanel.svelte
       └─► invoke('run_benchmark', {model, iterations})

2. TAURI IPC BOUNDARY
   └─► commands/benchmark::run_benchmark()
       ├─► create_readme(&app_handle)
       ├─► run_benchmark_suite(...) ──┐
       └─► export_to_csv(...)         │
                                      │
3. INITIALIZATION                     │
   ┌──────────────────────────────────┘
   │
   ├─► HardwareSnapshot::detect()
   │   └─► hardware::detect_all() (hardware-query crate)
   │       └─► Returns: CPU model, GPU, AVX2, NPU detection
   │
   └─► warmup_and_find_process(model, ...)
       ├─► POST /api/chat (warmup request with short prompt)
       ├─► Sleep 500ms (model loading stabilization)
       └─► find_inference_process()
           ├─► sysinfo: enumerate all processes
           ├─► Filter by name.contains("ollama")
           ├─► Sort by memory (descending)
           └─► Return PID with >500MB memory

4. TEST ITERATION LOOP (1..=iterations)
   └─► For each of 12 prompts (short×3, medium×3, long×3, followup×3):
       │
       ├─► Build context (if follow-up prompt)
       │   └─► Use response from "short_1" test
       │
       ├─► Emit progress event
       │   └─► app_handle.emit("benchmark_progress", {current, total, ...})
       │
       ├─► run_single_test(...) ──────┐
       │                              │
       └─► Sleep 500ms (stabilization)│
                                      │
5. SINGLE TEST EXECUTION              │
   ┌──────────────────────────────────┘
   │
   ├─► Memory snapshot (before)
   │   └─► sysinfo::process(ollama_pid).memory()
   │
   ├─► Create SamplingState::new(memory_before)
   │   └─► Pre-allocate vectors (capacity: 100)
   │
   ├─► spawn_resource_sampler(ollama_pid, state) ──┐
   │   └─► Returns oneshot::Receiver               │
   │                                                │
   ├─► HTTP POST /api/chat (Ollama)                │
   │   ├─► Timeout: 300s                           │
   │   ├─► Body: {model, messages, stream: false}  │
   │   └─► Await response                          │
   │                                                │
   ├─► collect_sampling_results(state, rx) ◄───────┘
   │   ├─► state.stop() (signal sampler to exit)
   │   ├─► await rx (wait for sampler completion)
   │   └─► Returns: SamplingResults {cpu_samples, memory_samples, ...}
   │
   ├─► calculate_timing_metrics(response, client_elapsed)
   │   ├─► Try native: response.eval_count, eval_duration, ...
   │   └─► Fallback: char-based token estimation
   │
   ├─► Memory snapshot (after)
   │
   ├─► Statistical aggregation
   │   ├─► calculate_average(cpu_samples)
   │   ├─► calculate_median(memory_samples)
   │   └─► Extract peak_memory from samples
   │
   └─► Return: (BenchmarkMetrics, response_content)

6. BACKGROUND SAMPLER TASK (async, parallel)
   │
   ├─► Establish CPU baseline (2 refreshes, 200ms apart)
   │
   └─► While state.is_active():
       ├─► sys.refresh_all()
       ├─► Read ollama process: CPU%, memory
       ├─► Read tauri process: CPU%
       ├─► Calculate system average CPU
       ├─► state.record_sample(ollama_cpu, tauri_cpu, system_cpu, memory)
       ├─► Sleep 50ms
       └─► Loop

7. SUMMARY CALCULATION
   └─► calculate_summary(&all_metrics)
       ├─► Group by category: ["short", "medium", "long", "follow-up"]
       ├─► For each category:
       │   ├─► Average first_token_latency
       │   ├─► Average tokens_per_second
       │   ├─► Average total_time
       │   ├─► Average peak_memory
       │   └─► Average all CPU metrics
       └─► Return Vec<BenchmarkSummary>

8. CSV EXPORT
   └─► export_to_csv(&results, "benchmark", &app_handle)
       ├─► get_benchmarks_dir_with_app_handle()
       │   └─► Platform-specific app data dir + "/benchmarks"
       ├─► generate_filename("benchmark")
       │   └─► "benchmark-2025-01-28_14-30-45.csv"
       ├─► For each metric:
       │   ├─► Convert to CsvMetricRow (format floats to 2 decimals)
       │   ├─► wtr.serialize(csv_row)
       │   └─► Flush every 10 rows (crash safety)
       ├─► Final flush
       └─► Return filepath

9. COMPLETION
   ├─► app_handle.emit("benchmark_complete", filepath)
   └─► Return BenchmarkResults to frontend
```

### Tauri Integration Points

**Managed State (Dependency Injection):**
```rust
// Registered in lib.rs during app setup
.manage(HttpClient::new())
.manage(OllamaConfig::default())

// Injected into commands via State<'_, T>
pub async fn run_benchmark(
    client: State<'_, HttpClient>,
    config: State<'_, OllamaConfig>,
) { ... }
```

**Event Emission (Progress Reporting):**
```rust
// Backend emits events
app_handle.emit("benchmark_progress", BenchmarkProgress { ... })?;
app_handle.emit("benchmark_complete", filepath)?;

// Frontend listens
const unlisten = await listen<BenchmarkProgress>('benchmark_progress', (event) => {
    benchmarkStore.updateProgress(event.payload);
});
```

**AppHandle (Directory Resolution):**
```rust
// Stable, platform-specific paths
let app_data_dir = app_handle.path().app_data_dir()?;
// macOS: ~/Library/Application Support/com.smolpc.codehelper/benchmarks/
// Windows: %APPDATA%\com.smolpc.codehelper\benchmarks\
// Linux: ~/.local/share/com.smolpc.codehelper/benchmarks/
```

### External Crate Usage and Rationale

| Crate | Version | Purpose | Why This Crate |
|-------|---------|---------|----------------|
| **sysinfo** | 0.32 | Process enumeration, CPU/memory monitoring | Cross-platform abstraction, mature, well-tested |
| **csv** | 1.3 | CSV file generation | Automatic serde serialization, header management |
| **chrono** | 0.4 | ISO 8601 timestamps | Industry standard for datetime handling |
| **hardware-query** | 0.2.1 | CPU/GPU/NPU detection | Unified cross-platform hardware detection |
| **reqwest** | 0.12 | HTTP client for Ollama API | Async, connection pooling, JSON support |
| **tokio** | 1.x | Async runtime | Required by Tauri, standard for Rust async |
| **serde** | 1.0 | Serialization | IPC communication, CSV export, JSON parsing |

---

## 3. Data Collection Deep Dive

This section provides exhaustive detail on every aspect of data collection - the core of the benchmarking system.

### 3.1 Test Suite Design

**12-Prompt Structure:**

The test suite consists of 12 prompts across 4 categories, designed to simulate realistic secondary school student coding interactions:

```rust
// src-tauri/src/benchmark/test_suite.rs:32-57

pub const SHORT_PROMPTS: [&str; 3] = [
    "What is a variable in Python?",
    "How do I print in JavaScript?",
    "Explain a for loop briefly",
];

pub const MEDIUM_PROMPTS: [&str; 3] = [
    "Write a bubble sort function in Python with comments",
    "Create a simple calculator program in JavaScript",
    "Explain classes and objects in Python with an example",
];

pub const LONG_PROMPTS: [&str; 3] = [
    "Explain object-oriented programming concepts with detailed examples in Python",
    "Write a complete web scraper in Python with error handling and documentation",
    "Create a detailed guide for beginners on how to use Git and GitHub",
];

pub const FOLLOW_UP_PROMPTS: [&str; 3] = [
    "Can you explain that more simply?",
    "Can you add more comments to the code?",
    "What are some common mistakes beginners make with this?",
];
```

**Category Rationale:**

1. **Short (3-10 words)** - Quick factual questions, tests responsiveness
2. **Medium (10-20 words)** - Code generation with documentation, tests typical student requests
3. **Long (15-30 words)** - Comprehensive explanations, tests sustained generation quality
4. **Follow-up (5-15 words)** - Conversational context, tests multi-turn dialogue performance

**Test Suite Generation:**

```rust
// src-tauri/src/benchmark/test_suite.rs:59-100

pub fn get_test_suite() -> Vec<TestPrompt> {
    let mut suite = Vec::new();

    // Add short prompts
    for (idx, prompt) in SHORT_PROMPTS.iter().enumerate() {
        suite.push(TestPrompt {
            id: format!("short_{}", idx + 1),  // short_1, short_2, short_3
            category: PromptCategory::Short,
            prompt: (*prompt).to_string(),
        });
    }

    // Add medium prompts
    for (idx, prompt) in MEDIUM_PROMPTS.iter().enumerate() {
        suite.push(TestPrompt {
            id: format!("medium_{}", idx + 1),  // medium_1, medium_2, medium_3
            category: PromptCategory::Medium,
            prompt: (*prompt).to_string(),
        });
    }

    // Add long prompts
    for (idx, prompt) in LONG_PROMPTS.iter().enumerate() {
        suite.push(TestPrompt {
            id: format!("long_{}", idx + 1),  // long_1, long_2, long_3
            category: PromptCategory::Long,
            prompt: (*prompt).to_string(),
        });
    }

    // Add follow-up prompts
    for (idx, prompt) in FOLLOW_UP_PROMPTS.iter().enumerate() {
        suite.push(TestPrompt {
            id: format!("followup_{}", idx + 1),  // followup_1, followup_2, followup_3
            category: PromptCategory::FollowUp,
            prompt: (*prompt).to_string(),
        });
    }

    suite  // Returns Vec<TestPrompt> with 12 elements
}
```

**Follow-Up Context Mechanism:**

Follow-up prompts require context from a previous interaction. The system uses the `short_1` test response:

```rust
// src-tauri/src/benchmark/runner.rs:146-153

fn build_followup_context(previous_response: &str) -> Vec<OllamaMessage> {
    let base_prompt = SHORT_PROMPTS.first().copied()
        .unwrap_or("What is a variable in Python?");

    vec![
        OllamaMessage { role: "user".to_string(), content: base_prompt.to_string() },
        OllamaMessage { role: "assistant".to_string(), content: previous_response.to_string() },
    ]
}
```

```rust
// src-tauri/src/benchmark/runner.rs:305-337

for iteration in 1..=iterations {
    for test in &test_suite {
        current_test += 1;

        // Build context for follow-up prompts
        let context = if test.category == PromptCategory::FollowUp {
            last_response.as_ref().map(|r| build_followup_context(r))
        } else {
            None
        };

        // ... execute test ...

        // Store first short test response for follow-up context
        if test.category == PromptCategory::Short && test.id == CONTEXT_SOURCE_TEST_ID {
            last_response = Some(response_content);
        }
    }
}
```

**System Prompt (Educational Context):**

Every request includes a system prompt designed for secondary school students:

```rust
// src-tauri/src/benchmark/runner.rs:29-39

const SYSTEM_PROMPT: &str = r"You are a helpful coding assistant designed for secondary school students (ages 11-18).
Your goal is to explain programming concepts clearly and provide well-commented code examples.

Guidelines:
- Use simple, encouraging language
- Break down complex concepts into steps
- Always include helpful comments in code
- Be patient and supportive
- Adapt explanations to the student's level
- Encourage learning and experimentation";
```

This ensures benchmarks test performance in the actual use case (educational assistance), not generic LLM performance.

---

### 3.2 Hardware Detection

**Purpose:** Capture hardware metadata to correlate performance with system capabilities (AVX2 support, GPU presence, NPU detection).

**HardwareSnapshot Structure:**

```rust
// src-tauri/src/benchmark/process.rs:20-28

#[derive(Debug, Clone)]
pub struct HardwareSnapshot {
    pub cpu_model: String,
    pub gpu_name: String,
    pub avx2_supported: bool,
    pub npu_detected: bool,
    pub detection_failed: bool,  // Flag for fallback data
}
```

**Detection Flow:**

```rust
// src-tauri/src/benchmark/process.rs:30-55

impl HardwareSnapshot {
    pub async fn detect() -> Self {
        match hardware::detect_all().await {
            Ok(info) => {
                // Prefer discrete GPU, fallback to first GPU, fallback to "No GPU"
                let gpu_name = info.gpus.iter()
                    .find(|g| g.device_type.eq_ignore_ascii_case("discrete"))
                    .or(info.gpus.first())
                    .map(|g| g.name.clone())
                    .unwrap_or_else(|| "No GPU".to_string());

                Self {
                    cpu_model: info.cpu.brand.clone(),
                    gpu_name,
                    avx2_supported: info.cpu.features.contains(&CPUFeature::AVX2),
                    npu_detected: info.npu.as_ref().is_some_and(|n| n.detected),
                    detection_failed: false,
                }
            }
            Err(e) => {
                log::warn!("Hardware detection failed: {e}");
                Self::default()  // Fallback to "Unknown" placeholders
            }
        }
    }
}
```

**Fallback Mechanism:**

```rust
// src-tauri/src/benchmark/process.rs:57-67

impl Default for HardwareSnapshot {
    fn default() -> Self {
        Self {
            cpu_model: "Unknown CPU".to_string(),
            gpu_name: "Unknown GPU".to_string(),
            avx2_supported: false,
            npu_detected: false,
            detection_failed: true,  // Signals unreliable metadata
        }
    }
}
```

**Integration with hardware-query Crate:**

The `hardware::detect_all()` function comes from the `hardware-query` crate (v0.2.1), which provides unified cross-platform detection:

- **CPU:** Brand, cores, features (AVX2/AVX512/NEON/SVE)
- **GPU:** Name, VRAM, backend (Metal/DirectX/Vulkan/CUDA), discrete vs integrated
- **NPU:** Detection with confidence (Apple Neural Engine, Intel AI Boost, AMD Ryzen AI)

**Why This Matters:**

- **AVX2 support** - Indicates CPU can run optimized llama.cpp builds (20-40% faster)
- **GPU presence** - Potential for layer offloading (10-100x speedup for large models)
- **NPU detection** - Future optimization target for inference acceleration

---

### 3.3 Process Detection & Warmup

**Challenge:** Ollama runs multiple processes (server, CLI, inference engine). We need to identify the inference process specifically for accurate resource monitoring.

**Solution:** Memory-based detection. Inference processes have loaded models (GBs), while server/CLI processes are lightweight (~50-100MB).

**Warmup Request:**

```rust
// src-tauri/src/benchmark/process.rs:69-115

pub async fn warmup_and_find_process(
    model: &str,
    client: &reqwest::Client,
    config: &OllamaConfig,
) -> Result<sysinfo::Pid, String> {
    // Send warmup request to force model loading
    let warmup_prompt = SHORT_PROMPTS.first().copied()
        .unwrap_or("What is a variable in Python?");

    let request = OllamaRequest {
        model: model.to_string(),
        messages: vec![OllamaMessage {
            role: "user".to_string(),
            content: warmup_prompt.to_string(),
        }],
        stream: false,
    };

    let url = format!("{}/api/chat", config.base_url());

    let response = client
        .post(&url)
        .json(&request)
        .timeout(REQUEST_TIMEOUT)  // 300s
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!("Warmup timed out after {REQUEST_TIMEOUT:?} - model may be too large")
            } else {
                format!("Warmup request failed: {e}")
            }
        })?;

    if !response.status().is_success() {
        return Err(format!(
            "Warmup failed with status {}: model '{}' may not be available",
            response.status(), model
        ));
    }

    // Consume response body to ensure model is fully loaded
    let _ = response.bytes().await
        .map_err(|e| format!("Failed to read warmup response: {e}"))?;

    tokio::time::sleep(WARMUP_STABILIZATION_DELAY).await;  // 500ms

    find_inference_process()
}
```

**Process Identification Algorithm:**

```rust
// src-tauri/src/benchmark/process.rs:119-174

fn find_inference_process() -> Result<sysinfo::Pid, String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    // Find all processes with "ollama" in name (case-insensitive)
    let mut candidates: Vec<(sysinfo::Pid, u64, String)> = sys
        .processes()
        .iter()
        .filter_map(|(pid, proc)| {
            let name = proc.name().to_string_lossy();
            if name.to_ascii_lowercase().contains("ollama") {
                Some((*pid, proc.memory(), name.to_string()))
            } else {
                None
            }
        })
        .collect();

    if candidates.is_empty() {
        return Err("No Ollama process found - ensure Ollama is running".to_string());
    }

    log::debug!("Found {} Ollama process(es):", candidates.len());
    for (pid, mem, name) in &candidates {
        log::debug!("  PID {}: {} ({:.1} MB)", pid, name, *mem as f64 / BYTES_PER_MB);
    }

    // Sort by memory descending, select highest
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    let (pid, mem, name) = candidates.first().unwrap();

    // Validate memory threshold (500MB minimum)
    if *mem < INFERENCE_PROCESS_MIN_MEMORY {
        let threshold_mb = INFERENCE_PROCESS_MIN_MEMORY as f64 / BYTES_PER_MB;
        let found_mb = *mem as f64 / BYTES_PER_MB;
        return Err(format!(
            "No inference process found with loaded model. \
             Highest memory process '{name}' has {found_mb:.1} MB, need >{threshold_mb:.0} MB"
        ));
    }

    log::info!("Selected Ollama process: '{}' (PID {}, {:.1} MB)", name, pid, *mem as f64 / BYTES_PER_MB);

    // Warn if multiple high-memory processes detected (uncommon, may indicate issue)
    if candidates.len() > 1 {
        let (_, second_mem, second_name) = &candidates[1];
        let memory_ratio = *mem as f64 / (*second_mem).max(1) as f64;
        if memory_ratio < 2.0 && *second_mem >= INFERENCE_PROCESS_MIN_MEMORY {
            log::warn!(
                "Multiple high-memory Ollama processes detected: '{}' ({:.1} MB) vs '{}' ({:.1} MB)",
                name, *mem as f64 / BYTES_PER_MB, second_name, *second_mem as f64 / BYTES_PER_MB
            );
        }
    }

    Ok(*pid)
}
```

**Why 500MB Threshold:**

- Ollama server: ~50-100MB
- Ollama CLI: ~50-100MB
- Qwen 2.5 Coder 7B (loaded): ~4-8GB
- DeepSeek Coder 6.7B (loaded): ~4-7GB

500MB threshold cleanly separates inference processes from lightweight components.

---

### 3.4 Resource Sampling System (CRITICAL)

This is the **most sophisticated** part of the data collection system. It provides continuous, rigorous resource monitoring during inference.

#### Sampling vs Snapshots

**Problem with Snapshots:**

Before/after memory/CPU snapshots miss transient resource usage:

```
Memory Usage
    │
8GB │         ╱╲  ← Peak spike (missed by snapshots)
    │        ╱  ╲
6GB │       ╱    ╲
    │      ╱      ╲___________
4GB │─────╱                   ╲─────
    │    ▲                     ▲
    │  Before                After
    │  Snapshot              Snapshot
    └─────────────────────────────────→ Time
```

**Solution: Continuous Sampling (50ms intervals):**

```
Memory Usage
    │
8GB │         ╱╲  ← Captured!
    │        • •
6GB │      •     •
    │     •       •___•_•_•_•
4GB │─•──•                    •─•──
    │ ●●●●●●●●●●●●●●●●●●●●●●●●●
    │ Sample points (50ms intervals)
    └─────────────────────────────────→ Time
```

Sampling captures:
- **Peak memory** - Absolute maximum (critical for out-of-memory risks)
- **Median memory** - Robust central tendency (insensitive to outliers)
- **Average CPU** - Overall resource cost across test duration

#### Multi-Process CPU Tracking

**Why Track 3 Separate CPU Metrics?**

The system tracks CPU usage for **three separate entities**:

1. **Ollama process** (`cpu_ollama_percent`) - Inference engine only
2. **Tauri process** (`cpu_tauri_percent`) - HTTP parsing, JSON, async runtime
3. **System-wide** (`cpu_system_percent`) - Context for overall system load
4. **Total** (`cpu_total_percent`) - ollama + tauri (true cost of current architecture)

**Architecture Comparison Strategy:**

Current architecture (Ollama HTTP-based):
```
┌──────────────┐  HTTP   ┌──────────────┐
│    Tauri     │◄───────►│    Ollama    │
│   Process    │  JSON   │   Process    │
│              │         │              │
│ CPU: 15-25%  │         │  CPU: 60-80% │
│ (overhead)   │         │  (inference) │
└──────────────┘         └──────────────┘
      ▲                         ▲
      │                         │
      └─────────┬───────────────┘
                │
        Total CPU: 75-105%
```

Future architecture (llama.cpp in-process):
```
┌────────────────────────┐
│     Tauri Process      │
│                        │
│  HTTP overhead: ~0%    │
│  Inference: 60-80%     │
│                        │
│  Total CPU: 60-80%     │
└────────────────────────┘
```

**Expected Change:** `cpu_tauri_percent` drops from 15-25% to near-zero, proving HTTP overhead elimination.

**This is why separate tracking exists** - to quantify architectural improvements post-migration.

#### Implementation Details

**SamplingState Structure:**

```rust
// src-tauri/src/benchmark/sampling.rs:28-47

#[derive(Debug)]
struct SamplingData {
    cpu_ollama_samples: Vec<f64>,
    cpu_tauri_samples: Vec<f64>,
    cpu_system_samples: Vec<f64>,
    memory_samples: Vec<f64>,
    peak_memory: f64,
    sampling_active: bool,
}

#[derive(Clone)]
pub struct SamplingState {
    inner: Arc<Mutex<SamplingData>>,  // Thread-safe shared state
}

impl SamplingState {
    pub fn new(initial_memory: f64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SamplingData {
                cpu_ollama_samples: Vec::with_capacity(100),  // Pre-allocate
                cpu_tauri_samples: Vec::with_capacity(100),
                cpu_system_samples: Vec::with_capacity(100),
                memory_samples: Vec::with_capacity(100),
                peak_memory: initial_memory,
                sampling_active: true,
            })),
        }
    }
}
```

**Why `std::sync::Mutex` vs `tokio::sync::Mutex`?**

Critical design decision from Tokio documentation:

> "The primary use case for the `std::sync::Mutex` is when the lock needs to be held across an `.await` point. If the lock is not held across an `.await` point, the `std::sync::Mutex` will perform better."

Our usage pattern:

```rust
// src-tauri/src/benchmark/sampling.rs:65-78

pub fn record_sample(&self, ollama_cpu: f64, tauri_cpu: f64, system_cpu: f64, memory: f64) {
    let mut data = self.inner.lock()
        .unwrap_or_else(|poisoned| {
            log::warn!("SamplingState mutex poisoned, recovering data");
            poisoned.into_inner()  // Recover from poison
        });
    data.cpu_ollama_samples.push(ollama_cpu);
    data.cpu_tauri_samples.push(tauri_cpu);
    data.cpu_system_samples.push(system_cpu);
    data.memory_samples.push(memory);
    if memory > data.peak_memory {
        data.peak_memory = memory;
    }
    // Lock released here (before any .await)
}
```

**Lock characteristics:**
- Held for **nanoseconds** (4 vector pushes + 1 comparison)
- **Never** held across `.await` points
- **Zero async overhead** compared to tokio::sync::Mutex

**Poisoned Mutex Recovery:**

Rust mutexes become "poisoned" if a thread panics while holding the lock. We recover gracefully:

```rust
.unwrap_or_else(|poisoned| {
    log::warn!("SamplingState mutex poisoned, recovering data");
    poisoned.into_inner()  // Extract data despite panic
})
```

This ensures benchmark continues even if sampling thread panics (extremely rare).

**Pre-Allocated Vectors (Capacity: 100):**

```rust
Vec::with_capacity(100)
```

**Why 100?**

- Typical test duration: 3-5 seconds
- Sampling interval: 50ms
- Expected samples: 60-100
- Pre-allocation avoids reallocation during sampling (performance + consistency)

#### Background Sampler Task

**Spawn Pattern:**

```rust
// src-tauri/src/benchmark/sampling.rs:128-179

pub fn spawn_resource_sampler(
    ollama_pid: sysinfo::Pid,
    state: SamplingState,
) -> tokio::sync::oneshot::Receiver<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tauri_pid = sysinfo::Pid::from_u32(std::process::id());

    tokio::spawn(async move {
        let mut sys = System::new_all();
        sys.refresh_all();

        // Establish CPU baseline (sysinfo requirement: 2 refreshes)
        sys.refresh_cpu_all();
        tokio::time::sleep(CPU_BASELINE_DELAY).await;  // 200ms
        sys.refresh_cpu_all();

        while state.is_active() {
            sys.refresh_all();
            sys.refresh_cpu_all();

            // Read Ollama process metrics
            let ollama_data = sys.process(ollama_pid).map(|p| {
                (f64::from(p.cpu_usage()), (p.memory() as f64) / BYTES_PER_MB)
            });

            // Read Tauri process CPU
            let tauri_cpu = sys.process(tauri_pid)
                .map_or(0.0, |p| f64::from(p.cpu_usage()));

            // Calculate system-wide average CPU
            let system_cpu = {
                let cpus = sys.cpus();
                if cpus.is_empty() {
                    0.0
                } else {
                    let total: f32 = cpus.iter().map(|c| c.cpu_usage()).sum();
                    f64::from(total / cpus.len() as f32)
                }
            };

            if let Some((ollama_cpu, memory)) = ollama_data {
                state.record_sample(ollama_cpu, tauri_cpu, system_cpu, memory);
            } else {
                log::warn!("Ollama process (PID {ollama_pid}) disappeared during sampling");
                break;
            }

            tokio::time::sleep(SAMPLING_INTERVAL).await;  // 50ms
        }

        let _ = tx.send(());  // Signal completion
    });

    rx  // Return receiver for completion notification
}
```

**CPU Baseline Requirement:**

sysinfo requires **2 refresh cycles** separated by time to calculate accurate CPU percentages:

```rust
sys.refresh_cpu_all();  // First refresh (baseline)
tokio::time::sleep(200ms).await;
sys.refresh_cpu_all();  // Second refresh (delta calculation)
```

Without this, first sample would be 0% (no delta). 200ms delay ensures accurate measurements.

**Sampling Loop:**

```rust
while state.is_active() {
    // 1. Refresh system info
    sys.refresh_all();
    sys.refresh_cpu_all();

    // 2. Read process metrics
    let ollama_data = sys.process(ollama_pid).map(|p| {
        (f64::from(p.cpu_usage()), (p.memory() as f64) / BYTES_PER_MB)
    });

    // 3. Read Tauri CPU
    let tauri_cpu = sys.process(tauri_pid)
        .map_or(0.0, |p| f64::from(p.cpu_usage()));

    // 4. Calculate system-wide average CPU
    let system_cpu = {
        let cpus = sys.cpus();
        let total: f32 = cpus.iter().map(|c| c.cpu_usage()).sum();
        f64::from(total / cpus.len() as f32)
    };

    // 5. Record sample
    state.record_sample(ollama_cpu, tauri_cpu, system_cpu, memory);

    // 6. Sleep 50ms
    tokio::time::sleep(SAMPLING_INTERVAL).await;
}
```

**Process Disappearance Handling:**

```rust
if let Some((ollama_cpu, memory)) = ollama_data {
    state.record_sample(ollama_cpu, tauri_cpu, system_cpu, memory);
} else {
    log::warn!("Ollama process (PID {ollama_pid}) disappeared during sampling");
    break;  // Exit sampling loop gracefully
}
```

If Ollama crashes mid-test, sampler exits gracefully instead of panicking.

**Completion Signaling:**

```rust
let (tx, rx) = tokio::sync::oneshot::channel();

tokio::spawn(async move {
    // ... sampling loop ...
    let _ = tx.send(());  // Signal done
});

rx  // Return receiver to caller
```

Caller awaits `rx` to know when sampler has stopped.

#### Statistical Processing

**Median Calculation (NaN-safe):**

```rust
// src-tauri/src/benchmark/sampling.rs:201-216

pub fn calculate_median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);  // NaN-safe comparison

    let len = sorted.len();
    if len % 2 == 0 {
        (sorted[len / 2 - 1] + sorted[len / 2]) / 2.0
    } else {
        sorted[len / 2]
    }
}
```

**Why `f64::total_cmp` instead of partial_cmp?**

```rust
// partial_cmp fails on NaN:
values.sort_by(|a, b| a.partial_cmp(b).unwrap());  // PANICS if NaN present

// total_cmp handles NaN:
values.sort_by(f64::total_cmp);  // NaNs sorted to end, no panic
```

If sampling produces `NaN` (rare, but possible with divide-by-zero in sysinfo), median calculation doesn't crash.

**Average Calculation:**

```rust
// src-tauri/src/benchmark/sampling.rs:218-224

pub fn calculate_average(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}
```

Simple mean - appropriate for CPU measurements (symmetric distribution).

**Peak Memory Tracking:**

```rust
// Updated on every sample
if memory > data.peak_memory {
    data.peak_memory = memory;
}
```

Tracks absolute maximum across entire test duration.

#### Collection and Results

**Stopping the Sampler:**

```rust
// src-tauri/src/benchmark/sampling.rs:182-195

pub async fn collect_sampling_results(
    state: SamplingState,
    sampling_done: tokio::sync::oneshot::Receiver<()>,
    ollama_pid: sysinfo::Pid,
) -> Result<SamplingResults, String> {
    state.stop();  // Signal sampler to exit
    let _ = sampling_done.await;  // Wait for sampler to finish

    state.into_results().ok_or_else(|| {
        format!(
            "Resource sampling failed: no samples collected for Ollama process (PID {ollama_pid})"
        )
    })
}
```

**Results Extraction:**

```rust
// src-tauri/src/benchmark/sampling.rs:101-119

pub fn into_results(self) -> Option<SamplingResults> {
    let mut data = self.inner.lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if data.cpu_ollama_samples.is_empty() || data.memory_samples.is_empty() {
        return None;  // Sampling failed
    }

    Some(SamplingResults {
        cpu_ollama_samples: std::mem::take(&mut data.cpu_ollama_samples),
        cpu_tauri_samples: std::mem::take(&mut data.cpu_tauri_samples),
        cpu_system_samples: std::mem::take(&mut data.cpu_system_samples),
        memory_samples: std::mem::take(&mut data.memory_samples),
        peak_memory_mb: data.peak_memory,
    })
}
```

`std::mem::take` moves vectors out of mutex without cloning (zero-copy).

---

### 3.5 Timing Metrics Extraction

**Two-Path Strategy:** Prefer Ollama's native nanosecond-precision timing, fallback to client-side estimation.

**TimingSource Enum:**

```rust
// src-tauri/src/benchmark/metrics.rs:4-19

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TimingSource {
    /// Timing data from Ollama's native nanosecond-precision metrics (preferred)
    Native,
    /// Timing data calculated from client-side measurements (fallback)
    Client,
}

impl TimingSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            TimingSource::Native => "native",
            TimingSource::Client => "client",
        }
    }
}
```

**Purpose:** Track data reliability. When analyzing results, `timing_source: "client"` indicates estimated (less accurate) timing.

**Timing Metrics Structure:**

```rust
// src-tauri/src/benchmark/runner.rs:58-66

struct TimingMetrics {
    first_token_latency_ms: f64,
    total_response_time_ms: f64,
    tokens_per_second: f64,
    avg_token_latency_ms: f64,
    response_tokens: usize,
    timing_source: TimingSource,
}
```

**Extraction Function:**

```rust
// src-tauri/src/benchmark/runner.rs:68-122

fn calculate_timing_metrics(response: &OllamaResponse, client_elapsed_ms: f64) -> TimingMetrics {
    // PATH 1: Native Ollama timing (nanosecond precision)
    if let (Some(eval_count), Some(eval_duration_ns), Some(total_duration_ns)) = (
        response.eval_count,
        response.eval_duration,
        response.total_duration,
    ) {
        let eval_duration_ms = (eval_duration_ns as f64) / 1_000_000.0;
        let prompt_eval_ms = response.prompt_eval_duration
            .map_or(0.0, |ns| (ns as f64) / 1_000_000.0);
        let total_duration_ms = (total_duration_ns as f64) / 1_000_000.0;

        let tokens_per_second = if eval_duration_ms > 0.0 {
            (eval_count as f64) / (eval_duration_ms / 1000.0)
        } else {
            0.0
        };

        let avg_token_latency_ms = if eval_count > 0 {
            eval_duration_ms / eval_count as f64
        } else {
            0.0
        };

        return TimingMetrics {
            first_token_latency_ms: prompt_eval_ms,
            total_response_time_ms: total_duration_ms,
            tokens_per_second,
            avg_token_latency_ms,
            response_tokens: eval_count,
            timing_source: TimingSource::Native,
        };
    }

    // PATH 2: Client-side estimation (fallback)
    log::warn!("Ollama did not provide native timing data, using client-side estimates");

    let response_content = response.message.as_ref().map_or("", |m| m.content.as_str());
    let estimated_tokens = (response_content.len() / CHARS_PER_TOKEN_ESTIMATE).max(1);

    let tokens_per_second = if client_elapsed_ms > 0.0 {
        (estimated_tokens as f64) / (client_elapsed_ms / 1000.0)
    } else {
        0.0
    };

    TimingMetrics {
        first_token_latency_ms: 0.0,  // Cannot estimate
        total_response_time_ms: client_elapsed_ms,
        tokens_per_second,
        avg_token_latency_ms: if estimated_tokens > 0 {
            client_elapsed_ms / estimated_tokens as f64
        } else {
            0.0
        },
        response_tokens: estimated_tokens,
        timing_source: TimingSource::Client,
    }
}
```

**Native Path (Preferred):**

Ollama provides these fields in the response JSON:

```json
{
  "eval_count": 153,             // Token count (actual tokenizer)
  "eval_duration": 3456789012,   // Response generation time (nanoseconds)
  "prompt_eval_duration": 123456789,  // Prompt processing time (nanoseconds)
  "total_duration": 3580245801   // Total request time (nanoseconds)
}
```

**Calculations:**

```rust
// Nanoseconds → Milliseconds
let eval_duration_ms = eval_duration_ns / 1_000_000.0;
let prompt_eval_ms = prompt_eval_duration_ns / 1_000_000.0;
let total_duration_ms = total_duration_ns / 1_000_000.0;

// Tokens per second
let tokens_per_second = eval_count / (eval_duration_ms / 1000.0);
// Example: 153 tokens / (3456.78 ms / 1000) = 153 / 3.457 = 44.28 tokens/sec

// Average latency per token
let avg_token_latency_ms = eval_duration_ms / eval_count;
// Example: 3456.78 ms / 153 tokens = 22.60 ms/token

// First token latency = prompt processing time
let first_token_latency_ms = prompt_eval_ms;
```

**Fallback Path (Client-side estimation):**

When Ollama doesn't provide native timing (rare, but possible with old versions or errors):

```rust
// Token estimation: 4 characters ≈ 1 token (rough approximation)
const CHARS_PER_TOKEN_ESTIMATE: usize = 4;

let response_content = "In Python, a variable is a named container...";
let estimated_tokens = response_content.len() / 4;
// Example: 180 chars / 4 = 45 tokens

// Client-side elapsed time (from Instant::now())
let client_elapsed_ms = 3500.0;  // Example

// Tokens per second
let tokens_per_second = 45 / (3500.0 / 1000.0) = 12.86 tokens/sec

// Average token latency
let avg_token_latency_ms = 3500.0 / 45 = 77.78 ms/token

// First token latency: CANNOT estimate (set to 0.0)
```

**Limitations of Fallback:**

1. **Token count** - Character-based estimation is inaccurate (actual tokenizers vary)
2. **First token latency** - Impossible to estimate from end-to-end time
3. **Throughput** - Includes network/JSON overhead, not pure inference speed

**This is why TimingSource tracking is critical** - analysts can filter out less reliable data.

---

### 3.6 Single Test Execution Flow

This brings together all data collection components into the complete test execution pipeline.

**Function Signature:**

```rust
// src-tauri/src/benchmark/runner.rs:159-169

async fn run_single_test(
    prompt: String,
    category: PromptCategory,
    model: String,
    iteration: usize,
    context: Option<Vec<OllamaMessage>>,  // For follow-up prompts
    client: &reqwest::Client,
    config: &OllamaConfig,
    ollama_pid: sysinfo::Pid,
    hardware: &HardwareSnapshot,
) -> Result<(BenchmarkMetrics, String), String>
```

**Step-by-Step Execution:**

**STEP 1: Memory Snapshot (Before)**

```rust
// src-tauri/src/benchmark/runner.rs:170-176

let mut sys = System::new_all();
sys.refresh_all();

let memory_before_mb = sys
    .process(ollama_pid)
    .map(|p| (p.memory() as f64) / BYTES_PER_MB)
    .ok_or_else(|| format!("Ollama process (PID {ollama_pid}) not found"))?;
```

**STEP 2: Build Request**

```rust
// src-tauri/src/benchmark/runner.rs:178-182

let request = OllamaRequest {
    model: model.clone(),
    messages: build_request_messages(&prompt, context),
    stream: false,  // Non-streaming for timing consistency
};
```

```rust
// src-tauri/src/benchmark/runner.rs:128-144

fn build_request_messages(prompt: &str, context: Option<Vec<OllamaMessage>>) -> Vec<OllamaMessage> {
    let mut messages = vec![OllamaMessage {
        role: "system".to_string(),
        content: SYSTEM_PROMPT.to_string(),
    }];

    if let Some(ctx) = context {
        messages.extend(ctx);  // Add follow-up context
    }

    messages.push(OllamaMessage {
        role: "user".to_string(),
        content: prompt.to_string(),
    });

    messages
}
```

**STEP 3: Initialize Sampling**

```rust
// src-tauri/src/benchmark/runner.rs:185-186

let sampling_state = SamplingState::new(memory_before_mb);
let sampling_done = spawn_resource_sampler(ollama_pid, sampling_state.clone());
```

At this point, background sampler is running in parallel, collecting CPU/memory samples every 50ms.

**STEP 4: Execute HTTP Request**

```rust
// src-tauri/src/benchmark/runner.rs:188-221

let url = format!("{}/api/chat", config.base_url());
let request_start = Instant::now();

let response = client
    .post(&url)
    .json(&request)
    .timeout(REQUEST_TIMEOUT)  // 300 seconds
    .send()
    .await
    .map_err(|e| {
        if e.is_timeout() {
            format!("Request timed out after {REQUEST_TIMEOUT:?}")
        } else if e.is_connect() {
            "Failed to connect to Ollama - ensure it's running".to_string()
        } else {
            format!("HTTP request failed: {e}")
        }
    })?;

if !response.status().is_success() {
    return Err(format!(
        "Ollama returned error status {}: {}",
        response.status(),
        response.status().canonical_reason().unwrap_or("Unknown error")
    ));
}

let ollama_response: OllamaResponse = response
    .json()
    .await
    .map_err(|e| format!("Failed to parse response: {e}"))?;

let client_elapsed_ms = request_start.elapsed().as_secs_f64() * 1000.0;
```

**STEP 5: Collect Sampling Results**

```rust
// src-tauri/src/benchmark/runner.rs:224

let sampling_results = collect_sampling_results(sampling_state, sampling_done, ollama_pid).await?;
```

This stops the background sampler and retrieves all collected samples.

**STEP 6: Extract Timing Metrics**

```rust
// src-tauri/src/benchmark/runner.rs:226

let timing = calculate_timing_metrics(&ollama_response, client_elapsed_ms);
```

**STEP 7: Get Response Content**

```rust
// src-tauri/src/benchmark/runner.rs:228-231

let response_content = ollama_response.message
    .as_ref()
    .map(|m| m.content.clone())
    .unwrap_or_default();
```

**STEP 8: Memory Snapshot (After)**

```rust
// src-tauri/src/benchmark/runner.rs:234-238

sys.refresh_all();
let memory_after_mb = sys
    .process(ollama_pid)
    .map(|p| (p.memory() as f64) / BYTES_PER_MB)
    .ok_or_else(|| format!("Ollama process (PID {ollama_pid}) disappeared"))?;
```

**STEP 9: Statistical Aggregation**

```rust
// src-tauri/src/benchmark/runner.rs:241-244

let avg_cpu_ollama = calculate_average(&sampling_results.cpu_ollama_samples);
let avg_cpu_tauri = calculate_average(&sampling_results.cpu_tauri_samples);
let avg_cpu_system = calculate_average(&sampling_results.cpu_system_samples);
let median_memory_during = calculate_median(&sampling_results.memory_samples);
```

**STEP 10: Construct BenchmarkMetrics**

```rust
// src-tauri/src/benchmark/runner.rs:246-275

Ok((
    BenchmarkMetrics {
        // Timing metrics
        first_token_latency_ms: timing.first_token_latency_ms,
        total_response_time_ms: timing.total_response_time_ms,
        tokens_per_second: timing.tokens_per_second,
        avg_token_latency_ms: timing.avg_token_latency_ms,
        timing_source: timing.timing_source,

        // Memory metrics
        memory_before_mb,
        memory_during_mb: median_memory_during,
        memory_after_mb,
        peak_memory_mb: sampling_results.peak_memory_mb,

        // CPU metrics
        cpu_ollama_percent: avg_cpu_ollama,
        cpu_tauri_percent: avg_cpu_tauri,
        cpu_system_percent: avg_cpu_system,
        cpu_total_percent: avg_cpu_ollama + avg_cpu_tauri,

        // Metadata
        model_name: model,
        prompt_type: category.as_str().to_string(),
        prompt,
        response_tokens: timing.response_tokens,
        timestamp: get_timestamp(),
        iteration,

        // Hardware
        cpu_model: hardware.cpu_model.clone(),
        gpu_name: hardware.gpu_name.clone(),
        avx2_supported: hardware.avx2_supported,
        npu_detected: hardware.npu_detected,
        hardware_detection_failed: hardware.detection_failed,
    },
    response_content,
))
```

**Timeline Visualization:**

```
Time →  0ms          200ms         500ms     [HTTP Request]    3500ms      3700ms
        │             │             │              │              │           │
        ├─────────────┼─────────────┼──────────────┼──────────────┼───────────┤
        │             │             │              │              │           │
     Memory      CPU Baseline   Sampler       Request        Response    Memory
     Before      (2 cycles)     Running       Sent           Received    After
        │             │             │              │              │           │
        │             │         ●───●───●───●───●───●───●───●    │           │
        │             │         Samples (50ms intervals)          │           │
        │             │                                           │           │
        └─────────────┴───────────────────────────────────────────┴───────────┘
                                                                  │
                                                            Sampler stopped
                                                            Results collected
```

**Constants:**

```rust
// src-tauri/src/benchmark/runner.rs:19-27

const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
const TEST_STABILIZATION_DELAY: Duration = Duration::from_millis(500);
const BYTES_PER_MB: f64 = 1024.0 * 1024.0;
const CHARS_PER_TOKEN_ESTIMATE: usize = 4;
const CONTEXT_SOURCE_TEST_ID: &str = "short_1";
```

**Error Handling:**

Every step has comprehensive error handling:
- Process not found → descriptive error
- HTTP timeout → specific message about timeout duration
- Connection failure → check Ollama status message
- Non-2xx status → include status code and reason
- JSON parse failure → include parse error details

This ensures users get actionable error messages instead of cryptic panics.

---

## 4. Data Processing

### 4.1 BenchmarkMetrics Structure

The complete data structure for a single test measurement:

```rust
// src-tauri/src/benchmark/metrics.rs:22-110

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetrics {
    // ========== TIMING METRICS (PRIMARY) ==========

    /// Time from request start to first token received (ms)
    /// Measures user-perceived responsiveness
    pub first_token_latency_ms: f64,

    /// Total time to generate complete response (ms)
    pub total_response_time_ms: f64,

    /// Throughput: tokens generated per second
    /// Critical metric for long responses
    pub tokens_per_second: f64,

    /// Average time per token (ms)
    pub avg_token_latency_ms: f64,

    /// Source of timing data ("native" = Ollama's metrics, "client" = client-side fallback)
    pub timing_source: TimingSource,

    // ========== MEMORY METRICS (SECONDARY) ==========

    /// RAM usage before inference started (MB)
    pub memory_before_mb: f64,

    /// RAM usage during inference (MB) - MEDIAN from continuous sampling
    pub memory_during_mb: f64,

    /// RAM usage after inference completed (MB)
    pub memory_after_mb: f64,

    /// Peak RAM usage during test (MB) - ABSOLUTE MAXIMUM from sampling
    pub peak_memory_mb: f64,

    // ========== CPU METRICS (SECONDARY) ==========
    // Multi-process tracking for architecture comparison

    /// Average CPU utilization of the Ollama/inference process during inference (%)
    /// Measures only the inference engine's CPU usage
    pub cpu_ollama_percent: f64,

    /// Average CPU utilization of this Tauri process during inference (%)
    /// Captures HTTP overhead, JSON parsing, and async runtime costs
    /// Will be near-zero after llama.cpp migration (no HTTP overhead)
    pub cpu_tauri_percent: f64,

    /// Average system-wide CPU utilization during inference (%)
    /// Provides context for overall system load
    pub cpu_system_percent: f64,

    /// Combined CPU usage: ollama + tauri processes (%)
    /// True total CPU cost of inference in current architecture
    /// PRIMARY metric for comparing Ollama vs llama.cpp performance
    pub cpu_total_percent: f64,

    // ========== METADATA ==========

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

    // ========== HARDWARE INFORMATION ==========

    /// CPU model/brand
    pub cpu_model: String,

    /// Primary GPU name
    pub gpu_name: String,

    /// Whether AVX2 is supported
    pub avx2_supported: bool,

    /// Whether NPU is detected
    pub npu_detected: bool,

    /// Whether hardware detection failed (metadata may be unreliable)
    pub hardware_detection_failed: bool,
}
```

**Field Count:** 30 fields total
- **Timing:** 5 fields
- **Memory:** 4 fields
- **CPU:** 4 fields
- **Metadata:** 6 fields
- **Hardware:** 5 fields
- **Flags:** 1 field (hardware_detection_failed)

### 4.2 Summary Calculation

Aggregates metrics by category (short, medium, long, follow-up) across all iterations.

**BenchmarkSummary Structure:**

```rust
// src-tauri/src/benchmark/metrics.rs:113-132

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub category: String,
    pub avg_first_token_ms: f64,
    pub avg_tokens_per_sec: f64,
    pub avg_total_time_ms: f64,
    pub avg_memory_mb: f64,

    // CPU summary metrics
    pub avg_cpu_ollama_percent: f64,
    pub avg_cpu_tauri_percent: f64,
    pub avg_cpu_system_percent: f64,
    pub avg_cpu_total_percent: f64,

    pub test_count: usize,
}
```

**Calculation Function:**

```rust
// src-tauri/src/benchmark/metrics.rs:148-190

pub fn calculate_summary(metrics: &[BenchmarkMetrics]) -> Vec<BenchmarkSummary> {
    let categories = ["short", "medium", "long", "follow-up"];
    let mut summaries = Vec::new();

    for category in categories {
        // Filter metrics for this category
        let category_metrics: Vec<_> = metrics
            .iter()
            .filter(|m| m.prompt_type == category)
            .collect();

        if category_metrics.is_empty() {
            continue;  // Skip empty categories
        }

        let count = category_metrics.len();

        // Calculate averages
        let avg_first_token = category_metrics.iter()
            .map(|m| m.first_token_latency_ms).sum::<f64>() / count as f64;
        let avg_tokens_per_sec = category_metrics.iter()
            .map(|m| m.tokens_per_second).sum::<f64>() / count as f64;
        let avg_total_time = category_metrics.iter()
            .map(|m| m.total_response_time_ms).sum::<f64>() / count as f64;
        let avg_memory = category_metrics.iter()
            .map(|m| m.peak_memory_mb).sum::<f64>() / count as f64;

        // Calculate CPU averages
        let avg_cpu_ollama = category_metrics.iter()
            .map(|m| m.cpu_ollama_percent).sum::<f64>() / count as f64;
        let avg_cpu_tauri = category_metrics.iter()
            .map(|m| m.cpu_tauri_percent).sum::<f64>() / count as f64;
        let avg_cpu_system = category_metrics.iter()
            .map(|m| m.cpu_system_percent).sum::<f64>() / count as f64;
        let avg_cpu_total = category_metrics.iter()
            .map(|m| m.cpu_total_percent).sum::<f64>() / count as f64;

        summaries.push(BenchmarkSummary {
            category: category.to_string(),
            avg_first_token_ms: avg_first_token,
            avg_tokens_per_sec,
            avg_total_time_ms: avg_total_time,
            avg_memory_mb: avg_memory,
            avg_cpu_ollama_percent: avg_cpu_ollama,
            avg_cpu_tauri_percent: avg_cpu_tauri,
            avg_cpu_system_percent: avg_cpu_system,
            avg_cpu_total_percent: avg_cpu_total,
            test_count: count,
        });
    }

    summaries
}
```

**Example Output:**

3 iterations × 12 prompts = 36 total metrics

Summary:
- **Short** (3 prompts × 3 iterations = 9 tests): avg_first_token_ms: 120.5, avg_tokens_per_sec: 45.2
- **Medium** (9 tests): avg_first_token_ms: 135.8, avg_tokens_per_sec: 42.1
- **Long** (9 tests): avg_first_token_ms: 145.3, avg_tokens_per_sec: 40.8
- **Follow-up** (9 tests): avg_first_token_ms: 125.9, avg_tokens_per_sec: 44.3

### 4.3 Complete Results Aggregation

**BenchmarkResults Structure:**

```rust
// src-tauri/src/benchmark/metrics.rs:135-141

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    pub metrics: Vec<BenchmarkMetrics>,  // All individual test results
    pub summary: Vec<BenchmarkSummary>,  // Category aggregations
    pub total_duration_seconds: f64,     // Entire benchmark suite duration
    pub timestamp: String,               // ISO 8601 timestamp
}
```

**Construction:**

```rust
// src-tauri/src/benchmark/runner.rs:345-352

let summary = calculate_summary(&all_metrics);

Ok(BenchmarkResults {
    metrics: all_metrics,
    summary,
    total_duration_seconds: suite_start.elapsed().as_secs_f64(),
    timestamp: get_timestamp(),
})
```

**Timestamp Helper:**

```rust
// src-tauri/src/benchmark/metrics.rs:143-146

pub fn get_timestamp() -> String {
    chrono::Local::now().to_rfc3339()
    // Example: "2025-01-28T14:30:45.123456789+00:00"
}
```

---

## 5. Export System

### 5.1 Directory Resolution

**Platform-Specific Paths:**

The benchmark system uses Tauri's app data directory for stable, OS-appropriate storage:

```rust
// src-tauri/src/benchmark/export.rs:70-89

pub fn get_benchmarks_dir_with_app_handle(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;

    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;

    let benchmarks_dir = app_data_dir.join("benchmarks");

    // Create directory if it doesn't exist
    if !benchmarks_dir.exists() {
        fs::create_dir_all(&benchmarks_dir)
            .map_err(|e| format!("Failed to create benchmarks directory: {e}"))?;
    }

    Ok(benchmarks_dir)
}
```

**Resolved Paths:**

- **macOS:** `~/Library/Application Support/com.smolpc.codehelper/benchmarks/`
- **Windows:** `%APPDATA%\com.smolpc.codehelper\benchmarks\`
- **Linux:** `~/.local/share/com.smolpc.codehelper/benchmarks/`

**Why Not Current Working Directory?**

```rust
// Deprecated legacy function
#[deprecated(note = "Use get_benchmarks_dir_with_app_handle for stable paths")]
pub fn get_benchmarks_dir() -> Result<PathBuf, String> {
    let current_dir = std::env::current_dir()?;  // UNSTABLE - can change!
    let benchmarks_dir = current_dir.join("benchmarks");
    // ...
}
```

Problems with CWD:
- Changes based on how app is launched
- Unreliable in production builds
- Platform-specific quirks

AppHandle path is stable across all launch methods.

### 5.2 CSV Generation

**Crash Safety Pattern:**

Benchmarks can run for 10+ minutes. If the app crashes mid-benchmark, data is lost unless periodically flushed.

**Solution: Flush Every 10 Rows**

```rust
// src-tauri/src/benchmark/export.rs:10
const FLUSH_INTERVAL: usize = 10;

// src-tauri/src/benchmark/export.rs:132-142
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
wtr.flush().map_err(|e| format!("Failed to flush CSV writer: {e}"))?;
```

**Result:** If app crashes at test 25, rows 1-20 are safely written to disk (flushed at 10, 20). Only 21-25 are lost.

**Serde Auto-Column Management:**

**Problem:** Manual CSV writing is error-prone:
```rust
// Manual approach (BAD):
writeln!(file, "timestamp,iteration,category,model,...")?;
writeln!(file, "{},{},{},{},..."" , m.timestamp, m.iteration, m.prompt_type, m.model)?;
// Easy to mismatch columns!
```

**Solution: Struct Field Order = Column Order**

```rust
// src-tauri/src/benchmark/export.rs:15-39

#[derive(Debug, Serialize)]
struct CsvMetricRow {
    timestamp: String,
    iteration: usize,
    category: String,
    model: String,
    first_token_ms: String,  // Formatted to 2 decimals
    total_time_ms: String,
    tokens_per_sec: String,
    avg_token_ms: String,
    timing_source: String,
    memory_before_mb: String,
    memory_peak_mb: String,
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
```

**Serde automatically:**
1. Generates CSV header from field names
2. Serializes fields in declaration order
3. Prevents column mismatches

**2-Decimal Formatting:**

```rust
// src-tauri/src/benchmark/export.rs:41-68

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
            cpu_ollama_percent: format!("{:.2}", metric.cpu_ollama_percent),
            cpu_tauri_percent: format!("{:.2}", metric.cpu_tauri_percent),
            cpu_total_percent: format!("{:.2}", metric.cpu_total_percent),
            cpu_system_percent: format!("{:.2}", metric.cpu_system_percent),
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
```

All floats formatted to 2 decimal places for CSV readability.

**Filename Generation:**

```rust
// src-tauri/src/benchmark/export.rs:110-113

pub fn generate_filename(prefix: &str) -> String {
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    format!("{prefix}-{timestamp}.csv")
    // Example: "benchmark-2025-01-28_14-30-45.csv"
}
```

**Complete Export Function:**

```rust
// src-tauri/src/benchmark/export.rs:116-149

pub fn export_to_csv(
    results: &BenchmarkResults,
    prefix: &str,
    app_handle: &tauri::AppHandle,
) -> Result<PathBuf, String> {
    let benchmarks_dir = get_benchmarks_dir_with_app_handle(app_handle)?;
    let filename = generate_filename(prefix);
    let filepath = benchmarks_dir.join(&filename);

    // Create CSV writer
    let mut wtr = Writer::from_path(&filepath)
        .map_err(|e| format!("Failed to create CSV file: {e}"))?;

    // Write all metrics using serde serialization (automatic headers and column ordering)
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
```

### 5.3 README Generation

**Purpose:** Document CSV format and methodology for users who analyze benchmark data in Excel/Python/R.

```rust
// src-tauri/src/benchmark/export.rs:152-228

pub fn create_readme(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let benchmarks_dir = get_benchmarks_dir_with_app_handle(app_handle)?;
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
```

**Known Issue:** README mentions "streaming" but code uses non-streaming (`stream: false`). This is outdated documentation from earlier implementation.

---
