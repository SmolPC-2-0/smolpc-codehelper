# SmolPC Engine HTTP API (v1)

> Status note (2026-03-12): this document describes the current implemented API on this branch. The engine status and model-readiness surface is lane-based, and native `openvino_npu` activation is now implemented behind the existing startup-probe and preflight gates.

All endpoints are localhost-only and require a bearer token:

`Authorization: Bearer <token>`

Default base URL: `http://127.0.0.1:19432`

## Engine Control

- `GET /engine/health`
  - Returns `{ "ok": true }`

- `GET /engine/meta`
  - Returns protocol/runtime metadata, including:
    - `protocol_version`
    - `engine_api_version`
    - `engine_version`
    - `pid`
    - `busy`

- `GET /engine/status`
  - Returns canonical readiness fields plus compatibility aliases/diagnostics.
  - Canonical readiness fields:
    - `attempt_id`
    - `state`
    - `state_since`
    - `active_backend`
    - `active_model_id`
    - `error_code`
    - `error_message`
    - `retryable`
  - Compatibility aliases:
    - `ready`
    - `startup_phase`
    - `last_error`
  - Metadata:
    - `engine_api_version`
    - `engine_version`
  - Legacy compatibility fields are still included:
    - `current_model`
    - `generating`
    - `backend_status`
  - The embedded `backend_status` payload returns loaded model, generation activity, and lane-based backend status.
  - `backend_status.active_backend` and `backend_status.active_artifact_backend` serialize as:
    - `cpu`
    - `directml`
    - `openvino_npu`
    - `null`
  - Top-level backend fields:
    - `backend_status.runtime_engine`
      - Current implemented values: `ort_cpu`, `genai_dml`, `ov_genai_npu`, `null`
    - `backend_status.selection_state`: `pending | ready | fallback | error`
    - `backend_status.selection_reason`: host-side reason code for the latest backend decision
    - `backend_status.decision_persistence_state`: `none | persisted | temporary_fallback`
    - `backend_status.available_backends`: currently detected lanes on this machine
    - `backend_status.selected_device`
      - Shape: `{ "backend": "directml", "device_id": 0, "device_name": "..." }`
      - Present for the active or candidate DirectML device when known
    - `backend_status.selection_fingerprint`
      - Opaque full selection fingerprint for the current model load
    - `backend_status.decision_key`
      - Current fields:
        - `model_id`
        - `model_artifact_fingerprint`
        - `app_version`
        - `selector_engine_id`
        - `ort_runtime_version`
        - `ort_bundle_fingerprint`
        - `openvino_runtime_version`
        - `openvino_genai_version`
        - `openvino_tokenizers_version`
        - `openvino_bundle_fingerprint`
        - `gpu_adapter_identity`
        - `gpu_driver_version`
        - `gpu_device_id`
        - `npu_adapter_identity`
        - `npu_driver_version`
        - `selection_profile`
    - `backend_status.last_decision`
    - `backend_status.failure_counters`
    - `backend_status.force_override`
    - `backend_status.store_path`
  - Runtime bundle validation is now grouped under `backend_status.runtime_bundles`:
    - `load_mode`: `production | development`
    - `ort`, `directml`, `openvino`
      - Each contains `root`, `fingerprint`, `validated`, `failure`
    - These fields still mean file inventory validation only. Runtime initialization remains lazy.
  - Lane readiness is now grouped under `backend_status.lanes.openvino_npu`, `backend_status.lanes.directml`, and `backend_status.lanes.cpu`
    - Each lane currently exposes:
      - `detected`
      - `bundle_ready`
      - `artifact_ready`
      - `startup_probe_state`: `not_started | ready | error`
      - `preflight_state`: `not_started | pending | ready | timeout | error`
      - `persisted_eligibility`
      - `last_failure_class`
      - `last_failure_message`
      - `driver_version`
      - `runtime_version`
      - `cache_state`: `unknown | cold | warm`
      - `device_id`
      - `device_name`
  - Current implementation notes:
    - `directml` lane startup detection is implemented.
    - `directml` lane preflight becomes `ready` only after a successful DirectML model load.
    - `openvino_npu` lane bundle/artifact readiness and startup-probe truth are reported now.
    - `openvino_npu` lane preflight now reflects real compile plus first-token smoke tests during `/engine/load`.
    - successful OpenVINO preflight can activate `runtime_engine=ov_genai_npu`.

- `POST /engine/ensure-started`
  - Body:
    - `{ "mode": "auto", "startup_policy": { "default_model_id": "..." } }`
  - `mode`:
    - `auto`
    - `directml_required`
  - Blocking single-flight startup handshake.
  - Returns:
    - `200` when ready
    - `503` when startup fails (structured readiness error fields populated)
    - `409` with `error_code=STARTUP_POLICY_CONFLICT` when already ready under a different effective mode/policy.
  - Effective default model precedence:
    - request `startup_policy.default_model_id` > env/config (`SMOLPC_ENGINE_DEFAULT_MODEL_ID` or `SMOLPC_DEFAULT_MODEL_ID`) > built-in model default.

- `POST /engine/load`
  - Body: `{ "model_id": "qwen2.5-1.5b-instruct" }`

- `POST /engine/unload`
  - Body: `{ "force": false }`

- `POST /engine/cancel`
  - Cancels the currently active generation.

