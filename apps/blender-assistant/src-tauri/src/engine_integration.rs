use crate::ollama::OllamaMetrics;
use smolpc_engine_client::{
    connect_or_spawn, read_runtime_env_overrides, EngineClient, EngineClientError,
    EngineConnectOptions, RuntimeModePreference,
};
use smolpc_engine_core::{GenerationConfig, GenerationMetrics};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const DEFAULT_ENGINE_PORT: u16 = 19432;
const DEFAULT_MODEL: &str = "qwen2.5-coder-1.5b";
const MODEL_FALLBACKS: &[&str] = &["qwen3.5-2b", "qwen3-4b-instruct-2507"];
const DEFAULT_MAX_TOKENS_NON_STREAM: usize = 768;
const DEFAULT_MAX_TOKENS_STREAM: usize = 1024;
const ENGINE_AVAILABILITY_CACHE_TTL: Duration = Duration::from_secs(10);
const SHARED_MODELS_VENDOR_DIR: &str = "SmolPC";
const SHARED_MODELS_DIR: &str = "models";

static ENGINE_RUNTIME: OnceLock<EngineRuntime> = OnceLock::new();
static LOADED_MODEL_ID: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

struct EngineRuntime {
    client: Arc<Mutex<Option<EngineClient>>>,
    connect_lock: Arc<Mutex<()>>,
    availability_cache: std::sync::Mutex<Option<(bool, Instant)>>,
    resource_dir: RwLock<Option<PathBuf>>,
    shared_runtime_dir: PathBuf,
    data_dir: PathBuf,
    port: u16,
    app_version: String,
    runtime_mode: RuntimeModePreference,
    dml_device_id: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct EngineStatusInfo {
    pub connected: bool,
    pub current_model: Option<String>,
}

pub fn configure(resource_dir: Option<PathBuf>) {
    let runtime = runtime();
    let Ok(mut guard) = runtime.resource_dir.write() else {
        return;
    };
    *guard = resource_dir;
}

pub fn invalidate_availability_cache() {
    if let Ok(mut guard) = runtime().availability_cache.lock() {
        *guard = None;
    }
}

pub fn is_engine_connection_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    err.contains("ENGINE_UNREACHABLE")
        || lower.contains("connection refused")
        || lower.contains("connect error")
        || lower.contains("tcp connect error")
        || lower.contains("failed to connect or spawn engine host")
        || lower.contains("failed to become healthy")
        || lower.contains("timed out waiting for engine spawn lock")
        || lower.contains("unable to locate smolpc-engine-host binary")
        || lower.contains("engine protocol mismatch")
        || lower.contains("shared engine client is unavailable")
}

pub fn is_model_not_loaded_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("no model loaded")
        || lower.contains("model is not loaded")
        || lower.contains("call /engine/load first")
        || (lower.contains("/engine/load") && lower.contains("model"))
}

pub async fn is_engine_available() -> bool {
    if let Ok(guard) = runtime().availability_cache.lock() {
        if let Some((cached, timestamp)) = *guard {
            if timestamp.elapsed() < ENGINE_AVAILABILITY_CACHE_TTL {
                return cached;
            }
        }
    }

    let available = is_engine_available_uncached().await;
    if let Ok(mut guard) = runtime().availability_cache.lock() {
        *guard = Some((available, Instant::now()));
    }
    available
}

pub async fn stream_chat<F>(
    system_prompt: &str,
    user_prompt: &str,
    cancelled: Arc<AtomicBool>,
    mut on_token: F,
) -> Result<OllamaMetrics, String>
where
    F: FnMut(String),
{
    let client = resolve_client(false).await?;
    let prompt = compose_chat_prompt(system_prompt, user_prompt);
    let config = Some(build_generation_config(0.7, DEFAULT_MAX_TOKENS_STREAM));

    if cancelled.load(Ordering::Relaxed) {
        let _ = client.cancel().await;
        return Err("GENERATION_CANCELLED: Generation cancelled by user".to_string());
    }

    let cancelled_for_callback = Arc::clone(&cancelled);
    let mut generation = std::pin::pin!(client.generate_stream(&prompt, config, move |token| {
        if !cancelled_for_callback.load(Ordering::Relaxed) {
            on_token(token);
        }
    }));

    tokio::select! {
        result = &mut generation => {
            let metrics = result.map_err(map_engine_error)?;
            if cancelled.load(Ordering::Relaxed) {
                let _ = client.cancel().await;
                return Err("GENERATION_CANCELLED: Generation cancelled by user".to_string());
            }
            Ok(to_ollama_metrics(metrics))
        }
        _ = wait_for_cancel(Arc::clone(&cancelled)) => {
            let _ = client.cancel().await;
            Err("GENERATION_CANCELLED: Generation cancelled by user".to_string())
        }
    }
}

