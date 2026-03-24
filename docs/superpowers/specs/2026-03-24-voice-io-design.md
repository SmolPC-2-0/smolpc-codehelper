# Voice I/O: Whisper STT + KittenTTS

**Date:** 2026-03-24
**Status:** Approved
**Supersedes:** `2026-03-23-voice-io-design.md` (draft — contained incorrect assumptions; see Research Corrections below)
**Scope:** Speech-to-text input and text-to-speech output for SmolPC Code Helper

---

## Overview

Add voice input (speech-to-text) and voice output (text-to-speech) to SmolPC Code Helper. Students can dictate into the chat input and hear AI responses read aloud. Both capabilities run fully offline on 8 GB RAM Intel laptops.

**In scope:**
- Whisper-based speech-to-text (voice input)
- KittenTTS-based text-to-speech (read-aloud on AI responses)
- English-only for v1

**Out of scope:**
- Voice commands / wake word detection
- Multilingual voice support
- Streaming TTS synthesis
- Auto-read (TTS is on-demand only)

---

## Research Corrections

Five technical assumptions from the original draft were verified on 2026-03-24. Three required corrections:

| # | Assumption | Verdict | Correction |
|---|-----------|---------|------------|
| 1 | KittenTTS can live in-process with the engine | **Blocked** | `kitten_tts_rs` statically links ORT 1.24 via the `ort` crate. The engine dynamically loads ORT 1.23 via `libloading` for DirectML/GenAI. Two ORT instances in one process causes undefined behavior (conflicting `OrtEnv` singletons). **TTS must run as a separate sidecar process.** Version alignment was evaluated and rejected — even with matching versions, two ORT init paths sharing one DLL is untested territory. |
| 2 | WhisperPipeline C API symbols exist | **Confirmed** | `ov_genai_whisper_pipeline_create`, `_generate`, `_free` all exist in `openvino_genai_c.dll` (same DLL already bundled). 15 total C API functions for Whisper. |
| 3 | cpal captures 16kHz mono directly | **Corrected** | Windows WASAPI captures at the device's native rate (typically 48kHz). cpal 0.17.2+ has `AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM` but this is driver-dependent and unreliable on budget hardware. **Must capture at native rate and resample to 16kHz using `rubato`.** |
| 4 | Whisper model has encoder/decoder subdirectories | **Corrected** | Actual layout at `OpenVINO/whisper-base.en-int8-ov` is **flat** — all files at root, distinguished by filename prefix (`openvino_encoder_model.*`, `openvino_decoder_model.*`). No subdirectories. |
| 5 | KittenTTS model is `model.onnx` + config files | **Corrected** | Exactly three files: `config.json` (688 B), `kitten_tts_nano_v0_8.onnx` (24.4 MB), `voices.npz` (3.28 MB). Also discovered: `kitten_tts_rs` shells out to `espeak-ng` as a subprocess for phonemization — **must bundle `espeak-ng.exe` + `espeak-ng-data/`**. |

---

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| STT model | `whisper-base.en` INT8 OpenVINO IR | Best accuracy/size tradeoff. ~85 MB disk, ~400–500 MB RAM. Pre-exported on HuggingFace (`OpenVINO/whisper-base.en-int8-ov`). |
| STT runtime | OpenVINO GenAI WhisperPipeline (C API) | Same `openvino_genai_c.dll` already loaded for LLM inference. Zero new DLL dependencies. |
| STT device | CPU always | Whisper bursts are <2s. Not worth NPU contention with LLM. |
| TTS engine | KittenTTS nano INT8 via `kitten_tts_rs` | 27.7 MB total model. RTF 0.6–0.73 on laptop CPU. 8 built-in voices. |
| TTS process model | **Separate detached sidecar process** (`smolpc-tts-server`) | ORT DLL conflict prevents in-process integration. Sidecar follows the existing engine spawn pattern from `spawn.rs`. |
| TTS startup | **Eager** — spawned alongside engine during startup | Cold start is 0.5–1.5s (process spawn + ORT init + model load). Eager start eliminates first-click latency. RAM cost is ~150–200 MB, well within budget. |
| TTS fallback | Piper TTS (same sidecar pattern, same API contract) | If KittenTTS proves unstable, Piper is battle-tested. Same ORT conflict applies → same sidecar solution. |
| Audio capture | `cpal` in Tauri app + `rubato` for resampling | Capture at device native rate (typically 48kHz), downsample to 16kHz mono via `rubato`. Handles any source sample rate. |
| Audio playback | `rodio` in Tauri app | Lightweight, pairs with `cpal`. Plays WAV bytes from TTS endpoint. |
| Architecture | STT in engine host, TTS in sidecar, both behind engine HTTP API | Tauri app sees a single API surface. TTS sidecar is an implementation detail hidden behind the engine's `/v1/audio/speech` proxy endpoint. |
| Voice input UX | Click-to-start, click-to-stop. Transcription appends at cursor. | Student controls recording explicitly. Text appears in input box for review before send. |
| Voice output UX | On-demand play button per AI response. | Non-intrusive. Students click to hear, click again to stop. |
| Language | English-only | Target audience. Multilingual is same architecture, different model files — future expansion. |