- `POST /engine/check-model`
  - Body: `{ "model_id": "qwen2.5-1.5b-instruct" }`
  - Returns lane readiness, not a single boolean:
  - Primary programmatic readiness surfaces:
    - HTTP: `POST /engine/check-model`
    - Rust client: `EngineClient::check_model_readiness()`
    - Tauri: `check_model_readiness(model_id)`
  - Compatibility shims:
    - Rust client: `EngineClient::check_model_exists()`
    - Tauri: `check_model_exists(model_id)`
    - These return `true` only when at least one lane has `ready = true`
    - Artifact presence alone is not enough
    - New callers should prefer the readiness API above

```json
{
  "model_id": "qwen2.5-1.5b-instruct",
  "lanes": {
    "openvino_npu": {
      "artifact_ready": false,
      "bundle_ready": true,
      "ready": false,
      "reason": "artifact_missing"
    },
    "directml": {
      "artifact_ready": true,
      "bundle_ready": true,
      "ready": true,
      "reason": "ready"
    },
    "cpu": {
      "artifact_ready": true,
      "bundle_ready": true,
      "ready": true,
      "reason": "ready"
    }
  }
}
```

  - Current reason values include:
    - `ready`
    - `unknown_model`
    - `artifact_missing`
    - `artifact_invalid`
    - `artifact_incomplete`
    - `startup_probe_pending`
    - `startup_probe_failed`
    - `directml_candidate_missing`
    - bundle validation failure codes such as `missing_root`, `directml_missing`, `openvino_npu_plugin_missing`
    - blocking OpenVINO startup-probe failure classes such as `no_npu_hardware`, `openvino_npu_driver_missing`, and `openvino_npu_plugin_unavailable`
  - Current implemented reason precedence:
    - `cpu`
      - `ready` when CPU artifact and ORT bundle are both ready
      - `artifact_missing` when CPU artifact is incomplete
      - otherwise the current ORT bundle failure code
    - `directml`
      - `artifact_missing` when the DirectML artifact is incomplete
      - then the DirectML bundle failure code when the runtime bundle is not ready
      - then `startup_probe_pending` before the startup probe completes
      - then `directml_candidate_missing` when the startup probe completed without a DirectML-capable adapter
      - otherwise `ready`
    - `openvino_npu`
      - `artifact_missing` when `openvino_npu/manifest.json` is absent
      - `artifact_invalid` or `artifact_incomplete` when the manifest exists but is not usable
      - then the current OpenVINO bundle failure code when the bundle is incomplete
      - then `startup_probe_pending` while the async OpenVINO startup probe is still running
      - then a blocking OpenVINO startup-probe failure class when the startup probe completed but the lane is unusable
      - then `startup_probe_failed` if the startup probe completed without a usable result but without a classified blocking failure
      - otherwise `ready`
      - `openvino_npu.ready=true` means the lane is viable enough to attempt `/engine/load`; model load still performs the final native OpenVINO preflight

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
- Model idle unload defaults to 300 seconds; set `SMOLPC_ENGINE_MODEL_IDLE_UNLOAD_SECS=0` to disable it.
- Process idle exit is disabled by default; set `SMOLPC_ENGINE_PROCESS_IDLE_EXIT_SECS=<seconds>` to opt into host shutdown after inactivity.

## Backend Selection Policy (Windows)

- `engine-host` remains the sole selector, probe owner, fallback owner, and persistence owner.
- Current implemented automatic selection is `openvino_npu -> directml -> cpu` when the OpenVINO lane passes preflight.
- `engine-host` already runs async DirectML and OpenVINO startup probes and folds those results into lane status.
- `openvino_npu` is now a live runtime lane and can be selected or forced when preflight succeeds.
- Runtime loading policy:
  - production uses app-local absolute runtime bundle paths only
  - production does not fall back to `PATH`, bare DLL names, or user-installed ORT/OpenVINO copies
  - production uses restricted absolute-path DLL loading for runtime bundles
  - development can use explicit absolute bundle-root overrides (`SMOLPC_ORT_BUNDLE_ROOT`, `SMOLPC_OPENVINO_BUNDLE_ROOT`)
- Forced overrides for diagnostics:
  - `SMOLPC_FORCE_EP=cpu|dml|directml|openvino|openvino_npu`
  - `SMOLPC_DML_DEVICE_ID=<non-negative int>`
  - invalid forced device ids fail with explicit load error `invalid_directml_device_id`
- Runtime failure handling:
  - if DirectML fails during initialization or generation, host can fall back to CPU for the current load
  - OpenVINO preflight timeouts mark the current load as `temporary_fallback` without overwriting a prior good OpenVINO record
  - OpenVINO compile/runtime preflight failures fall through to `directml` or `cpu` unless OpenVINO was explicitly forced
  - `backend_status.decision_persistence_state=temporary_fallback` means the current active backend is a fallback without overwriting a previously persisted eligible record
  - repeated DirectML failures still demote to persisted CPU after the existing threshold is reached

## Persistence Contract

- Backend decisions are now stored in `inference/backend_decisions.v2.json`
- Records are keyed by the full selection fingerprint, not one winner per model
- Multiple records for the same model are retained when the fingerprint differs
- Each record now stores:
  - `key`
  - `persisted_decision`
  - `failure_counters`
  - `updated_at`
- `persisted_decision` may be `null`
  - this is how the engine preserves failure counters for a fingerprint without poisoning a previous good persisted decision
- Temporary fallbacks update status immediately and may update counters in the matching fingerprint record, but they do not overwrite an existing persisted eligible lane unless the host explicitly demotes that lane

## Protocol Compatibility

- Current protocol major: `1`.
- Client and host must match protocol major versions.
- On mismatch, clients return an explicit protocol mismatch error.
