use smolpc_engine_client::{connect_or_spawn, EngineConnectOptions, RuntimeModePreference};
use smolpc_engine_core::GenerationConfig;

const ENGINE_PORT: u16 = 19432;

/// Build the connection options for the shared SmolPC engine.
fn engine_options() -> Result<EngineConnectOptions, String> {
    let shared_runtime_dir = dirs::data_local_dir()
        .ok_or_else(|| "Cannot determine local data directory".to_string())?
        .join("SmolPC")
        .join("engine-runtime");

    Ok(EngineConnectOptions {
        port: ENGINE_PORT,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: shared_runtime_dir.join("host-data"),
        shared_runtime_dir,
        resource_dir: None,  // engine binary located via PATH / launcher
        models_dir: None,
        host_binary: None,
        runtime_mode: RuntimeModePreference::Auto,
        dml_device_id: None,
        force_respawn: false,
    })
}

/// Send a single-turn prompt to the SmolPC engine and return the text response.
pub async fn chat(prompt: &str) -> Result<String, String> {
    let options = engine_options()?;

    let client = connect_or_spawn(options)
        .await
        .map_err(|e| format!("smolpc-engine-unavailable: {e}"))?;

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

/// Quick health check — true if the engine is up and responding.
pub async fn check_engine_health() -> bool {
    let token_path = match dirs::data_local_dir() {
        Some(p) => p
            .join("SmolPC")
            .join("engine-runtime")
            .join("engine-token.txt"),
        None => return false,
    };

    let token = match std::fs::read_to_string(&token_path) {
        Ok(s) => s.trim().to_string(),
        Err(_) => return false,
    };

    if token.is_empty() {
        return false;
    }

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    client
        .get(format!("http://127.0.0.1:{ENGINE_PORT}/engine/health"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}
