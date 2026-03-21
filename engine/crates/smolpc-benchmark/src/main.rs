mod config;
mod engine_lifecycle;
mod memory;
mod output;
mod prompts;
mod runner;
mod stats;

use clap::Parser;

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

    /// Print test matrix and exit without running
    #[arg(long)]
    dry_run: bool,
}

fn main() {
    let _cli = Cli::parse();
    println!("smolpc-benchmark: {} prompts loaded", prompts::PROMPTS.len());
}
