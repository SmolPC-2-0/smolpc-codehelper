use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Json;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

// ── CLI ──────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "smolpc-tts-server", about = "Offline TTS sidecar for SmolPC")]
struct Args {
    /// HTTP port to listen on.
    #[arg(long, default_value_t = 19433)]
    port: u16,

    /// Path to KittenTTS model directory (must contain config.json, .onnx, voices.npz).
    #[arg(long)]
    model_dir: PathBuf,

    /// Optional path to directory containing espeak-ng binary (prepended to PATH).
    #[arg(long)]
    espeak_dir: Option<PathBuf>,
}

// ── Shared types ─────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    token: Arc<String>,
    tts: Arc<Mutex<kitten_tts::model::KittenTTS>>,
}

#[derive(serde::Serialize)]
struct ErrorResponse {
    error: String,
}

type ApiError = (StatusCode, Json<ErrorResponse>);

fn api_err(status: StatusCode, msg: impl Into<String>) -> ApiError {
    (status, Json(ErrorResponse { error: msg.into() }))
}

#[derive(serde::Deserialize)]
struct SynthesizeRequest {
    text: String,
    #[serde(default = "default_voice")]
    voice: String,
    #[serde(default = "default_speed")]
    speed: f32,
}

fn default_voice() -> String {
    "Bella".to_string()
}
fn default_speed() -> f32 {
    1.0
}

// ── Auth (matches engine host pattern) ───────────────────────────────

fn auth(headers: &HeaderMap, token: &str) -> Result<(), ApiError> {
    let value = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| api_err(StatusCode::UNAUTHORIZED, "Unauthorized"))?;
    let expected = format!("Bearer {token}");
    if !constant_time_eq(value.as_bytes(), expected.as_bytes()) {
        return Err(api_err(StatusCode::UNAUTHORIZED, "Unauthorized"));
    }
    Ok(())
}

fn constant_time_eq(lhs: &[u8], rhs: &[u8]) -> bool {
    let len_diff = (lhs.len() ^ rhs.len()) as u8;
    let min_len = lhs.len().min(rhs.len());
    let mut diff = len_diff;
    for i in 0..min_len {
        diff |= lhs[i] ^ rhs[i];
    }
    diff == 0
}

// ── Text preprocessing ───────────────────────────────────────────────

fn strip_markdown(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_code_fence = false;

    for line in input.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            continue;
        }

        // Strip heading markers
        let line = trimmed.trim_start_matches('#').trim_start();
        // Strip blockquote markers
        let line = line.trim_start_matches('>').trim_start();
        // Strip unordered list markers
        let line = if line.starts_with("- ") || line.starts_with("* ") {
            &line[2..]
        } else {
            line
        };
        // Strip bold/italic markers
        let line = line.replace("**", "").replace("__", "");
        // Strip inline backticks (keep content inside)
        let line = line.replace('`', "");

        if !line.trim().is_empty() {
            if !result.is_empty() {
                result.push(' ');
            }
            result.push_str(line.trim());
        }
    }

    // Collapse multiple spaces
    let mut prev_space = false;
    let collapsed: String = result
        .chars()
        .filter(|&c| {
            if c == ' ' {
                if prev_space {
                    return false;
                }
                prev_space = true;
            } else {
                prev_space = false;
            }
            true
        })
        .collect();

    collapsed.trim().to_string()
}

// ── WAV encoding ─────────────────────────────────────────────────────

fn encode_wav(samples: &[f32]) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 24000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut writer =
        hound::WavWriter::new(&mut cursor, spec).map_err(|e| format!("WAV init: {e}"))?;
    for &s in samples {
        let clamped = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
        writer
            .write_sample(clamped)
            .map_err(|e| format!("WAV write: {e}"))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("WAV finalize: {e}"))?;
    Ok(cursor.into_inner())
}

// ── Route handlers ───────────────────────────────────────────────────