---

## Architecture

### System Flow

```
┌───────────────────────────────────────────────────────────────┐
│  Tauri App                                                    │
│                                                               │
│  ┌──────────┐  cpal    ┌──────────────┐                      │
│  │ Mic btn  │─────────>│ Audio capture │                      │
│  │ (start/  │          │ native rate   │                      │
│  │  stop)   │          │ (e.g. 48kHz) │                      │
│  └──────────┘          └──────┬───────┘                      │
│                               │ rubato resample               │
│                               │ to 16kHz mono f32             │
│                               │                               │
│                               │ POST /v1/audio/               │
│                               │   transcriptions              │
│  ┌──────────┐          ┌──────▼───────┐                      │
│  │ Chat     │<─────────│ Transcribed  │                      │
│  │ input    │  insert  │ text         │                      │
│  │ (cursor) │  at pos  └──────────────┘                      │
│  └──────────┘                                                 │
│                                                               │
│  ┌──────────┐  rodio   ┌──────────────┐                      │
│  │ Play btn │<─────────│ WAV playback │                      │
│  │ (per     │          │              │                      │
│  │  message)│          └──────┬───────┘                      │
│  └──────────┘                 │ POST /v1/audio/               │
│                               │   speech                      │
└───────────────────────────────┼───────────────────────────────┘
                                │ HTTP (localhost:19432)
┌───────────────────────────────▼───────────────────────────────┐
│  Engine Host (:19432)                                         │
│                                                               │
│  ┌──────────────────────┐  ┌────────────────────────────┐    │
│  │ WhisperPipeline      │  │ TTS proxy                   │    │
│  │ (OpenVINO GenAI      │  │ POST /v1/audio/speech       │    │
│  │  C API, CPU)         │  │   → forwards to sidecar     │    │
│  │                      │  │   ← returns WAV bytes       │    │
│  │ whisper-base.en      │  └─────────────┬──────────────┘    │
│  │ INT8, ~85 MB         │                │                    │
│  │ ~400–500 MB RAM      │                │ HTTP :19433        │
│  └──────────────────────┘                │                    │
│                                          │                    │
│  ┌──────────────────────────────────┐    │                    │
│  │ LLMPipeline (Qwen2.5 / Qwen3)  │    │                    │
│  │ DirectML / OpenVINO NPU / CPU   │    │                    │
│  └──────────────────────────────────┘    │                    │
│                                          │                    │
│  ┌───────────────────────────────────────▼──────────────┐    │
│  │ smolpc-tts-server (sidecar process, :19433)          │    │
│  │                                                       │    │
│  │  kitten_tts_rs + ort (own ORT instance, CPU)         │    │
│  │  espeak-ng (bundled, subprocess for phonemization)   │    │
│  │  KittenTTS nano INT8 (24.4 MB model)                 │    │
│  │  ~150–200 MB RAM                                      │    │
│  │                                                       │    │
│  │  Spawned by engine host during startup.               │    │
│  │  Same detached process pattern as engine spawn.       │    │
│  └───────────────────────────────────────────────────────┘    │
└───────────────────────────────────────────────────────────────┘
```

### Resource Budget (8 GB machine)

| Component | RAM | Disk |
|-----------|-----|------|
| OS + Tauri app | ~2.5 GB | — |
| Qwen2.5-1.5B INT4 (loaded) | ~1.5–2.0 GB | 900 MB |
| whisper-base.en INT8 (loaded on first STT request) | ~400–500 MB | ~85 MB |
| TTS sidecar (pre-warmed at startup) | ~150–200 MB | ~28 MB model + ~20 MB ORT DLL + ~15 MB espeak-ng |
| **Total** | **~4.6–5.2 GB** | **+~148 MB** |
| **Headroom** | **~2.8–3.4 GB** | |

