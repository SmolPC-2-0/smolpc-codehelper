use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const TTS_BINARY_NAME: &str = "smolpc-tts-server";
const TTS_PID_FILENAME: &str = "tts.pid";
const TTS_SPAWN_LOG_FILENAME: &str = "tts-spawn.log";
const TTS_HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(200);
const TTS_HEALTH_POLL_BUDGET: Duration = Duration::from_secs(15);
const TTS_MODEL_DIR_NAME: &str = "kittentts-nano";

// ── TtsSidecarState ──────────────────────────────────────────────────

pub(crate) struct TtsSidecarState {
    pid: Mutex<Option<u32>>,
    port: u16,
    http_client: reqwest::Client,
    token: Arc<String>,
    data_dir: PathBuf,
    resource_dir: Option<PathBuf>,
}

impl TtsSidecarState {
    pub(crate) fn new(
        port: u16,
        token: Arc<String>,
        data_dir: PathBuf,
        resource_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            pid: Mutex::new(None),
            port,
            http_client: reqwest::Client::new(),
            token,
            data_dir,
            resource_dir,
        }
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }

    // ── Spawn ────────────────────────────────────────────────────────

    pub(crate) async fn spawn_tts_sidecar(&self) {
        let binary = resolve_tts_binary(self.resource_dir.as_deref());
        let binary = match binary {
            Some(b) => b,
            None => {
                log::warn!(
                    "TTS sidecar binary not found — voice output disabled. \
                     Searched resource_dir={:?}",
                    self.resource_dir
                );
                return;
            }
        };

        let model_dir =
            smolpc_engine_core::models::ModelLoader::models_dir().join(TTS_MODEL_DIR_NAME);
        if !model_dir.is_dir() {
            log::warn!(
                "TTS model directory not found at {} — voice output disabled",
                model_dir.display()
            );
            return;
        }

        // Resolve espeak-ng directory. Check both production layout and dev extracted layout.
        let espeak_dir = self.resolve_espeak_dir();

        log::info!(
            "Spawning TTS sidecar: binary={}, port={}, model_dir={}",
            binary.display(),
            self.port,
            model_dir.display()
        );

        let spawn_log_path = self.data_dir.join(TTS_SPAWN_LOG_FILENAME);
        let stderr_target = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&spawn_log_path)
            .map(Stdio::from)
            .unwrap_or_else(|_| Stdio::null());

        let mut cmd = Command::new(&binary);
        cmd.arg("--port")
            .arg(self.port.to_string())
            .arg("--model-dir")
            .arg(&model_dir)
            .env("SMOLPC_ENGINE_TOKEN", self.token.as_str())
            .env("RUST_LOG", "info");

        if let Some(ref espeak_dir) = espeak_dir {
            cmd.arg("--espeak-dir").arg(espeak_dir);
            // CLAUDE.md learning: espeak-ng requires ESPEAK_DATA_PATH env var.
            let espeak_data = espeak_dir.join("espeak-ng-data");
            if espeak_data.is_dir() {
                cmd.env("ESPEAK_DATA_PATH", &espeak_data);
            } else {
                // Dev extracted layout: data is alongside the exe.
                cmd.env("ESPEAK_DATA_PATH", espeak_dir);
            }
        }

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x00000008;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
            cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(stderr_target);
        }

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(stderr_target);
        }

        let child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                log::error!("Failed to spawn TTS sidecar: {e}");
                return;
            }
        };

        let pid = child.id();
        log::info!("TTS sidecar spawned with PID {pid}");

        // Write PID file.
        let pid_path = self.data_dir.join(TTS_PID_FILENAME);
        if let Err(e) = fs::write(&pid_path, pid.to_string()) {
            log::warn!("Failed to write TTS PID file {}: {e}", pid_path.display());
        }

        // Poll health until ready.
        let started = Instant::now();
        let mut healthy = false;
        while started.elapsed() < TTS_HEALTH_POLL_BUDGET {
            tokio::time::sleep(TTS_HEALTH_POLL_INTERVAL).await;
            if self.check_health().await {
                healthy = true;
                break;
            }
        }

        if healthy {
            log::info!(
                "TTS sidecar healthy after {:?}",
                started.elapsed()
            );
            *self.pid.lock().await = Some(pid);
        } else {
            log::error!(
                "TTS sidecar failed to become healthy within {:?} — voice output unavailable. \
                 Check {}",
                TTS_HEALTH_POLL_BUDGET,
                spawn_log_path.display()
            );
            // Kill the unhealthy process.
            terminate_pid(pid);
            let _ = fs::remove_file(&pid_path);
        }
    }

    // ── Health check ─────────────────────────────────────────────────

    pub(crate) async fn check_health(&self) -> bool {
        let url = format!("http://127.0.0.1:{}/health", self.port);
        self.http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    // ── Kill ─────────────────────────────────────────────────────────

    pub(crate) async fn kill_sidecar(&self) {
        let mut guard = self.pid.lock().await;
        let pid = match guard.take() {
            Some(pid) => pid,
            None => return,
        };

        if is_pid_alive(pid) {
            log::info!("Terminating TTS sidecar PID {pid}");
            terminate_pid(pid);
        }

        let pid_path = self.data_dir.join(TTS_PID_FILENAME);
        let _ = fs::remove_file(&pid_path);
    }

    // ── Respawn ──────────────────────────────────────────────────────

    pub(crate) async fn attempt_respawn(&self) {
        log::info!("Attempting TTS sidecar respawn");
        self.kill_sidecar().await;
        self.spawn_tts_sidecar().await;
    }

    // ── Helpers ──────────────────────────────────────────────────────

    fn resolve_espeak_dir(&self) -> Option<PathBuf> {
        if let Some(ref resource_dir) = self.resource_dir {
            // Production layout: libs/tts/espeak-ng/
            let production = resource_dir.join("libs").join("tts").join("espeak-ng");
            if production.is_dir() {
                return Some(production);
            }
        }

        // Dev layout: libs/tts/espeak-ng/extracted/eSpeak NG/
        let dev_extracted = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("libs")
            .join("tts")
            .join("espeak-ng")
            .join("extracted")
            .join("eSpeak NG");
        if dev_extracted.is_dir() {
            return Some(dev_extracted);
        }

        // System espeak-ng on PATH — don't pass --espeak-dir, let it find it.
        None
    }
}

