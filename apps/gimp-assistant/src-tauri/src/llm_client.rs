use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::Value;

const DEFAULT_ENGINE_PORT: u16 = 19432;

/// Resolve engine port — respects SMOLPC_ENGINE_PORT env var for testing / non-default setups.
fn engine_port() -> u16 {
    std::env::var("SMOLPC_ENGINE_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(DEFAULT_ENGINE_PORT)
}

/// Shared HTTP client — reused across calls for connection pooling.
static HTTP: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

/// Read the shared SmolPC engine bearer token from the runtime directory.
fn read_token() -> Option<String> {
    let token_path = dirs::data_local_dir()?
        .join("SmolPC")
        .join("engine-runtime")
        .join("engine-token.txt");
    std::fs::read_to_string(token_path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct CompletionRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    max_tokens: usize,
    temperature: f32,
}

/// Send a single-turn prompt to the SmolPC engine and return the text response.
pub async fn chat(prompt: &str) -> Result<String, String> {
    let port = engine_port();
    let token = read_token().ok_or_else(|| {
        "smolpc-engine-unavailable: token file not found — is the SmolPC engine running?".to_string()
    })?;

    let req = CompletionRequest {
        model: "smolpc-engine".to_string(),
        messages: vec![Message {
            role: "user".into(),
            content: prompt.into(),
        }],
        stream: false,
        max_tokens: 2048,
        temperature: 0.7,
    };

    let resp = HTTP
        .post(format!("http://127.0.0.1:{port}/v1/chat/completions"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("smolpc-engine-unavailable: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Engine returned HTTP {}", resp.status()));
    }

    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse engine response: {e}"))?;

    // OpenAI-compatible: choices[0].message.content
    body.get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|first| first.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Unexpected engine response shape: {body}"))
}

/// Quick health check — true if the engine is up and responding.
/// Uses the same port and token path as chat() to avoid divergence.
pub async fn check_engine_health() -> bool {
    let port = engine_port();
    let Some(token) = read_token() else {
        return false;
    };

    HTTP.get(format!("http://127.0.0.1:{port}/engine/health"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}