All components fit comfortably on 8 GB machines with room to spare.

---

## Engine Host: Whisper STT

### FFI Layer

New module `whisper_ffi.rs` in `smolpc-engine-core`, following the same pattern as `openvino_ffi.rs`.

**Symbol resolution goes through `runtime_loading.rs`** — `whisper_ffi.rs` does not call `Library::new()` directly, per the DLL loading centralization convention enforced by the source-invariant test at `runtime_loading.rs:905-933`.

Symbols loaded from the **already-loaded** `openvino_genai_c.dll`:

```c
// Pipeline lifecycle
ov_genai_whisper_pipeline_create(models_path, device, prop_count, &pipeline, ...)
ov_genai_whisper_pipeline_free(pipeline)

// Inference
ov_genai_whisper_pipeline_generate(pipeline, float* raw_speech, size, config, &results)

// Results
ov_genai_whisper_decoded_results_to_string(results) → char*
ov_genai_whisper_decoded_results_free(results)

// Config (optional — defaults work for English transcription)
ov_genai_whisper_generation_config_set_language(config, "en")
ov_genai_whisper_generation_config_set_task(config, "transcribe")
```

**Input:** `*const f32` audio samples — 16kHz mono, normalized to [-1.0, 1.0].
**Output:** `WhisperDecodedResults` containing transcribed text string.
**Device:** Always `"CPU"`. No backend selection needed.

### Endpoint

`POST /v1/audio/transcriptions`

- **Request:** Raw audio bytes as `application/octet-stream` (f32 PCM, 16kHz mono, little-endian). The Tauri app handles all resampling before sending.
- **Response:** `{ "text": "the transcribed text" }`
- Synchronous — no streaming. Typical latency: 0.5–2 seconds for up to 30 seconds of audio.
- Returns `503 Service Unavailable` if Whisper model fails to load.
- Guarded by the `voice_semaphore` (capacity 1), not the `generation_semaphore`.

### WhisperPipeline Lifecycle

- **Load:** On first transcription request (~1s cold start on CPU).
- **Resident:** Stays loaded at ~400–500 MB after first use.
- **Unload:** If engine memory pressure hits a warning threshold, drop the pipeline. Reload on next request (~1s). Student sees a slightly longer spinner.
- **No backend selection:** Always OpenVINO CPU. No preflight probes, no backend ranking.

### Whisper Model Directory

Source: `OpenVINO/whisper-base.en-int8-ov` from HuggingFace. Pre-exported — no conversion pipeline needed.

**Layout (flat — no subdirectories):**

```
models/whisper-base.en/openvino/
  openvino_encoder_model.xml          (296 KB)
  openvino_encoder_model.bin          (23.1 MB)
  openvino_decoder_model.xml          (564 KB)
  openvino_decoder_model.bin          (52.4 MB)
  openvino_tokenizer.xml              (27 KB)
  openvino_tokenizer.bin              (1.93 MB)
  openvino_detokenizer.xml            (9.7 KB)
  openvino_detokenizer.bin            (750 KB)
  config.json                         (1.27 KB)
  generation_config.json              (1.53 KB)
  openvino_config.json                (449 B)
  preprocessor_config.json            (356 B)
  tokenizer.json                      (3.86 MB)
  tokenizer_config.json               (283 KB)
  added_tokens.json                   (34.6 KB)
  special_tokens_map.json             (2.17 KB)
  vocab.json                          (798 KB)
  merges.txt                          (456 KB)
  normalizer.json                     (52.7 KB)
```

**Artifact validation:** `WhisperPipeline` takes a single directory path. Validate by checking for `openvino_encoder_model.xml` and `openvino_decoder_model.xml` (analogous to the existing `openvino/manifest.json` gate for LLM models).

---

## TTS Sidecar: `smolpc-tts-server`

### Crate

New binary crate at `engine/crates/smolpc-tts-server/`.

**Dependencies:**
- `kitten_tts_rs` (git dependency from `github.com/second-state/kitten_tts_rs`) — includes `ort 2.0.0-rc.12` internally
- `axum` + `tokio` — HTTP server
- `clap` — CLI args
- `log` + `env_logger` — logging

This crate is **not** part of the engine workspace's default build. It is built separately (own `cargo build -p smolpc-tts-server`) and its binary is placed alongside the engine binary in the bundle.

### Endpoints

**`GET /health`**
- Returns `{ "ok": true }` if model is loaded.
- Returns `503` if model failed to load.