pub async fn chat_once(
    system_prompt: &str,
    user_prompt: &str,
    temperature: f64,
) -> Result<String, String> {
    let client = resolve_client(false).await?;
    let prompt = compose_chat_prompt(system_prompt, user_prompt);
    let config = Some(build_generation_config(
        temperature,
        DEFAULT_MAX_TOKENS_NON_STREAM,
    ));
    let result = client
        .generate_text(&prompt, config)
        .await
        .map_err(map_engine_error)?;
    let content = result.text.trim().to_string();
    if content.is_empty() {
        return Err("ENGINE_RUNTIME_ERROR: Shared engine returned an empty response".to_string());
    }
    Ok(content)
}

pub async fn engine_status() -> Result<EngineStatusInfo, String> {
    let client = runtime()
        .client
        .lock()
        .await
        .clone()
        .ok_or_else(|| "ENGINE_UNREACHABLE: Shared engine client is unavailable".to_string())?;
    let status = client.status().await.map_err(map_engine_error)?;

    if let Some(current_model) = status
        .current_model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        set_loaded_model(current_model.to_string());
    }

    Ok(EngineStatusInfo {
        connected: true,
        current_model: status.current_model,
    })
}

pub async fn ensure_model_loaded() -> Result<String, String> {
    let client = resolve_client(false).await?;
    let preferred_model = resolve_preferred_model();
    let explicit_preference = preferred_model_from_env().is_some();
    let mut existing_model: Option<String> = None;

    if let Ok(status) = client.status().await {
        if let Some(current_model) = status
            .current_model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let current_model = current_model.to_string();
            existing_model = Some(current_model.clone());
            set_loaded_model(current_model.clone());

            if current_model == preferred_model {
                return Ok(current_model);
            }

            if !explicit_preference {
                return Ok(current_model);
            }

            log::info!(
                "[SharedEngine] Current model '{}' differs from preferred '{}'; attempting switch",
                current_model,
                preferred_model
            );
        }
    }

    let candidates = model_candidates(&preferred_model);
    let mut load_errors: Vec<String> = Vec::new();
    let mut any_model_exists = false;

    for model_id in &candidates {
        match client.check_model_readiness(model_id).await {
            Ok(readiness) if readiness.any_ready() => {
                any_model_exists = true;
                log::info!(
                    "[SharedEngine] Model '{}' exists, attempting load",
                    model_id
                );
                match client.load_model(model_id).await {
                    Ok(()) => {
                        log::info!("[SharedEngine] Model '{}' loaded successfully", model_id);
                        set_loaded_model(model_id.clone());
                        return Ok(model_id.clone());
                    }
                    Err(error) => {
                        let mapped = map_engine_error(error);
                        log::info!(
                            "[SharedEngine] Model '{}' load failed: {}",
                            model_id,
                            mapped
                        );
                        load_errors.push(format!("{} => {}", model_id, mapped));
                    }
                }
            }
            Ok(_) => {
                log::info!("[SharedEngine] Model '{}' not found on engine", model_id);
            }
            Err(error) => {
                let mapped = map_engine_error(error);
                log::info!(
                    "[SharedEngine] Model '{}' check error: {}",
                    model_id,
                    mapped
                );
                load_errors.push(format!("{} => {}", model_id, mapped));
            }
        }
    }

    if !any_model_exists {
        if let Some(current_model) = existing_model {
            log::info!(
                "[SharedEngine] Preferred model unavailable; keeping current loaded model '{}'",
                current_model
            );
            set_loaded_model(current_model.clone());
            return Ok(current_model);
        }

        let models_dir = resolve_models_dir(configured_resource_dir().as_ref())
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "unresolved".to_string());
        return Err(format!(
            "No shared model artifacts found for [{}] under '{}'. Ensure bundled resources include models or run `npm run bundle:stage:model` before building.",
            candidates.join(", "),
            models_dir
        ));
    }

    if let Some(current_model) = existing_model {
        log::info!(
            "[SharedEngine] Preferred model load failed; keeping current loaded model '{}'",
            current_model
        );
        set_loaded_model(current_model.clone());
        return Ok(current_model);
    }

    Err(format!(
        "Shared model artifacts were found, but engine load failed: {}",
        load_errors.join(" | ")
    ))
}

