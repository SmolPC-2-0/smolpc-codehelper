use serde::Serialize;
use serde_json::Value;

const OLLAMA_URL: &str = "http://localhost:11434/api/chat";
const MODEL_NAME: &str = "llama3"; // change if you use a different model

#[derive(Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

/// Simple chat call to the local LLM (Ollama).
pub async fn chat(prompt: &str) -> Result<String, String> {
    let client = reqwest::Client::new();

    let req = OllamaRequest {
        model: MODEL_NAME.to_string(),
        messages: vec![OllamaMessage {
            role: "user".into(),
            content: prompt.into(),
        }],
        // Important: disable streaming so we get a single JSON object
        stream: false,
    };

    let resp = client
        .post(OLLAMA_URL)
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("Failed to call LLM: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("LLM returned HTTP {}", resp.status()));
    }

    // Parse as generic JSON so extra fields don't break us
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse LLM response: {e}"))?;

    // Expect something like: { "message": { "content": "..." }, ... }
    if let Some(content) = body
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
    {
        Ok(content.to_string())
    } else {
        Err(format!("Unexpected LLM response shape: {body}"))
    }
}
