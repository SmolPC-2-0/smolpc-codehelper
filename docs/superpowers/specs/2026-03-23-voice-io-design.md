# Voice I/O: Whisper STT + KittenTTS

**Date:** 2026-03-23
**Status:** Draft
**Scope:** Speech-to-text input and text-to-speech output for SmolPC Code Helper

---

## Overview

Add voice input (speech-to-text) and voice output (text-to-speech) to SmolPC Code Helper. Students can dictate into the chat input and hear AI responses read aloud. Both capabilities run fully offline on 8GB RAM Intel laptops.

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

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| STT model | `whisper-base.en` INT8 OpenVINO IR | Best accuracy/size tradeoff. 85 MB disk, ~400-500 MB RAM. Pre-exported on HuggingFace. |
| STT runtime | OpenVINO GenAI WhisperPipeline (C API) | Same DLL already loaded for LLM inference. Zero new dependencies. |
| STT device | CPU always | Whisper bursts are <2s. Not worth NPU contention with LLM. |
| TTS engine | KittenTTS nano INT8 | Pure Rust (`kitten_tts_rs` crate). 25 MB model. RTF 0.6-0.73 on laptop CPU. No C++/Python deps. |
| TTS fallback | Piper TTS (drop-in behind same endpoint) | If KittenTTS proves unstable, Piper is battle-tested with identical API contract. |
| Audio capture | `cpal` crate in Tauri app (Rust side) | Direct 16kHz mono f32 capture. Raw samples feed straight to WhisperPipeline. |
| Audio playback | `rodio` crate in Tauri app (Rust side) | Lightweight, pairs with `cpal`. Plays WAV bytes from TTS endpoint. |
| Architecture | Engine-side — both models live in the engine host | Follows "engine owns all inference" convention. Other apps get voice for free. |
| Voice input UX | Click-to-start, click-to-stop. Transcription appends at cursor. | Student controls recording explicitly. Text appears in input box for review before send. |
| Voice output UX | On-demand play button per AI response. | Non-intrusive. Students click to hear, click again to stop. |
| Language | English-only | Target audience. Multilingual is same architecture, different model files — future expansion. |

---

## Architecture

### System Flow

```
┌─────────────────────────────────────────────────────┐
│  Tauri App                                          │
│                                                     │
│  ┌──────────┐  cpal   ┌──────────────┐             │
│  │ Mic btn  │────────>│ Audio capture │             │
│  │ (start/  │         │ 16kHz mono   │             │
│  │  stop)   │         │ f32 samples  │             │
│  └──────────┘         └──────┬───────┘             │
│                              │ POST /v1/audio/      │
│                              │   transcriptions     │
│  ┌──────────┐         ┌──────▼───────┐             │
│  │ Chat     │<────────│ Transcribed  │             │
│  │ input    │  insert │ text         │             │
│  │ (cursor) │  at pos └──────────────┘             │
│  └──────────┘                                      │
│                                                     │
│  ┌──────────┐  rodio  ┌──────────────┐             │
│  │ Play btn │<────────│ WAV playback │             │
│  │ (per     │         │              │             │
│  │  message)│         └──────┬───────┘             │
│  └──────────┘                │ POST /v1/audio/      │
│                              │   speech             │
└──────────────────────────────┼──────────────────────┘
                               │ HTTP (localhost)
┌──────────────────────────────▼──────────────────────┐
│  Engine Host                                        │
│                                                     │
│  ┌─────────────────┐  ┌──────────────────┐         │
│  │ WhisperPipeline │  │ KittenTTS        │         │
│  │ (OpenVINO GenAI │  │ (kitten_tts_rs   │         │
│  │  C API, CPU)    │  │  + ort, CPU)     │         │
│  │                 │  │                  │         │
│  │ whisper-base.en │  │ nano INT8        │         │
│  │ INT8, 85 MB     │  │ 25 MB            │         │
│  │ ~400-500 MB RAM │  │ minimal RAM      │         │
│  └─────────────────┘  └──────────────────┘         │
│                                                     │
│  ┌─────────────────────────────────────────┐       │
│  │ Existing: LLMPipeline (Qwen2.5 / Qwen3)│       │
│  │ DirectML / OpenVINO NPU / CPU           │       │
│  └─────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────┘
```

