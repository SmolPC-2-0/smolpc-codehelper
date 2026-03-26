use anyhow::{Context, Result};
use smolpc_engine_client::{EngineChatMessage, EngineClient};
use smolpc_engine_core::GenerationConfig;

use crate::config::{BenchmarkBackend, BenchmarkConfig, BENCH_REPETITION_PENALTY, BENCH_TEMPERATURE};
use crate::engine_lifecycle;
use crate::output::{BackendModelResult, PromptResult, PromptStats, RunMetrics, SkippedCombo};
use crate::prompts::{BenchmarkPrompt, PROMPTS};
use crate::reliability::{self, RunOutcome, StopReason};
use crate::resource_sampler::{IdleBaseline, ResourceSampler};
use crate::stats::compute_stats;

/// Maximum consecutive failures before bailing out of a combo.
const MAX_CONSECUTIVE_FAILURES: usize = 3;

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

    // --- Capture idle baseline ---
    log::info!("  Capturing idle baseline...");
    let idle_baseline = IdleBaseline::capture(engine_pid).await;
    log::info!(
        "  Idle baseline: RSS={:.0}MB CPU={:.1}%",
        idle_baseline.rss_mb,
        idle_baseline.cpu_percent
    );

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
    let mut all_outcomes: Vec<RunOutcome> = Vec::new();

    for prompt in PROMPTS {
        log::info!(
            "  Prompt '{}' ({:?}, max_tokens={})",
            prompt.id,
            prompt.tier,
            prompt.tier.max_tokens(),
        );

        let messages = make_messages(prompt);
        let gen_config = make_gen_config(prompt);
        let max_tokens = prompt.tier.max_tokens();
        let mut runs = Vec::with_capacity(config.runs);
        let mut consecutive_failures: usize = 0;

        for run_idx in 0..config.runs {
            let sampler = ResourceSampler::start(engine_pid);

            let result = client
                .generate_stream_messages(&messages, Some(gen_config.clone()), |_| {})
                .await;

            let snapshot = sampler.stop().await;

            match result {
                Ok(metrics) => {
                    consecutive_failures = 0;
                    let peak_mb = snapshot.peak_rss_bytes as f64 / (1024.0 * 1024.0);

                    // Derive TPOT: (total_time - ttft) / (tokens - 1)
                    let tpot_ms = match (metrics.time_to_first_token_ms, metrics.total_tokens) {
                        (Some(ttft), n) if n > 1 => {
                            Some((metrics.total_time_ms as f64 - ttft as f64) / (n as f64 - 1.0))
                        }
                        _ => None,
                    };

                    // Determine stop reason
                    let stop_reason = if metrics.truncated {
                        StopReason::MaxTokens
                    } else if metrics.total_tokens == max_tokens {
                        StopReason::MaxTokensExact
                    } else {
                        StopReason::NaturalEos
                    };

                    let run_metrics = RunMetrics {
                        ttft_ms: metrics.time_to_first_token_ms,
                        tokens_per_second: metrics.tokens_per_second,
                        tpot_ms,
                        total_time_ms: metrics.total_time_ms,
                        total_tokens: metrics.total_tokens,
                        peak_memory_mb: peak_mb,
                        mean_cpu_percent: Some(snapshot.mean_cpu_percent),
                        peak_cpu_percent: Some(snapshot.peak_cpu_percent),
                        stop_reason,
                        error: None,
                        truncated: metrics.truncated,
                    };

                    log::debug!(
                        "    Run {}/{}: ttft={:?}ms tok/s={:.1} tokens={} time={}ms mem={:.0}MB cpu={:.1}% stop={:?}",
                        run_idx + 1,
                        config.runs,
                        metrics.time_to_first_token_ms,
                        metrics.tokens_per_second,
                        metrics.total_tokens,
                        metrics.total_time_ms,
                        peak_mb,
                        snapshot.mean_cpu_percent,
                        stop_reason,
                    );

                    all_outcomes.push(RunOutcome::Success);
                    runs.push(run_metrics);
                }
                Err(err) => {
                    consecutive_failures += 1;
                    let error_msg = format!("{err:#}");
                    log::warn!(
                        "    Run {}/{} FAILED (consecutive: {}): {}",
                        run_idx + 1,
                        config.runs,
                        consecutive_failures,
                        error_msg,
                    );

                    all_outcomes.push(RunOutcome::Error {
                        message: error_msg.clone(),
                    });

                    runs.push(RunMetrics {
                        ttft_ms: None,
                        tokens_per_second: 0.0,
                        tpot_ms: None,
                        total_time_ms: 0,
                        total_tokens: 0,
                        peak_memory_mb: snapshot.peak_rss_bytes as f64 / (1024.0 * 1024.0),
                        mean_cpu_percent: Some(snapshot.mean_cpu_percent),
                        peak_cpu_percent: Some(snapshot.peak_cpu_percent),
                        stop_reason: StopReason::Error,
                        error: Some(error_msg),
                        truncated: false,
                    });

                    if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        log::error!(
                            "    Bailing out after {} consecutive failures",
                            MAX_CONSECUTIVE_FAILURES,
                        );
                        // Record remaining runs as errors
                        for remaining in (run_idx + 1)..config.runs {
                            let _ = remaining; // suppress unused warning
                            all_outcomes.push(RunOutcome::Error {
                                message: "skipped after consecutive failures".to_string(),
                            });
                        }
                        break;
                    }
                }
            }
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
            max_tokens,
            runs,
            stats,
        });
    }

    let combo_reliability = reliability::aggregate(&all_outcomes, &prompt_results);

    Ok(BackendModelResult {
        backend,
        model_id: model_id.to_string(),
        cold_start_ttft_ms,
        idle_baseline: Some(idle_baseline),
        reliability: combo_reliability,
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
    // Only include successful runs in performance stats
    let successful: Vec<&RunMetrics> = runs
        .iter()
        .filter(|r| r.error.is_none())
        .collect();

    let ttft_vals: Vec<f64> = successful
        .iter()
        .filter_map(|r| r.ttft_ms.map(|v| v as f64))
        .collect();
    let tps_vals: Vec<f64> = successful.iter().map(|r| r.tokens_per_second).collect();
    let tpot_vals: Vec<f64> = successful.iter().filter_map(|r| r.tpot_ms).collect();
    let time_vals: Vec<f64> = successful.iter().map(|r| r.total_time_ms as f64).collect();
    let mem_vals: Vec<f64> = successful.iter().map(|r| r.peak_memory_mb).collect();
    let cpu_vals: Vec<f64> = successful
        .iter()
        .filter_map(|r| r.mean_cpu_percent.map(|v| v as f64))
        .collect();
    let peak_cpu_vals: Vec<f64> = successful
        .iter()
        .filter_map(|r| r.peak_cpu_percent.map(|v| v as f64))
        .collect();

    PromptStats {
        ttft: compute_stats(&ttft_vals),
        tokens_per_second: compute_stats(&tps_vals),
        tpot: compute_stats(&tpot_vals),
        total_time: compute_stats(&time_vals),
        peak_memory_mb: compute_stats(&mem_vals),
        mean_cpu_percent: compute_stats(&cpu_vals),
        peak_cpu_percent: compute_stats(&peak_cpu_vals),
    }
}
