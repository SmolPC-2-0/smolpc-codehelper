use super::errors::Error;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::broadcast;

// Student-friendly system prompt for coding assistance
const SYSTEM_PROMPT: &str = r#"You are a helpful coding assistant designed for secondary school students (ages 11-18).
Your goal is to explain programming concepts clearly and provide well-commented code examples.

Guidelines:
- Use simple, encouraging language
- Break down complex concepts into steps
- Always include helpful comments in code
- Be patient and supportive
- Adapt explanations to the student's level
- Encourage learning and experimentation"#;

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
    sender: Arc<Mutex<Option<broadcast::Sender<()>>>>,
}

impl Default for StreamCancellation {
    fn default() -> Self {
        Self {
            sender: Arc::new(Mutex::new(None)),
        }
    }
}

impl StreamCancellation {
    pub fn create_channel(&self) -> broadcast::Receiver<()> {
        let mut sender_lock = self.sender.lock().unwrap();
        let (tx, rx) = broadcast::channel(1);
        *sender_lock = Some(tx);
        rx
    }

    pub fn cancel(&self) {
        let sender_lock = self.sender.lock().unwrap();
        if let Some(sender) = sender_lock.as_ref() {
            let _ = sender.send(());
        }
    }

    pub fn clear(&self) {
        let mut sender_lock = self.sender.lock().unwrap();
        *sender_lock = None;
    }
}

/// Check if Ollama server is running and available
#[tauri::command]
pub async fn check_ollama() -> Result<bool, Error> {
    let client = reqwest::Client::new();
    let response = client
        .get("http://localhost:11434/api/tags")
        .send()
        .await;

    match response {
        Ok(resp) => Ok(resp.status().is_success()),
        Err(_) => Ok(false),
    }
}

/// Get list of available Ollama models
#[tauri::command]
pub async fn get_ollama_models() -> Result<Vec<String>, Error> {
    let client = reqwest::Client::new();
    let response = client
        .get("http://localhost:11434/api/tags")
        .send()
        .await
        .map_err(|e| Error::Other(format!("Failed to connect to Ollama: {}", e)))?;

    let models: OllamaModelsResponse = response
        .json()
        .await
        .map_err(|e| Error::Other(format!("Failed to parse models: {}", e)))?;

    Ok(models.models.into_iter().map(|m| m.name).collect())
}

/// Cancel ongoing generation
#[tauri::command]
pub fn cancel_generation(cancellation: State<StreamCancellation>) -> Result<(), Error> {
    cancellation.cancel();
    Ok(())
}

/// Generate streaming response from Ollama
#[tauri::command]
pub async fn generate_stream(
    app_handle: AppHandle,
    prompt: String,
    model: String,
    context: Option<Vec<OllamaMessage>>,
    cancellation: State<'_, StreamCancellation>,
) -> Result<(), Error> {
    // Create a new cancellation receiver for this stream
    let mut cancel_rx = cancellation.create_channel();

    let client = reqwest::Client::new();

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

    let response = client
        .post("http://localhost:11434/api/chat")
        .json(&request)
        .send()
        .await
        .map_err(|e| Error::Other(format!("Failed to send request: {}", e)))?;

    let mut stream = response.bytes_stream();

    loop {
        tokio::select! {
            // Check for cancellation
            _ = cancel_rx.recv() => {
                // Stream was cancelled
                cancellation.clear();
                let _ = app_handle.emit("ollama_cancelled", ());
                return Ok(());
            }
            // Process stream chunks
            chunk_result = stream.next() => {
                match chunk_result {
                    Some(Ok(bytes)) => {
                        if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                            // Parse each line as JSON
                            for line in text.lines() {
                                if let Ok(response) = serde_json::from_str::<OllamaResponse>(line) {
                                    if let Some(message) = response.message {
                                        // Emit chunk event with content
                                        let _ = app_handle.emit("ollama_chunk", message.content);
                                    }

                                    if response.done {
                                        // Emit done event
                                        cancellation.clear();
                                        let _ = app_handle.emit("ollama_done", ());
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        cancellation.clear();
                        let _ = app_handle.emit("ollama_error", format!("Stream error: {}", e));
                        return Err(Error::Other(format!("Stream error: {}", e)));
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
