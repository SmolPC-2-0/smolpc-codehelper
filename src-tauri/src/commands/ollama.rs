use super::errors::Error;
use crate::security;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::broadcast;

// Student-friendly system prompt for coding assistance
const SYSTEM_PROMPT: &str = r"You are a helpful coding assistant designed for secondary school students (ages 11-18).
Your goal is to explain programming concepts clearly and provide well-commented code examples.

Guidelines:
- Use simple, encouraging language
- Break down complex concepts into steps
- Always include helpful comments in code
- Be patient and supportive
- Adapt explanations to the student's level
- Encourage learning and experimentation";

/// Shared HTTP client for connection pooling
pub struct HttpClient {
    client: reqwest::Client,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl HttpClient {
    pub fn get(&self) -> &reqwest::Client {
        &self.client
    }
}

/// Configuration for Ollama server URL
pub struct OllamaConfig {
    base_url: String,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        // Read from environment variable or use default
        let base_url = env::var("OLLAMA_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());

        // Validate URL is localhost only for security (uses proper URL parsing)
        let validated_url = security::validate_ollama_url(&base_url)
            .unwrap_or_else(|err| {
                log::error!("{err}");
                log::warn!("Falling back to default: http://localhost:11434");
                "http://localhost:11434".to_string()
            });

        Self { base_url: validated_url }
    }
}

impl OllamaConfig {
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaRequest {
    pub model: String,
    pub messages: Vec<OllamaMessage>,
    pub stream: bool,
}

#[derive(Debug, Deserialize)]
pub struct OllamaResponse {
    pub message: Option<OllamaMessage>,
    pub done: bool,
    // Token count metadata (only present when done=true)
    pub eval_count: Option<usize>,        // Number of tokens in the response
    // Timing metadata (only present when done=true)
    pub total_duration: Option<u64>,      // Total time in nanoseconds
    pub prompt_eval_duration: Option<u64>, // Prompt evaluation time in nanoseconds
    pub eval_duration: Option<u64>,       // Response generation time in nanoseconds
}

#[derive(Debug, Deserialize)]
pub struct OllamaModel {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct OllamaModelsResponse {
    pub models: Vec<OllamaModel>,
}

/// Global state to manage stream cancellation
pub struct StreamCancellation {
    sender: Mutex<Option<broadcast::Sender<()>>>,
}

impl Default for StreamCancellation {
    fn default() -> Self {
        Self {
            sender: Mutex::new(None),
        }
    }
}

impl StreamCancellation {
    pub fn create_channel(&self) -> broadcast::Receiver<()> {
        let mut sender_lock = self.sender.lock()
            .expect("StreamCancellation mutex poisoned - indicates panic in stream handler");
        let (tx, rx) = broadcast::channel(1); 
        *sender_lock = Some(tx); // Transmitter stored globally for cancellation
        rx // Return receiver for this stream
    }

    pub fn cancel(&self) {
        let sender_lock = self.sender.lock() // 
            .expect("StreamCancellation mutex poisoned - indicates panic in stream handler");
        if let Some(sender) = sender_lock.as_ref() {
            let _ = sender.send(());
        }
    }

    pub fn clear(&self) {
        let mut sender_lock = self.sender.lock()
            .expect("StreamCancellation mutex poisoned - indicates panic in stream handler");
        *sender_lock = None;
    }
}

/// Check if Ollama server is running and available
#[tauri::command]
pub async fn check_ollama(
    client: State<'_, HttpClient>,
    config: State<'_, OllamaConfig>,
) -> Result<bool, Error> {
    let url = format!("{}/api/tags", config.base_url());
    let response = client.get()
        .get(&url)
        .send()
        .await;

    match response {
        Ok(resp) => Ok(resp.status().is_success()),
        Err(_) => Ok(false),
    }
}

/// Get list of available Ollama models
#[tauri::command]
pub async fn get_ollama_models(
    client: State<'_, HttpClient>,
    config: State<'_, OllamaConfig>,
) -> Result<Vec<String>, Error> {
    let url = format!("{}/api/tags", config.base_url());
    let response = client.get()
        .get(&url)
        .send()
        .await
        .map_err(|e| Error::Other(format!("Failed to connect to Ollama: {e}")))?;

    let models: OllamaModelsResponse = response
        .json()
        .await
        .map_err(|e| Error::Other(format!("Failed to parse models: {e}")))?;

    Ok(models.models.into_iter().map(|m| m.name).collect())
}

/// Cancel ongoing generation
#[tauri::command]
pub fn cancel_generation(cancellation: State<StreamCancellation>) {
    cancellation.cancel();
}

/// Generate streaming response from Ollama
#[tauri::command]
pub async fn generate_stream(
    app_handle: AppHandle,
    prompt: String,
    model: String,
    context: Option<Vec<OllamaMessage>>,
    client: State<'_, HttpClient>,
    config: State<'_, OllamaConfig>,
    cancellation: State<'_, StreamCancellation>,
) -> Result<(), Error> {
    // Create a new cancellation receiver for this stream
    let mut cancel_rx = cancellation.create_channel();

    // Build messages array with system prompt, context, and current prompt
    let mut messages = vec![OllamaMessage {
        role: "system".to_string(),
        content: SYSTEM_PROMPT.to_string(),
    }];

    // Add context messages if provided
    if let Some(ctx) = context {
        messages.extend(ctx);
    }

    // Add current user prompt
    messages.push(OllamaMessage {
        role: "user".to_string(),
        content: prompt,
    });

    let request = OllamaRequest {
        model,
        messages,
        stream: true,
    };

    let url = format!("{}/api/chat", config.base_url());
    let response = client.get()
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| Error::Other(format!("Failed to send request: {e}")))?;

    let mut stream = response.bytes_stream();

    loop {
        tokio::select! {
            // Check for cancellation
            _ = cancel_rx.recv() => {
                // Stream was cancelled
                cancellation.clear();
                if let Err(e) = app_handle.emit("ollama_cancelled", ()) {
                    log::debug!("Failed to emit cancellation event (frontend may be closed): {e}");
                }
                return Ok(());
            }
            // Process stream chunks
            chunk_result = stream.next() => {
                match chunk_result {
                    Some(Ok(bytes)) => {
                        if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                            // Parse each line as JSON
                            for line in text.lines() {
                                if line.trim().is_empty() {
                                    continue;
                                }

                                match serde_json::from_str::<OllamaResponse>(line) {
                                    Ok(response) => {
                                        if let Some(message) = response.message {
                                            // Emit chunk event with content
                                            if let Err(e) = app_handle.emit("ollama_chunk", message.content) {
                                                log::debug!("Frontend disconnected during stream, stopping: {e}");
                                                cancellation.clear();
                                                return Ok(());
                                            }
                                        }

                                        if response.done {
                                            // Emit done event
                                            cancellation.clear();
                                            if let Err(e) = app_handle.emit("ollama_done", ()) {
                                                log::debug!("Failed to emit done event (frontend may be closed): {e}");
                                            }
                                            return Ok(());
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to parse Ollama response: {e} | Line: {line}");
                                        // Continue processing other lines - don't fail entire stream
                                    }
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        cancellation.clear();
                        if let Err(emit_err) = app_handle.emit("ollama_error", format!("Stream error: {e}")) {
                            log::debug!("Failed to emit error event (frontend may be closed): {emit_err}");
                        }
                        return Err(Error::Other(format!("Stream error: {e}")));
                    }
                    None => {
                        // Stream ended
                        cancellation.clear();
                        return Ok(());
                    }
                }
            }
        }
    }
}
