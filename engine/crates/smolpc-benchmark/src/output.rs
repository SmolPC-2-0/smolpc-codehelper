use crate::config::BenchmarkBackend;
use crate::prompts::PromptTier;
use crate::stats::Stats;
use serde::Serialize;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// JSON schema types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
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

#[derive(Serialize)]
pub struct MachineInfo {
    pub label: String,
    pub os: String,
    pub hostname: String,
}

#[derive(Clone, Serialize)]
pub struct HardwareSnapshot {
    pub cpu_brand: String,
    pub cpu_cores_physical: usize,
    pub cpu_cores_logical: usize,
    pub gpus: Vec<GpuSnapshot>,
    pub npu: Option<String>,
    pub memory_total_gb: f64,
}

#[derive(Clone, Serialize)]
pub struct GpuSnapshot {
    pub name: String,
    pub vendor: String,
    pub vram_mb: Option<u64>,
}

#[derive(Serialize)]
pub struct TestConfig {
    pub backends: Vec<BenchmarkBackend>,
    pub models: Vec<String>,
    pub runs_per_prompt: usize,
    pub warmup_runs: usize,
    pub temperature: f64,
    pub repetition_penalty: f64,
}

#[derive(Clone, Serialize)]
pub struct BackendModelResult {
    pub backend: BenchmarkBackend,
    pub model_id: String,
    pub cold_start_ttft_ms: Option<u64>,
    pub prompts: Vec<PromptResult>,
}

#[derive(Clone, Serialize)]
pub struct PromptResult {
    pub prompt_id: String,
    pub tier: PromptTier,
    pub max_tokens: usize,
    pub runs: Vec<RunMetrics>,
    pub stats: PromptStats,
}

#[derive(Clone, Serialize)]
pub struct RunMetrics {
    pub ttft_ms: Option<u64>,
    pub tokens_per_second: f64,
    pub tpot_ms: Option<f64>,
    pub total_time_ms: u64,
    pub total_tokens: usize,
    pub peak_memory_mb: f64,
    pub truncated: bool,
}

#[derive(Clone, Serialize)]
pub struct PromptStats {
    pub ttft: Option<Stats>,
    pub tokens_per_second: Option<Stats>,
    pub tpot: Option<Stats>,
    pub total_time: Option<Stats>,
    pub peak_memory_mb: Option<Stats>,
}

#[derive(Clone, Serialize)]
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
        "{:<15} {:<25} {:>10} {:>10} {:>10} {:>12}",
        "Backend", "Model", "TTFT(ms)", "Tok/s", "TPOT(ms)", "Memory(MB)"
    );
    println!("{}", "-".repeat(85));

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

        println!(
            "{:<15} {:<25} {:>10} {:>10} {:>10} {:>12}",
            result.backend.engine_label(),
            result.model_id,
            avg(&ttft_vals),
            avg(&tps_vals),
            avg(&tpot_vals),
            avg(&mem_vals),
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
