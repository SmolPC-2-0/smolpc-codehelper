// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use serde_json::json;

// Check if Ollama is running
#[tauri::command]
async fn check_ollama() -> Result<String, String> {
    let client = reqwest::Client::new();
    
    match client
        .get("http://localhost:11434/api/tags")
        .send()
        .await
    {
        Ok(_) => Ok("Ollama is running".to_string()),
        Err(_) => Err("Ollama is not running. Please start Ollama first.".to_string()),
    }
}

// Generate code using Ollama
#[tauri::command]
async fn generate_code(prompt: String, model: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    
    // Build system prompt
    let system_prompt = "You are a friendly coding tutor helping secondary school students (ages 11-18). \
    Provide clear, well-commented code with explanations. Keep it simple and educational. \
    Always explain what the code does and why it works that way.";
    
    let full_prompt = format!("{}\n\nStudent question: {}", system_prompt, prompt);
    
    // Call Ollama API
    let response = client
        .post("http://localhost:11434/api/generate")
        .json(&json!({
            "model": model,
            "prompt": full_prompt,
            "stream": false
        }))
        .timeout(std::time::Duration::from_secs(120)) // 2 minute timeout
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama: {}", e))?;
    
    // Parse response
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    // Extract generated text
    let result = json["response"]
        .as_str()
        .ok_or("No response from model")?
        .to_string();
    
    Ok(result)
}

// Save code to file
#[tauri::command]
async fn save_code(content: String) -> Result<(), String> {
    use std::fs;
    use rfd::AsyncFileDialog;
    
    // Open save dialog
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
            fs::write(file.path(), content)
                .map_err(|e| format!("Failed to save file: {}", e))?;
            Ok(())
        }
        None => Err("Save cancelled".to_string()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            check_ollama,
            generate_code,
            save_code
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}