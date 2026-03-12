use smolpc_engine_client::EngineClient;
use smolpc_engine_core::GenerationConfig;

/// Non-streaming text generation (for tool selection + plan generation — needs complete JSON).
pub async fn chat(client: &EngineClient, prompt: &str) -> Result<String, String> {
    let config = GenerationConfig {
        max_length: 2048,
        temperature: 0.7,
        ..Default::default()
    };
    let result = client
        .generate_text(prompt, Some(config))
        .await
        .map_err(|e| format!("Engine generation failed: {e}"))?;
    Ok(result.text)
}

/// Streaming text generation (for natural language answers).
pub async fn chat_stream(
    client: &EngineClient,
    prompt: &str,
    on_token: impl FnMut(String),
) -> Result<(), String> {
    let config = GenerationConfig {
        max_length: 2048,
        temperature: 0.7,
        ..Default::default()
    };
    client
        .generate_stream(prompt, Some(config), on_token)
        .await
        .map_err(|e| format!("Engine streaming failed: {e}"))?;
    Ok(())
}
