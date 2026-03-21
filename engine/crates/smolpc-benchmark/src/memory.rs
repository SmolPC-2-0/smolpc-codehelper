use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

/// Samples engine process RSS in a background task.
pub struct MemorySampler {
    peak_bytes: Arc<AtomicU64>,
    stop_flag: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

impl MemorySampler {
    /// Start sampling RSS of the given PID every 200ms.
    pub fn start(pid: u32) -> Self {
        let peak_bytes = Arc::new(AtomicU64::new(0));
        let stop_flag = Arc::new(AtomicBool::new(false));

        let peak = peak_bytes.clone();
        let stop = stop_flag.clone();

        let handle = tokio::spawn(async move {
            use sysinfo::{Pid, ProcessRefreshKind, System};
            let mut sys = System::new();
            let sysinfo_pid = Pid::from_u32(pid);
            let refresh_kind = ProcessRefreshKind::new().with_memory();

            while !stop.load(Ordering::Relaxed) {
                sys.refresh_processes_specifics(
                    sysinfo::ProcessesToUpdate::Some(&[sysinfo_pid]),
                    true,
                    refresh_kind,
                );
                if let Some(process) = sys.process(sysinfo_pid) {
                    let rss = process.memory();
                    peak.fetch_max(rss, Ordering::Relaxed);
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        });

        Self {
            peak_bytes,
            stop_flag,
            handle,
        }
    }

    /// Stop sampling and return peak RSS in bytes.
    pub async fn stop(self) -> u64 {
        self.stop_flag.store(true, Ordering::Relaxed);
        let _ = self.handle.await;
        self.peak_bytes.load(Ordering::Relaxed)
    }
}
