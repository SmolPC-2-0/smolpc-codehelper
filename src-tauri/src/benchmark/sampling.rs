//! Resource sampling for benchmark data collection.
//!
//! Provides background CPU and memory monitoring during inference.
//! Uses `std::sync::Mutex` (not tokio) since locks aren't held across `.await` points.

use std::sync::{Arc, Mutex};
use sysinfo::System;

/// Bytes to megabytes conversion factor.
const BYTES_PER_MB: f64 = 1024.0 * 1024.0;

/// Sampling interval during inference (50ms for rigorous monitoring).
const SAMPLING_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

/// CPU baseline delay required by sysinfo (needs two refresh cycles).
const CPU_BASELINE_DELAY: std::time::Duration = std::time::Duration::from_millis(200);

/// Collected resource samples from a benchmark run.
#[derive(Debug)]
pub struct SamplingResults {
    pub cpu_ollama_samples: Vec<f64>,
    pub cpu_tauri_samples: Vec<f64>,
    pub cpu_system_samples: Vec<f64>,
    pub memory_samples: Vec<f64>,
    pub peak_memory_mb: f64,
}

/// Internal sampling data protected by a mutex.
#[derive(Debug)]
struct SamplingData {
    cpu_ollama_samples: Vec<f64>,
    cpu_tauri_samples: Vec<f64>,
    cpu_system_samples: Vec<f64>,
    memory_samples: Vec<f64>,
    peak_memory: f64,
    sampling_active: bool,
}

/// Thread-safe state shared between the sampler task and benchmark runner.
///
/// Uses `std::sync::Mutex` rather than `tokio::sync::Mutex` because lock operations
/// are trivial (nanoseconds) and never held across `.await` points.
/// See: https://tokio.rs/tokio/tutorial/shared-state
#[derive(Clone)]
pub struct SamplingState {
    inner: Arc<Mutex<SamplingData>>,
}

impl SamplingState {
    /// Create new state with pre-allocated vectors (~100 samples capacity).
    pub fn new(initial_memory: f64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SamplingData {
                cpu_ollama_samples: Vec::with_capacity(100),
                cpu_tauri_samples: Vec::with_capacity(100),
                cpu_system_samples: Vec::with_capacity(100),
                memory_samples: Vec::with_capacity(100),
                peak_memory: initial_memory,
                sampling_active: true,
            })),
        }
    }

    /// Record a sample (single lock acquisition for all metrics).
    pub fn record_sample(&self, ollama_cpu: f64, tauri_cpu: f64, system_cpu: f64, memory: f64) {
        let mut data = self.inner.lock().expect("SamplingState mutex poisoned");
        data.cpu_ollama_samples.push(ollama_cpu);
        data.cpu_tauri_samples.push(tauri_cpu);
        data.cpu_system_samples.push(system_cpu);
        data.memory_samples.push(memory);
        if memory > data.peak_memory {
            data.peak_memory = memory;
        }
    }

    /// Check if sampling should continue.
    pub fn is_active(&self) -> bool {
        self.inner.lock().expect("SamplingState mutex poisoned").sampling_active
    }

    /// Signal the sampler to stop.
    pub fn stop(&self) {
        self.inner.lock().expect("SamplingState mutex poisoned").sampling_active = false;
    }

    /// Extract results, returning `None` if no samples were collected.
    pub fn into_results(self) -> Option<SamplingResults> {
        let mut data = self.inner.lock().expect("SamplingState mutex poisoned");

        if data.cpu_ollama_samples.is_empty() || data.memory_samples.is_empty() {
            return None;
        }

        Some(SamplingResults {
            cpu_ollama_samples: std::mem::take(&mut data.cpu_ollama_samples),
            cpu_tauri_samples: std::mem::take(&mut data.cpu_tauri_samples),
            cpu_system_samples: std::mem::take(&mut data.cpu_system_samples),
            memory_samples: std::mem::take(&mut data.memory_samples),
            peak_memory_mb: data.peak_memory,
        })
    }
}

/// Spawn a background task that samples CPU and memory at regular intervals.
///
/// Collects three CPU metrics for Ollama vs llama.cpp comparison:
/// - Ollama process CPU (inference)
/// - Tauri process CPU (HTTP overhead)
/// - System-wide CPU (context)
pub fn spawn_resource_sampler(
    ollama_pid: sysinfo::Pid,
    state: SamplingState,
) -> tokio::sync::oneshot::Receiver<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tauri_pid = sysinfo::Pid::from_u32(std::process::id());

    tokio::spawn(async move {
        let mut sys = System::new_all();
        sys.refresh_all();

        // Establish CPU baseline (sysinfo requires two refresh cycles)
        sys.refresh_cpu_all();
        tokio::time::sleep(CPU_BASELINE_DELAY).await;
        sys.refresh_cpu_all();

        while state.is_active() {
            sys.refresh_all();
            sys.refresh_cpu_all();

            let ollama_data = sys.process(ollama_pid).map(|p| {
                (f64::from(p.cpu_usage()), (p.memory() as f64) / BYTES_PER_MB)
            });

            let tauri_cpu = sys.process(tauri_pid)
                .map_or(0.0, |p| f64::from(p.cpu_usage()));

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

            tokio::time::sleep(SAMPLING_INTERVAL).await;
        }

        let _ = tx.send(());
    });

    rx
}

/// Stop sampling and collect results.
pub async fn collect_sampling_results(
    state: SamplingState,
    sampling_done: tokio::sync::oneshot::Receiver<()>,
    ollama_pid: sysinfo::Pid,
) -> Result<SamplingResults, String> {
    state.stop();
    let _ = sampling_done.await;

    state.into_results().ok_or_else(|| {
        format!(
            "Resource sampling failed: no samples collected for Ollama process (PID {ollama_pid})"
        )
    })
}

// =============================================================================
// Statistical Helpers
// =============================================================================

/// Calculate median of values (clones input, uses total_cmp for NaN safety).
pub fn calculate_median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);

    let len = sorted.len();
    if len % 2 == 0 {
        (sorted[len / 2 - 1] + sorted[len / 2]) / 2.0
    } else {
        sorted[len / 2]
    }
}

/// Calculate average of values.
pub fn calculate_average(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}
