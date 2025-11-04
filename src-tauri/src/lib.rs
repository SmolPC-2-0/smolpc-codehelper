// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Instant;
use tauri::{Emitter, Manager};

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
            base_url: "http://127.0.0.1:11434".to_string(),
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
    /// Defaults optimized for high-performance machines with balanced speed and quality.
    fn with_reasonable_defaults() -> Self {
        GenerationOptions {
            temperature: Some(0.7),       // Balanced diversity
            top_p: Some(0.9),            // Slightly reduced sampling range
            top_k: Some(30),             // Smaller token pool for faster sampling
            repeat_penalty: Some(1.1),   // Slight penalty to avoid repetition
            num_thread: Some(4),         // Match to available cores (adjust as needed)
            num_predict: None,           // Shorter outputs for faster generation
            num_ctx: None,               // Default context size
            stop: None,
        }
    }
}

/// Default tutor system instructions shared between streaming and non-streaming flows.
const TUTOR_SYSTEM_PROMPT: &str = "You are a friendly, encouraging coding tutor for secondary school students aged 11-18. Your goals in every response are to:\n\
\n\
1. Stay on topic - focus only on the exact question or task the student asked about.\n\
2. Gauge complexity - tailor explanations to the learner's likely age and prior knowledge.\n\
3. Use clear, simple language for basic questions; provide richer but still accessible detail for advanced ones.\n\
4. Explain clearly - break ideas into digestible steps, highlight key terms, and connect new ideas to things students may already know.\n\
5. Add helpful context - explain why the code works, mention alternatives, and point out common mistakes to deepen understanding.\n\
6. Always show well-commented code examples inside fenced code blocks with the correct language identifier, explaining what each part does and why.\n\
7. Stay encouraging and patient - remind students that practice builds skill and invite curiosity in every response.\n\
8. Always deliver explanations that are kind, informative, and age-appropriate for learners aged 11-18.\n";

#[derive(Debug, Deserialize)]
struct StreamRequest {
    model: String,
    prompt: String,
    system: Option<String>,
    options: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct ChunkEvent {
    text: String,
    done: bool,
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
    // Start tracking time
    let start_time = Instant::now();

    // Student-friendly instructions
    let system_prompt = TUTOR_SYSTEM_PROMPT.to_string();

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

    // Stop tracking time and log the duration
    let duration = start_time.elapsed();
    println!("DEBUG: Model generation took {:?}", duration);

    Ok(result)
}

/// Streaming generation command. Emits incremental chunks back to the frontend.
#[tauri::command]
async fn generate_code_stream(app: tauri::AppHandle, req: StreamRequest) -> Result<(), String> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "Failed to access shared application state".to_string())?;
    let client = state.client.clone();
    let base_url = state.base_url.clone();
    drop(state);

    let start_time = Instant::now();

    let StreamRequest {
        model,
        prompt,
        system,
        options,
    } = req;

    let mut opts = GenerationOptions::with_reasonable_defaults();
    if let Some(raw_options) = options {
        if let Ok(overrides) = serde_json::from_value::<GenerationOptions>(raw_options) {
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
    }

    let system_prompt = system.unwrap_or_else(|| TUTOR_SYSTEM_PROMPT.to_string());
    let composed_prompt = format!("{}\n\nStudent question: {}", system_prompt, prompt);

    let url = format!("{}/api/generate", base_url);
    let response = client
        .post(url)
        .json(&json!({
            "model": model,
            "prompt": composed_prompt,
            "stream": true,
            "options": opts,
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to start streaming request: {}", e))?;

    println!("DEBUG: streaming request started for model {}", model);

    let _ = app.emit(
        "llm_chunk",
        ChunkEvent {
            text: String::new(),
            done: false,
        },
    );

    let mut stream = response.bytes_stream();
    let mut buffer: Vec<u8> = Vec::new();
    let mut saw_done = false;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("Stream error: {}", e))?;
        println!("DEBUG: recv chunk {} bytes", bytes.len());
        buffer.extend_from_slice(&bytes);

        while let Some(idx) = buffer.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buffer.drain(..=idx).collect();
            if line.is_empty() {
                continue;
            }
            let mut trimmed = &line[..line.len().saturating_sub(1)];
            if trimmed.ends_with(&[b'\r']) {
                trimmed = &trimmed[..trimmed.len().saturating_sub(1)];
            }
            if trimmed.is_empty() {
                continue;
            }

            println!("DEBUG: processing line: {}", String::from_utf8_lossy(trimmed));

            let json_line: serde_json::Value = serde_json::from_slice(trimmed)
                .map_err(|e| format!("Failed to parse stream chunk: {}", e))?;

            println!("DEBUG: parsed JSON: {:?}", json_line);

            let text = json_line
                .get("response")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let done = json_line
                .get("done")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if done {
                saw_done = true;
            }

            if !text.is_empty() || done {
                println!(
                    "DEBUG: emitting chunk ({} chars, done={})",
                    text.chars().count(),
                    done
                );
            }

            let _ = app.emit(
                "llm_chunk",
                ChunkEvent {
                    text,
                    done,
                },
            );
        }
    }

    if !buffer.is_empty() {
        let trimmed = if buffer.last() == Some(&b'\n') {
            &buffer[..buffer.len().saturating_sub(1)]
        } else {
            buffer.as_slice()
        };

        if !trimmed.is_empty() {
            let json_line: serde_json::Value = serde_json::from_slice(trimmed)
                .map_err(|e| format!("Failed to parse trailing stream chunk: {}", e))?;

            let text = json_line
                .get("response")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let done = json_line
                .get("done")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if done {
                saw_done = true;
            }

            if !text.is_empty() || done {
                println!(
                    "DEBUG: emitting trailing chunk ({} chars, done={})",
                    text.chars().count(),
                    done
                );
            }

            let _ = app.emit(
                "llm_chunk",
                ChunkEvent {
                    text,
                    done,
                },
            );
        }
        buffer.clear();
    }

    if !saw_done {
        println!("DEBUG: no done flag received; emitting final done");
    }
    let _ = app.emit(
        "llm_chunk",
        ChunkEvent {
            text: String::new(),
            done: true,
        },
    );
    
    let elapsed = start_time.elapsed();
    println!(
        "DEBUG: Streaming generation completed in {:?}",
        elapsed
    );

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