use crate::ollama::OllamaMetrics;
use futures_util::StreamExt;
use rand::distributions::Alphanumeric;
use rand::Rng;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const DEFAULT_ENGINE_BASE_URL: &str = "http://127.0.0.1:19432";
const DEFAULT_MODEL: &str = "qwen2.5-1.5b-instruct";
const MODEL_FALLBACKS: &[&str] = &["qwen3-4b"];
const DEFAULT_MAX_TOKENS_NON_STREAM: usize = 768;
const DEFAULT_MAX_TOKENS_STREAM: usize = 1024;
const ENGINE_AVAILABILITY_CACHE_TTL: Duration = Duration::from_secs(10);
const MODELS_DIR_OVERRIDE_ENV: &str = "SMOLPC_MODELS_DIR";
const SHARED_MODELS_VENDOR_DIR: &str = "SmolPC";
const SHARED_MODELS_DIR: &str = "models";

static ENGINE_AVAILABLE_CACHE: std::sync::Mutex<Option<(bool, Instant)>> =
    std::sync::Mutex::new(None);

static LOADED_MODEL_ID: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

static ENGINE_PID: std::sync::Mutex<Option<u32>> = std::sync::Mutex::new(None);

pub fn set_loaded_model(id: String) {
    if let Ok(mut guard) = LOADED_MODEL_ID.lock() {
        *guard = Some(id);
    }
}

pub fn get_loaded_model() -> Option<String> {
    LOADED_MODEL_ID.lock().ok().and_then(|guard| guard.clone())
}

#[derive(Debug, Clone)]
pub struct EngineStatusInfo {
    pub connected: bool,
    pub current_model: Option<String>,
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    stream: bool,
    messages: Vec<ChatMessage>,
    temperature: f64,
    max_tokens: usize,
}

#[derive(Serialize)]
struct EngineLoadRequest<'a> {
    model_id: &'a str,
}

#[derive(Serialize)]
struct EngineCheckModelRequest<'a> {
    model_id: &'a str,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct EngineHealthResponse {
    #[serde(default)]
    ok: bool,
}

#[derive(Debug, Deserialize)]
struct EngineStatusResponse {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    current_model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EngineCheckModelResponse {
    #[serde(default)]
    exists: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct EngineGenerationMetrics {
    #[serde(default)]
    total_tokens: usize,
    #[serde(default)]
    tokens_per_second: f64,
    #[serde(default)]
    total_time_ms: u64,
}

/// Invalidate the engine availability cache (call on connection errors).
pub fn invalidate_availability_cache() {
    if let Ok(mut guard) = ENGINE_AVAILABLE_CACHE.lock() {
        *guard = None;
    }
}

/// Check if an error string indicates the engine is unreachable.
pub fn is_engine_connection_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    err.contains("ENGINE_UNREACHABLE")
        || lower.contains("connection refused")
        || lower.contains("connect error")
        || lower.contains("error sending request")
        || lower.contains("shared engine request failed")
        || lower.contains("tcp connect error")
        || err.contains("Stream read error")
}

/// Check if an error string indicates the engine has no model loaded.
pub fn is_model_not_loaded_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("no model loaded")
        || lower.contains("model is not loaded")
        || lower.contains("call /engine/load first")
        || (lower.contains("/engine/load") && lower.contains("model"))
}

