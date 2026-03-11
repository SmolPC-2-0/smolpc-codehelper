use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const OLLAMA_URL: &str = "http://127.0.0.1:11434/api/chat";
pub const DEFAULT_MODEL: &str = "qwen3:4b-instruct-2507-q4_K_M";

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    stream: bool,
    messages: Vec<OllamaMessage>,
    options: OllamaOptions,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f64,
}

#[derive(Deserialize)]
struct OllamaStreamChunk {
    #[serde(default)]
    message: Option<OllamaChunkMessage>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    eval_count: Option<u64>,
    #[serde(default)]
    eval_duration: Option<u64>,
}

#[derive(Deserialize)]
struct OllamaChunkMessage {
    #[serde(default)]
    content: String,
}

#[derive(Serialize, Clone)]
pub struct OllamaMetrics {
    pub total_tokens: u64,
    pub total_time_ms: u64,
    pub tokens_per_second: f64,
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
    let client = build_client(Duration::from_secs(180))?;

    let request_body = OllamaChatRequest {
        model: resolve_model(None),
        stream: true,
        messages: vec![
            OllamaMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            OllamaMessage {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            },
        ],
        options: OllamaOptions { temperature: 0.7 },
    };

    let response = client
        .post(OLLAMA_URL)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                "Ollama not running. Start it with: ollama serve".to_string()
            } else {
                format!("Ollama request failed: {}", e)
            }
        })?;

    if !response.status().is_success() {
        return Err(format!("Ollama returned HTTP {}", response.status()));
    }

    let mut stream = response.bytes_stream();
    let mut total_tokens: u64 = 0;
    let mut buffer = String::new();
    let start = Instant::now();

    while let Some(chunk_result) = stream.next().await {
        if cancelled.load(Ordering::Relaxed) {
            return Err("GENERATION_CANCELLED: Generation cancelled by user".to_string());
        }

        let chunk_bytes = chunk_result.map_err(|e| format!("Stream read error: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk_bytes));

        while let Some(newline_pos) = buffer.find('\n') {
            let mut remaining = buffer.split_off(newline_pos + 1);
            std::mem::swap(&mut remaining, &mut buffer);
            let line = remaining.trim();

            if line.is_empty() {
                continue;
            }

            if let Some(metrics) = process_chunk_line(line, &mut total_tokens, &mut on_token, start)? {
                return Ok(metrics);
            }
        }
    }

    let trailing = buffer.trim();
    if !trailing.is_empty() {
        if let Some(metrics) = process_chunk_line(trailing, &mut total_tokens, &mut on_token, start)? {
            return Ok(metrics);
        }
    }

    let elapsed = start.elapsed();
    Ok(OllamaMetrics {
        total_tokens,
        total_time_ms: elapsed.as_millis() as u64,
        tokens_per_second: if elapsed.as_secs_f64() > 0.0 {
            total_tokens as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        },
    })
}

pub async fn chat_once(
    system_prompt: &str,
    user_prompt: &str,
    model: Option<String>,
    temperature: f64,
) -> Result<String, String> {
    let client = build_client(Duration::from_secs(180))?;

    let request_body = OllamaChatRequest {
        model: resolve_model(model),
        stream: false,
        messages: vec![
            OllamaMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            OllamaMessage {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            },
        ],
        options: OllamaOptions { temperature },
    };

    let response = client
        .post(OLLAMA_URL)
        .json(&request_body)
        .send()
        .await
        .map_err(map_ollama_error)?;

    if !response.status().is_success() {
        return Err(format!("Ollama returned HTTP {}", response.status()));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to decode Ollama response: {}", e))?;

    let content = body
        .get("message")
        .and_then(|msg| msg.get("content"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if content.is_empty() {
        return Err("Ollama returned an empty response".to_string());
    }

    Ok(content)
}

static OLLAMA_AVAILABLE_CACHE: std::sync::Mutex<Option<(bool, Instant)>> =
    std::sync::Mutex::new(None);
const OLLAMA_CACHE_TTL: Duration = Duration::from_secs(10);

pub async fn is_ollama_available() -> bool {
    // Return cached result if still fresh
    if let Ok(guard) = OLLAMA_AVAILABLE_CACHE.lock() {
        if let Some((cached, when)) = *guard {
            if when.elapsed() < OLLAMA_CACHE_TTL {
                return cached;
            }
        }
    }

    let result = is_ollama_available_uncached().await;

    if let Ok(mut guard) = OLLAMA_AVAILABLE_CACHE.lock() {
        *guard = Some((result, Instant::now()));
    }
    result
}

async fn is_ollama_available_uncached() -> bool {
    let client = match build_client(Duration::from_secs(3)) {
        Ok(client) => client,
        Err(_) => return false,
    };

    match client.get("http://127.0.0.1:11434/api/tags").send().await {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

fn build_client(timeout: Duration) -> Result<Client, String> {
    Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))
}

fn resolve_model(model: Option<String>) -> String {
    model
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()))
}

fn map_ollama_error(err: reqwest::Error) -> String {
    if err.is_connect() {
        "Ollama not running. Start it with: ollama serve".to_string()
    } else {
        format!("Ollama request failed: {}", err)
    }
}

fn process_chunk_line<F>(
    line: &str,
    total_tokens: &mut u64,
    on_token: &mut F,
    start: Instant,
) -> Result<Option<OllamaMetrics>, String>
where
    F: FnMut(String),
{
    match serde_json::from_str::<OllamaStreamChunk>(line) {
        Ok(chunk) => {
            if let Some(msg) = chunk.message {
                if !msg.content.is_empty() {
                    *total_tokens += 1;
                    on_token(msg.content);
                }
            }

            if chunk.done {
                let elapsed = start.elapsed();
                let total_time_ms = elapsed.as_millis() as u64;
                let actual_tokens = chunk.eval_count.unwrap_or(*total_tokens);
                let eval_duration_ms = chunk
                    .eval_duration
                    .map(|ns| ns / 1_000_000)
                    .unwrap_or(total_time_ms);

                let tps = if eval_duration_ms > 0 {
                    actual_tokens as f64 / (eval_duration_ms as f64 / 1000.0)
                } else {
                    0.0
                };

                return Ok(Some(OllamaMetrics {
                    total_tokens: actual_tokens,
                    total_time_ms,
                    tokens_per_second: tps,
                }));
            }

            Ok(None)
        }
        Err(e) => {
            log::warn!("Failed to parse Ollama stream chunk: {} (line: {})", e, line);
            Ok(None)
        }
    }
}
