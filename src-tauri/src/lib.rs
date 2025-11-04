// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::Emitter; // required for window.emit()

/// Shared application state
/// - Reuses a single reqwest client (connection pooling, keep-alive)
/// - Keeps base URL for local Ollama (offline, localhost)
struct AppState {
    client: reqwest::Client,
    base_url: String,
}

impl AppState {
    fn new() -> Result<Self, String> {
        // Keep the client lightweight for weak machines: smaller connection
        // pool and short keep-alive. Ollama runs locally so latency is low and
        // high concurrency is unlikely in the target use-case.
        let client = reqwest::ClientBuilder::new()
            .pool_max_idle_per_host(4)
            .tcp_keepalive(std::time::Duration::from_secs(15))
            .connect_timeout(std::time::Duration::from_secs(3))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

        Ok(Self {
            client,
            base_url: "http://localhost:11434".to_string(),
        })
    }
}

/// Optional generation controls to balance speed/accuracy on weak machines.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", default)]
struct GenerationOptions {
    temperature: Option<f32>,
    top_p: Option<f32>,
    top_k: Option<u32>,
    repeat_penalty: Option<f32>,
    num_ctx: Option<u32>,
    num_predict: Option<u32>,
    num_thread: Option<u32>,
    stop: Option<Vec<String>>,
}

impl GenerationOptions {
    /// Defaults targeting educational code output with low overhead.
    fn with_reasonable_defaults() -> Self {
        GenerationOptions {
            temperature: Some(0.3),
            top_p: Some(0.9),
            top_k: Some(40),
            repeat_penalty: Some(1.1),
            // Leave num_ctx and num_thread to Ollama defaults (hardware-aware)
            num_ctx: None,
            // Cap output to avoid long, CPU-heavy generations
            num_predict: Some(512),
            num_thread: None,
            stop: None,
        }
    }
}

// Check if Ollama is running (offline, local)
#[tauri::command]
async fn check_ollama(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let url = format!("{}/api/tags", state.base_url);
    match state
        .client
        .get(url)
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
    {
        Ok(_) => Ok("Ollama is running".to_string()),
        Err(_) => Err("Ollama is not running. Please start Ollama first.".to_string()),
    }
}

// Non-streaming generation (compatible with existing frontend)
#[tauri::command]
async fn generate_code(
    state: tauri::State<'_, AppState>,
    prompt: String,
    model: String,
    options: Option<GenerationOptions>,
) -> Result<String, String> {
    // Student-friendly instructions
    let system_prompt = "You are a friendly coding tutor helping secondary school students (ages 11-18). \
Provide clear, well-commented code with explanations. Keep it simple and educational. \
Always explain what the code does and why it works that way.";

    let full_prompt = format!("{}\n\nStudent question: {}", system_prompt, prompt);

    let url = format!("{}/api/generate", state.base_url);

    // Merge defaults with provided overrides
    let mut opts = GenerationOptions::with_reasonable_defaults();
    if let Some(overrides) = options {
        if overrides.temperature.is_some() {
            opts.temperature = overrides.temperature;
        }
        if overrides.top_p.is_some() {
            opts.top_p = overrides.top_p;
        }
        if overrides.top_k.is_some() {
            opts.top_k = overrides.top_k;
        }
        if overrides.repeat_penalty.is_some() {
            opts.repeat_penalty = overrides.repeat_penalty;
        }
        if overrides.num_ctx.is_some() {
            opts.num_ctx = overrides.num_ctx;
        }
        if overrides.num_predict.is_some() {
            opts.num_predict = overrides.num_predict;
        }
        if overrides.num_thread.is_some() {
            opts.num_thread = overrides.num_thread;
        }
        if overrides.stop.is_some() {
            opts.stop = overrides.stop;
        }
    }

    let response = state
        .client
        .post(url)
        .json(&json!({
            "model": model,
            "prompt": full_prompt,
            "stream": false,
            "options": opts,
        }))
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama: {}", e))?;

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let result = json
        .get("response")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "No response from model".to_string())?
        .to_string();

    Ok(result)
}

