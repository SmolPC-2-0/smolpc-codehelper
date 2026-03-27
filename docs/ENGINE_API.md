# Engine API Reference

The engine exposes an HTTP API on `localhost:19432`. All endpoints require Bearer token authentication.

## Authentication

Every request must include the auth header:

```
Authorization: Bearer <token>
```

The token is set via the `SMOLPC_ENGINE_TOKEN` environment variable. Comparison uses constant-time equality to prevent timing attacks.

**401 response on failure:**
```json
{"error": "Unauthorized"}
```

---

## Endpoints

### GET /engine/health

Readiness check. Returns whether the engine is healthy.

**Response (200):**
```json
{
  "ok": true,
  "state": "ready"
}
```

**Response (503) — engine failed:**
```json
{
  "ok": false,
  "state": "failed"
}
```

`state` is one of: `idle`, `starting`, `resolving_assets`, `probing`, `loading_model`, `ready`, `failed`.

**Example:**
```bash
curl -H "Authorization: Bearer $TOKEN" http://localhost:19432/engine/health
```

---

### GET /engine/meta

Engine metadata including version and process info.

**Response (200):**
```json
{
  "ok": true,
  "protocol_version": "1.0.0",
  "engine_api_version": "1.0.0",
  "engine_version": "2.2.0",
  "pid": 12345,
  "busy": false
}
```

`busy` is `true` when a generation is in progress.

---

### GET /engine/status

Full readiness payload with backend status, model info, and startup state.

**Response (200):**
```json
{
  "ok": true,
  "ready": true,
  "attempt_id": "1",
  "state": "ready",
  "startup_phase": "ready",
  "state_since": "2025-03-27T14:30:00Z",
  "active_backend": "openvino_npu",
  "active_model_id": "qwen2.5-1.5b-instruct",
  "error_code": null,
  "error_message": null,
  "retryable": null,
  "last_error": null,
  "engine_version": "2.2.0",
  "engine_api_version": "1.0.0",
  "effective_mode": "auto",
  "effective_startup_policy": {},
  "current_model": "qwen2.5-1.5b-instruct",
  "generating": false,
  "backend_status": { ... }
}
```

`active_backend` is one of: `"cpu"`, `"directml"`, `"openvino_npu"`, or `null` if not loaded.

---

### POST /engine/ensure-started

Start the engine with a specific backend policy. Triggers the background probe: hardware detection, backend selection, and model loading.

**Request:**
```json
{
  "mode": "auto",
  "startup_policy": {
    "default_model_id": "qwen2.5-1.5b-instruct"
  }
}
```

`mode` is one of:
- `"auto"` — detect best backend automatically (default)
- `"directml_required"` — require DirectML or fail

`startup_policy.default_model_id` is optional — if omitted, the engine selects a model based on available RAM.

**Response (200) — started successfully:**

Returns the full readiness payload (same shape as `/engine/status`).

**Response (409) — conflict with existing startup:**

Returns the readiness payload. The engine is already running with a different startup mode.

**Response (503) — startup failed:**

Returns the readiness payload with error details in `error_code`, `error_message`, and `retryable`.

**Example:**
```bash
curl -X POST http://localhost:19432/engine/ensure-started \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"mode": "auto", "startup_policy": {}}'
```

---

### POST /engine/load

Load a model by ID. Replaces the currently loaded model.

**Request:**
```json
{
  "model_id": "qwen3-4b"
}
```

**Response (200):**
```json
{"ok": true}
```

**Response (500) — load failed:**
```json
{"error": "Model directory not found for 'qwen3-4b'"}
```

Memory pressure errors include a hint: `"[Memory pressure detected. Try switching to 'qwen2.5-1.5b-instruct' or close other heavy apps and retry.]"`

**Timeout:** 600 seconds (model loading can involve NPU compilation on first run).

---

### POST /engine/unload

Unload the current model.

**Request:**
```json
{
  "force": false
}
```

`force` is optional (default `false`). When `true`, unloads even during active generation.

**Response (200):**
```json
{"ok": true}
```

---

### POST /engine/cancel

Cancel the active generation. The next token callback will detect the cancellation flag and stop.

**Response (200):**
```json
{"ok": true}
```

---

### POST /engine/shutdown

Gracefully shut down the engine process.

**Response (200):**
```json
{"ok": true}
```

The engine notifies the shutdown channel, which triggers the Axum server to stop accepting new connections and exit.

---

### POST /engine/check-model

Check whether a model's artifacts are available on disk, per backend lane.

**Request:**
```json
{
  "model_id": "qwen2.5-1.5b-instruct"
}
```

**Response (200):**

Returns a lane-based readiness report showing which backends have the model's artifacts available.

---

### GET /v1/models

List all registered models and their RAM requirements.

**Response (200):**
```json
{
  "object": "list",
  "data": [
    {
      "id": "qwen2.5-1.5b-instruct",
      "object": "model",
      "owned_by": "smolpc",
      "min_ram_gb": 3.0,
      "estimated_runtime_ram_gb": 4.5
    },
    {
      "id": "qwen3-4b",
      "object": "model",
      "owned_by": "smolpc",
      "min_ram_gb": 16.0,
      "estimated_runtime_ram_gb": 8.0
    }
  ]
}
```

---

### POST /v1/chat/completions