### Resource Budget (8GB machine)

| Component | RAM | Disk |
|-----------|-----|------|
| OS + Tauri app | ~2.5 GB | — |
| Qwen2.5-1.5B INT4 (loaded) | ~1.5-2.0 GB | 900 MB |
| whisper-base.en INT8 | ~400-500 MB | 85 MB |
| KittenTTS nano INT8 | minimal | 25 MB |
| **Total** | **~4.5-5.0 GB** | **+110 MB** |
| **Headroom** | **~3.0-3.5 GB** | |

All three models can stay loaded simultaneously on 8GB machines.

---

## Engine: Whisper STT

### FFI Layer

New module `whisper_ffi.rs` in `smolpc-engine-core`, following the same pattern as `openvino_ffi.rs`. Defines Whisper-specific FFI types and function signatures. **Symbol resolution goes through `runtime_loading.rs`** (the centralized DLL loader) — `whisper_ffi.rs` does not call `Library::new()` directly, per the DLL loading convention.

Symbols loaded from the already-loaded `openvino_genai_c.dll`:

- `ov_genai_whisper_pipeline_create(models_path, device, ...)` — create pipeline pointing at model directory, device = `"CPU"`
- `ov_genai_whisper_pipeline_generate(pipeline, raw_speech, raw_speech_size, config, &results)` — transcribe audio samples
- `ov_genai_whisper_pipeline_free(pipeline)` — cleanup
- `ov_genai_whisper_generation_config_*` — configure language, task (transcribe)

Input: `*const f32` audio samples (16kHz mono, normalized to [-1.0, 1.0])
Output: `WhisperDecodedResults` containing transcribed text

### Endpoint

`POST /v1/audio/transcriptions`

- Request: raw audio bytes as `application/octet-stream` (f32 PCM, 16kHz mono)
- Response: `{ "text": "the transcribed text" }`
- Synchronous — no streaming. Typical latency: 0.5-2 seconds for 30 seconds of audio.
- Returns 503 if model fails to load.

### Lifecycle

- WhisperPipeline loads on first transcription request (~1s cold start)
- Stays resident at ~400-500 MB
- If engine memory pressure hits warning threshold, can be dropped and reloaded on next request
- No backend selection needed — always OpenVINO CPU

### Model Directory

```
models/
  whisper-base.en/
    openvino/
      manifest.json
      encoder/
        openvino_encoder_model.xml
        openvino_encoder_model.bin
      decoder/
        openvino_decoder_model.xml
        openvino_decoder_model.bin
```

Pre-exported model: `OpenVINO/whisper-base.en-int8-ov` from HuggingFace. No export pipeline needed.

---

## Engine: KittenTTS

### Integration

Add `kitten_tts_rs` as a Rust dependency in `smolpc-engine-host`. This is a pure Rust crate that uses `ort` for ONNX inference internally. No DLL loading, no FFI, no C++ — the simplest integration in the project.

### Endpoint

`POST /v1/audio/speech`

- Request: `{ "text": "the text to speak", "voice": "bella" }`
- `voice` is optional, defaults to a chosen default voice (e.g., `bella`)
- Response: raw WAV audio bytes as `application/octet-stream`
- Synchronous. Typical latency: <1 second for a paragraph.
- Returns 503 if model fails to load.

### Text Preprocessing

Before passing text to KittenTTS, strip content that shouldn't be spoken:

1. Remove code fences and their contents (` ```...``` `)
2. Remove inline code backticks
3. Strip markdown symbols (`**`, `##`, `- `, `>`, etc.)
4. Collapse whitespace
5. Pass only natural language prose

