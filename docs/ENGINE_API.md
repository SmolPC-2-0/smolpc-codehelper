# SmolPC Engine HTTP API (v1)

All endpoints are localhost-only and require a bearer token:

`Authorization: Bearer <token>`

Default base URL: `http://127.0.0.1:19432`

## Engine Control

- `GET /engine/health`
  - Returns `{ "ok": true }`

- `GET /engine/meta`
  - Returns protocol and runtime metadata.

- `GET /engine/status`
  - Returns loaded model, generation activity, and backend status.
  - `backend_status.active_backend` and `backend_status.active_artifact_backend` are serialized as:
    - `cpu`
    - `directml`
  - Selector/runtime diagnostics:
    - `backend_status.available_backends`: detected candidates on this machine (`cpu`, optional `directml`)
    - `backend_status.selection_state`: `pending | ready | fallback | error`
    - `backend_status.selection_reason`: host-side reason code for latest backend decision
      - Examples: `default_directml_candidate`, `persisted_decision`, `forced_override`, `directml_initialization_failed`, `runtime_failure_fallback`
    - `backend_status.selected_device_id`: active or candidate DirectML device id
    - `backend_status.selected_device_name`: active or candidate DirectML device name

- `POST /engine/load`
  - Body: `{ "model_id": "qwen2.5-coder-1.5b" }`

- `POST /engine/unload`
  - Body: `{ "force": false }`

- `POST /engine/cancel`
  - Cancels the currently active generation.

- `POST /engine/check-model`
  - Body: `{ "model_id": "qwen2.5-coder-1.5b" }`
  - Returns `{ "exists": true | false }`

- `POST /engine/shutdown`
  - Graceful daemon shutdown.

## OpenAI-Compatible Surface

- `GET /v1/models`

- `POST /v1/chat/completions`
  - Supports `stream: true` (SSE) and `stream: false`.
  - Request fields supported:
    - `model`
    - `messages`
    - `stream`
    - `max_tokens`
    - `temperature`
    - `top_k`
    - `top_p`
    - `repetition_penalty`
    - `repetition_penalty_last_n`
  - Non-stream responses include `smolpc_metrics`:
    - `total_tokens`
    - `time_to_first_token_ms`
    - `tokens_per_second`
    - `total_time_ms`
  - Streaming SSE emits:
    - token chunks (`chat.completion.chunk`)
    - one metrics event (`chat.completion.metrics`) with `smolpc_metrics`
    - terminal `[DONE]`
  - Streaming errors are emitted as:
    - `{"error":{"message":"...","code":"...","type":"..."}}`
    - Codes include `INFERENCE_GENERATION_CANCELLED` and `ENGINE_STREAM_ERROR`

## Scheduling Defaults

- Single active generation globally.
- Queue capacity: 3.
- Queue timeout: 60 seconds.
- Queue full: HTTP 429.
- Queue timeout: HTTP 504.

## Backend Selection Policy (Windows)

- Host starts an async startup probe and ranks DirectML candidates (discrete-first, then higher VRAM).
- On model load, host waits up to ~1.5s for probe completion; if probe is still pending, load continues with safe defaults.
- Default auto policy is capability-first:
  - Prefer DirectML when available and model `dml/model.onnx` artifact exists.
  - Fallback to CPU when DirectML init/runtime fails.
- Forced overrides for diagnostics:
  - `SMOLPC_FORCE_EP=cpu|dml`
  - `SMOLPC_DML_DEVICE_ID=<non-negative int>`
- Runtime failure handling:
  - If DirectML fails during generation, host demotes to CPU for the current session/model and keeps serving requests without app restart.

## Protocol Compatibility

- Current protocol major: `1`.
- Client and host must match protocol major versions.
- On mismatch, clients return an explicit protocol mismatch error.
