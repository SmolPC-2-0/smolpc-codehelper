# Voice I/O — Implementation Phases

**Full spec:** `docs/superpowers/specs/2026-03-24-voice-io-design.md`
**Date:** 2026-03-24

This document splits the voice I/O feature into 5 self-contained phases. Each phase produces working, testable code. A fresh session should read **this phase section + the referenced spec sections** — not the entire spec.

## Dependency Graph

```
Phase 1 (TTS sidecar binary) ──────────────┐
                                            ├──→ Phase 3 (sidecar lifecycle + proxy)
Phase 2 (Whisper STT in engine) ───────────┤
                                            └──→ Phase 4 (Tauri audio I/O) ──→ Phase 5 (Frontend UI)
```

Phases 1 and 2 are independent — can be implemented in either order.

---

## Phase 1: TTS Sidecar Binary

**Goal:** Build a standalone `smolpc-tts-server` binary that wraps `kitten_tts_rs` behind an HTTP API. Runs independently — no engine integration yet.

**Depends on:** Nothing. First thing to build.

### What to Build

A new binary crate at `engine/crates/smolpc-tts-server/` — a minimal axum HTTP server (~200 lines) that:

1. Loads a KittenTTS nano INT8 model from a directory
2. Exposes `GET /health`, `POST /synthesize`, `GET /voices`
3. Validates auth via `Authorization: Bearer <token>` (from `SMOLPC_ENGINE_TOKEN` env var)
4. Invokes `espeak-ng` for phonemization (must be on PATH or specified via `--espeak-dir`)
5. Returns WAV audio bytes (mono, 24kHz, 16-bit signed PCM)

### Key Constraints

- **ORT conflict is why this is a separate binary.** `kitten_tts_rs` statically links `ort 2.0.0-rc.12` (ONNX Runtime 1.24). The engine dynamically loads `onnxruntime.dll` v1.23 via `libloading`. Two ORT instances in one process = undefined behavior. Separate process = zero conflict.
- **`kitten_tts_rs` is a git dependency**, not on crates.io. Source: `github.com/second-state/kitten_tts_rs`.
- **`kitten_tts_rs` shells out to `espeak-ng`** as a subprocess (`Command::new("espeak-ng")`) for phonemization. On Windows, `espeak-ng.exe` must be on PATH. The `--espeak-dir` CLI arg prepends a directory to PATH so bundled espeak-ng is found.
- **This crate is NOT part of the engine workspace's default build.** It builds separately (`cargo build -p smolpc-tts-server`) to avoid pulling `ort` into the engine's dependency tree.

### CLI Interface

```
smolpc-tts-server --port 19433 --model-dir /path/to/kittentts-nano [--espeak-dir /path/to/espeak-ng]
```

| Arg | Required | Default | Description |
|-----|----------|---------|-------------|
| `--port` | No | `19433` | HTTP listen port |
| `--model-dir` | Yes | — | Directory containing `config.json`, ONNX model, `voices.npz` |
| `--espeak-dir` | No | System PATH | Directory containing `espeak-ng.exe`. Prepended to PATH at startup. |

Environment variables:
- `SMOLPC_ENGINE_TOKEN` — required, used for Bearer auth
- `RUST_LOG` — standard `env_logger` control (default: `info`)

### Endpoints

**`GET /health`**
- Returns `200 { "ok": true }` if model is loaded.
- Returns `503 { "ok": false }` if model failed to load.

**`POST /synthesize`**
- Request: `{ "text": "Hello world", "voice": "bella" }`
- `voice` is optional, defaults to `"bella"`.
- Available voices: `bella`, `jasper`, `luna`, `bruno`, `rosie`, `hugo`, `kiki`, `leo`.
- Response: WAV bytes as `Content-Type: audio/wav`.
- Returns `400` for empty text.
- Returns `503` if model is not loaded.

