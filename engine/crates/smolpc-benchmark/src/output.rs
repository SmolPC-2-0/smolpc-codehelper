use crate::config::BenchmarkBackend;
use crate::prompts::PromptTier;
use crate::reliability::{ComboReliability, StopReason};
use crate::resource_sampler::IdleBaseline;
use crate::stats::Stats;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// JSON schema types (v2.0.0)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub schema_version: String,
    pub generated_at: String,
    pub tool_version: String,
    pub machine: MachineInfo,
    pub hardware: HardwareSnapshot,
    pub test_config: TestConfig,
    pub results: Vec<BackendModelResult>,
    pub skipped: Vec<SkippedCombo>,
}

#[derive(Serialize, Deserialize)]
pub struct MachineInfo {
    pub label: String,
    pub os: String,
    pub hostname: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HardwareSnapshot {
    pub cpu_brand: String,
    pub cpu_cores_physical: usize,
    pub cpu_cores_logical: usize,
    pub gpus: Vec<GpuSnapshot>,
    pub npu: Option<String>,
    pub memory_total_gb: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GpuSnapshot {
    pub name: String,
    pub vendor: String,
    pub vram_mb: Option<u64>,
}

#[derive(Serialize, Deserialize)]
pub struct TestConfig {
    pub backends: Vec<BenchmarkBackend>,
    pub models: Vec<String>,
    pub runs_per_prompt: usize,
    pub warmup_runs: usize,
    pub temperature: f64,
    pub repetition_penalty: f64,
    /// SHA-256 hash of concatenated prompt contents for comparison validity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_corpus_hash: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BackendModelResult {
    pub backend: BenchmarkBackend,
    pub model_id: String,
    pub cold_start_ttft_ms: Option<u64>,
    /// Resource baseline captured before inference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_baseline: Option<IdleBaseline>,
    /// Aggregated reliability metrics.
    pub reliability: ComboReliability,
    pub prompts: Vec<PromptResult>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PromptResult {
    pub prompt_id: String,
    pub tier: PromptTier,
    pub max_tokens: usize,
    pub runs: Vec<RunMetrics>,
    pub stats: PromptStats,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RunMetrics {
    pub ttft_ms: Option<u64>,
    pub tokens_per_second: f64,
    pub tpot_ms: Option<f64>,
    pub total_time_ms: u64,
    pub total_tokens: usize,
    pub peak_memory_mb: f64,
    /// Mean CPU% of engine process during this run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_cpu_percent: Option<f32>,
    /// Peak CPU% of engine process during this run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peak_cpu_percent: Option<f32>,
    /// Why generation stopped.
    pub stop_reason: StopReason,
    /// Error message if this run failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub truncated: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PromptStats {
    pub ttft: Option<Stats>,
    pub tokens_per_second: Option<Stats>,
    pub tpot: Option<Stats>,
    pub total_time: Option<Stats>,
    pub peak_memory_mb: Option<Stats>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_cpu_percent: Option<Stats>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peak_cpu_percent: Option<Stats>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SkippedCombo {
    pub backend: BenchmarkBackend,
    pub model_id: String,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// File operations
// ---------------------------------------------------------------------------

/// Build the final output filename.
pub fn output_filename(machine: &str) -> String {
    let date = chrono::Local::now().format("%Y-%m-%d");
    format!("benchmark-{machine}-{date}.json")
}

/// Write partial results after each completed combo.
pub fn write_partial(dir: &Path, machine: &str, report: &BenchmarkReport) -> anyhow::Result<()> {
    let partial_path = dir.join(format!("benchmark-{machine}.partial.json"));
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(&partial_path, &json)?;
    log::info!("Partial results written to {}", partial_path.display());
    Ok(())
}

/// Finalize: write the final file and remove the partial.
pub fn write_final(dir: &Path, machine: &str, report: &BenchmarkReport) -> anyhow::Result<PathBuf> {
    let filename = output_filename(machine);
    let final_path = dir.join(&filename);
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(&final_path, &json)?;

    // Remove partial if it exists
    let partial_path = dir.join(format!("benchmark-{machine}.partial.json"));
    let _ = std::fs::remove_file(partial_path);

    Ok(final_path)
}

/// Print a summary table to stdout.
pub fn print_summary(report: &BenchmarkReport) {
    println!();
    println!(
        "{:<15} {:<25} {:>10} {:>10} {:>10} {:>10} {:>8} {:>8}",
        "Backend", "Model", "TTFT(ms)", "Tok/s", "TPOT(ms)", "Mem(MB)", "Trunc%", "Errors"
    );
    println!("{}", "-".repeat(100));

    for result in &report.results {
        // Aggregate across all prompts
        let ttft_vals: Vec<f64> = result
            .prompts
            .iter()
            .filter_map(|p| p.stats.ttft.as_ref().map(|s| s.median))
            .collect();
        let tps_vals: Vec<f64> = result
            .prompts
            .iter()
            .filter_map(|p| p.stats.tokens_per_second.as_ref().map(|s| s.median))
            .collect();
        let tpot_vals: Vec<f64> = result
            .prompts
            .iter()
            .filter_map(|p| p.stats.tpot.as_ref().map(|s| s.median))
            .collect();
        let mem_vals: Vec<f64> = result
            .prompts
            .iter()
            .filter_map(|p| p.stats.peak_memory_mb.as_ref().map(|s| s.median))
            .collect();

        let avg = |v: &[f64]| -> String {
            if v.is_empty() {
                "N/A".to_string()
            } else {
                format!("{:.1}", v.iter().sum::<f64>() / v.len() as f64)
            }
        };

        let trunc_pct = format!("{:.0}%", result.reliability.truncation_rate * 100.0);
        let errors = result.reliability.failed_runs.to_string();

        println!(
            "{:<15} {:<25} {:>10} {:>10} {:>10} {:>10} {:>8} {:>8}",
            result.backend.engine_label(),
            result.model_id,
            avg(&ttft_vals),
            avg(&tps_vals),
            avg(&tpot_vals),
            avg(&mem_vals),
            trunc_pct,
            errors,
        );
    }

    if !report.skipped.is_empty() {
        println!();
        println!("Skipped:");
        for s in &report.skipped {
            println!("  {} / {}: {}", s.backend, s.model_id, s.reason);
        }
    }
    println!();
}
