mod config;
mod engine_lifecycle;
mod memory;
mod output;
mod prompts;
mod runner;
mod stats;

use anyhow::{Context, Result};
use clap::Parser;
use smolpc_engine_core::models::registry::ModelRegistry;
use std::path::PathBuf;

use config::{BenchmarkBackend, BenchmarkConfig};
use output::{
    BackendModelResult, BenchmarkReport, GpuSnapshot, HardwareSnapshot, MachineInfo, SkippedCombo,
    TestConfig,
};
use prompts::PROMPTS;

#[derive(Parser)]
#[command(
    name = "smolpc-benchmark",
    about = "Standalone inference benchmark for the SmolPC engine"
)]
struct Cli {
    /// Human label for this machine (used in output filename)
    #[arg(long)]
    machine: String,

    /// Comma-separated backends: cpu,directml,openvino_npu (default: auto-detect)
    #[arg(long, value_delimiter = ',')]
    backends: Option<Vec<String>>,

    /// Comma-separated model IDs (default: all fitting RAM)
    #[arg(long, value_delimiter = ',')]
    models: Option<Vec<String>>,

    /// Measured runs per prompt (default: 10)
    #[arg(long, default_value_t = 10)]
    runs: usize,

    /// Warmup runs before measurement (default: 2, NPU always >= 3)
    #[arg(long, default_value_t = 2)]
    warmup: usize,

    /// Seconds to pause between backend switches (default: 30)
    #[arg(long, default_value_t = 30)]
    cooldown: u64,

    /// Output directory for JSON results (default: ./benchmark-results/)
    #[arg(long)]
    output_dir: Option<String>,

    /// Engine port (default: 19432)
    #[arg(long, default_value_t = 19432)]
    port: u16,

    /// Resource directory containing libs/ and binaries/ (e.g. installed app dir)
    #[arg(long)]
    resource_dir: Option<String>,

    /// Print test matrix and exit without running
    #[arg(long)]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    // --- Detect hardware ---
    let hw = smolpc_engine_core::hardware::detect_all()
        .await
        .context("hardware detection failed")?;

    let memory_total_gb = hw.memory.total_gb;

    // --- Resolve backends ---
    let backends = match &cli.backends {
        Some(list) => list
            .iter()
            .map(|s| s.parse::<BenchmarkBackend>().map_err(|e| anyhow::anyhow!(e)))
            .collect::<Result<Vec<_>>>()?,
        None => auto_detect_backends(&hw),
    };

    // --- Resolve models ---
    let models = match &cli.models {
        Some(list) => list.clone(),
        None => ModelRegistry::available_models()
            .into_iter()
            .map(|m| m.id)
            .collect(),
    };

    let output_dir = PathBuf::from(
        cli.output_dir
            .as_deref()
            .unwrap_or("./benchmark-results"),
    );

    let resource_dir = cli.resource_dir.map(PathBuf::from);

    let config = BenchmarkConfig {
        machine: cli.machine.clone(),
        backends: backends.clone(),
        models: models.clone(),
        runs: cli.runs,
        warmup: cli.warmup,
        cooldown_secs: cli.cooldown,
        output_dir: output_dir.clone(),
        port: cli.port,
        resource_dir: resource_dir.clone(),
        dry_run: cli.dry_run,
    };

    // --- Dry-run: print matrix and exit ---
    if config.dry_run {
        print_matrix(&config, memory_total_gb);
        return Ok(());
    }

    // --- Create output dir ---
    std::fs::create_dir_all(&output_dir).context("failed to create output directory")?;

    // --- Build initial report shell ---
    let hw_snapshot = build_hardware_snapshot(&hw);
    let machine_info = MachineInfo {
        label: cli.machine.clone(),
        os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        hostname: sysinfo::System::host_name().unwrap_or_else(|| "unknown".to_string()),
    };

    let mut results: Vec<BackendModelResult> = Vec::new();
    let mut skipped: Vec<SkippedCombo> = Vec::new();