Generate a chat completion. **Streaming only** — `stream: true` is required.

**Request:**
```json
{
  "model": "smolpc-engine",
  "stream": true,
  "messages": [
    {"role": "system", "content": "You are a helpful coding assistant."},
    {"role": "user", "content": "Write a Python hello world"}
  ],
  "max_tokens": 2048,
  "temperature": 0.7,
  "top_p": 0.9,
  "top_k": 50,
  "repetition_penalty": 1.1,
  "repetition_penalty_last_n": 64
}
```

All fields except `messages` and `stream` are optional. `model` defaults to `"smolpc-engine"`.

**Response: Server-Sent Events (text/event-stream)**

Events are sent in this order:

**1. Initial chunk (role announcement):**
```
data: {"id":"chatcmpl-1711547400000","object":"chat.completion.chunk","created":1711547400,"model":"qwen2.5-1.5b-instruct","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}
```

**2. Token chunks (repeated for each token):**
```
data: {"id":"chatcmpl-1711547400000","object":"chat.completion.chunk","created":1711547400,"model":"qwen2.5-1.5b-instruct","choices":[{"index":0,"delta":{"content":"print"},"finish_reason":null}]}
```

**3. Metrics event (after generation completes):**
```
data: {"id":"chatcmpl-1711547400000","object":"chat.completion.metrics","created":1711547400,"model":"qwen2.5-1.5b-instruct","smolpc_metrics":{"total_tokens":42,"time_to_first_token_ms":150,"tokens_per_second":12.5,"total_time_ms":3200,"truncated":false}}
```

**4. Final chunk (finish reason):**
```
data: {"id":"chatcmpl-1711547400000","object":"chat.completion.chunk","created":1711547400,"model":"qwen2.5-1.5b-instruct","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}
```

**5. Done signal:**
```
data: [DONE]
```

**Error event (replaces metrics if generation fails):**
```
data: {"error":{"message":"Generation cancelled by user","code":"INFERENCE_GENERATION_CANCELLED","type":"cancelled"}}
```

Error codes:
- `INFERENCE_GENERATION_CANCELLED` — user cancelled (type: `cancelled`)
- `ENGINE_STREAM_ERROR` — runtime error (type: `runtime_error`)

**Qwen3 thinking mode:** For Qwen3 models, the engine automatically enables thinking mode suppression — `<think>...</think>` blocks are stripped from the token stream before sending to the client.

**Queue behavior:**
- Requests wait in the queue semaphore (default capacity 3)
- **429** if the queue is full: `{"error": "Engine queue is full"}`
- **504** if the queue wait times out: `{"error": "Queued request timed out"}`

**Non-streaming requests return 400:**
```json
{"error": "Non-streaming completions are not supported. Set \"stream\": true."}
```

**Example:**
```bash
curl -N -X POST http://localhost:19432/v1/chat/completions \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "smolpc-engine",
    "stream": true,
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

---

### POST /v1/audio/transcriptions

Transcribe audio using Whisper STT. **Windows only.**

**Request:**

Raw binary body — f32 PCM samples in little-endian byte order, 16 kHz mono.

The body length must be a multiple of 4 bytes (one f32 per sample).

**Response (200):**
```json
{
  "text": "Hello world"
}
```

**Errors:**
- **400** — empty body or body length not multiple of 4
- **500** — Whisper model not found or transcription failed
- **504** — voice semaphore timeout (30s)

The Whisper model is lazy-loaded on first transcription request from `models/whisper-base.en/openvino/`.

---

### POST /v1/audio/speech

Text-to-speech synthesis. **Windows only.** Proxied to the TTS sidecar on port 19433.

**Request:**
```json
{
  "text": "Hello world",
  "voice": "Bella",
  "speed": 1.5
}
```

`voice` defaults to `"Bella"`. `speed` defaults to `1.0`.

**Response (200):**

Binary WAV audio with `Content-Type: audio/wav`.

**Errors:**
- **400** — empty text
- **502** — TTS sidecar returned an error or is unreachable
- **503** — TTS sidecar not available (even after respawn attempt)
- **504** — voice semaphore timeout (30s)

If the sidecar health check fails, the engine attempts one respawn before returning 503.

---

## Error Response Format

All errors follow this shape:

```json
{
  "error": "Error message here"
}
```

**Status codes used:**

| Code | Meaning |
|------|---------|
| 400 | Bad request (invalid input, missing fields, non-streaming request) |
| 401 | Unauthorized (missing or invalid token) |
| 409 | Conflict (startup policy mismatch) |
| 429 | Too many requests (queue full) |
| 500 | Internal server error (model load failure, worker error) |
| 502 | Bad gateway (TTS sidecar failure) |
| 503 | Service unavailable (engine not ready, TTS not available) |
| 504 | Gateway timeout (queue timeout, voice processing timeout) |

---

## Timeouts

| Operation | Timeout |
|-----------|---------|
| Non-streaming requests | 30 seconds |
| Model load/unload | 600 seconds |
| Queue wait | 60 seconds (configurable) |
| Voice operations (STT/TTS) | 30 seconds (semaphore) + 60 seconds (HTTP) |
| TTS sidecar health check | 3 seconds per check, 15 seconds total |
| Health polling (client-side) | 60 seconds default, 250ms interval |
