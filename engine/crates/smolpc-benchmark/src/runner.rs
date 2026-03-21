use anyhow::{Context, Result};
use smolpc_engine_client::{EngineChatMessage, EngineClient};
use smolpc_engine_core::GenerationConfig;

use crate::config::{BenchmarkBackend, BenchmarkConfig, BENCH_REPETITION_PENALTY, BENCH_TEMPERATURE};
use crate::engine_lifecycle;
use crate::memory::MemorySampler;
use crate::output::{BackendModelResult, PromptResult, PromptStats, RunMetrics, SkippedCombo};
use crate::prompts::{BenchmarkPrompt, PROMPTS};
use crate::stats::compute_stats;

/// Run the full benchmark for one backend × model combo.
/// Returns `Ok(result)` on success, `Err` on unrecoverable failure.
pub async fn run_combo(
    client: &EngineClient,
    backend: BenchmarkBackend,
    model_id: &str,
    config: &BenchmarkConfig,
) -> Result<BackendModelResult> {
    // Load model
    engine_lifecycle::load_and_wait(client, model_id).await?;

    // Verify backend
    engine_lifecycle::verify_backend(client, backend).await?;

    let engine_pid = engine_lifecycle::engine_pid(client).await?;

    // Determine warmup count (NPU always >= 3)
    let warmup_count = if backend.is_npu() {
        config.warmup.max(3)
    } else {
        config.warmup
    };

    // --- Warmup ---
    let warmup_prompt = &PROMPTS[0];
    let warmup_messages = make_messages(warmup_prompt);
    let warmup_gen_config = make_gen_config(warmup_prompt);
    let mut cold_start_ttft_ms: Option<u64> = None;

    for i in 0..warmup_count {
        log::info!(
            "  Warmup {}/{} (backend={backend}, model={model_id})",
            i + 1,
            warmup_count,
        );
        let metrics = client
            .generate_stream_messages(&warmup_messages, Some(warmup_gen_config.clone()), |_| {})
            .await
            .context("warmup generation failed")?;

        if i == 0 {
            cold_start_ttft_ms = metrics.time_to_first_token_ms;
            log::info!("  Cold-start TTFT: {cold_start_ttft_ms:?} ms");
        }
    }

    // --- Measured runs ---
    let mut prompt_results = Vec::with_capacity(PROMPTS.len());

    for prompt in PROMPTS {
        log::info!(
            "  Prompt '{}' ({:?}, max_tokens={})",
            prompt.id,
            prompt.tier,
            prompt.tier.max_tokens(),
        );

        let messages = make_messages(prompt);
        let gen_config = make_gen_config(prompt);
        let mut runs = Vec::with_capacity(config.runs);

        for run_idx in 0..config.runs {
            let sampler = MemorySampler::start(engine_pid);

            let metrics = client
                .generate_stream_messages(&messages, Some(gen_config.clone()), |_| {})
                .await
                .with_context(|| {
                    format!(
                        "generation failed: prompt={} run={}/{}",
                        prompt.id,
                        run_idx + 1,
                        config.runs
                    )
                })?;

            let peak_bytes = sampler.stop().await;
            let peak_mb = peak_bytes as f64 / (1024.0 * 1024.0);

            // Derive TPOT: (total_time - ttft) / (tokens - 1)
            let tpot_ms = match (metrics.time_to_first_token_ms, metrics.total_tokens) {
                (Some(ttft), n) if n > 1 => {
                    Some((metrics.total_time_ms as f64 - ttft as f64) / (n as f64 - 1.0))
                }
                _ => None,
            };

            runs.push(RunMetrics {
                ttft_ms: metrics.time_to_first_token_ms,
                tokens_per_second: metrics.tokens_per_second,
                tpot_ms,
                total_time_ms: metrics.total_time_ms,
                total_tokens: metrics.total_tokens,
                peak_memory_mb: peak_mb,
                truncated: metrics.truncated,
            });

            log::debug!(
                "    Run {}/{}: ttft={:?}ms tok/s={:.1} tokens={} time={}ms mem={:.0}MB",
                run_idx + 1,
                config.runs,
                metrics.time_to_first_token_ms,
                metrics.tokens_per_second,
                metrics.total_tokens,
                metrics.total_time_ms,
                peak_mb,
            );
        }

        let stats = compute_prompt_stats(&runs);

        log::info!(
            "    Median: ttft={} tok/s={} tpot={}",
            stats
                .ttft
                .as_ref()
                .map(|s| format!("{:.0}ms", s.median))
                .unwrap_or_else(|| "N/A".into()),
            stats
                .tokens_per_second
                .as_ref()
                .map(|s| format!("{:.1}", s.median))
                .unwrap_or_else(|| "N/A".into()),
            stats
                .tpot
                .as_ref()
                .map(|s| format!("{:.1}ms", s.median))
                .unwrap_or_else(|| "N/A".into()),
        );

        prompt_results.push(PromptResult {
            prompt_id: prompt.id.to_string(),
            tier: prompt.tier,
            max_tokens: prompt.tier.max_tokens(),
            runs,
            stats,
        });
    }

    Ok(BackendModelResult {
        backend,
        model_id: model_id.to_string(),
        cold_start_ttft_ms,
        prompts: prompt_results,
    })
}

/// Try to run a combo, returning a skip record on failure instead of propagating.
pub async fn try_run_combo(
    client: &EngineClient,
    backend: BenchmarkBackend,
    model_id: &str,
    config: &BenchmarkConfig,
) -> Result<BackendModelResult, SkippedCombo> {
    match run_combo(client, backend, model_id, config).await {
        Ok(result) => Ok(result),
        Err(err) => {
            log::error!("Combo {backend}/{model_id} failed: {err:#}");
            Err(SkippedCombo {
                backend,
                model_id: model_id.to_string(),
                reason: format!("{err:#}"),
            })
        }
    }
}

fn make_messages(prompt: &BenchmarkPrompt) -> Vec<EngineChatMessage> {
    vec![EngineChatMessage {
        role: "user".to_string(),
        content: prompt.content.to_string(),
    }]
}

fn make_gen_config(prompt: &BenchmarkPrompt) -> GenerationConfig {
    GenerationConfig {
        max_length: prompt.tier.max_tokens(),
        temperature: BENCH_TEMPERATURE,
        top_k: None,
        top_p: None,
        repetition_penalty: BENCH_REPETITION_PENALTY,
        repetition_penalty_last_n: 0,
    }
}

fn compute_prompt_stats(runs: &[RunMetrics]) -> PromptStats {
    let ttft_vals: Vec<f64> = runs
        .iter()
        .filter_map(|r| r.ttft_ms.map(|v| v as f64))
        .collect();
    let tps_vals: Vec<f64> = runs.iter().map(|r| r.tokens_per_second).collect();
    let tpot_vals: Vec<f64> = runs.iter().filter_map(|r| r.tpot_ms).collect();
    let time_vals: Vec<f64> = runs.iter().map(|r| r.total_time_ms as f64).collect();
    let mem_vals: Vec<f64> = runs.iter().map(|r| r.peak_memory_mb).collect();

    PromptStats {
        ttft: compute_stats(&ttft_vals),
        tokens_per_second: compute_stats(&tps_vals),
        tpot: compute_stats(&tpot_vals),
        total_time: compute_stats(&time_vals),
        peak_memory_mb: compute_stats(&mem_vals),
    }
}
