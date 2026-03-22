use reqwest::header::AUTHORIZATION;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::client::EngineClient;
use crate::token::load_or_create_token;
use crate::version::protocol_major_matches;
use crate::{
    EngineClientError, EngineConnectOptions, DML_DEVICE_ENV, ENGINE_HOST_BASENAME,
    ENGINE_PROTOCOL_VERSION, FORCE_EP_ENV, NON_STREAMING_REQUEST_TIMEOUT, SHARED_MODELS_DIR,
    SHARED_MODELS_VENDOR_DIR, SPAWN_LOCK_FILENAME, SPAWN_LOCK_STALE_AGE, SPAWN_LOCK_WAIT,
    SPAWN_LOG_FILENAME,
};

pub async fn connect_or_spawn(
    options: EngineConnectOptions,
) -> Result<EngineClient, EngineClientError> {
    std::fs::create_dir_all(&options.shared_runtime_dir)?;
    std::fs::create_dir_all(&options.data_dir)?;

    let token_path = options.shared_runtime_dir.join("engine-token.txt");
    let token = load_or_create_token(&token_path)?;
    let base_url = format!("http://127.0.0.1:{}", options.port);
    let client = EngineClient::new(base_url, token.clone());
    let force_override = options.runtime_mode.as_force_override();
    let force_respawn = options.force_respawn;

    if enforce_running_host_policy(&client, force_override, force_respawn).await? {
        return Ok(client);
    }

    let _spawn_lock = acquire_spawn_lock(&options.shared_runtime_dir).await?;
    if enforce_running_host_policy(&client, force_override, force_respawn).await? {
        return Ok(client);
    }

    if !client.health().await.unwrap_or(false) {
        // Kill any stale engine processes before spawning a fresh one.
        kill_stale_engine_processes();

        // Regenerate the token so client and freshly-spawned host share the
        // same secret -- stale tokens from a dead host are the #1 cause of
        // "failed to become healthy" on clean installs.
        let _ = std::fs::remove_file(&token_path);
        let fresh_token = load_or_create_token(&token_path)?;
        let client = EngineClient::new(
            format!("http://127.0.0.1:{}", options.port),
            fresh_token.clone(),
        );

        spawn_host(&options, &fresh_token)?;

        let spawn_log = options.shared_runtime_dir.join(SPAWN_LOG_FILENAME);
        let started = std::time::Instant::now();
        loop {
            if client.health().await.unwrap_or(false) {
                return finish_connect(client).await;
            }
            if started.elapsed() > Duration::from_secs(30) {
                let log_hint = if spawn_log.exists() {
                    format!(" Check spawn log: {}", spawn_log.display())
                } else {
                    String::new()
                };
                return Err(EngineClientError::Message(format!(
                    "Engine failed to become healthy within 30s.{log_hint}"
                )));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    finish_connect(client).await
}

async fn finish_connect(client: EngineClient) -> Result<EngineClient, EngineClientError> {
    let meta = client.meta().await?;
    if !protocol_major_matches(&meta.protocol_version, ENGINE_PROTOCOL_VERSION) {
        return Err(EngineClientError::Message(format!(
            "Engine protocol mismatch: {}",
            meta.protocol_version
        )));
    }
    Ok(client)
}

async fn enforce_running_host_policy(
    client: &EngineClient,
    force_override: Option<&str>,
    force_respawn: bool,
) -> Result<bool, EngineClientError> {
    if !client.health().await.unwrap_or(false) {
        return Ok(false);
    }

    let meta = client.meta().await?;
    let protocol_matches = protocol_major_matches(&meta.protocol_version, ENGINE_PROTOCOL_VERSION);
    let needs_status_probe = !protocol_matches || force_override.is_some() || force_respawn;
    if !needs_status_probe {
        return Ok(true);
    }

    let status = client.status().await?;
    match decide_running_host_policy(
        protocol_matches,
        status.generating,
        force_override,
        force_respawn,
    ) {
        RunningHostPolicyDecision::Reuse => Ok(true),
        RunningHostPolicyDecision::Restart => {
            request_engine_shutdown(client).await?;
            wait_for_engine_down(client, Duration::from_secs(5)).await?;
            Ok(false)
        }
        RunningHostPolicyDecision::Reject(message) => Err(EngineClientError::Message(message)),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RunningHostPolicyDecision {
    Reuse,
    Restart,
    Reject(String),
}

pub(crate) fn decide_running_host_policy(
    protocol_matches: bool,
    generating: bool,
    force_override: Option<&str>,
    force_respawn: bool,
) -> RunningHostPolicyDecision {
    if !protocol_matches {
        if generating {
            return RunningHostPolicyDecision::Reject(
                "Running engine protocol is incompatible and daemon is busy".to_string(),
            );
        }
        return RunningHostPolicyDecision::Restart;
    }

    if force_override.is_some() || force_respawn {
        if generating {
            let policy = force_override
                .map(|value| format!("{FORCE_EP_ENV}={value}"))
                .unwrap_or_else(|| "forced respawn policy".to_string());
            return RunningHostPolicyDecision::Reject(format!(
                "Engine is busy and cannot apply {policy}. Cancel generation and retry."
            ));
        }
        return RunningHostPolicyDecision::Restart;
    }

    RunningHostPolicyDecision::Reuse
}

async fn request_engine_shutdown(client: &EngineClient) -> Result<(), EngineClientError> {
    let response = client
        .http
        .post(client.url("/engine/shutdown"))
        .header(AUTHORIZATION, client.auth_header())
        .timeout(NON_STREAMING_REQUEST_TIMEOUT)
        .send()
        .await;

    match response {
        Ok(r) => {
            r.error_for_status()?;
            Ok(())
        }
        Err(e) if e.is_connect() => Ok(()),
        Err(e) => Err(e.into()),
    }
}

async fn wait_for_engine_down(
    client: &EngineClient,
    timeout: Duration,
) -> Result<(), EngineClientError> {
    let started = Instant::now();
    while client.health().await.unwrap_or(false) {
        if started.elapsed() > timeout {
            return Err(EngineClientError::Message(
                "Engine shutdown timed out while applying runtime policy".to_string(),
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Ok(())
}

fn spawn_host(options: &EngineConnectOptions, token: &str) -> Result<(), EngineClientError> {
    let host_bin = resolve_host_binary(options)?;

    // Write spawn diagnostics before attempting launch.
    let spawn_log_path = options.shared_runtime_dir.join(SPAWN_LOG_FILENAME);
    let spawn_log = write_spawn_diagnostics(&spawn_log_path, &host_bin, options, token);

    let mut cmd = Command::new(&host_bin);
    cmd.arg("--port")
        .arg(options.port.to_string())
        .arg("--data-dir")
        .arg(&options.data_dir)
        .arg("--app-version")
        .arg(&options.app_version)
        .env("SMOLPC_ENGINE_TOKEN", token)
        .env("SMOLPC_ENGINE_PORT", options.port.to_string())
        .env("RUST_LOG", "info");

    if let Some(force_ep) = options.runtime_mode.as_force_override() {
        cmd.env(FORCE_EP_ENV, force_ep);
    } else {
        cmd.env_remove(FORCE_EP_ENV);
    }

    if let Some(device_id) = options.dml_device_id {
        cmd.env(DML_DEVICE_ENV, device_id.to_string());
    } else {
        cmd.env_remove(DML_DEVICE_ENV);
    }

    if let Some(resource_dir) = &options.resource_dir {
        cmd.arg("--resource-dir").arg(resource_dir);
    }
    if let Some(models_dir) = options
        .models_dir
        .as_ref()
        .cloned()
        .or_else(default_shared_models_dir)
    {
        cmd.env("SMOLPC_MODELS_DIR", &models_dir);
    }

    // Redirect engine stderr to the spawn log so crash output is captured.
    let stderr_target = spawn_log
        .and_then(|path| std::fs::File::options().append(true).open(path).ok())
        .map(std::process::Stdio::from)
        .unwrap_or_else(std::process::Stdio::null);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(stderr_target);
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(stderr_target);
    }

    let child = cmd.spawn()?;
    let pid_path = options.shared_runtime_dir.join("engine.pid");
    if let Err(e) = std::fs::write(&pid_path, child.id().to_string()) {
        log::warn!("Failed to write engine PID file {}: {e}", pid_path.display());
    }
    Ok(())
}

/// Write pre-spawn diagnostics to a log file for post-mortem debugging.
fn write_spawn_diagnostics(
    log_path: &Path,
    host_bin: &Path,
    options: &EngineConnectOptions,
    _token: &str,
) -> Option<PathBuf> {
    use std::fmt::Write as _;
    let mut buf = String::new();
    let _ = writeln!(buf, "--- spawn diagnostics {} ---", chrono_stamp());
    let _ = writeln!(buf, "engine_binary: {}", host_bin.display());
    let _ = writeln!(buf, "binary_exists: {}", host_bin.exists());
    let _ = writeln!(buf, "port: {}", options.port);
    let _ = writeln!(
        buf,
        "resource_dir: {}",
        options
            .resource_dir
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".into())
    );
    let _ = writeln!(buf, "data_dir: {}", options.data_dir.display());
    let _ = writeln!(
        buf,
        "shared_runtime_dir: {}",
        options.shared_runtime_dir.display()
    );
    let models_dir = options
        .models_dir
        .as_ref()
        .cloned()
        .or_else(default_shared_models_dir);
    let _ = writeln!(
        buf,
        "models_dir: {}",
        models_dir
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".into())
    );
    let token_path = options.shared_runtime_dir.join("engine-token.txt");
    let _ = writeln!(buf, "token_path: {}", token_path.display());
    let _ = writeln!(buf, "token_exists: {}", token_path.exists());

    // Check libs directory reachable from resource_dir
    if let Some(rd) = &options.resource_dir {
        let libs = rd.join("libs");
        let ov_libs = rd.join("libs").join("openvino");
        let _ = writeln!(buf, "libs_dir_exists: {}", libs.exists());
        let _ = writeln!(buf, "openvino_libs_dir_exists: {}", ov_libs.exists());
    }

    let _ = writeln!(buf, "---");
    match OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .and_then(|mut f| f.write_all(buf.as_bytes()))
    {
        Ok(()) => Some(log_path.to_path_buf()),
        Err(_) => None,
    }
}

fn chrono_stamp() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Convert to rough UTC date-time without pulling in chrono crate.
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    // Days since epoch -> year/month/day (simplified, ignoring leap-second edge cases).
    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}:{seconds:02} UTC")
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let month_lengths: [u64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1;
    for &ml in &month_lengths {
        if days < ml {
            break;
        }
        days -= ml;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

struct SpawnLockGuard {
    path: PathBuf,
}

impl Drop for SpawnLockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

async fn acquire_spawn_lock(
    shared_runtime_dir: &Path,
) -> Result<SpawnLockGuard, EngineClientError> {
    let lock_path = shared_runtime_dir.join(SPAWN_LOCK_FILENAME);
    let started = Instant::now();

    loop {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                let _ = writeln!(file, "pid={}", std::process::id());
                return Ok(SpawnLockGuard { path: lock_path });
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                // Check if lock holder PID is still alive.
                if is_lock_holder_dead(&lock_path) {
                    let _ = std::fs::remove_file(&lock_path);
                    continue;
                }

                let stale = std::fs::metadata(&lock_path)
                    .and_then(|meta| meta.modified())
                    .ok()
                    .and_then(|modified| modified.elapsed().ok())
                    .is_some_and(|age| age > SPAWN_LOCK_STALE_AGE);
                if stale {
                    let _ = std::fs::remove_file(&lock_path);
                    continue;
                }

                if started.elapsed() > SPAWN_LOCK_WAIT {
                    // Force-remove and try exactly once more. If a
                    // concurrent process re-creates the lock, give up.
                    let _ = std::fs::remove_file(&lock_path);
                    return match OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&lock_path)
                    {
                        Ok(mut file) => {
                            let _ = writeln!(file, "pid={}", std::process::id());
                            Ok(SpawnLockGuard { path: lock_path })
                        }
                        Err(_) => Err(EngineClientError::Message(
                            "Timed out waiting for engine spawn lock".to_string(),
                        )),
                    };
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

/// Check if the PID recorded in the lock file is still running.
fn is_lock_holder_dead(lock_path: &Path) -> bool {
    let Ok(contents) = std::fs::read_to_string(lock_path) else {
        return true; // Can't read -> treat as dead.
    };
    let Some(pid_str) = contents.lines().find_map(|line| line.strip_prefix("pid=")) else {
        return true; // No PID recorded -> treat as dead.
    };
    let Ok(pid) = pid_str.trim().parse::<u32>() else {
        return true;
    };

    #[cfg(target_os = "windows")]
    {
        // OpenProcess with SYNCHRONIZE (0x00100000) returns null if process doesn't exist.
        extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
            fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        }
        const SYNCHRONIZE: u32 = 0x0010_0000;
        let handle = unsafe { OpenProcess(SYNCHRONIZE, 0, pid) };
        if handle.is_null() {
            return true; // Process doesn't exist.
        }
        unsafe { CloseHandle(handle) };
        false // Process is alive.
    }

    #[cfg(unix)]
    {
        // kill(pid, 0) checks if process exists without sending a signal.
        unsafe { libc::kill(pid as i32, 0) != 0 }
    }
}

/// Best-effort kill of any stale smolpc-engine-host process.
///
/// This kills all `smolpc-engine-host.exe` processes by name. On a
/// single-app install (the current deployment model) this is correct.
/// A future multi-app setup may need port-specific PID lookup via
/// `Get-NetTCPConnection`.
fn kill_stale_engine_processes() {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "smolpc-engine-host.exe"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        // No blocking sleep -- the health-check loop retries every 100ms
        // and will naturally wait for the port to become available.
    }
}

fn default_shared_models_dir() -> Option<PathBuf> {
    let base = dirs::data_local_dir()?;
    let path = base.join(SHARED_MODELS_VENDOR_DIR).join(SHARED_MODELS_DIR);
    path.exists().then_some(path)
}

fn host_binary_candidates() -> Vec<String> {
    let mut candidates = vec![format!(
        "{}{}",
        ENGINE_HOST_BASENAME,
        std::env::consts::EXE_SUFFIX
    )];
    if let Ok(target_triple) = std::env::var("TAURI_ENV_TARGET_TRIPLE") {
        candidates.push(format!(
            "{}-{}{}",
            ENGINE_HOST_BASENAME,
            target_triple,
            std::env::consts::EXE_SUFFIX
        ));
    }
    candidates
}

fn find_host_binary_in_dir(dir: &Path) -> Option<PathBuf> {
    for candidate in host_binary_candidates() {
        let full_path = dir.join(&candidate);
        if full_path.exists() {
            return Some(full_path);
        }
    }

    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        if file_name.starts_with(ENGINE_HOST_BASENAME) {
            return Some(path);
        }
    }

    None
}

fn resolve_host_binary(options: &EngineConnectOptions) -> Result<PathBuf, EngineClientError> {
    if let Some(path) = &options.host_binary {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    if let Ok(path) = std::env::var("SMOLPC_ENGINE_HOST_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }

    if let Some(resource_dir) = &options.resource_dir {
        if let Some(path) = find_host_binary_in_dir(resource_dir) {
            return Ok(path);
        }
        let binaries_dir = resource_dir.join("binaries");
        if let Some(path) = find_host_binary_in_dir(&binaries_dir) {
            return Ok(path);
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            if let Some(path) = find_host_binary_in_dir(dir) {
                return Ok(path);
            }

            let resources_dir = dir.join("resources");
            if let Some(path) = find_host_binary_in_dir(&resources_dir) {
                return Ok(path);
            }
        }
    }

    #[cfg(debug_assertions)]
    {
        let fallback = PathBuf::from("target").join("debug").join(format!(
            "{}{}",
            ENGINE_HOST_BASENAME,
            std::env::consts::EXE_SUFFIX
        ));
        if fallback.exists() {
            return Ok(fallback);
        }

        let fallback_release = PathBuf::from("target").join("release").join(format!(
            "{}{}",
            ENGINE_HOST_BASENAME,
            std::env::consts::EXE_SUFFIX
        ));
        if fallback_release.exists() {
            return Ok(fallback_release);
        }
    }

    Err(EngineClientError::Message(
        "Unable to locate smolpc-engine-host binary".to_string(),
    ))
}