pub async fn is_engine_available() -> bool {
    if let Ok(guard) = ENGINE_AVAILABLE_CACHE.lock() {
        if let Some((cached, timestamp)) = *guard {
            if timestamp.elapsed() < ENGINE_AVAILABILITY_CACHE_TTL {
                return cached;
            }
        }
    }

    let available = is_engine_available_uncached().await;
    if let Ok(mut guard) = ENGINE_AVAILABLE_CACHE.lock() {
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
    let client = build_client(Duration::from_secs(2), Duration::from_secs(600))?;
    let token = read_engine_token()?;
    let request = build_chat_request(
        system_prompt,
        user_prompt,
        true,
        0.7,
        DEFAULT_MAX_TOKENS_STREAM,
    );

    let response = client
        .post(url("/v1/chat/completions"))
        .bearer_auth(&token)
        .json(&request)
        .send()
        .await
        .map_err(map_request_error)?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(map_http_error(status, &body));
    }

    let started = Instant::now();
    let mut emitted_chunks: usize = 0;
    let mut host_metrics: Option<EngineGenerationMetrics> = None;
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        if cancelled.load(Ordering::Relaxed) {
            cancel_best_effort(&client, &token).await;
            return Err("GENERATION_CANCELLED: Generation cancelled by user".to_string());
        }

        let chunk = chunk.map_err(|e| format!("ENGINE_RUNTIME_ERROR: Stream read error: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_pos) = buffer.find('\n') {
            let mut remainder = buffer.split_off(newline_pos + 1);
            std::mem::swap(&mut remainder, &mut buffer);
            let line = remainder.trim();

            if !line.starts_with("data:") {
                continue;
            }

            let data = line[5..].trim();
            if let Some(metrics) = process_sse_data(
                data,
                &mut emitted_chunks,
                &mut host_metrics,
                started,
                &mut on_token,
            )? {
                return Ok(metrics);
            }
        }
    }

    let trailing = buffer.trim();
    if trailing.starts_with("data:") {
        let data = trailing[5..].trim();
        if let Some(metrics) = process_sse_data(
            data,
            &mut emitted_chunks,
            &mut host_metrics,
            started,
            &mut on_token,
        )? {
            return Ok(metrics);
        }
    }

    if cancelled.load(Ordering::Relaxed) {
        cancel_best_effort(&client, &token).await;
        return Err("GENERATION_CANCELLED: Generation cancelled by user".to_string());
    }

    Ok(finalize_metrics(host_metrics, emitted_chunks, started))
}

pub async fn chat_once(
    system_prompt: &str,
    user_prompt: &str,
    temperature: f64,
) -> Result<String, String> {
    let client = build_client(Duration::from_secs(2), Duration::from_secs(600))?;
    let token = read_engine_token()?;
    let request = build_chat_request(
        system_prompt,
        user_prompt,
        false,
        temperature,
        DEFAULT_MAX_TOKENS_NON_STREAM,
    );

    let response = client
        .post(url("/v1/chat/completions"))
        .bearer_auth(&token)
        .json(&request)
        .send()
        .await
        .map_err(map_request_error)?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(map_http_error(status, &body));
    }

    let value: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("ENGINE_RUNTIME_ERROR: Failed to decode engine response: {}", e))?;

    if let Some((code, message)) = parse_error_payload(&value) {
        return Err(normalize_engine_error(code.as_deref(), &message));
    }

    let content = value
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if content.is_empty() {
        return Err("ENGINE_RUNTIME_ERROR: Shared engine returned an empty response".to_string());
    }

    Ok(content)
}

pub async fn engine_status() -> Result<EngineStatusInfo, String> {
    let client = build_client(Duration::from_secs(2), Duration::from_secs(5))?;
    let token = read_engine_token()?;

    let response = client
        .get(url("/engine/status"))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(map_request_error)?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(map_http_error(status, &body));
    }

    let status_code = response.status();
    let body: EngineStatusResponse = response
        .json()
        .await
        .map_err(|e| format!("ENGINE_RUNTIME_ERROR: Failed to decode engine status: {}", e))?;

    Ok(EngineStatusInfo {
        connected: body.ok || status_code.is_success(),
        current_model: body.current_model,
    })
}