    // --- Main benchmark loop ---
    for (backend_idx, &backend) in config.backends.iter().enumerate() {
        println!(
            "\n=== Backend: {} ({}/{}) ===",
            backend,
            backend_idx + 1,
            config.backends.len()
        );

        // Cooldown between backend switches
        if backend_idx > 0 {
            println!(
                "Cooling down for {}s before switching backend...",
                config.cooldown_secs
            );
            tokio::time::sleep(std::time::Duration::from_secs(config.cooldown_secs)).await;
        }

        // Spawn engine for this backend
        let client = match engine_lifecycle::spawn_engine(backend, config.port, resource_dir.clone())
            .await
        {
            Ok(c) => c,
            Err(err) => {
                log::error!("Failed to spawn engine for {backend}: {err:#}");
                for model_id in &config.models {
                    skipped.push(SkippedCombo {
                        backend,
                        model_id: model_id.clone(),
                        reason: format!("engine spawn failed: {err:#}"),
                    });
                }
                continue;
            }
        };

        for model_id in &config.models {
            println!(
                "\n--- {backend} / {model_id} ({} prompts x {} runs) ---",
                PROMPTS.len(),
                config.runs,
            );

            match runner::try_run_combo(&client, backend, model_id, &config).await {
                Ok(result) => results.push(result),
                Err(skip) => skipped.push(skip),
            }

            // Write partial results after each combo
            let partial_report =
                build_report(&machine_info, &hw_snapshot, &config, &results, &skipped);
            if let Err(err) = output::write_partial(&output_dir, &cli.machine, &partial_report) {
                log::warn!("Failed to write partial results: {err}");
            }
        }

        // Shut down engine before switching backend
        if let Err(err) = engine_lifecycle::shutdown_engine(&client).await {
            log::warn!("Engine shutdown warning: {err}");
        }
    }

    // --- Final output ---
    let report = build_report(&machine_info, &hw_snapshot, &config, &results, &skipped);
    let final_path = output::write_final(&output_dir, &cli.machine, &report)?;
    output::print_summary(&report);

    println!("Results written to: {}", final_path.display());
    Ok(())
}

fn build_report(
    machine: &MachineInfo,
    hardware: &HardwareSnapshot,
    config: &BenchmarkConfig,
    results: &[BackendModelResult],
    skipped: &[SkippedCombo],
) -> BenchmarkReport {
    BenchmarkReport {
        schema_version: "1.0.0".to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        machine: MachineInfo {
            label: machine.label.clone(),
            os: machine.os.clone(),
            hostname: machine.hostname.clone(),
        },
        hardware: hardware.clone(),
        test_config: TestConfig {
            backends: config.backends.clone(),
            models: config.models.clone(),
            runs_per_prompt: config.runs,
            warmup_runs: config.warmup,
            temperature: config::BENCH_TEMPERATURE as f64,
            repetition_penalty: config::BENCH_REPETITION_PENALTY as f64,
        },
        results: results.to_vec(),
        skipped: skipped.to_vec(),
    }
}

fn build_hardware_snapshot(
    hw: &smolpc_engine_core::hardware::types::HardwareInfo,
) -> HardwareSnapshot {
    HardwareSnapshot {
        cpu_brand: hw.cpu.brand.clone(),
        cpu_cores_physical: hw.cpu.cores_physical,
        cpu_cores_logical: hw.cpu.cores_logical,
        gpus: hw
            .gpus
            .iter()
            .map(|g| GpuSnapshot {
                name: g.name.clone(),
                vendor: format!("{:?}", g.vendor),
                vram_mb: g.vram_mb,
            })
            .collect(),
        npu: hw.npu.as_ref().map(|n| n.details.clone()),
        memory_total_gb: hw.memory.total_gb,
    }
}

fn auto_detect_backends(
    hw: &smolpc_engine_core::hardware::types::HardwareInfo,
) -> Vec<BenchmarkBackend> {
    let mut backends = vec![BenchmarkBackend::Cpu];

    // Only discrete GPUs qualify for DirectML benchmarking
    let has_directml_gpu = hw.gpus.iter().any(|g| {
        let name_lower = g.name.to_lowercase();
        name_lower.contains("nvidia") || name_lower.contains("radeon") || name_lower.contains("arc")
    });
    if has_directml_gpu {
        backends.push(BenchmarkBackend::DirectMl);
    }

    if hw.npu.as_ref().is_some_and(|n| n.detected) {
        backends.push(BenchmarkBackend::OpenVinoNpu);
    }

    backends
}

fn print_matrix(config: &BenchmarkConfig, ram_gb: f64) {
    println!("=== Benchmark Dry Run ===");
    println!("Machine:     {}", config.machine);
    println!("RAM:         {ram_gb:.1} GB");
    println!("Port:        {}", config.port);
    println!("Runs/prompt: {}", config.runs);
    println!("Warmup:      {}", config.warmup);
    println!("Cooldown:    {}s", config.cooldown_secs);
    println!();

    println!(
        "{:<15} {:<25} {:>8} {:>8}",
        "Backend", "Model", "Prompts", "Runs"
    );
    println!("{}", "-".repeat(60));

    let mut total_combos = 0;
    for backend in &config.backends {
        for model_id in &config.models {
            println!(
                "{:<15} {:<25} {:>8} {:>8}",
                backend.engine_label(),
                model_id,
                PROMPTS.len(),
                config.runs,
            );
            total_combos += 1;
        }
    }

    println!();
    println!(
        "Total: {} combos, {} generation calls",
        total_combos,
        total_combos * PROMPTS.len() * config.runs
    );
}
