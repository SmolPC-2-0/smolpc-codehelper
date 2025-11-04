// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use serde::{Deserialize, Serialize};
use serde_json::json;
use futures_util::StreamExt;
use tauri::Emitter;

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
    /// Defaults optimized for high-performance machines with balanced speed and quality.
    fn with_reasonable_defaults() -> Self {
        GenerationOptions {
            temperature: Some(0.7),       // Balanced diversity
            top_p: Some(0.9),            // Slightly reduced sampling range
            top_k: Some(30),             // Smaller token pool for faster sampling
            repeat_penalty: Some(1.1),   // Slight penalty to avoid repetition
            num_thread: Some(4),         // Match to available cores (adjust as needed)
            num_predict: Some(512),      // Shorter outputs for faster generation
            num_ctx: None,               // Default context size
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
    // Start tracking time
    let start_time = std::time::Instant::now();

    // Student-friendly instructions
    let system_prompt = "You are a friendly, encouraging coding tutor for secondary school students aged 11–18. Your goals in every response are to: 

1. Stay on topic — focus only on the exact question or task the student asked about.  
2. Gauge complexity — tailor explanations to the learner's likely age and prior knowledge.  
3. Use clear, simple language for basic questions; provide richer but still accessible detail for advanced ones.  
4. Explain clearly — break ideas into digestible steps, highlight key terms, and connect new ideas to things students may already know.  
5. Add helpful context — explain why the code works, mention alternatives, and point out common mistakes to deepen understanding.  
6. Always show well-commented code examples inside fenced code blocks with the correct language identifier, explaining what each part does and why.  
7. Stay encouraging and patient — remind students that practice builds skill and invite curiosity in every response.  
8. Always deliver explanations that are kind, informative, and age-appropriate for learners aged 11–18.
";

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

// Streaming generation - emits chunks via Tauri events
#[tauri::command]
async fn generate_code_stream(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    prompt: String,
    model: String,
    options: Option<GenerationOptions>,
) -> Result<(), String> {
    println!("=== STREAMING CALLED ===");
    println!("Model: {}, Prompt length: {}", model, prompt.len());
    // Start tracking time
    let start_time = std::time::Instant::now();

    // Student-friendly instructions (same as non-streaming)
    let system_prompt = "You are a friendly, encouraging coding tutor for secondary school students aged 11–18. Your goals in every response are to: 

1. Stay on topic — focus only on the exact question or task the student asked about.  
2. Gauge complexity — tailor explanations to the learner's likely age and prior knowledge.  
3. Use clear, simple language for basic questions; provide richer but still accessible detail for advanced ones.  
4. Explain clearly — break ideas into digestible steps, highlight key terms, and connect new ideas to things students may already know.  
5. Add helpful context — explain why the code works, mention alternatives, and point out common mistakes to deepen understanding.  
6. Always show well-commented code examples inside fenced code blocks with the correct language identifier, explaining what each part does and why.  
7. Stay encouraging and patient — remind students that practice builds skill and invite curiosity in every response.  
8. Always deliver explanations that are kind, informative, and age-appropriate for learners aged 11–18.
";

    let full_prompt = format!("{}\n\nStudent question: {}", system_prompt, prompt);

    let url = format!("{}/api/generate", state.base_url);

    // Merge defaults with provided overrides (same as non-streaming)
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

    // Make streaming request
    let response = state
        .client
        .post(url)
        .json(&json!({
            "model": model,
            "prompt": full_prompt,
            "stream": true,  // KEY CHANGE: Enable streaming
            "options": opts,
        }))
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| {
            // Emit error event
            let _ = app.emit("gen_error", json!({"error": format!("Failed to connect to Ollama: {}", e)}));
            format!("Failed to connect to Ollama: {}", e)
        })?;

    // Process stream line by line
    let mut stream = response.bytes_stream();
    
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                // Parse JSON from this chunk
                match serde_json::from_slice::<serde_json::Value>(&bytes) {
                    Ok(json) => {
                        // Extract the response text
                        if let Some(text) = json.get("response").and_then(|v| v.as_str()) {
                            println!("Chunk received: {} chars", text.len());
                            // Emit chunk to frontend
                            if !text.is_empty() {
                                if let Err(e) = app.emit("gen_chunk", json!({"chunk": text})) {
                                    eprintln!("Failed to emit chunk: {}", e);
                                    let _ = app.emit("gen_error", json!({"error": "Failed to emit chunk"}));
                                    return Err("Failed to emit chunk".to_string());
                                }
                            }
                        }

                        // Check if generation is done
                        if json.get("done").and_then(|v| v.as_bool()) == Some(true) {
                            // Stop tracking time and log
                            let duration = start_time.elapsed();
                            println!("DEBUG: Streaming generation took {:?}", duration);
                            
                            // Emit completion event
                            if let Err(e) = app.emit("gen_done", json!({})) {
                                eprintln!("Failed to emit done: {}", e);
                            }
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to parse JSON chunk: {}", e);
                        let _ = app.emit("gen_error", json!({"error": format!("Failed to parse response: {}", e)}));
                        return Err(format!("Failed to parse response: {}", e));
                    }
                }
            }
            Err(e) => {
                eprintln!("Stream error: {}", e);
                let _ = app.emit("gen_error", json!({"error": format!("Stream error: {}", e)}));
                return Err(format!("Stream error: {}", e));
            }
        }
    }

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
            generate_code_stream,  // NEW: Register streaming command
            save_code
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}