// ── Binary resolution ────────────────────────────────────────────────

pub(crate) fn resolve_tts_binary(resource_dir: Option<&Path>) -> Option<PathBuf> {
    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let filename = format!("{TTS_BINARY_NAME}{exe_suffix}");

    // 1. Production: resource_dir/binaries/
    if let Some(rd) = resource_dir {
        let candidate = rd.join("binaries").join(&filename);
        if candidate.exists() {
            return Some(candidate);
        }
        let candidate = rd.join(&filename);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // 2. Dev: workspace target/debug/ (relative to this crate's manifest dir)
    #[cfg(debug_assertions)]
    {
        let workspace_target = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("engine")
            .join("crates")
            .join("smolpc-tts-server")
            .join("target")
            .join("debug")
            .join(&filename);
        if workspace_target.exists() {
            return Some(workspace_target);
        }
    }

    None
}

// ── PID management ───────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn is_pid_alive(pid: u32) -> bool {
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut exit_code: u32 = 0;
        let ok = GetExitCodeProcess(handle, &mut exit_code);
        CloseHandle(handle);
        ok != 0 && exit_code == STILL_ACTIVE
    }
}

#[cfg(unix)]
fn is_pid_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(all(not(windows), not(unix)))]
fn is_pid_alive(_pid: u32) -> bool {
    true // Fallback: assume alive.
}

#[cfg(target_os = "windows")]
fn terminate_pid(pid: u32) {
    const PROCESS_TERMINATE: u32 = 0x0001;
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if !handle.is_null() {
            TerminateProcess(handle, 1);
            CloseHandle(handle);
        }
    }
}

#[cfg(unix)]
fn terminate_pid(pid: u32) {
    unsafe { libc::kill(pid as i32, libc::SIGTERM) };
}

#[cfg(all(not(windows), not(unix)))]
fn terminate_pid(_pid: u32) {}

// ── Windows FFI ──────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
extern "system" {
    fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> *mut c_void;
    fn TerminateProcess(process: *mut c_void, exit_code: u32) -> i32;
    fn GetExitCodeProcess(process: *mut c_void, exit_code: *mut u32) -> i32;
    fn CloseHandle(object: *mut c_void) -> i32;
}