### Lifecycle

- KittenTTS model loads on first speech request
- Stays resident (25 MB model — negligible memory footprint)
- No backend selection — always ONNX on CPU via `ort`

### Model Directory

```
models/
  kittentts-nano/
    model.onnx
    (voice config files)
```

### Fallback: Piper TTS

If KittenTTS proves unstable during testing, swap to Piper behind the same endpoint:

- Replace `kitten_tts_rs` dependency with `piper-rs` (or Piper C++ shared library)
- Same `POST /v1/audio/speech` contract — frontend doesn't change
- Piper model: `en_US-amy-medium.onnx` (~63 MB) replaces KittenTTS nano (~25 MB)
- RTF improves (0.19-0.28 vs 0.6-0.73) at the cost of slightly lower voice quality

---

## Engine: Model Registry Changes

### ModelType Enum

```rust
pub enum ModelType {
    TextGeneration,
    SpeechToText,
    TextToSpeech,
}
```

Added to `ModelDefinition` in the registry. Existing Qwen models are `TextGeneration`. Whisper is `SpeechToText`. KittenTTS is `TextToSpeech`.

### Validation

Each model type has different validation:
- `TextGeneration`: requires `genai_config.json`, tokenizer, chat template
- `SpeechToText`: requires OpenVINO IR artifacts (`.xml` + `.bin`) + `manifest.json`
- `TextToSpeech`: requires `model.onnx` (file presence check, no manifest)

### No Backend Selection for Voice Models

Voice models skip the backend selection system entirely:
- Whisper: always OpenVINO CPU. No preflight probes, no backend ranking.
- KittenTTS: always ONNX CPU via `ort`. No preflight probes.

---

## Tauri App: Audio Capture

### Recording (STT)

Two Tauri commands using `cpal`:

**`start_recording`**
- Opens default audio input device via `cpal`
- Configures for 16kHz mono f32
- Accumulates samples into `Arc<Mutex<Vec<f32>>>`
- Returns immediately (recording runs on a background thread)

**`stop_recording`**
- Stops the `cpal` stream
- Sends accumulated samples to engine `POST /v1/audio/transcriptions`
- Returns the transcribed text string to the frontend
- Frontend inserts text at cursor position in chat input

### Playback (TTS)

Two Tauri commands using `rodio`:

**`speak_text(text: String)`**
- Sends text to engine `POST /v1/audio/speech`
- Receives WAV bytes
- Plays via `rodio` audio output

**`stop_playback`**
- Stops current audio playback immediately

### Microphone Permissions

Microphone access requested via Tauri's capability system on first use. If denied or unavailable, the mic button is disabled with a tooltip.

---

## Frontend UI

### Mic Button (STT)

- Located next to the send button in the chat input area
- **Idle state:** Mic icon. Clickable.
- **Recording state:** Red pulsing indicator. Click again to stop.
- **Processing state:** Loading spinner while Whisper transcribes. Not clickable.
- **Disabled state:** Greyed out mic with tooltip "No microphone detected" if device unavailable, or "Voice input unavailable" if Whisper model failed to load.

On successful transcription: text inserts at current cursor position in the chat input. Student reviews and sends as normal.

On empty transcription (silence/noise): loading spinner disappears, no text inserted, no error shown.

### Play Button (TTS)

- Located on each AI response message bubble
- **Idle state:** Small speaker icon
- **Loading state:** Loading spinner while TTS synthesizes
- **Playing state:** Animated speaker icon. Click to stop.
- **Disabled state:** Hidden or greyed out if TTS model unavailable

---

## Model Bundling

### Installer Impact

| Model | Disk | Notes |
|-------|------|-------|
| whisper-base.en INT8 OV | 85 MB | Pre-exported from HuggingFace |
| KittenTTS nano INT8 | 25 MB | Ships with `kitten_tts_rs` or bundled separately |
| **Total addition** | **~110 MB** | vs Qwen2.5's 900 MB — trivial |