pub async fn ensure_model_loaded() -> Result<String, String> {
    let preferred_model = resolve_preferred_model();
    let explicit_preference = preferred_model_from_env().is_some();
    let mut existing_model: Option<String> = None;

    if let Ok(status) = engine_status().await {
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
                "[SharedEngine] Current model '{}' differs from preferred '{}'; attempting switch.",
                current_model,
                preferred_model
            );
        }
    }

    let candidates = model_candidates(&preferred_model);
    let mut load_errors: Vec<String> = Vec::new();
    let mut any_model_exists = false;

    for model_id in &candidates {
        match check_model_exists(model_id).await {
            Ok(true) => {
                any_model_exists = true;
                log::info!("[SharedEngine] Model '{}' exists, attempting load...", model_id);
                match load_model(model_id).await {
                    Ok(()) => {
                        log::info!("[SharedEngine] Model '{}' loaded successfully", model_id);
                        set_loaded_model(model_id.clone());
                        return Ok(model_id.clone());
                    }
                    Err(err) => {
                        log::info!("[SharedEngine] Model '{}' load failed: {}", model_id, err);
                        load_errors.push(format!("{} => {}", model_id, err));
                    }
                }
            }
            Ok(false) => {
                log::info!("[SharedEngine] Model '{}' not found on engine", model_id);
            }
            Err(err) => {
                log::info!("[SharedEngine] Model '{}' check error: {}", model_id, err);
                load_errors.push(format!("{} => {}", model_id, err));
            }
        }
    }

    if !any_model_exists {
        if let Some(current_model) = existing_model.clone() {
            log::info!(
                "[SharedEngine] Preferred model unavailable; keeping current loaded model '{}'",
                current_model
            );
            set_loaded_model(current_model.clone());
            return Ok(current_model);
        }

        let models_dir = resolve_models_dir(None);
        log::info!(
            "[SharedEngine] No loadable models found. Ensure bundled models exist (resources/models) or run 'npm run bundle:stage:model'."
        );
        return Err(format!(
            "No shared model artifacts found for [{}] under '{}'. Ensure bundled resources include models or run `npm run bundle:stage:model` before building.",
            candidates.join(", "),
            models_dir.display()
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

async fn is_engine_available_uncached() -> bool {
    let client = match build_client(Duration::from_secs(2), Duration::from_secs(2)) {
        Ok(client) => client,
        Err(_) => return false,
    };

    let token = match read_engine_token() {
        Ok(token) => token,
        Err(_) => return false,
    };

    match client
        .get(url("/engine/health"))
        .bearer_auth(token)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => match response.json::<EngineHealthResponse>().await {
            Ok(health) => health.ok,
            Err(_) => true,
        },
        _ => false,
    }
}

async fn load_model(model_id: &str) -> Result<(), String> {
    let client = build_client(Duration::from_secs(2), Duration::from_secs(120))?;
    let token = read_engine_token()?;

    let response = client
        .post(url("/engine/load"))
        .bearer_auth(&token)
        .json(&EngineLoadRequest { model_id })
        .send()
        .await
        .map_err(map_request_error)?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(map_http_error(status, &body));
    }

    Ok(())
}

async fn check_model_exists(model_id: &str) -> Result<bool, String> {
    let client = build_client(Duration::from_secs(2), Duration::from_secs(20))?;
    let token = read_engine_token()?;

    let response = client
        .post(url("/engine/check-model"))
        .bearer_auth(&token)
        .json(&EngineCheckModelRequest { model_id })
        .send()
        .await
        .map_err(map_request_error)?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(map_http_error(status, &body));
    }

    let body: EngineCheckModelResponse = response
        .json()
        .await
        .map_err(|e| format!("ENGINE_RUNTIME_ERROR: Failed to decode check-model response: {}", e))?;
    Ok(body.exists)
}

fn build_chat_request(
    system_prompt: &str,
    user_prompt: &str,
    stream: bool,
    temperature: f64,
    max_tokens: usize,
) -> ChatCompletionRequest {
    ChatCompletionRequest {
        model: resolve_model(),
        stream,
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            },
        ],
        temperature,
        max_tokens,
    }
}

fn resolve_model() -> String {
    // 1. Check the static loaded model (set after successful engine load)
    if let Some(model) = get_loaded_model() {
        return model;
    }

    // 2. Resolve preferred model override/default
    resolve_preferred_model()
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
    push_unique(DEFAULT_MODEL.to_string());
    for fallback in MODEL_FALLBACKS {
        push_unique((*fallback).to_string());
    }

    candidates
}

fn shared_models_dir() -> Option<PathBuf> {
    std::env::var("LOCALAPPDATA")
        .ok()
        .map(PathBuf::from)
        .map(|base| base.join(SHARED_MODELS_VENDOR_DIR).join(SHARED_MODELS_DIR))
}

