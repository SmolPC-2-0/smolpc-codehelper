//! Benchmarking module for KV Cache and inference performance
//!
//! Run benchmarks with:
//! ```bash
//! cd src-tauri
//! cargo test bench_ -- --ignored --nocapture
//! ```

use super::kv_cache::{KVCache, HEAD_DIM, NUM_KV_HEADS, NUM_LAYERS};
use super::session::InferenceSession;
use super::tokenizer::TokenizerWrapper;
use super::types::GenerationConfig;
use super::Generator;
use ndarray::{Array2, Array4};
use ort::session::SessionInputValue;
use ort::value::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Benchmark results for a generation run
#[derive(Debug)]
pub struct BenchmarkResult {
    pub name: String,
    pub prompt_tokens: usize,
    pub generated_tokens: usize,
    pub prefill_time_ms: f64,
    pub decode_time_ms: f64,
    pub total_time_ms: f64,
    pub time_to_first_token_ms: f64,
    pub tokens_per_second: f64,
    pub decode_tokens_per_second: f64,
    pub memory_usage_mb: f64,
}

impl BenchmarkResult {
    pub fn print_report(&self) {
        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║ BENCHMARK: {:^49} ║", self.name);
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║ Prompt tokens:         {:>8}                              ║", self.prompt_tokens);
        println!("║ Generated tokens:      {:>8}                              ║", self.generated_tokens);
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║ Prefill time:          {:>8.2} ms                          ║", self.prefill_time_ms);
        println!("║ Decode time:           {:>8.2} ms                          ║", self.decode_time_ms);
        println!("║ Total time:            {:>8.2} ms                          ║", self.total_time_ms);
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║ Time to first token:   {:>8.2} ms                          ║", self.time_to_first_token_ms);
        println!("║ Overall tok/s:         {:>8.2}                              ║", self.tokens_per_second);
        println!("║ Decode tok/s:          {:>8.2} (excludes prefill)          ║", self.decode_tokens_per_second);
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║ KV Cache memory:       {:>8.2} MB                           ║", self.memory_usage_mb);
        println!("╚══════════════════════════════════════════════════════════════╝\n");
    }
}

/// Benchmark KV cache operations (no model, pure cache performance)
pub fn bench_kv_cache_operations() -> Vec<(String, Duration)> {
    let mut results = Vec::new();
    let max_context = 2048;
    let sink_size = 4;

    // 1. Cache creation
    let start = Instant::now();
    let mut cache = KVCache::new(max_context, sink_size);
    results.push(("Cache creation (2048 ctx)".to_string(), start.elapsed()));

    // 2. Single append operation
    let key_emb = vec![1.0f32; NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM];
    let val_emb = vec![2.0f32; NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM];

    let start = Instant::now();
    for _ in 0..100 {
        cache.append(&key_emb, &val_emb);
    }
    let append_100 = start.elapsed();
    results.push(("100 appends".to_string(), append_100));

    // 3. to_array() calls (the expensive operation)
    let start = Instant::now();
    for layer in 0..NUM_LAYERS {
        let _k = cache.get_key_array(layer);
        let _v = cache.get_value_array(layer);
    }
    results.push(("56 to_array() calls".to_string(), start.elapsed()));

    // 4. Fill cache to trigger shifts
    cache.clear();
    for _ in 0..max_context {
        cache.append(&key_emb, &val_emb);
    }

    // 5. Append with shift (cache full)
    let start = Instant::now();
    for _ in 0..100 {
        cache.append(&key_emb, &val_emb);
    }
    let shift_100 = start.elapsed();
    results.push(("100 appends with shift".to_string(), shift_100));

    // 6. Memory usage
    let memory_mb = cache.memory_usage_bytes() as f64 / (1024.0 * 1024.0);
    println!("KV Cache memory: {:.2} MB", memory_mb);

    results
}