**`POST /synthesize`**
- Request: `{ "text": "Hello world", "voice": "bella" }`
- `voice` is optional, defaults to `"bella"`.
- Available voices: `bella`, `jasper`, `luna`, `bruno`, `rosie`, `hugo`, `kiki`, `leo` (4 female, 4 male — defined in KittenTTS `config.json` `voice_aliases`).
- Response: WAV bytes as `audio/wav` (mono, 24kHz, 16-bit signed PCM).
- Returns `400` for empty text.
- Returns `503` if model is not loaded.

**`GET /voices`**
- Returns `{ "voices": ["bella", "jasper", "luna", "bruno", "rosie", "hugo", "kiki", "leo"] }`

### Auth

Reuses the engine's shared token. The engine host passes `SMOLPC_ENGINE_TOKEN` as an env var when spawning the sidecar. The sidecar validates `Authorization: Bearer <token>` on all requests, identical to the engine host's `auth()` middleware.

### CLI Interface

```
smolpc-tts-server --port 19433 --model-dir /path/to/kittentts-nano --espeak-dir /path/to/espeak-ng
```

| Arg | Required | Default | Description |
|-----|----------|---------|-------------|
| `--port` | No | `19433` | HTTP listen port |
| `--model-dir` | Yes | — | Path to directory containing `config.json`, ONNX model, `voices.npz` |
| `--espeak-dir` | No | System PATH | Path to directory containing `espeak-ng.exe`. If provided, the server sets this directory on `PATH` before invoking `kitten_tts_rs`. |

Environment variables:
- `SMOLPC_ENGINE_TOKEN` — required, used for auth
- `RUST_LOG` — standard `env_logger` control (default: `info`)

### Text Preprocessing

Before passing text to KittenTTS, strip content that should not be spoken. This preprocessing runs in the TTS sidecar, not the engine host:

1. Remove code fences and their contents (` ```...``` `)
2. Remove inline code backticks
3. Strip markdown symbols (`**`, `##`, `- `, `>`, etc.)
4. Collapse whitespace
5. Pass only natural language prose

### espeak-ng Integration

`kitten_tts_rs` invokes `espeak-ng` as a subprocess (`Command::new("espeak-ng")`) for phonemization. On Windows, `espeak-ng` must be discoverable on `PATH`.

**Bundling strategy:** Ship `espeak-ng.exe` + `espeak-ng-data/` under `libs/tts/espeak-ng/`. The TTS sidecar prepends this directory to `PATH` at startup so `kitten_tts_rs` finds it. The `ESPEAK_DATA_PATH` env var is set to point at the bundled `espeak-ng-data/` directory.

**espeak-ng version:** 1.52.0 (latest stable, December 2024).

**Required files:**
```
libs/tts/espeak-ng/
  espeak-ng.exe
  espeak-ng-data/
    phontab
    phonindex
    phondata
    intonations
    en_dict                  (English dictionary)
    voices/                  (voice definitions)
```

### KittenTTS Model Directory

Source: `KittenML/kitten-tts-nano-0.8-int8` from HuggingFace. Download separately — not bundled with the crate.

**Layout (exactly 3 files):**

```
models/kittentts-nano/
  config.json                      (688 B)
  kitten_tts_nano_v0_8.onnx       (24.4 MB)
  voices.npz                       (3.28 MB)
```

`config.json` references the other two files by name:
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
  "speed_priors": { ... }
}
```

**Artifact validation:** Check for `config.json` in the model directory. Parse it and verify `model_file` and `voices` files exist.

---

## TTS Sidecar Lifecycle (Owned by Engine Host)

The engine host manages the TTS sidecar process. This follows the same pattern as `spawn.rs` and `supervisor.rs` but is simpler — the TTS server has no backend selection, no model switching, and no restart policy beyond basic health recovery.

### Spawn

The engine host spawns the TTS sidecar during its own startup sequence, in `startup.rs`, after the engine reaches `ReadinessState::Ready`. The spawn follows `spawn.rs` conventions:

```rust
// Pseudocode — follows spawn_engine() pattern from smolpc-engine-client/src/spawn.rs
let mut cmd = Command::new(&tts_binary_path);
cmd.arg("--port").arg("19433")
   .arg("--model-dir").arg(&tts_model_dir)
   .arg("--espeak-dir").arg(&espeak_dir)
   .env("SMOLPC_ENGINE_TOKEN", &token)
   .env("RUST_LOG", "info");

