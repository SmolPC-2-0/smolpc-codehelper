use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;

/// Result of resource sampling over a generation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    pub peak_rss_bytes: u64,
    pub mean_cpu_percent: f32,
    pub peak_cpu_percent: f32,
}

/// Idle resource baseline captured before inference starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdleBaseline {
    pub rss_mb: f64,
    pub cpu_percent: f32,
}

impl IdleBaseline {
    /// Sample for 2 seconds with no inference to establish a baseline.
    pub async fn capture(pid: u32) -> Self {
        let sampler = ResourceSampler::start(pid);
        tokio::time::sleep(Duration::from_secs(2)).await;
        let snapshot = sampler.stop().await;
        IdleBaseline {
            rss_mb: snapshot.peak_rss_bytes as f64 / (1024.0 * 1024.0),
            cpu_percent: snapshot.mean_cpu_percent,
        }
    }
}

/// Samples engine process RSS and CPU% in a background task.
pub struct ResourceSampler {
    peak_rss_bytes: Arc<AtomicU64>,
    /// CPU% stored as fixed-point: value * 100 (e.g. 9543 = 95.43%)
    peak_cpu_fixed: Arc<AtomicU32>,
    cpu_samples: Arc<Mutex<Vec<f32>>>,
    stop_flag: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

impl ResourceSampler {
    /// Start sampling RSS and CPU of the given PID every 200ms.
    pub fn start(pid: u32) -> Self {
        let peak_rss_bytes = Arc::new(AtomicU64::new(0));
        let peak_cpu_fixed = Arc::new(AtomicU32::new(0));
        let cpu_samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let stop_flag = Arc::new(AtomicBool::new(false));

        let peak_rss = peak_rss_bytes.clone();
        let peak_cpu = peak_cpu_fixed.clone();
        let samples = cpu_samples.clone();
        let stop = stop_flag.clone();

        let handle = tokio::spawn(async move {
            use sysinfo::{Pid, ProcessRefreshKind, System};
            let mut sys = System::new();
            let sysinfo_pid = Pid::from_u32(pid);
            let refresh_kind = ProcessRefreshKind::new().with_memory().with_cpu();

            // Bootstrap: first refresh primes the CPU counter (sysinfo needs
            // two consecutive refreshes to compute a delta).
            sys.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&[sysinfo_pid]),
                true,
                refresh_kind,
            );
            tokio::time::sleep(Duration::from_millis(100)).await;

            while !stop.load(Ordering::Relaxed) {
                sys.refresh_processes_specifics(
                    sysinfo::ProcessesToUpdate::Some(&[sysinfo_pid]),
                    true,
                    refresh_kind,
                );
                if let Some(process) = sys.process(sysinfo_pid) {
                    // RSS
                    let rss = process.memory();
                    peak_rss.fetch_max(rss, Ordering::Relaxed);

                    // CPU (f32: 0.0 to N*100.0 for N cores)
                    let cpu = process.cpu_usage();
                    let cpu_fixed = (cpu * 100.0) as u32;
                    peak_cpu.fetch_max(cpu_fixed, Ordering::Relaxed);

                    if let Ok(mut s) = samples.lock() {
                        s.push(cpu);
                    }
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        });

        Self {
            peak_rss_bytes,
            peak_cpu_fixed,
            cpu_samples,
            stop_flag,
            handle,
        }
    }

    /// Stop sampling and return the resource snapshot.
    pub async fn stop(self) -> ResourceSnapshot {
        self.stop_flag.store(true, Ordering::Relaxed);
        let _ = self.handle.await;

        let peak_rss_bytes = self.peak_rss_bytes.load(Ordering::Relaxed);
        let peak_cpu_percent = self.peak_cpu_fixed.load(Ordering::Relaxed) as f32 / 100.0;

        let mean_cpu_percent = self
            .cpu_samples
            .lock()
            .ok()
            .and_then(|s| {
                if s.is_empty() {
                    None
                } else {
                    Some(s.iter().sum::<f32>() / s.len() as f32)
                }
            })
            .unwrap_or(0.0);

        ResourceSnapshot {
            peak_rss_bytes,
            mean_cpu_percent,
            peak_cpu_percent,
        }
    }
}