fn bundled_models_dir(resource_dir: &Path) -> Option<PathBuf> {
    let direct = resource_dir.join("models");
    if direct.exists() {
        return Some(direct);
    }

    let nested = resource_dir.join("resources").join("models");
    if nested.exists() {
        return Some(nested);
    }

    None
}

fn resolve_models_dir(resource_dir: Option<&Path>) -> PathBuf {
    if let Ok(override_dir) = std::env::var(MODELS_DIR_OVERRIDE_ENV) {
        let override_path = PathBuf::from(override_dir.trim());
        if !override_path.as_os_str().is_empty() {
            return override_path;
        }
    }

    if let Some(resource_dir) = resource_dir {
        if let Some(bundled) = bundled_models_dir(resource_dir) {
            return bundled;
        }
    }

    if let Some(shared) = shared_models_dir() {
        if shared.exists() {
            return shared;
        }
    }

    #[cfg(debug_assertions)]
    {
        let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models");
        if dev_path.exists() {
            return dev_path;
        }
    }

    #[cfg(debug_assertions)]
    {
        return shared_models_dir()
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models"));
    }

    #[cfg(not(debug_assertions))]
    {
        shared_models_dir().unwrap_or_else(|| PathBuf::from("models"))
    }
}

async fn cancel_best_effort(client: &Client, token: &str) {
    let _ = client
        .post(url("/engine/cancel"))
        .bearer_auth(token)
        .send()
        .await;
}

fn process_sse_data<F>(
    data: &str,
    emitted_chunks: &mut usize,
    host_metrics: &mut Option<EngineGenerationMetrics>,
    started: Instant,
    on_token: &mut F,
) -> Result<Option<OllamaMetrics>, String>
where
    F: FnMut(String),
{
    if data.is_empty() {
        return Ok(None);
    }

    if data == "[DONE]" {
        return Ok(Some(finalize_metrics(
            host_metrics.take(),
            *emitted_chunks,
            started,
        )));
    }

    let value: serde_json::Value = match serde_json::from_str(data) {
        Ok(value) => value,
        Err(e) => {
            log::warn!("Failed to parse shared engine SSE chunk: {} (data: {})", e, data);
            return Ok(None);
        }
    };

    if let Some((code, message)) = parse_error_payload(&value) {
        return Err(normalize_engine_error(code.as_deref(), &message));
    }

    if let Some(metrics_value) = value.get("smolpc_metrics") {
        match serde_json::from_value::<EngineGenerationMetrics>(metrics_value.clone()) {
            Ok(metrics) => *host_metrics = Some(metrics),
            Err(e) => log::warn!("Failed to decode shared engine metrics payload: {}", e),
        }
        return Ok(None);
    }

    if let Some(content) = extract_chunk_content(&value) {
        *emitted_chunks += 1;
        on_token(content);
    }

    Ok(None)
}

