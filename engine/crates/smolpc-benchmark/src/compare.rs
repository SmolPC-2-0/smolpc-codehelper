use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

use crate::config::DEFAULT_REGRESSION_THRESHOLD;
use crate::output::BenchmarkReport;

#[derive(Parser)]
pub struct CompareArgs {
    /// Path to the baseline benchmark JSON
    #[arg(long)]
    baseline: String,

    /// Path to the current benchmark JSON
    #[arg(long)]
    current: String,

    /// Regression threshold as a fraction (default: 0.10 = 10%)
    #[arg(long, default_value_t = DEFAULT_REGRESSION_THRESHOLD)]
    threshold: f64,

    /// Optional output path for comparison JSON
    #[arg(long)]
    output: Option<String>,
}

#[derive(Debug)]
#[allow(dead_code)]
struct ComboComparison {
    backend: String,
    model_id: String,
    ttft_delta_pct: Option<f64>,
    tps_delta_pct: Option<f64>,
    tpot_delta_pct: Option<f64>,
    memory_delta_pct: Option<f64>,
    trunc_rate_baseline: f64,
    trunc_rate_current: f64,
    errors_baseline: usize,
    errors_current: usize,
    regressions: Vec<Regression>,
}

#[derive(Debug, serde::Serialize)]
struct Regression {
    metric: String,
    baseline_value: f64,
    current_value: f64,
    delta_percent: f64,
}