/// Benchmark decode step overhead (excluding ONNX inference)
pub fn bench_decode_overhead(cache: &KVCache) -> Duration {
    let start = Instant::now();

    // Simulate what run_decode does before and after session.run()
    let mut inputs: HashMap<String, Array4<f32>> = HashMap::new();

    // Build KV cache inputs (this is the expensive part)
    for layer in 0..NUM_LAYERS {
        let key_cache = cache.get_key_array(layer);
        let value_cache = cache.get_value_array(layer);

        inputs.insert(format!("past_key_values.{}.key", layer), key_cache);
        inputs.insert(format!("past_key_values.{}.value", layer), value_cache);
    }

    start.elapsed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::init_onnx_runtime;

    /// Benchmark pure KV cache operations (no model needed)
    #[test]
    fn bench_kv_cache_pure() {
        println!("\n=== KV Cache Pure Operation Benchmarks ===\n");

        let results = bench_kv_cache_operations();

        for (name, duration) in &results {
            println!("{:35} {:>10.3} ms", name, duration.as_secs_f64() * 1000.0);
        }

        println!("\n=== Per-Operation Analysis ===\n");

        // Calculate per-op times
        if let Some((_, append_100)) = results.iter().find(|(n, _)| n.contains("100 appends") && !n.contains("shift")) {
            let per_append_us = append_100.as_micros() as f64 / 100.0;
            println!("Single append (no shift):    {:>8.2} µs", per_append_us);
        }

        if let Some((_, shift_100)) = results.iter().find(|(n, _)| n.contains("shift")) {
            let per_shift_us = shift_100.as_micros() as f64 / 100.0;
            println!("Single append (with shift):  {:>8.2} µs", per_shift_us);
        }

        if let Some((_, to_array)) = results.iter().find(|(n, _)| n.contains("to_array")) {
            let per_layer_us = to_array.as_micros() as f64 / 56.0;
            println!("Single to_array():           {:>8.2} µs", per_layer_us);
            println!("56× to_array() total:        {:>8.2} ms", to_array.as_secs_f64() * 1000.0);
        }
    }

    /// Benchmark decode step overhead (builds inputs HashMap)
    #[test]
    fn bench_decode_step_overhead() {
        println!("\n=== Decode Step Overhead Benchmark ===\n");

        // Create cache with some data
        let mut cache = KVCache::new(512, 4);
        let key_emb = vec![1.0f32; NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM];
        let val_emb = vec![2.0f32; NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM];

        // Fill with 100 tokens
        for _ in 0..100 {
            cache.append(&key_emb, &val_emb);
        }

        // Warm up
        for _ in 0..5 {
            let _ = bench_decode_overhead(&cache);
        }

        // Measure multiple runs
        let mut times = Vec::new();
        for _ in 0..20 {
            times.push(bench_decode_overhead(&cache));
        }

        let avg_ms = times.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / times.len() as f64;
        let min_ms = times.iter().map(|d| d.as_secs_f64() * 1000.0).fold(f64::INFINITY, f64::min);
        let max_ms = times.iter().map(|d| d.as_secs_f64() * 1000.0).fold(f64::NEG_INFINITY, f64::max);

        println!("Cache size: {} tokens", cache.physical_length());
        println!("Overhead per decode step:");
        println!("  Average: {:>8.3} ms", avg_ms);
        println!("  Min:     {:>8.3} ms", min_ms);
        println!("  Max:     {:>8.3} ms", max_ms);
        println!("\nThis is the time spent building inputs BEFORE session.run()");
        println!("Does NOT include ONNX inference time.");
    }

    /// Full end-to-end benchmark with model
    #[tokio::test]
    #[ignore] // Requires model files
    async fn bench_full_generation() {
        println!("\n=== Full Generation Benchmark (with KV Cache) ===\n");

        init_onnx_runtime(None).expect("Failed to init ONNX Runtime");

        let model_path = "models/qwen2.5-coder-1.5b/model.onnx";
        let tokenizer_path = "models/qwen2.5-coder-1.5b/tokenizer.json";

        let session = InferenceSession::new(model_path).expect("Failed to load model");
        let tokenizer = TokenizerWrapper::from_file(tokenizer_path).expect("Failed to load tokenizer");

        let generator = Generator::with_context(session, tokenizer, 2048, 4);
        let cancelled = Arc::new(AtomicBool::new(false));

        // Test prompts of varying lengths
        let prompts = vec![
            ("Short prompt", "def hello"),
            ("Medium prompt", "Write a Python function that calculates the factorial of a number:"),
            ("Long prompt", "You are a helpful coding assistant. Write a comprehensive Python class that implements a binary search tree with insert, delete, search, and traversal methods. Include proper error handling and documentation:"),
        ];

        for (name, prompt) in prompts {
            let config = GenerationConfig {
                max_length: 100,
                temperature: 0.0, // Greedy for reproducibility
                top_k: None,
                top_p: None,
            };

            let prompt_tokens = generator.tokenizer().encode(prompt, true).unwrap().len();

            let start = Instant::now();
            let mut token_count = 0;
            let decode_start: Arc<std::sync::Mutex<Option<Instant>>> = Arc::new(std::sync::Mutex::new(None));
            let decode_start_clone = decode_start.clone();

            let metrics = generator.generate_stream(
                prompt,
                Some(config),
                cancelled.clone(),
                |_token| {
                    if token_count == 0 {
                        let mut ds = decode_start_clone.lock().unwrap();
                        *ds = Some(Instant::now());
                    }
                    token_count += 1;
                }
            ).await.expect("Generation failed");

            let total_time = start.elapsed();
            let decode_time = decode_start.lock().unwrap()
                .map(|s| Instant::now().duration_since(s))
                .unwrap_or(Duration::ZERO);
            let prefill_time = metrics.time_to_first_token_ms.unwrap_or(0) as f64;

            let result = BenchmarkResult {
                name: name.to_string(),
                prompt_tokens,
                generated_tokens: metrics.total_tokens,
                prefill_time_ms: prefill_time,
                decode_time_ms: (metrics.total_time_ms as f64) - prefill_time,
                total_time_ms: metrics.total_time_ms as f64,
                time_to_first_token_ms: prefill_time,
                tokens_per_second: metrics.tokens_per_second,
                decode_tokens_per_second: if decode_time.as_secs_f64() > 0.0 {
                    (metrics.total_tokens - 1) as f64 / decode_time.as_secs_f64()
                } else {
                    0.0
                },
                memory_usage_mb: (2 * NUM_LAYERS * NUM_KV_HEADS * 2048 * HEAD_DIM * 4) as f64 / (1024.0 * 1024.0),
            };

            result.print_report();
        }
    }

    /// Compare performance at different cache fill levels
    #[tokio::test]
    #[ignore] // Requires model files
    async fn bench_cache_fill_levels() {
        println!("\n=== Cache Fill Level Comparison ===\n");

        init_onnx_runtime(None).expect("Failed to init ONNX Runtime");

        let model_path = "models/qwen2.5-coder-1.5b/model.onnx";
        let tokenizer_path = "models/qwen2.5-coder-1.5b/tokenizer.json";

        // Test with different context sizes to see cache overhead scaling
        let context_sizes = vec![128, 256, 512, 1024, 2048];

        for ctx_size in context_sizes {
            let session = InferenceSession::new(model_path).expect("Failed to load model");
            let tokenizer = TokenizerWrapper::from_file(tokenizer_path).expect("Failed to load tokenizer");

            let generator = Generator::with_context(session, tokenizer, ctx_size, 4);
            let cancelled = Arc::new(AtomicBool::new(false));

            let config = GenerationConfig {
                max_length: 50,
                temperature: 0.0,
                top_k: None,
                top_p: None,
            };

            let metrics = generator.generate_stream(
                "def factorial(n):",
                Some(config),
                cancelled.clone(),
                |_| {}
            ).await.expect("Generation failed");

            println!("Context size: {:>5} | Tokens: {:>3} | tok/s: {:>6.2} | TTFT: {:>6}ms",
                ctx_size,
                metrics.total_tokens,
                metrics.tokens_per_second,
                metrics.time_to_first_token_ms.unwrap_or(0)
            );
        }
    }

    /// Benchmark to compare single token inference time with/without cache data
    #[tokio::test]
    #[ignore] // Requires model files
    async fn bench_single_decode_timing() {
        println!("\n=== Single Decode Step Timing Analysis ===\n");
        println!("This measures where time is spent in each decode step.\n");

        init_onnx_runtime(None).expect("Failed to init ONNX Runtime");

        let model_path = "models/qwen2.5-coder-1.5b/model.onnx";
        let tokenizer_path = "models/qwen2.5-coder-1.5b/tokenizer.json";

        let session = InferenceSession::new(model_path).expect("Failed to load model");
        let tokenizer = TokenizerWrapper::from_file(tokenizer_path).expect("Failed to load tokenizer");

        let generator = Generator::with_context(session, tokenizer, 512, 4);
        let cancelled = Arc::new(AtomicBool::new(false));

        // Generate tokens and track per-token timing
        let mut token_times = Vec::new();
        let mut last_time = Instant::now();

        let config = GenerationConfig {
            max_length: 30,
            temperature: 0.0,
            top_k: None,
            top_p: None,
        };

        let _ = generator.generate_stream(
            "def fibonacci(n):",
            Some(config),
            cancelled,
            |_token| {
                let now = Instant::now();
                token_times.push(now.duration_since(last_time));
                last_time = now;
            }
        ).await;

        println!("Token-by-token timing (first 20 tokens):");
        println!("{:>5} {:>10}", "Token", "Time (ms)");
        println!("{:-<17}", "");

        for (i, time) in token_times.iter().take(20).enumerate() {
            let marker = if i == 0 { " (TTFT)" } else { "" };
            println!("{:>5} {:>10.2}{}", i + 1, time.as_secs_f64() * 1000.0, marker);
        }

        if token_times.len() > 1 {
            let decode_times: Vec<_> = token_times.iter().skip(1).collect();
            let avg_decode = decode_times.iter()
                .map(|d| d.as_secs_f64() * 1000.0)
                .sum::<f64>() / decode_times.len() as f64;
            let min_decode = decode_times.iter()
                .map(|d| d.as_secs_f64() * 1000.0)
                .fold(f64::INFINITY, f64::min);
            let max_decode = decode_times.iter()
                .map(|d| d.as_secs_f64() * 1000.0)
                .fold(f64::NEG_INFINITY, f64::max);

            println!("\n=== Decode Statistics (excluding TTFT) ===");
            println!("Average: {:>8.2} ms/token", avg_decode);
            println!("Min:     {:>8.2} ms/token", min_decode);
            println!("Max:     {:>8.2} ms/token", max_decode);
            println!("Implied tok/s: {:.2}", 1000.0 / avg_decode);
        }
    }
}