fn extract_chunk_content(value: &serde_json::Value) -> Option<String> {
    value
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("delta"))
        .and_then(|delta| delta.get("content"))
        .and_then(|content| content.as_str())
        .filter(|content| !content.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_error_payload(value: &serde_json::Value) -> Option<(Option<String>, String)> {
    let error = value.get("error")?;
    let message = error
        .get("message")
        .and_then(|message| message.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| error.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| error.to_string());
    let code = error
        .get("code")
        .and_then(|code| code.as_str())
        .map(ToOwned::to_owned);
    Some((code, message))
}

fn normalize_engine_error(code: Option<&str>, message: &str) -> String {
    match code {
        Some("INFERENCE_GENERATION_CANCELLED") => {
            format!("GENERATION_CANCELLED: {}", message)
        }
        Some(code) if !code.trim().is_empty() => format!("{}: {}", code, message),
        _ => format!("ENGINE_RUNTIME_ERROR: {}", message),
    }
}

fn finalize_metrics(
    host_metrics: Option<EngineGenerationMetrics>,
    emitted_chunks: usize,
    started: Instant,
) -> OllamaMetrics {
    if let Some(host_metrics) = host_metrics {
        return OllamaMetrics {
            total_tokens: host_metrics.total_tokens as u64,
            total_time_ms: host_metrics.total_time_ms,
            tokens_per_second: host_metrics.tokens_per_second,
        };
    }

    let total_time_ms = started.elapsed().as_millis() as u64;
    let tokens_per_second = if emitted_chunks > 0 && total_time_ms > 0 {
        emitted_chunks as f64 / (total_time_ms as f64 / 1000.0)
    } else {
        0.0
    };

    OllamaMetrics {
        total_tokens: emitted_chunks as u64,
        total_time_ms,
        tokens_per_second,
    }
}

fn build_client(connect_timeout: Duration, timeout: Duration) -> Result<Client, String> {
    Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(timeout)
        .build()
        .map_err(|e| format!("ENGINE_RUNTIME_ERROR: Failed to build HTTP client: {}", e))
}

fn url(path: &str) -> String {
    let base = std::env::var("ENGINE_BASE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_ENGINE_BASE_URL.to_string());
    format!("{}{}", base, path)
}

fn read_engine_token() -> Result<String, String> {
    let path = engine_token_path()?;
    let token = std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "ENGINE_AUTH_FAILED: Failed to read engine token at {}: {}",
            path.display(),
            e
        )
    })?;
    let token = token.trim();
    if token.is_empty() {
        return Err(format!(
            "ENGINE_AUTH_FAILED: Engine token file is empty at {}",
            path.display()
        ));
    }
    Ok(token.to_string())
}

fn engine_token_path() -> Result<PathBuf, String> {
    let local_app_data = std::env::var("LOCALAPPDATA").map_err(|_| {
        "ENGINE_AUTH_FAILED: LOCALAPPDATA is not set; cannot locate shared engine token".to_string()
    })?;
    Ok(PathBuf::from(local_app_data)
        .join("SmolPC")
        .join("engine-runtime")
        .join("engine-token.txt"))
}

fn map_request_error(err: reqwest::Error) -> String {
    if err.is_connect() || err.is_timeout() {
        format!(
            "ENGINE_UNREACHABLE: Shared engine is not reachable at {}",
            url("")
        )
    } else {
        format!("ENGINE_RUNTIME_ERROR: Shared engine request failed: {}", err)
    }
}

fn map_http_error(status: StatusCode, body: &str) -> String {
    let detail = extract_error_detail(body)
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));

    match status.as_u16() {
        401 | 403 => format!("ENGINE_AUTH_FAILED: {}", detail),
        429 => format!("ENGINE_QUEUE_FULL: {}", detail),
        504 => format!("ENGINE_QUEUE_TIMEOUT: {}", detail),
        _ => format!("ENGINE_RUNTIME_ERROR: {}", detail),
    }
}

fn extract_error_detail(body: &str) -> Option<String> {
    let body = body.trim();
    if body.is_empty() {
        return None;
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some((_, message)) = parse_error_payload(&value) {
            return Some(message);
        }
    }

    Some(body.chars().take(240).collect())
}

// ---------------------------------------------------------------------------
// Engine spawn / lifecycle
// ---------------------------------------------------------------------------

fn engine_runtime_dir() -> Result<PathBuf, String> {
    let local_app_data = std::env::var("LOCALAPPDATA").map_err(|_| {
        "LOCALAPPDATA is not set; cannot locate engine runtime directory".to_string()
    })?;
    let dir = PathBuf::from(local_app_data)
        .join("SmolPC")
        .join("engine-runtime");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create engine runtime dir: {}", e))?;
    Ok(dir)
}

fn ensure_token() -> Result<String, String> {
    let runtime_dir = engine_runtime_dir()?;
    let token_path = runtime_dir.join("engine-token.txt");

    if let Ok(contents) = std::fs::read_to_string(&token_path) {
        let trimmed = contents.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }

    let token: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .map(char::from)
        .take(48)
        .collect();

    std::fs::write(&token_path, &token)
        .map_err(|e| format!("Failed to write engine token: {}", e))?;
    Ok(token)
}