/// Streaming generation over the chat API.
/// Emits events to the originating window for incremental UI updates.
/// - "gen_chunk": { chunk: String }
/// - "gen_done": {}
/// - "gen_error": { error: String }
#[tauri::command]
async fn generate_code_stream(
    window: tauri::Window,
    state: tauri::State<'_, AppState>,
    prompt: String,
    model: String,
    options: Option<GenerationOptions>,
) -> Result<(), String> {
    // System prompt tailored for students (kept concise here).
    let system_prompt = "You are a friendly, encouraging coding tutor for secondary school students aged 11–18. Your goals are to keep answers simple, well-structured, and educational.";

    // Merge defaults with overrides
    let mut opts = GenerationOptions::with_reasonable_defaults();
    if let Some(overrides) = options {
        if overrides.temperature.is_some() { opts.temperature = overrides.temperature; }
        if overrides.top_p.is_some() { opts.top_p = overrides.top_p; }
        if overrides.top_k.is_some() { opts.top_k = overrides.top_k; }
        if overrides.repeat_penalty.is_some() { opts.repeat_penalty = overrides.repeat_penalty; }
        if overrides.num_ctx.is_some() { opts.num_ctx = overrides.num_ctx; }
        if overrides.num_predict.is_some() { opts.num_predict = overrides.num_predict; }
        if overrides.num_thread.is_some() { opts.num_thread = overrides.num_thread; }
        if overrides.stop.is_some() { opts.stop = overrides.stop; }
    }

    let url = format!("{}/api/chat", state.base_url);

    let body = json!({
        "model": model,
        "stream": true,
        "options": opts,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": prompt}
        ]
    });

    let resp = state
        .client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama: {}", e))?;

    // Stream response bytes; parse each JSON line
    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;

    while let Some(item) = stream.next().await {
        let bytes = match item {
            Ok(b) => b,
            Err(e) => {
                let _ = window.emit("gen_error", json!({"error": e.to_string()}));
                return Err(format!("Stream error: {}", e));
            }
        };

        for line in bytes.split(|&c| c == b'\n') {
            if line.is_empty() { continue; }
            let parsed: Result<serde_json::Value, _> = serde_json::from_slice(line);
            match parsed {
                Ok(val) => {
                    if val.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
                        let _ = window.emit("gen_done", json!({}));
                        return Ok(());
                    }

                    if let Some(chunk) = val
                        .get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        let _ = window.emit("gen_chunk", json!({"chunk": chunk}));
                    }

                    if let Some(chunk) = val.get("response").and_then(|c| c.as_str()) {
                        let _ = window.emit("gen_chunk", json!({"chunk": chunk}));
                    }
                }
                Err(_) => {
                    // Ignore non-JSON lines to keep UX clean
                }
            }
        }
    }

    // End of stream without explicit done
    let _ = window.emit("gen_done", json!({}));
    Ok(())
}

// Save code to file (native save dialog)
#[tauri::command]
async fn save_code(content: String) -> Result<(), String> {
    use rfd::AsyncFileDialog;
    use std::fs;

    let file_handle = AsyncFileDialog::new()
        .set_title("Save Code")
        .add_filter("Python", &["py"])
        .add_filter("JavaScript", &["js"])
        .add_filter("HTML", &["html"])
        .add_filter("Java", &["java"])
        .add_filter("C++", &["cpp"])
        .add_filter("Text", &["txt"])
        .add_filter("All Files", &["*"])
        .save_file()
        .await;

    match file_handle {
        Some(file) => {
            fs::write(file.path(), content).map_err(|e| format!("Failed to save file: {}", e))?;
            Ok(())
        }
        None => Err("Save cancelled".to_string()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::new().expect("Failed to initialize app state");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            check_ollama,
            generate_code,
            generate_code_stream,
            save_code
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