**`GET /voices`**
- Returns `{ "voices": ["bella", "jasper", "luna", "bruno", "rosie", "hugo", "kiki", "leo"] }`

### Text Preprocessing

Before passing text to KittenTTS, strip content that should not be spoken:

1. Remove code fences and their contents (` ```...``` `)
2. Remove inline code backticks (keep the text inside)
3. Strip markdown symbols (`**`, `##`, `- `, `>`, etc.)
4. Collapse whitespace
5. Pass only natural language prose

### KittenTTS Model Directory

Source: `KittenML/kitten-tts-nano-0.8-int8` from HuggingFace. Download manually:

```bash
mkdir -p models/kittentts-nano
for FILE in config.json kitten_tts_nano_v0_8.onnx voices.npz; do
  curl -L -o "models/kittentts-nano/$FILE" \
    "https://huggingface.co/KittenML/kitten-tts-nano-0.8-int8/resolve/main/$FILE"
done
```

**Layout (exactly 3 files):**
```
models/kittentts-nano/
  config.json                      (688 B)
  kitten_tts_nano_v0_8.onnx       (24.4 MB)
  voices.npz                       (3.28 MB)
```

`config.json` contents (for reference):
```json
{
  "name": "Kitten TTS Nano",
  "version": "0.8",
  "type": "ONNX2",
  "model_file": "kitten_tts_nano_v0_8.onnx",
  "voices": "voices.npz",
  "voice_aliases": {
    "Bella": "expr-voice-2-f",
    "Jasper": "expr-voice-2-m",
    "Luna": "expr-voice-3-f",
    "Bruno": "expr-voice-3-m",
    "Rosie": "expr-voice-4-f",
    "Hugo": "expr-voice-4-m",
    "Kiki": "expr-voice-5-f",
    "Leo": "expr-voice-5-m"
  },
  "speed_priors": {
    "expr-voice-2-f": 0.8, "expr-voice-2-m": 0.8,
    "expr-voice-3-m": 0.8, "expr-voice-3-f": 0.8,
    "expr-voice-4-m": 0.9, "expr-voice-4-f": 0.8,
    "expr-voice-5-m": 0.8, "expr-voice-5-f": 0.8
  }
}
```

### espeak-ng Bundling (for production — not needed for dev testing)

For dev testing: install `espeak-ng` system-wide and it'll be on PATH.

For production bundling (Phase 3 handles this):
```
libs/tts/espeak-ng/
  espeak-ng.exe
  espeak-ng-data/
    phontab, phonindex, phondata, intonations
    en_dict
    voices/
```

### Verification

After building, test standalone:

```bash
# Terminal 1: start the server
SMOLPC_ENGINE_TOKEN=test-token cargo run -p smolpc-tts-server -- \
  --port 19433 --model-dir models/kittentts-nano

# Terminal 2: test endpoints
curl -s http://localhost:19433/health -H "Authorization: Bearer test-token"
# → {"ok":true}

curl -s http://localhost:19433/voices -H "Authorization: Bearer test-token"
# → {"voices":["bella","jasper","luna","bruno","rosie","hugo","kiki","leo"]}

curl -s -X POST http://localhost:19433/synthesize \
  -H "Authorization: Bearer test-token" \
  -H "Content-Type: application/json" \
  -d '{"text":"Hello, I am a coding assistant.","voice":"bella"}' \
  --output test.wav
# → test.wav should be a playable WAV file
```

### Files to Create

| File | Purpose |
|------|---------|
| `engine/crates/smolpc-tts-server/Cargo.toml` | Crate manifest — `kitten_tts_rs` (git dep), `axum`, `tokio`, `clap`, `serde`, `serde_json`, `log`, `env_logger` |
| `engine/crates/smolpc-tts-server/src/main.rs` | CLI parsing, server setup, model loading, route handlers |

Optionally split into `main.rs` + `routes.rs` + `preprocessing.rs` if it exceeds ~300 lines, but start with a single file — YAGNI.