pub async fn ensure_engine_running(resource_dir: Option<&Path>) -> Result<bool, String> {
    if let Some(resource_dir) = resource_dir {
        configure(Some(resource_dir.to_path_buf()));
    }

    let was_running = if let Some(client) = runtime().client.lock().await.clone() {
        client.health().await.unwrap_or(false)
    } else {
        is_port_open(runtime().port)
    };

    let _ = resolve_client(false).await?;
    if let Ok(mut guard) = runtime().availability_cache.lock() {
        *guard = Some((true, Instant::now()));
    }
    Ok(!was_running)
}

pub async fn shutdown_engine() -> Result<(), String> {
    let runtime = runtime();
    let client = if let Some(client) = runtime.client.lock().await.clone() {
        client
    } else {
        if !is_port_open(runtime.port) {
            return Ok(());
        }
        resolve_client(false).await?
    };

    client.shutdown().await.map_err(map_engine_error)?;
    if let Err(error) = client.wait_for_shutdown(Duration::from_secs(5)).await {
        log::warn!("[SharedEngine] Engine shutdown wait failed: {}", error);
    }

    *runtime.client.lock().await = None;
    if let Ok(mut guard) = runtime.availability_cache.lock() {
        *guard = Some((false, Instant::now()));
    }
    Ok(())
}

pub fn cleanup_stale_engine() {
    // connect_or_spawn() handles runtime lock coordination and startup races.
}

fn runtime() -> &'static EngineRuntime {
    ENGINE_RUNTIME.get_or_init(build_runtime)
}

fn build_runtime() -> EngineRuntime {
    let runtime_overrides = read_runtime_env_overrides();
    let shared_runtime_dir = dirs::data_local_dir()
        .map(|base| base.join(SHARED_MODELS_VENDOR_DIR).join("engine-runtime"))
        .or_else(|| {
            std::env::var("LOCALAPPDATA")
                .ok()
                .map(PathBuf::from)
                .map(|base| base.join(SHARED_MODELS_VENDOR_DIR).join("engine-runtime"))
        })
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("engine-runtime"));

    EngineRuntime {
        client: Arc::new(Mutex::new(None)),
        connect_lock: Arc::new(Mutex::new(())),
        availability_cache: std::sync::Mutex::new(None),
        resource_dir: RwLock::new(None),
        data_dir: shared_runtime_dir.join("host-data"),
        shared_runtime_dir,
        port: std::env::var("SMOLPC_ENGINE_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(DEFAULT_ENGINE_PORT),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        runtime_mode: runtime_overrides.runtime_mode,
        dml_device_id: runtime_overrides.dml_device_id,
    }
}

fn set_loaded_model(model_id: String) {
    if let Ok(mut guard) = LOADED_MODEL_ID.lock() {
        *guard = Some(model_id);
    }
}

fn get_loaded_model() -> Option<String> {
    LOADED_MODEL_ID.lock().ok().and_then(|guard| guard.clone())
}

async fn resolve_client(force_respawn: bool) -> Result<EngineClient, String> {
    let runtime = runtime();

    if !force_respawn {
        if let Some(client) = runtime.client.lock().await.clone() {
            if client.health().await.unwrap_or(false) {
                return Ok(client);
            }
            log::warn!("Cached shared engine client is unhealthy; reconnecting");
            *runtime.client.lock().await = None;
        }
    }

    let _guard = runtime.connect_lock.lock().await;

    if !force_respawn {
        if let Some(client) = runtime.client.lock().await.clone() {
            if client.health().await.unwrap_or(false) {
                return Ok(client);
            }
            log::warn!("Cached shared engine client is unhealthy after lock; reconnecting");
            *runtime.client.lock().await = None;
        }
    } else {
        *runtime.client.lock().await = None;
    }

    let resource_dir =
        configured_resource_dir().or_else(|| Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))));
    let models_dir = resolve_models_dir(resource_dir.as_ref());
    let host_binary = resolve_host_binary_path();

    let options = EngineConnectOptions {
        port: runtime.port,
        app_version: runtime.app_version.clone(),
        shared_runtime_dir: runtime.shared_runtime_dir.clone(),
        data_dir: runtime.data_dir.clone(),
        resource_dir,
        models_dir,
        host_binary,
        runtime_mode: runtime.runtime_mode,
        dml_device_id: runtime.dml_device_id,
        force_respawn,
    };

    log::info!(
        "Resolving shared engine client: port={} runtime_mode={} dml_device_id={:?} force_respawn={}",
        options.port,
        runtime_mode_label(options.runtime_mode),
        options.dml_device_id,
        options.force_respawn
    );

    let client = connect_or_spawn(options)
        .await
        .map_err(|error| format!("Failed to connect or spawn engine host: {}", error))?;
    *runtime.client.lock().await = Some(client.clone());
    Ok(client)
}