// Detached process on Windows (same flags as engine spawn)
cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
   .stdin(Stdio::null())
   .stdout(Stdio::null())
   .stderr(tts_spawn_log);  // → tts-spawn.log

let child = cmd.spawn()?;
write_pid_file("tts.pid", child.id());
```

### Health Check

- **No periodic health check timer.** The TTS sidecar is not critical infrastructure — a failed TTS server means voice output is unavailable, but chat still works.
- **Lazy health check:** Before proxying a TTS request, the engine host checks `GET /health` on the sidecar. If unhealthy, attempt one respawn. If respawn also fails, return `503` to the caller.
- **PID check:** On engine shutdown, verify `tts.pid` identity before killing (same stale-PID protection as `engine.pid`).

### Shutdown

On engine host shutdown (graceful or crash):
1. Send `SIGTERM` / `TerminateProcess` to the TTS sidecar PID.
2. Clean up `tts.pid`.

The engine host's existing shutdown path (triggered by `POST /engine/shutdown` or supervisor `do_shutdown()`) is extended to also terminate the TTS sidecar.

### Graceful Degradation

If the TTS binary is missing from the bundle (e.g., minimal install without voice features):
- The engine host logs a warning and skips TTS sidecar spawn.
- `POST /v1/audio/speech` returns `503` with `{ "error": "TTS service unavailable" }`.
- Frontend hides the play buttons (same behavior as when TTS model is missing).
- No crash, no retry loop — voice output is simply unavailable.

---

## Engine Host: TTS Proxy Endpoint

The engine host exposes TTS to the Tauri app via a proxy endpoint. The Tauri app never communicates with the TTS sidecar directly — the engine host is the single API surface.

### Endpoint

`POST /v1/audio/speech`

- **Request:** `{ "text": "the text to speak", "voice": "bella" }`
- `voice` is optional, defaults to `"bella"`.
- **Response:** WAV bytes as `audio/wav` (proxied from TTS sidecar).
- Returns `503` if TTS sidecar is not healthy.
- Guarded by the `voice_semaphore` (capacity 1), not the `generation_semaphore`.

### Proxy Implementation

```rust
// Pseudocode
async fn v1_audio_speech(state, req) -> Response {
    let _permit = state.voice_semaphore.acquire().await;

    // Lazy health check
    if !tts_sidecar_healthy(&state.tts_client).await {
        attempt_tts_respawn(&state).await;
        if !tts_sidecar_healthy(&state.tts_client).await {
            return StatusCode::SERVICE_UNAVAILABLE;
        }
    }

    // Forward to sidecar
    let response = state.tts_client
        .post("http://127.0.0.1:19433/synthesize")
        .bearer_auth(&state.token)
        .json(&req)
        .send().await?;

    // Proxy WAV bytes back
    (StatusCode::OK, [("content-type", "audio/wav")], response.bytes().await?)
}
```

---

## Tauri App: Audio Capture

### Dependencies (added to `apps/codehelper/src-tauri/Cargo.toml`)

```toml
cpal = "0.17"       # Audio capture
rodio = "0.19"      # Audio playback
rubato = "1.0"      # Sample rate conversion
```

### Recording (STT)

Two Tauri commands:

**`start_recording`**
1. Query default input device via `cpal::default_host().default_input_device()`.
2. Query native format via `device.default_input_config()` — get sample rate and channel count.
3. Open input stream at the device's native format (f32 samples).
4. Accumulate samples into `Arc<Mutex<Vec<f32>>>`.
5. Store the native sample rate and channel count for use during stop.
6. Return immediately — recording runs on a background thread managed by cpal.

**`stop_recording`**
1. Stop the cpal stream.
2. Take the accumulated samples from the shared buffer.
3. **Mono mix** (if stereo): Average left and right channels — `mono[i] = (samples[2*i] + samples[2*i+1]) / 2.0`.
4. **Resample to 16kHz** (if native rate != 16000):
   - Create a `rubato` resampler with source rate = native, target rate = 16000.
   - For integer ratios (e.g., 48000/16000 = 3), `FftFixedIn` is optimal.
   - For non-integer ratios (e.g., 44100/16000 = 2.75625), `SincFixedIn` handles it.
   - Process all samples through the resampler.
5. **Send to engine:** POST the 16kHz mono f32 bytes to `POST /v1/audio/transcriptions`.
6. **Return** the transcribed text string to the frontend.
7. Frontend inserts text at cursor position in chat input.

**Resampling detail:** `rubato`'s API takes source and target rates as constructor params and handles ratio computation internally. The resampling runs synchronously in the `stop_recording` command — for 30 seconds of 48kHz audio, resampling takes <50ms on CPU. The student will not notice it.

```rust
// Pseudocode for the resampling step
let native_rate = captured_config.sample_rate;
let samples_16k = if native_rate == 16000 {
    mono_samples // no resampling needed
} else {
    let mut resampler = FftFixedIn::<f32>::new(
        native_rate as usize,
        16000,
        mono_samples.len(),
        1, // mono
    )?;
    let output = resampler.process(&[&mono_samples], None)?;
    output.into_iter().next().unwrap()
};
```

### Playback (TTS)

Two Tauri commands using `rodio`:

**`speak_text(text: String, voice: Option<String>)`**
1. Send text to engine `POST /v1/audio/speech` with optional voice parameter.
2. Receive WAV bytes.
3. Decode WAV and play via `rodio::OutputStream` + `rodio::Sink`.
4. Return when playback starts (not when it finishes — playback is async).

**`stop_playback`**
- Stop current audio playback immediately via `Sink::stop()`.

### Microphone Permissions

Microphone access is requested via the OS on first use. cpal's `default_input_device()` returns `None` if no mic is available or permission is denied.

If `default_input_device()` returns `None`:
- The mic button is disabled.
- Tooltip: "No microphone detected" or "Microphone permission denied".
- No error dialog — just a disabled button.

---

## Engine Host: Model Registry Changes

### ModelType Enum

```rust
pub enum ModelType {
    TextGeneration,
    SpeechToText,
    TextToSpeech,
}
```

Added to `ModelDefinition` in the registry. Existing Qwen models are `TextGeneration`. Whisper is `SpeechToText`. KittenTTS is `TextToSpeech`.

### Validation per Model Type

| Type | Validation |
|------|-----------|
| `TextGeneration` | Requires `genai_config.json`, tokenizer, chat template |
| `SpeechToText` | Requires `openvino_encoder_model.xml` + `openvino_decoder_model.xml` in model directory |
| `TextToSpeech` | Requires `config.json` in model directory (parsed to verify `model_file` and `voices` fields reference existing files). Validation runs in the TTS sidecar, not the engine host. |

### No Backend Selection for Voice Models

Voice models skip the backend selection system entirely:
- **Whisper:** Always OpenVINO CPU. No preflight probes, no backend ranking.
- **KittenTTS:** Always ONNX CPU via `ort` in the sidecar. No preflight probes.

---

## Frontend UI

### Mic Button (STT)

- Located next to the send button in the chat input area.
- **Idle state:** Mic icon. Clickable.
- **Recording state:** Red pulsing indicator. Click again to stop.
- **Processing state:** Loading spinner while Whisper transcribes. Not clickable.
- **Disabled state:** Greyed out mic with tooltip "No microphone detected" if device unavailable, or "Voice input unavailable" if Whisper model failed to load.

On successful transcription: text inserts at current cursor position in the chat input. Student reviews and sends as normal.

On empty transcription (silence/noise): loading spinner disappears, no text inserted, no error shown.

### Play Button (TTS)

- Located on each AI response message bubble.
- **Idle state:** Small speaker icon.
- **Loading state:** Loading spinner while TTS synthesizes.
- **Playing state:** Animated speaker icon. Click to stop.
- **Disabled state:** Hidden if TTS sidecar is unavailable (engine returns 503).

---

## Concurrency Model

Voice requests use a **separate semaphore** from LLM generation. STT and TTS are fast (<2s) and must not queue behind a long LLM generation, nor be subject to the LLM queue's 60-second timeout.

**Semaphores in `AppState`:**
- `generation_semaphore` (existing, capacity 1) — governs `POST /v1/chat/completions` only
- `queue_semaphore` (existing) — governs chat queue depth
- `voice_semaphore` (new, capacity 1) — governs `POST /v1/audio/transcriptions` and `POST /v1/audio/speech`

**Typical flow:**
1. Student clicks record → mic captures audio (no engine involvement, no semaphore)
2. Student clicks stop → Tauri resamples audio, sends `POST /v1/audio/transcriptions` → acquires voice semaphore → Whisper transcribes (~1–2s) → releases
3. Student reviews text, hits send → `POST /v1/chat/completions` → acquires generation semaphore → LLM generates → releases
4. Student clicks play on response → `POST /v1/audio/speech` → acquires voice semaphore → engine proxies to TTS sidecar → KittenTTS synthesizes (<1s) → releases

**Key behaviors:**
- A student can click Play on a previous response **while the LLM is generating a new one** — both run concurrently. Whisper/KittenTTS run on CPU and are brief; the LLM may be on DirectML/NPU.
- STT and TTS **cannot** run simultaneously (they share the voice semaphore). This is fine — a student will not dictate and listen at the same time.
- Even when LLM is also on CPU, the voice burst is short enough (<2s) that contention is negligible.

---

## Bundling

### Directory Layout

```
libs/
  onnxruntime.dll                         ← existing (engine DirectML)
  onnxruntime_providers_shared.dll        ← existing
  onnxruntime-genai.dll                   ← existing
  directml.dll                            ← existing
  openvino/                               ← existing (engine OpenVINO)
    openvino.dll
    openvino_c.dll
    openvino_genai_c.dll
    ... (full OpenVINO chain)
  tts/                                    ← NEW
    onnxruntime.dll                       ← TTS sidecar's own ORT copy
    espeak-ng/
      espeak-ng.exe
      espeak-ng-data/
        phontab, phonindex, phondata
        intonations
        en_dict
        voices/