### Quality Checks

```bash
cargo check -p smolpc-tts-server
cargo clippy -p smolpc-tts-server
```

Note: `cargo check --workspace` and `cargo clippy --workspace` should NOT include this crate (it's excluded from the workspace default members to avoid pulling `ort` into the engine build).

---

## Phase 2: Whisper STT in Engine Host

**Goal:** Add a `POST /v1/audio/transcriptions` endpoint to the engine host that transcribes 16kHz mono f32 audio using OpenVINO GenAI's WhisperPipeline C API.

**Depends on:** Nothing. Independent of Phase 1.

### What to Build

1. **`whisper_ffi.rs`** — FFI bindings for WhisperPipeline symbols from the already-loaded `openvino_genai_c.dll`
2. **WhisperPipeline lifecycle** — load on first request, stay resident (~400-500 MB)
3. **`POST /v1/audio/transcriptions` endpoint** — accepts raw f32 PCM bytes, returns `{ "text": "..." }`
4. **`voice_semaphore`** in `AppState` — separate from the existing `generation_semaphore`

### Key Constraints

- **All symbols come from `openvino_genai_c.dll`** — already loaded and retained by the OpenVINO runtime loader. No new DLLs.
- **DLL loading centralization** — `whisper_ffi.rs` must NOT call `Library::new()` directly. The source-invariant test at `runtime_loading.rs:905` scans all `.rs` files and fails CI if DLL loading happens outside `runtime_loading.rs`. Load symbols from the already-retained `openvino_genai_c.dll` library handle.
- **Device is always `"CPU"`** — no backend selection, no preflight probes.
- **Input format is fixed:** 16kHz mono f32, little-endian. The Tauri app (Phase 4) handles all resampling. The engine just validates the data.

### FFI Symbols (from `openvino_genai_c.dll`)

Verified to exist in the OpenVINO GenAI 2026.0.0 C API headers:

```c
// Pipeline lifecycle
ov_status_e ov_genai_whisper_pipeline_create(
    const char* models_path, const char* device,
    const size_t property_args_size,
    ov_genai_whisper_pipeline** pipeline, ...);

ov_status_e ov_genai_whisper_pipeline_generate(
    ov_genai_whisper_pipeline* pipeline,
    const float* raw_speech, size_t raw_speech_size,
    const ov_genai_whisper_generation_config* config,
    ov_genai_whisper_decoded_results** results);

void ov_genai_whisper_pipeline_free(ov_genai_whisper_pipeline* pipeline);

// Results
void ov_genai_whisper_decoded_results_free(ov_genai_whisper_decoded_results* results);
// Plus: _to_string, _get_texts_size, _get_text, etc.

// Config (optional — defaults work for English transcription)
// ov_genai_whisper_generation_config_* functions for language/task
```

### Pattern to Follow

Read `engine/crates/smolpc-engine-core/src/inference/genai/openvino_ffi.rs` — the existing OpenVINO GenAI FFI layer. `whisper_ffi.rs` follows the same structure:
- Opaque pointer types (`*mut c_void`)
- `load_symbol!` macro or equivalent from the retained library
- Wrapper struct holding function pointers
- Error handling via `ov_status_e` return codes

### Endpoint

`POST /v1/audio/transcriptions`

- **Request:** `Content-Type: application/octet-stream` — raw bytes of f32 PCM samples (16kHz, mono, little-endian)
- **Response:** `{ "text": "the transcribed text" }`
- **Errors:** `503` if Whisper model failed to load, `400` if body is empty or not valid f32 data
- **Concurrency:** Guarded by `voice_semaphore` (new, capacity 1), NOT the existing `generation_semaphore`

### Voice Semaphore

Add to `AppState` in `engine/crates/smolpc-engine-host/src/state.rs:364`:
```rust
pub(crate) voice_semaphore: Arc<Semaphore>,
```

Initialize in `engine/crates/smolpc-engine-host/src/main.rs:40` (next to `generation_semaphore`):
```rust
voice_semaphore: Arc::new(Semaphore::new(1)),
```

The voice semaphore governs both STT (this phase) and TTS proxy (Phase 3). It is independent of the generation semaphore — a student can play TTS while LLM is generating.

### Whisper Model Directory

Source: `OpenVINO/whisper-base.en-int8-ov` from HuggingFace. Download manually.

**Layout (flat — no subdirectories):**
```
models/whisper-base.en/openvino/
  openvino_encoder_model.xml + .bin   (23.4 MB)
  openvino_decoder_model.xml + .bin   (53.0 MB)
  openvino_tokenizer.xml + .bin
  openvino_detokenizer.xml + .bin
  config.json, generation_config.json, preprocessor_config.json
  tokenizer.json, vocab.json, merges.txt, ...
```

**WhisperPipeline takes a single directory path** — it discovers encoder/decoder/tokenizer by `openvino_*` filename convention. No subdirectory structure needed.

### Verification

```bash
# Start engine with Whisper model available
cargo run -p smolpc-engine-host

# Generate test audio (16kHz mono f32, 2 seconds of silence — should return empty/silence transcription)
python3 -c "
import struct, sys
samples = [0.0] * 32000  # 2 seconds at 16kHz
sys.stdout.buffer.write(struct.pack(f'{len(samples)}f', *samples))
" > /tmp/silence.raw

# Test endpoint
curl -s -X POST http://localhost:19432/v1/audio/transcriptions \
  -H "Authorization: Bearer $(cat path/to/engine-token.txt)" \
  -H "Content-Type: application/octet-stream" \
  --data-binary @/tmp/silence.raw
# → {"text":""}  (silence produces empty transcription)
```

For real speech testing, record a WAV file, convert to 16kHz mono f32 raw, and POST it.

### Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `engine/crates/smolpc-engine-core/src/inference/genai/whisper_ffi.rs` | Create | WhisperPipeline FFI bindings |
| `engine/crates/smolpc-engine-core/src/inference/genai/mod.rs` | Modify | Add `pub mod whisper_ffi;` |
| `engine/crates/smolpc-engine-host/src/routes.rs` | Modify | Add `v1_audio_transcriptions` handler |
| `engine/crates/smolpc-engine-host/src/state.rs` | Modify | Add `voice_semaphore` to `AppState` |
| `engine/crates/smolpc-engine-host/src/main.rs` | Modify | Initialize `voice_semaphore`, add route, add Whisper state |
| `engine/crates/smolpc-engine-host/src/whisper.rs` | Create | WhisperPipeline lifecycle management (load, hold, drop) |

### Quality Checks

```bash
cargo check --workspace
cargo clippy --workspace
cargo test -p smolpc-engine-core
cargo test -p smolpc-engine-host
```

---

## Phase 3: TTS Sidecar Lifecycle + Proxy

**Goal:** The engine host spawns the TTS sidecar (Phase 1's binary) during startup, manages its lifecycle, and proxies `POST /v1/audio/speech` requests to it.

**Depends on:** Phase 1 (TTS sidecar binary must be built) and Phase 2 (voice semaphore must exist).

### What to Build

1. **TTS sidecar spawn** — engine host launches `smolpc-tts-server` as a detached process during startup
2. **TTS sidecar health check** — lazy (on first TTS request), not periodic
3. **TTS sidecar shutdown** — killed on engine host shutdown
4. **`POST /v1/audio/speech` proxy endpoint** — forwards to sidecar, returns WAV bytes
5. **PID tracking** — `tts.pid` file, same pattern as `engine.pid`

### Key Constraints

- **Follow the existing spawn pattern exactly.** Read `engine/crates/smolpc-engine-client/src/spawn.rs` — the `spawn_engine()` function is the template. TTS spawn uses the same: `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`, stderr → `tts-spawn.log`, PID → `tts.pid`.
- **Engine host owns the TTS sidecar, not the Tauri supervisor.** The supervisor manages the engine process. The engine process manages the TTS sidecar. This keeps the Tauri app simple — it only talks to the engine.
- **Eager start.** TTS sidecar spawns during engine startup (in `startup.rs`), not on first TTS request. Cold start is 0.5–1.5s — eliminated by pre-warming.
- **Token reuse.** Pass `SMOLPC_ENGINE_TOKEN` to the sidecar so both use the same auth.
- **Graceful degradation.** If TTS binary is missing, log a warning and skip. `POST /v1/audio/speech` returns `503`. No crash.

### TTS Proxy Endpoint

`POST /v1/audio/speech`

- **Request:** `{ "text": "the text to speak", "voice": "bella" }`
- `voice` is optional, defaults to `"bella"`.
- **Response:** WAV bytes as `Content-Type: audio/wav` (proxied from TTS sidecar).
- **Errors:** `503` if TTS sidecar is not healthy.
- **Concurrency:** Guarded by `voice_semaphore` (from Phase 2).

### Proxy Implementation Sketch

```rust
async fn v1_audio_speech(state, req) -> Response {
    let _permit = state.voice_semaphore.acquire().await;

    // Lazy health check — only when a TTS request arrives
    if !tts_client_healthy(&state.tts_client).await {
        attempt_tts_respawn(&state).await;
        if !tts_client_healthy(&state.tts_client).await {
            return (StatusCode::SERVICE_UNAVAILABLE,
                    Json(ErrorResponse { error: "TTS service unavailable".into() }));
        }
    }

    // Forward to sidecar
    let response = state.tts_http_client
        .post(format!("http://127.0.0.1:{}/synthesize", state.tts_port))
        .bearer_auth(&state.token)
        .json(&req)
        .send().await?;

    // Proxy WAV bytes back
    let bytes = response.bytes().await?;
    (StatusCode::OK, [("content-type", "audio/wav")], bytes)
}
```

### Sidecar Spawn Pattern

Reference: `engine/crates/smolpc-engine-client/src/spawn.rs:25-106`

```rust
// In engine host startup — after engine is ready
fn spawn_tts_sidecar(resource_dir: &Path, token: &str, models_dir: &Path) -> Result<u32, String> {
    let tts_binary = resource_dir.join("binaries").join("smolpc-tts-server.exe");
    if !tts_binary.exists() {
        log::warn!("TTS binary not found at {}, voice output disabled", tts_binary.display());
        return Err("TTS binary not found".into());
    }

    let model_dir = models_dir.join("kittentts-nano");
    let espeak_dir = resource_dir.join("libs").join("tts").join("espeak-ng");

    let mut cmd = Command::new(&tts_binary);
    cmd.arg("--port").arg("19433")
       .arg("--model-dir").arg(&model_dir)
       .env("SMOLPC_ENGINE_TOKEN", token)
       .env("RUST_LOG", "info");

    if espeak_dir.exists() {
        cmd.arg("--espeak-dir").arg(&espeak_dir);
    }

    // Same detached process flags as spawn_engine()
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x00000008 | 0x00000200)  // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
           .stdin(Stdio::null())
           .stdout(Stdio::null())
           .stderr(/* tts-spawn.log */);
    }

    let child = cmd.spawn().map_err(|e| format!("TTS spawn failed: {e}"))?;
    let pid = child.id();
    // Write tts.pid
    Ok(pid)
}
```

### Shutdown

On engine host shutdown, kill the TTS sidecar:
1. Read `tts.pid`
2. Verify PID identity (same stale-PID protection as engine — `is_pid_alive()`)
3. Terminate the process
4. Remove `tts.pid`

This extends the engine's existing shutdown path (triggered by `POST /engine/shutdown`).

### Verification

```bash
# Start engine (it should spawn TTS sidecar automatically)
cargo run -p smolpc-engine-host
# Log should show: "TTS sidecar spawned with PID XXXX" or "TTS binary not found, voice output disabled"

# Test TTS proxy
curl -s -X POST http://localhost:19432/v1/audio/speech \
  -H "Authorization: Bearer $(cat path/to/engine-token.txt)" \
  -H "Content-Type: application/json" \
  -d '{"text":"Hello, I am a coding assistant.","voice":"bella"}' \
  --output test.wav
# → test.wav should be playable

# Verify shutdown kills sidecar
curl -s -X POST http://localhost:19432/engine/shutdown \
  -H "Authorization: Bearer $(cat path/to/engine-token.txt)"
# → TTS sidecar process should also exit
```

### Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `engine/crates/smolpc-engine-host/src/tts_sidecar.rs` | Create | TTS sidecar spawn, health check, shutdown, PID management |
| `engine/crates/smolpc-engine-host/src/routes.rs` | Modify | Add `v1_audio_speech` proxy handler |
| `engine/crates/smolpc-engine-host/src/state.rs` | Modify | Add TTS sidecar state (PID, HTTP client, port) to `AppState` |
| `engine/crates/smolpc-engine-host/src/main.rs` | Modify | Initialize TTS state, spawn sidecar during startup, add route |
| `engine/crates/smolpc-engine-host/src/startup.rs` | Modify | Call TTS sidecar spawn after engine reaches Ready state |

### Quality Checks

```bash
cargo check --workspace
cargo clippy --workspace
cargo test -p smolpc-engine-host
```

---

## Phase 4: Tauri Audio I/O

**Goal:** Add audio capture (cpal + rubato resampling) and playback (rodio) as Tauri commands. The Tauri app can now record speech, send it to the engine for transcription, and play TTS audio.

**Depends on:** Phases 2 (STT endpoint) and 3 (TTS proxy endpoint).

### What to Build

1. **`start_recording` command** — open mic via cpal, capture at native rate
2. **`stop_recording` command** — stop capture, resample to 16kHz mono, POST to engine, return text
3. **`speak_text` command** — POST text to engine TTS endpoint, play WAV via rodio
4. **`stop_playback` command** — stop audio playback

### Key Constraints

- **Resampling is mandatory.** Windows WASAPI captures at the device's native rate (typically 48kHz, sometimes 44.1kHz). Whisper requires 16kHz mono f32. The `rubato` crate handles downsampling for any source rate.
- **Use `tauri::async_runtime::spawn`** in setup — never bare `tokio::spawn` (CLAUDE.md convention).
- **Tauri Channels for streaming** are NOT needed here — recording/playback are command-based (start/stop), not streaming.

### Dependencies (add to `apps/codehelper/src-tauri/Cargo.toml`)

```toml
cpal = "0.17"       # Audio capture (WASAPI on Windows)
rodio = "0.19"      # Audio playback
rubato = "1.0"      # Sample rate conversion (any rate → 16kHz)
```

### Audio Capture Flow

```
Mic hardware (48kHz stereo, or whatever the device provides)
    ↓  cpal captures raw f32 samples at native rate
Arc<Mutex<Vec<f32>>> buffer (accumulates during recording)
    ↓  student clicks "stop recording"
Stereo → mono mix: mono[i] = (left + right) / 2.0  (if stereo)
    ↓
rubato downsample: native rate → 16kHz
    ↓
POST /v1/audio/transcriptions (16kHz mono f32 bytes, little-endian)
    ↓
Return transcribed text to frontend
```

### Resampling Detail

`rubato` handles any source-to-target rate ratio:
- **48000 → 16000** (3:1 integer ratio): `FftFixedIn` — efficient FFT-based
- **44100 → 16000** (2.75625 irrational ratio): `SincFixedIn` — sinc interpolation
- **16000 → 16000**: No resampling, pass through directly

The code queries `device.default_input_config()` at capture start to get the native rate, then selects the appropriate resampler. For 30 seconds of audio, resampling takes <50ms on CPU.

### Tauri Commands

**`start_recording`**
```rust
#[tauri::command]
async fn start_recording(state: State<'_, AudioState>) -> Result<(), String> {
    let host = cpal::default_host();
    let device = host.default_input_device()
        .ok_or("No microphone detected")?;
    let config = device.default_input_config()
        .map_err(|e| format!("Failed to get input config: {e}"))?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let buffer = Arc::new(Mutex::new(Vec::new()));
    // ... build input stream, store in AudioState
    Ok(())
}
```

**`stop_recording`**
```rust
#[tauri::command]
async fn stop_recording(
    audio_state: State<'_, AudioState>,
    engine_state: State<'_, SupervisorHandle>,
) -> Result<String, String> {
    // 1. Stop cpal stream
    // 2. Take samples from buffer
    // 3. Mono mix if stereo
    // 4. Resample to 16kHz via rubato (if native rate != 16000)
    // 5. Convert f32 samples to bytes (little-endian)
    // 6. POST to engine /v1/audio/transcriptions
    // 7. Return transcribed text
    Ok(transcribed_text)
}
```

**`speak_text`**
```rust
#[tauri::command]
async fn speak_text(
    text: String,
    voice: Option<String>,
    audio_state: State<'_, AudioState>,
    engine_state: State<'_, SupervisorHandle>,
) -> Result<(), String> {
    // 1. POST to engine /v1/audio/speech
    // 2. Receive WAV bytes
    // 3. Decode WAV, play via rodio Sink
    // 4. Store Sink handle in AudioState for stop_playback
    Ok(())
}
```

**`stop_playback`**
```rust
#[tauri::command]
fn stop_playback(state: State<'_, AudioState>) -> Result<(), String> {
    // Stop rodio Sink
    Ok(())
}
```

### AudioState (Tauri managed state)

```rust
pub struct AudioState {
    recording_stream: Mutex<Option<cpal::Stream>>,
    recording_buffer: Arc<Mutex<Vec<f32>>>,
    recording_config: Mutex<Option<RecordingConfig>>,
    playback_sink: Mutex<Option<rodio::Sink>>,
    output_stream: Mutex<Option<rodio::OutputStream>>,
}

struct RecordingConfig {
    sample_rate: u32,
    channels: u16,
}
```

### Microphone Permissions

cpal's `default_input_device()` returns `None` if no mic is available or permission is denied. The `start_recording` command returns an error string that the frontend can display.

### Verification

Manual testing only — audio requires actual hardware:

1. Start the full app (`npm run tauri:dev`)
2. Call `start_recording` from the Tauri dev console
3. Speak into the mic for a few seconds
4. Call `stop_recording` — verify transcribed text is returned
5. Call `speak_text` with some text — verify audio plays through speakers
6. Call `stop_playback` — verify audio stops

### Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `apps/codehelper/src-tauri/src/audio.rs` | Create | AudioState, recording commands, playback commands, resampling |
| `apps/codehelper/src-tauri/src/lib.rs` | Modify | Register audio commands and AudioState |
| `apps/codehelper/src-tauri/Cargo.toml` | Modify | Add `cpal`, `rodio`, `rubato` dependencies |

### Quality Checks

```bash
cargo check --workspace
cargo clippy --workspace
cd apps/codehelper && npm run check && npm run lint
```

---

## Phase 5: Frontend UI

**Goal:** Add mic button (STT) and play button (TTS) to the chat interface. Wire them to the Tauri commands from Phase 4.

**Depends on:** Phase 4 (Tauri audio commands).

### What to Build

1. **Mic button** — next to send button, click-to-start/click-to-stop recording, insert transcription at cursor
2. **Play button** — on each AI response, click-to-play/click-to-stop TTS
3. **State management** — recording state, playback state, availability detection

### Key Constraints

- **Svelte 5 runes only.** `$state`, `$derived`, `$effect`. No `writable`/`readable` from `svelte/store`.
- **Tailwind 4.** Utility classes only, no `@apply`.
- **Tauri invoke** for calling Rust commands: `import { invoke } from '@tauri-apps/api/core';`

### Mic Button States

| State | Visual | Behavior |
|-------|--------|----------|
| Idle | Mic icon | Click → start recording |
| Recording | Red pulsing indicator | Click → stop recording, begin transcription |
| Processing | Loading spinner | Not clickable. Waiting for Whisper. |
| Disabled | Greyed out mic | Tooltip: "No microphone detected" or "Voice input unavailable" |

### Play Button States

| State | Visual | Behavior |
|-------|--------|----------|
| Idle | Small speaker icon | Click → start TTS synthesis + playback |
| Loading | Loading spinner | Not clickable. Waiting for TTS. |
| Playing | Animated speaker icon | Click → stop playback |
| Hidden | Not rendered | TTS sidecar unavailable (engine returned 503) |

### Frontend State

```typescript
// In the chat/voice state module
let micState = $state<'idle' | 'recording' | 'processing' | 'disabled'>('idle');
let playbackState = $state<Record<string, 'idle' | 'loading' | 'playing'>>({});
// keyed by message ID — each message has independent playback state

let micAvailable = $state(true); // set to false if start_recording fails with "No microphone"
let ttsAvailable = $state(true); // set to false if speak_text returns 503
```

### Mic Button Logic

```typescript
async function toggleRecording() {
    if (micState === 'recording') {
        micState = 'processing';
        try {
            const text = await invoke<string>('stop_recording');
            if (text.trim()) {
                insertTextAtCursor(text);  // insert into chat input
            }
        } catch (e) {
            console.error('Transcription failed:', e);
        }
        micState = 'idle';
    } else if (micState === 'idle') {
        try {
            await invoke('start_recording');
            micState = 'recording';
        } catch (e) {
            if (e.includes('No microphone')) {
                micAvailable = false;
                micState = 'disabled';
            }
        }
    }
}
```

### Play Button Logic

```typescript
async function togglePlayback(messageId: string, text: string) {
    const current = playbackState[messageId] ?? 'idle';
    if (current === 'playing') {
        await invoke('stop_playback');
        playbackState[messageId] = 'idle';
    } else if (current === 'idle') {
        playbackState[messageId] = 'loading';
        try {
            await invoke('speak_text', { text, voice: 'bella' });
            playbackState[messageId] = 'playing';
        } catch (e) {
            if (e.includes('503') || e.includes('unavailable')) {
                ttsAvailable = false;
            }
            playbackState[messageId] = 'idle';
        }
    }
}
```

### Verification

Manual testing:

1. Open app — mic button should be visible and idle
2. Click mic → should turn red/pulsing
3. Speak → click mic again → spinner → text appears in input
4. Send a message → AI responds → play button visible on response
5. Click play → spinner → audio plays → animated speaker icon
6. Click speaker → audio stops
7. Test with no mic → button should be disabled
8. Test with TTS sidecar stopped → play button should be hidden

### Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `apps/codehelper/src/lib/components/MicButton.svelte` | Create | Mic button component with states |
| `apps/codehelper/src/lib/components/PlayButton.svelte` | Create | Play button component with states |
| `apps/codehelper/src/lib/components/ChatInput.svelte` (or equivalent) | Modify | Add MicButton next to send button |
| `apps/codehelper/src/lib/components/MessageBubble.svelte` (or equivalent) | Modify | Add PlayButton to AI response messages |

Note: Exact component file paths depend on the current frontend structure — read the existing components before creating new ones.

### Quality Checks

```bash
cd apps/codehelper && npm run check && npm run lint
```