fn runtime_mode_label(mode: RuntimeModePreference) -> &'static str {
    match mode {
        RuntimeModePreference::Auto => "auto",
        RuntimeModePreference::Cpu => "cpu",
        RuntimeModePreference::Dml => "dml",
    }
}

fn configured_resource_dir() -> Option<PathBuf> {
    runtime()
        .resource_dir
        .read()
        .ok()
        .and_then(|guard| guard.clone())
}

fn resolve_models_dir(resource_dir: Option<&PathBuf>) -> Option<PathBuf> {
    if let Ok(override_dir) = std::env::var("SMOLPC_MODELS_DIR") {
        let path = PathBuf::from(override_dir);
        if path.exists() {
            return Some(path);
        }
    }

    if let Some(base) = dirs::data_local_dir() {
        let shared = base.join(SHARED_MODELS_VENDOR_DIR).join(SHARED_MODELS_DIR);
        if shared.exists() {
            return Some(shared);
        }
    }

    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models");
    if dev_path.exists() {
        return Some(dev_path);
    }

    if let Some(res_dir) = resource_dir {
        let direct = res_dir.join("models");
        if direct.exists() {
            return Some(direct);
        }

        let nested = res_dir.join("resources").join("models");
        if nested.exists() {
            return Some(nested);
        }
    }

    None
}

fn resolve_host_binary_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("SMOLPC_ENGINE_HOST_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    let workspace_target = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("target")
        .join(if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        })
        .join(format!(
            "smolpc-engine-host{}",
            std::env::consts::EXE_SUFFIX
        ));
    if workspace_target.exists() {
        return Some(workspace_target);
    }

    None
}

async fn is_engine_available_uncached() -> bool {
    let Some(client) = runtime().client.lock().await.clone() else {
        return false;
    };
    client.health().await.unwrap_or(false)
}

fn map_engine_error(error: EngineClientError) -> String {
    let message = error.to_string();

    if message.starts_with("ENGINE_") || message.starts_with("GENERATION_CANCELLED") {
        return message;
    }

    if is_engine_connection_error(&message) {
        return format!("ENGINE_UNREACHABLE: {}", message);
    }

    let lower = message.to_ascii_lowercase();
    if lower.contains("http 401") || lower.contains("http 403") {
        return format!("ENGINE_AUTH_FAILED: {}", message);
    }
    if lower.contains("http 429") {
        return format!("ENGINE_QUEUE_FULL: {}", message);
    }
    if lower.contains("http 504") {
        return format!("ENGINE_QUEUE_TIMEOUT: {}", message);
    }

    format!("ENGINE_RUNTIME_ERROR: {}", message)
}

fn compose_chat_prompt(system_prompt: &str, user_prompt: &str) -> String {
    format!(
        "System instructions:\n{}\n\nUser request:\n{}",
        system_prompt, user_prompt
    )
}

fn build_generation_config(temperature: f64, max_tokens: usize) -> GenerationConfig {
    GenerationConfig {
        max_length: max_tokens,
        temperature: temperature as f32,
        ..GenerationConfig::default()
    }
}

fn to_ollama_metrics(metrics: GenerationMetrics) -> OllamaMetrics {
    OllamaMetrics {
        total_tokens: metrics.total_tokens as u64,
        total_time_ms: metrics.total_time_ms,
        tokens_per_second: metrics.tokens_per_second,
    }
}

async fn wait_for_cancel(cancelled: Arc<AtomicBool>) {
    while !cancelled.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn preferred_model_from_env() -> Option<String> {
    std::env::var("SHARED_ENGINE_MODEL_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn resolve_preferred_model() -> String {
    preferred_model_from_env().unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

fn model_candidates(preferred: &str) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();

    let mut push_unique = |candidate: String| {
        let candidate = candidate.trim().to_string();
        if candidate.is_empty() {
            return;
        }
        if !candidates.iter().any(|existing| existing == &candidate) {
            candidates.push(candidate);
        }
    };

    push_unique(preferred.to_string());
    if let Some(current) = get_loaded_model() {
        push_unique(current);
    }
    push_unique(DEFAULT_MODEL.to_string());
    for fallback in MODEL_FALLBACKS {
        push_unique((*fallback).to_string());
    }

    candidates
}

fn is_port_open(port: u16) -> bool {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok()
}