pub fn run_comparison(args: CompareArgs) -> Result<()> {
    let baseline_path = PathBuf::from(&args.baseline);
    let current_path = PathBuf::from(&args.current);

    let baseline_json = std::fs::read_to_string(&baseline_path)
        .with_context(|| format!("failed to read baseline: {}", baseline_path.display()))?;
    let current_json = std::fs::read_to_string(&current_path)
        .with_context(|| format!("failed to read current: {}", current_path.display()))?;

    let baseline: BenchmarkReport =
        serde_json::from_str(&baseline_json).context("failed to parse baseline JSON")?;
    let current: BenchmarkReport =
        serde_json::from_str(&current_json).context("failed to parse current JSON")?;

    // Check prompt corpus compatibility
    if let (Some(bh), Some(ch)) = (
        &baseline.test_config.prompt_corpus_hash,
        &current.test_config.prompt_corpus_hash,
    ) {
        if bh != ch {
            println!(
                "WARNING: Prompt corpus hash mismatch (baseline={bh}, current={ch}). \
                 Results may not be directly comparable."
            );
        }
    }

    // Per-metric threshold scaling
    let ttft_threshold = args.threshold * 1.5; // TTFT is noisier
    let tps_threshold = args.threshold;
    let tpot_threshold = args.threshold;
    let memory_threshold = args.threshold * 0.5; // Memory should be very stable

    // Match combos
    let mut comparisons: Vec<ComboComparison> = Vec::new();
    let mut baseline_only: Vec<(String, String)> = Vec::new();
    let mut current_only: Vec<(String, String)> = Vec::new();

    for br in &baseline.results {
        let key = (br.backend.engine_label().to_string(), br.model_id.clone());
        let cr = current
            .results
            .iter()
            .find(|r| r.backend == br.backend && r.model_id == br.model_id);

        match cr {
            Some(cr) => {
                let mut regressions = Vec::new();

                // Compute median aggregates across prompts
                let b_ttft = median_of_medians(&br.prompts, |p| p.stats.ttft.as_ref().map(|s| s.median));
                let c_ttft = median_of_medians(&cr.prompts, |p| p.stats.ttft.as_ref().map(|s| s.median));
                let ttft_delta = delta_pct(b_ttft, c_ttft);

                let b_tps = median_of_medians(&br.prompts, |p| {
                    p.stats.tokens_per_second.as_ref().map(|s| s.median)
                });
                let c_tps = median_of_medians(&cr.prompts, |p| {
                    p.stats.tokens_per_second.as_ref().map(|s| s.median)
                });
                let tps_delta = delta_pct(b_tps, c_tps);

                let b_tpot = median_of_medians(&br.prompts, |p| p.stats.tpot.as_ref().map(|s| s.median));
                let c_tpot = median_of_medians(&cr.prompts, |p| p.stats.tpot.as_ref().map(|s| s.median));
                let tpot_delta = delta_pct(b_tpot, c_tpot);

                let b_mem = median_of_medians(&br.prompts, |p| {
                    p.stats.peak_memory_mb.as_ref().map(|s| s.median)
                });
                let c_mem = median_of_medians(&cr.prompts, |p| {
                    p.stats.peak_memory_mb.as_ref().map(|s| s.median)
                });
                let mem_delta = delta_pct(b_mem, c_mem);

                // Check regressions (higher is worse for TTFT, TPOT, memory; lower is worse for tok/s)
                if let (Some(d), Some(bv), Some(cv)) = (ttft_delta, b_ttft, c_ttft) {
                    if d > ttft_threshold * 100.0 {
                        regressions.push(Regression {
                            metric: "TTFT(ms)".to_string(),
                            baseline_value: bv,
                            current_value: cv,
                            delta_percent: d,
                        });
                    }
                }
                if let (Some(d), Some(bv), Some(cv)) = (tps_delta, b_tps, c_tps) {
                    if d < -(tps_threshold * 100.0) {
                        regressions.push(Regression {
                            metric: "Tok/s".to_string(),
                            baseline_value: bv,
                            current_value: cv,
                            delta_percent: d,
                        });
                    }
                }
                if let (Some(d), Some(bv), Some(cv)) = (tpot_delta, b_tpot, c_tpot) {
                    if d > tpot_threshold * 100.0 {
                        regressions.push(Regression {
                            metric: "TPOT(ms)".to_string(),
                            baseline_value: bv,
                            current_value: cv,
                            delta_percent: d,
                        });
                    }
                }
                if let (Some(d), Some(bv), Some(cv)) = (mem_delta, b_mem, c_mem) {
                    if d > memory_threshold * 100.0 {
                        regressions.push(Regression {
                            metric: "Memory(MB)".to_string(),
                            baseline_value: bv,
                            current_value: cv,
                            delta_percent: d,
                        });
                    }
                }

                // Error rate regression (any increase)
                if cr.reliability.failed_runs > br.reliability.failed_runs {
                    regressions.push(Regression {
                        metric: "Errors".to_string(),
                        baseline_value: br.reliability.failed_runs as f64,
                        current_value: cr.reliability.failed_runs as f64,
                        delta_percent: 0.0,
                    });
                }

                // Truncation rate regression (>5pp increase)
                let trunc_pp = cr.reliability.truncation_rate - br.reliability.truncation_rate;
                if trunc_pp > 0.05 {
                    regressions.push(Regression {
                        metric: "Trunc%".to_string(),
                        baseline_value: br.reliability.truncation_rate * 100.0,
                        current_value: cr.reliability.truncation_rate * 100.0,
                        delta_percent: trunc_pp * 100.0,
                    });
                }

                comparisons.push(ComboComparison {
                    backend: key.0,
                    model_id: key.1,
                    ttft_delta_pct: ttft_delta,
                    tps_delta_pct: tps_delta,
                    tpot_delta_pct: tpot_delta,
                    memory_delta_pct: mem_delta,
                    trunc_rate_baseline: br.reliability.truncation_rate,
                    trunc_rate_current: cr.reliability.truncation_rate,
                    errors_baseline: br.reliability.failed_runs,
                    errors_current: cr.reliability.failed_runs,
                    regressions,
                });
            }
            None => {
                baseline_only.push(key);
            }
        }
    }

    // Find current-only combos
    for cr in &current.results {
        let found = baseline
            .results
            .iter()
            .any(|r| r.backend == cr.backend && r.model_id == cr.model_id);
        if !found {
            current_only.push((cr.backend.engine_label().to_string(), cr.model_id.clone()));
        }
    }

    // --- Print comparison ---
    println!("\n=== Benchmark Comparison ===");
    println!(
        "Baseline: {} ({})",
        baseline_path.display(),
        baseline.generated_at
    );
    println!(
        "Current:  {} ({})",
        current_path.display(),
        current.generated_at
    );
    println!("Threshold: {:.0}%\n", args.threshold * 100.0);

    println!(
        "{:<15} {:<25} {:>10} {:>10} {:>10} {:>10} {:>8}",
        "Backend", "Model", "TTFT", "Tok/s", "TPOT", "Memory", "Status"
    );
    println!("{}", "-".repeat(93));

    let total_regressions: usize = comparisons.iter().map(|c| c.regressions.len()).sum();

    for comp in &comparisons {
        let status = if comp.regressions.is_empty() {
            "\x1b[32mOK\x1b[0m" // green
        } else {
            "\x1b[31mREGRESS\x1b[0m" // red
        };

        println!(
            "{:<15} {:<25} {:>10} {:>10} {:>10} {:>10} {:>18}",
            comp.backend,
            comp.model_id,
            format_delta(comp.ttft_delta_pct),
            format_delta(comp.tps_delta_pct),
            format_delta(comp.tpot_delta_pct),
            format_delta(comp.memory_delta_pct),
            status,
        );
    }

    // Print regression details
    if total_regressions > 0 {
        println!("\nRegressions ({total_regressions}):");
        for comp in &comparisons {
            for reg in &comp.regressions {
                println!(
                    "  \x1b[31m{} / {}: {} — was {:.1}, now {:.1} ({:+.1}%)\x1b[0m",
                    comp.backend,
                    comp.model_id,
                    reg.metric,
                    reg.baseline_value,
                    reg.current_value,
                    reg.delta_percent,
                );
            }
        }
    }

    // Unmatched combos
    if !baseline_only.is_empty() || !current_only.is_empty() {
        println!("\nUnmatched combos:");
        for (b, m) in &baseline_only {
            println!("  Baseline only: {b} / {m}");
        }
        for (b, m) in &current_only {
            println!("  Current only:  {b} / {m} (new)");
        }
    }

    println!();

    // Optional JSON output
    if let Some(output_path) = &args.output {
        let output = serde_json::json!({
            "baseline": args.baseline,
            "current": args.current,
            "threshold": args.threshold,
            "total_regressions": total_regressions,
            "comparisons": comparisons.iter().map(|c| serde_json::json!({
                "backend": c.backend,
                "model_id": c.model_id,
                "ttft_delta_pct": c.ttft_delta_pct,
                "tps_delta_pct": c.tps_delta_pct,
                "tpot_delta_pct": c.tpot_delta_pct,
                "memory_delta_pct": c.memory_delta_pct,
                "regressions": c.regressions,
            })).collect::<Vec<_>>(),
        });
        let json = serde_json::to_string_pretty(&output)?;
        std::fs::write(output_path, &json)?;
        println!("Comparison written to: {output_path}");
    }

    if total_regressions > 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Compute average of median values across prompts.
fn median_of_medians<F>(
    prompts: &[crate::output::PromptResult],
    extract: F,
) -> Option<f64>
where
    F: Fn(&crate::output::PromptResult) -> Option<f64>,
{
    let vals: Vec<f64> = prompts.iter().filter_map(&extract).collect();
    if vals.is_empty() {
        None
    } else {
        Some(vals.iter().sum::<f64>() / vals.len() as f64)
    }
}

/// Compute percent change: (current - baseline) / baseline * 100.
fn delta_pct(baseline: Option<f64>, current: Option<f64>) -> Option<f64> {
    match (baseline, current) {
        (Some(b), Some(c)) if b.abs() > f64::EPSILON => Some((c - b) / b * 100.0),
        _ => None,
    }
}

/// Format a delta percentage with color.
fn format_delta(delta: Option<f64>) -> String {
    match delta {
        Some(d) => format!("{:+.1}%", d),
        None => "N/A".to_string(),
    }
}