binaries/
  smolpc-engine-host.exe                  ← existing
  smolpc-tts-server.exe                   ← NEW

models/
  qwen2.5-1.5b-instruct/                 ← existing
  qwen3-4b/                              ← existing
  whisper-base.en/                        ← NEW
    openvino/
      openvino_encoder_model.xml + .bin
      openvino_decoder_model.xml + .bin
      openvino_tokenizer.xml + .bin
      openvino_detokenizer.xml + .bin
      config.json, generation_config.json, ...
  kittentts-nano/                         ← NEW
    config.json
    kitten_tts_nano_v0_8.onnx
    voices.npz
```

### Tauri Resource Map Update

Add to `apps/codehelper/src-tauri/tauri.conf.json`:

```json
"resources": {
  "libs/": "libs/",
  "binaries/": "binaries/",
  "resources/models/": "models/",
  ...existing entries...
}
```

The existing `"libs/": "libs/"` directory mapping recursively copies all subdirectories including the new `libs/tts/` tree. The existing `"binaries/": "binaries/"` mapping picks up `smolpc-tts-server.exe`. No new resource map entries are needed if the TTS files follow this structure.

### Installer Impact

| Component | Disk | Notes |
|-----------|------|-------|
| whisper-base.en INT8 OV | ~85 MB | Pre-exported from HuggingFace |
| KittenTTS nano INT8 | ~28 MB | 3 files from HuggingFace |
| ORT DLL (TTS copy) | ~15–20 MB | Separate from engine's ORT |
| espeak-ng | ~10–15 MB | Executable + English data |
| TTS server binary | ~5 MB | Rust binary |
| **Total addition** | **~143–148 MB** | vs Qwen2.5's 900 MB — modest |

Both voice models and the TTS runtime ship bundled in the offline installer. No optional download UI needed — they are small enough to always include.

---

## Error Handling

| Scenario | Behavior |
|----------|----------|
| No microphone detected | Mic button disabled, tooltip explains |
| Whisper model missing/corrupt | Engine returns 503, mic button disabled, toast "Voice input unavailable" |
| TTS sidecar binary missing | Engine skips TTS spawn, logs warning. Play buttons hidden. |
| TTS sidecar fails to start | Engine logs error. `POST /v1/audio/speech` returns 503. Play buttons hidden. |
| TTS sidecar crashes mid-session | Next TTS request detects unhealthy sidecar, attempts one respawn. If respawn fails, returns 503. |
| espeak-ng not found by sidecar | TTS sidecar fails to synthesize, returns 500. Engine proxies error. Play buttons disabled. |
| Empty transcription (silence) | Loading spinner disappears, no text inserted, no error shown |
| Long recording (>30s) | Works fine — Whisper processes in 30s chunks internally. Loading spinner stays visible. |
| Memory pressure | Engine drops Whisper, reloads on next request (~1s). TTS sidecar unaffected (separate process). |
| LLM generating while student records | Recording works (just mic capture). Transcription uses voice semaphore, independent of LLM. |
| TTS while LLM generating | TTS uses voice semaphore, proxied to sidecar. Runs concurrently with LLM. |

---

## Testing Strategy

### Unit Tests

- **Whisper FFI symbol loading:** Test that `whisper_ffi.rs` correctly loads symbols from `openvino_genai_c.dll`. Skip on CI without OpenVINO runtime.
- **Text preprocessing:** Test markdown stripping (code fences, backticks, headers, bold, lists).
- **Model registry:** Test `ModelType` variants and per-type validation logic.
- **Audio format validation:** Test sample rate detection and resampling path selection.
- **TTS proxy:** Test request forwarding and error propagation (mock sidecar).

### Integration Tests

- **STT endpoint:** POST known audio bytes → verify transcription text.
- **TTS endpoint:** POST text → verify WAV bytes returned with correct headers.
- **TTS sidecar lifecycle:** Spawn, health check, shutdown, respawn-on-crash.
- **Round-trip:** Record audio → transcribe → send to LLM → synthesize response → verify WAV.

### Manual Testing

- Record real speech on target hardware (Core Ultra laptop, budget 8 GB machine).
- Verify transcription accuracy for coding-related vocabulary ("function", "variable", "for loop", "if statement").
- Verify TTS quality and latency for typical AI responses.
- Test on 8 GB RAM machine — confirm no memory pressure with all models loaded.
- Test mic permissions flow on fresh Windows install.
- Test with various audio hardware: built-in mic (48kHz), USB headset (44.1kHz/48kHz), no mic.
- Test TTS sidecar crash recovery — kill the TTS process mid-session, verify it respawns.

---

## Implementation Reference

### Key Files to Read Before Implementing

| File | Why |
|------|-----|
| `engine/crates/smolpc-engine-core/src/inference/genai/openvino_ffi.rs` | Pattern for FFI symbol loading from `openvino_genai_c.dll` — Whisper FFI follows the same structure |
| `engine/crates/smolpc-engine-core/src/inference/runtime_loading.rs` | Centralized DLL loading — all `Library::new()` calls must go through `load_runtime_library()` (source-invariant test at line 905) |
| `engine/crates/smolpc-engine-client/src/spawn.rs` | Detached process spawn pattern — TTS sidecar spawn follows this exactly (PID file, stderr redirect, spawn lock) |
| `apps/codehelper/src-tauri/src/engine/supervisor.rs` | Engine lifecycle management — reference for health check patterns and shutdown coordination |
| `engine/crates/smolpc-engine-host/src/routes.rs` | Existing endpoint structure and semaphore usage (line 232+) — voice endpoints follow the same auth + semaphore pattern |
| `engine/crates/smolpc-engine-host/src/state.rs` | `AppState` struct (line 364) — add `voice_semaphore` and TTS client here |
| `engine/crates/smolpc-engine-host/src/main.rs` | Semaphore creation (line 40) — add `voice_semaphore` initialization here |
| `apps/codehelper/src-tauri/tauri.conf.json` | Resource map — verify `libs/` and `binaries/` directory mappings cover new TTS files |

### Key Conventions

- **DLL loading centralization:** The source-invariant test scans all `.rs` files for `Library::new(` and `load_with_flags(`. The TTS sidecar lives in a separate crate that is NOT part of the engine workspace scan scope — it can use `ort` freely.
- **Auth:** All endpoints use `auth(&headers, &state.token)`. The TTS sidecar and engine host share the same token via `SMOLPC_ENGINE_TOKEN`.
- **No `tokio::spawn` in Tauri setup:** Use `tauri::async_runtime::spawn` (CLAUDE.md convention).
- **Conventional commits:** `feat(voice):`, `feat(tts):`, `feat(stt):`.

---

## Future Expansion (Not in v1)

- **Multilingual:** Swap `whisper-base.en` for `whisper-base` (99 languages). Add language-specific Piper/KittenTTS voices. Same architecture.
- **Streaming TTS:** Chunk text and synthesize progressively for long responses. Requires TTS sidecar to support chunked responses.
- **Voice selection UI:** Settings page to choose from the 8 available KittenTTS voices.
- **Piper TTS swap:** If KittenTTS quality is insufficient, replace `kitten_tts_rs` with `piper-rs` in the sidecar. Same HTTP contract — engine host proxy and frontend are unchanged.
- **Kokoro TTS upgrade:** When hardware improves or Kokoro CPU inference is optimized, swap in for higher quality. Same sidecar pattern.
- **Wake word / voice commands:** Would need VAD, wake word detection, intent parsing. Separate feature entirely.
- **OpenVINO TTS:** Convert KittenTTS ONNX model to OpenVINO IR format and run in-process. Eliminates the sidecar entirely but requires custom inference code. Evaluate if sidecar complexity becomes a maintenance burden.