fn resolve_engine_binary(resource_dir: &Path) -> Result<PathBuf, String> {
    let exe_name = format!("smolpc-engine-host{}", std::env::consts::EXE_SUFFIX);

    // 1. resource_dir/binaries/<exe>
    let candidate = resource_dir.join("binaries").join(&exe_name);
    if candidate.exists() {
        return Ok(candidate);
    }

    // 2. resource_dir/<exe>
    let candidate = resource_dir.join(&exe_name);
    if candidate.exists() {
        return Ok(candidate);
    }

    // 3. Same directory as current executable
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let candidate = exe_dir.join(&exe_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    // 4. Dev mode: src-tauri/binaries/ relative to working directory
    if let Ok(cwd) = std::env::current_dir() {
        let src_tauri = cwd.join("src-tauri");
        for base in [cwd.as_path(), src_tauri.as_path()] {
            let candidate = base.join("binaries").join(&exe_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    // 5. SMOLPC_ENGINE_HOST_BIN env var
    if let Ok(path_str) = std::env::var("SMOLPC_ENGINE_HOST_BIN") {
        let path = PathBuf::from(path_str);
        if path.exists() {
            return Ok(path);
        }
    }

    Err(format!(
        "Engine host binary '{}' not found in any search path",
        exe_name
    ))
}

/// Resolve the directory containing bundled ONNX Runtime DLLs.
/// Searches multiple locations to handle both dev and production layouts.
fn resolve_libs_dir(resource_dir: Option<&Path>, engine_binary: &Path) -> Option<PathBuf> {
    // 1. resource_dir/libs/ (standard bundled layout)
    if let Some(resource_dir) = resource_dir {
        let candidate = resource_dir.join("libs");
        if candidate.exists() {
            return Some(candidate);
        }
        // Tauri may flatten libs/* into the resource root — only use if expected DLLs are present
        if resource_dir.join("onnxruntime.dll").exists()
            || resource_dir.join("libonnxruntime.so").exists()
        {
            return Some(resource_dir.to_path_buf());
        }
    }

    // 2. Sibling of engine binary: binaries/../libs/
    if let Some(bin_parent) = engine_binary.parent() {
        if let Some(bin_grandparent) = bin_parent.parent() {
            let candidate = bin_grandparent.join("libs");
            if candidate.exists() {
                return Some(candidate);
            }
        }
        // libs/ might be a sibling to the binary directly
        let candidate = bin_parent.join("libs");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // 3. Dev mode: cwd/src-tauri/libs/
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join("src-tauri").join("libs");
        if candidate.exists() {
            return Some(candidate);
        }
        let candidate = cwd.join("libs");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

fn is_port_available(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn spawn_engine_host(resource_dir: Option<&Path>, token: &str) -> Result<(), String> {
    let host_bin = resolve_engine_binary(resource_dir.unwrap_or_else(|| Path::new(".")))?;

    let data_dir = engine_runtime_dir()?.join("host-data");
    let models_dir = resolve_models_dir(resource_dir);

    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Failed to create engine data dir: {}", e))?;
    std::fs::create_dir_all(&models_dir)
        .map_err(|e| format!("Failed to create models dir: {}", e))?;

    log::info!(
        "[SharedEngine] Spawning engine: binary={}, data_dir={}, models_dir={}",
        host_bin.display(),
        data_dir.display(),
        models_dir.display()
    );

    let mut cmd = std::process::Command::new(&host_bin);
    cmd.arg("--port").arg("19432").arg("--data-dir").arg(&data_dir);

    if let Some(resource_dir) = resource_dir {
        cmd.arg("--resource-dir").arg(resource_dir);
    }

    cmd.env("SMOLPC_ENGINE_TOKEN", token)
        .env("SMOLPC_MODELS_DIR", &models_dir);

    // Add libs/ directory to PATH so the engine can find ONNX Runtime DLLs
    // This is critical: System32 may have an older onnxruntime.dll (v1.17.1)
    // that shadows the bundled v1.23.0+ DLLs, causing engine crashes.
    let libs_dir = resolve_libs_dir(resource_dir, &host_bin);
    if let Some(ref libs_dir) = libs_dir {
        log::info!("[SharedEngine] Injecting libs PATH: {}", libs_dir.display());
        let separator = if cfg!(target_os = "windows") { ";" } else { ":" };
        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}{}{}", libs_dir.display(), separator, current_path);
        cmd.env("PATH", new_path);
    } else {
        log::info!("[SharedEngine] Warning: Could not locate libs/ directory for DLL injection");
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    let child = cmd.spawn()
        .map_err(|e| format!("Failed to spawn engine host at {}: {}", host_bin.display(), e))?;

    let pid = child.id();
    if let Ok(mut guard) = ENGINE_PID.lock() {
        *guard = Some(pid);
    }

    // Write PID file for orphan detection on next startup
    if let Ok(runtime_dir) = engine_runtime_dir() {
        let pid_path = runtime_dir.join("engine.pid");
        let _ = std::fs::write(&pid_path, pid.to_string());
    }

    log::info!("[SharedEngine] Engine spawned with PID {}", pid);
    Ok(())
}

/// Ensure the shared engine is running.
/// Returns `true` if we spawned the process (caller should shut it down on exit).
pub async fn ensure_engine_running(resource_dir: Option<&Path>) -> Result<bool, String> {
    // Already running? Reuse it.
    if is_engine_available_uncached().await {
        if let Ok(mut guard) = ENGINE_AVAILABLE_CACHE.lock() {
            *guard = Some((true, Instant::now()));
        }
        return Ok(false);
    }

    // Check if port 19432 is available before attempting spawn
    if !is_port_available(19432) {
        return Err("Port 19432 is already in use by another process. Close the conflicting application or set ENGINE_BASE_URL to use a different port.".to_string());
    }

    let token = ensure_token()?;
    spawn_engine_host(resource_dir, &token)?;

    // Poll for readiness
    let started = Instant::now();
    loop {
        if is_engine_available_uncached().await {
            if let Ok(mut guard) = ENGINE_AVAILABLE_CACHE.lock() {
                *guard = Some((true, Instant::now()));
            }
            return Ok(true);
        }

        if started.elapsed() > Duration::from_secs(60) {
            return Err("Engine failed to start within 60 seconds".to_string());
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Ask the engine to shut down gracefully, falling back to a hard kill via PID.
pub async fn shutdown_engine() -> Result<(), String> {
    let client = build_client(Duration::from_secs(2), Duration::from_secs(5))?;
    let token = match read_engine_token() {
        Ok(t) => t,
        Err(_) => {
            // Can't auth → try hard kill as last resort
            hard_kill_engine();
            return Ok(());
        }
    };

    let graceful_ok = client
        .post(url("/engine/shutdown"))
        .bearer_auth(&token)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    if !graceful_ok {
        log::info!("[SharedEngine] Graceful shutdown failed, attempting hard kill");
        hard_kill_engine();
    }

    cleanup_pid_file();
    Ok(())
}

/// Kill the engine process by PID (last resort).
fn hard_kill_engine() {
    let pid = ENGINE_PID.lock().ok().and_then(|guard| *guard);
    if let Some(pid) = pid {
        log::info!("[SharedEngine] Hard-killing engine PID {}", pid);
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .output();
        }
        #[cfg(not(target_os = "windows"))]
        {
            unsafe {
                libc::kill(pid as i32, libc::SIGKILL);
            }
        }
    }
}

/// Clean up stale engine processes from previous runs using the PID file.
pub fn cleanup_stale_engine() {
    let pid_path = match engine_runtime_dir() {
        Ok(dir) => dir.join("engine.pid"),
        Err(_) => return,
    };

    if let Ok(contents) = std::fs::read_to_string(&pid_path) {
        if let Ok(pid) = contents.trim().parse::<u32>() {
            log::info!("[SharedEngine] Found stale PID file (PID {}), checking if alive", pid);
            // Only kill if the engine is NOT healthy (could be legitimately running)
            // We skip killing here — ensure_engine_running will reuse a healthy engine
        }
    }
}

fn cleanup_pid_file() {
    if let Ok(runtime_dir) = engine_runtime_dir() {
        let pid_path = runtime_dir.join("engine.pid");
        let _ = std::fs::remove_file(&pid_path);
    }
    if let Ok(mut guard) = ENGINE_PID.lock() {
        *guard = None;
    }
}
