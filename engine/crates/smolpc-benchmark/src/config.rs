use smolpc_engine_client::RuntimeModePreference;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

/// Backends the benchmark can target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkBackend {
    Cpu,
    DirectMl,
    OpenVinoNpu,
}

impl BenchmarkBackend {
    pub fn to_runtime_mode(self) -> RuntimeModePreference {
        match self {
            Self::Cpu => RuntimeModePreference::Cpu,
            Self::DirectMl => RuntimeModePreference::Dml,
            Self::OpenVinoNpu => RuntimeModePreference::Npu,
        }
    }

    /// The string the engine reports in `EngineStatus.active_backend`.
    pub fn engine_label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::DirectMl => "directml",
            Self::OpenVinoNpu => "openvino_npu",
        }
    }

    pub fn is_npu(self) -> bool {
        matches!(self, Self::OpenVinoNpu)
    }
}

impl fmt::Display for BenchmarkBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.engine_label())
    }
}

impl FromStr for BenchmarkBackend {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cpu" => Ok(Self::Cpu),
            "directml" | "dml" => Ok(Self::DirectMl),
            "openvino_npu" | "npu" => Ok(Self::OpenVinoNpu),
            other => Err(format!("unknown backend: {other}")),
        }
    }
}

impl serde::Serialize for BenchmarkBackend {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.engine_label())
    }
}

/// Resolved benchmark configuration.
#[allow(dead_code)]
pub struct BenchmarkConfig {
    pub machine: String,
    pub backends: Vec<BenchmarkBackend>,
    pub models: Vec<String>,
    pub runs: usize,
    pub warmup: usize,
    pub cooldown_secs: u64,
    pub output_dir: PathBuf,
    pub port: u16,
    pub dry_run: bool,
}

/// Greedy generation config used for reproducible benchmarks.
pub const BENCH_TEMPERATURE: f32 = 0.0;
pub const BENCH_REPETITION_PENALTY: f32 = 1.0;