Both voice models ship bundled in the offline installer alongside Qwen. No optional download UI needed — they're small enough to always include.

### No Download UI

Voice models are not optional in v1. They ship with the app. If a student never uses voice features, 110 MB sits on disk. This is simpler than building a model download flow for such small files.

---

## Error Handling

| Scenario | Behavior |
|----------|----------|
| No microphone detected | Mic button disabled, tooltip explains |
| Whisper model missing/corrupt | Engine returns 503, mic button disabled, toast "Voice input unavailable" |
| KittenTTS model missing | Play buttons hidden, no error shown |
| Empty transcription (silence) | Loading spinner disappears, no text inserted, no error |
| Long recording (>30s) | Works fine — Whisper processes in 30s chunks internally. Loading spinner stays visible. |
| Memory pressure | Engine drops Whisper, reloads on next request (~1s). Student sees slightly longer spinner. |
| LLM generating while student records | Recording works (it's just mic capture). Transcription uses the voice semaphore, independent of LLM generation. No queuing. |
| TTS while LLM generating | TTS uses the voice semaphore, runs concurrently with LLM. No queuing — student hears playback immediately. |

---

## Concurrency Model

Voice requests use a **separate semaphore** from LLM generation. STT and TTS are fast (<2s) and should not queue behind a long LLM generation, nor should they be subject to the LLM queue's 60-second timeout.

**Two semaphores:**
- `generation_semaphore` (existing) — governs LLM chat completions only
- `voice_semaphore` (new, capacity = 1) — governs STT and TTS requests

This means:
1. Student clicks record → mic captures audio (no engine involvement)
2. Student clicks stop → `POST /v1/audio/transcriptions` acquires voice semaphore → Whisper transcribes (~1-2s) → releases
3. Student reviews text, hits send → `POST /v1/chat/completions` acquires generation semaphore → LLM generates → releases
4. Student clicks play on response → `POST /v1/audio/speech` acquires voice semaphore → KittenTTS synthesizes (<1s) → releases

**Key behavior:** A student can click Play on a previous response while the LLM is generating a new one — both run concurrently. This is safe because Whisper/KittenTTS run on CPU and are brief, while the LLM may be on a different device (DirectML/NPU). Even when both are on CPU, the voice burst is short enough that contention is negligible.

STT and TTS cannot run simultaneously (they share the voice semaphore), but this is fine — a student won't dictate and listen at the same time.

---

## Testing Strategy

### Unit Tests
- Whisper FFI symbol loading (mock or skip on CI without OpenVINO)
- Text preprocessing (markdown stripping)
- Model registry with new `ModelType` variants
- Audio sample format validation (sample rate, channels)

### Integration Tests
- Engine endpoint: POST audio bytes → get transcription text
- Engine endpoint: POST text → get WAV bytes
- Round-trip: record audio → transcribe → send to LLM → synthesize response

### Manual Testing
- Record real speech on target hardware (Core Ultra laptop)
- Verify transcription accuracy for coding-related vocabulary
- Verify TTS quality and latency for typical AI responses
- Test on 8GB RAM machine — confirm no memory pressure
- Test mic permissions flow on fresh install

---

## Future Expansion (Not in v1)

- **Multilingual:** Swap `whisper-base.en` for `whisper-base` (99 languages). Add language Piper/KittenTTS voices. Same architecture.
- **Streaming TTS:** Chunk text and synthesize progressively for long responses.
- **Voice selection UI:** Settings page to choose from available KittenTTS voices.
- **Kokoro TTS upgrade:** When hardware improves or Kokoro CPU inference is optimized, swap in for higher quality. Same endpoint contract.
- **Wake word / voice commands:** Would need VAD, wake word detection, intent parsing. Separate feature entirely.