async fn health(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn voices(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    let tts = state.tts.lock().await;
    let names: Vec<&str> = tts.available_voices();
    Ok(Json(serde_json::json!({"voices": names})))
}

async fn synthesize(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<SynthesizeRequest>,
) -> Result<impl IntoResponse, ApiError> {
    auth(&headers, &state.token)?;

    let text = strip_markdown(&req.text);
    if text.is_empty() {
        return Err(api_err(
            StatusCode::BAD_REQUEST,
            "Text is empty after preprocessing",
        ));
    }

    let voice = req.voice;
    let speed = req.speed;
    let tts = state.tts.clone();

    let samples = tokio::task::spawn_blocking(move || {
        let mut tts = tts.blocking_lock();
        tts.generate(&text, &voice, speed, true)
    })
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("TTS task failed: {e}")))?
    .map_err(|e| {
        api_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("TTS generation failed: {e}"),
        )
    })?;

    let wav = encode_wav(&samples).map_err(|e| {
        api_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("WAV encoding failed: {e}"),
        )
    })?;

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "audio/wav")],
        wav,
    ))
}

// ── Main ─────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = env_logger::try_init();
    let args = Args::parse();

    // Prepend espeak-ng dir to PATH if provided.
    if let Some(ref espeak_dir) = args.espeak_dir {
        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{};{}", espeak_dir.display(), current_path);
        std::env::set_var("PATH", &new_path);
        log::info!("Prepended {} to PATH for espeak-ng", espeak_dir.display());
    }

    let token =
        std::env::var("SMOLPC_ENGINE_TOKEN").map_err(|_| "SMOLPC_ENGINE_TOKEN is required")?;

    if !args.model_dir.is_dir() {
        return Err(format!(
            "Model directory does not exist: {}",
            args.model_dir.display()
        )
        .into());
    }

    log::info!("Loading KittenTTS model from {}", args.model_dir.display());
    let tts = kitten_tts::model::KittenTTS::from_dir(&args.model_dir)
        .map_err(|e| format!("Failed to load TTS model: {e}"))?;
    log::info!("KittenTTS model loaded successfully");

    let state = AppState {
        token: Arc::new(token),
        tts: Arc::new(Mutex::new(tts)),
    };

    let app = axum::Router::new()
        .route("/health", get(health))
        .route("/voices", get(voices))
        .route("/synthesize", post(synthesize))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", args.port)).await?;
    log::info!(
        "smolpc-tts-server listening on http://127.0.0.1:{}",
        args.port
    );
    println!(
        "smolpc-tts-server listening on http://127.0.0.1:{}",
        args.port
    );

    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
        log::info!("Shutdown signal received");
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;

    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_markdown_removes_code_fences() {
        let input = "Hello\n```rust\nfn main() {}\n```\nWorld";
        assert_eq!(strip_markdown(input), "Hello World");
    }

    #[test]
    fn strip_markdown_removes_inline_backticks() {
        assert_eq!(strip_markdown("Use `println!` here"), "Use println! here");
    }

    #[test]
    fn strip_markdown_removes_bold_and_headings() {
        let input = "## Title\n**bold text**\n> quote";
        assert_eq!(strip_markdown(input), "Title bold text quote");
    }

    #[test]
    fn strip_markdown_removes_list_markers() {
        let input = "- item one\n- item two";
        assert_eq!(strip_markdown(input), "item one item two");
    }

    #[test]
    fn strip_markdown_handles_empty_input() {
        assert_eq!(strip_markdown(""), "");
    }

    #[test]
    fn strip_markdown_collapses_whitespace() {
        assert_eq!(strip_markdown("hello   world"), "hello world");
    }

    #[test]
    fn encode_wav_produces_valid_header() {
        let samples = vec![0.0f32; 24000]; // 1 second of silence
        let wav = encode_wav(&samples).expect("encode");
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        // 24000 samples * 2 bytes = 48000 data bytes + 44 header = 48044
        assert_eq!(wav.len(), 48044);
    }
}
