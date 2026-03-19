# Shared Engine App Onboarding Playbook

Use this playbook when onboarding another app (Blender helper, GIMP helper, etc.) to the shared SmolPC inference engine.

This is the fastest path for both developers and AI-assisted integration sessions.

Monorepo placement convention for new apps:

1. Create app root under `apps/<app-name>/`.
2. Keep app UX/tool logic in the app root.
3. Integrate inference only through shared engine contract/client.

## Scope

1. Windows-first integration.
2. Stable contract surface only (`/engine/*`, `/v1/*`).
3. No dependency on engine internals.

## Start Here

1. Read [ENGINE_API.md](./ENGINE_API.md).
2. Read [SMOLPC_SUITE_INTEGRATION.md](./SMOLPC_SUITE_INTEGRATION.md).
3. Run shared model bootstrap once on the machine for the supported shared baseline:
   - `npm run runtime:setup:openvino`
   - `npm run model:setup:qwen25-instruct`
   - `npm run model:setup:qwen3-4b`
   - `qwen3-4b` now self-builds its DirectML artifact from `Qwen/Qwen3-4B` in an isolated Python `3.14` venv; export logs are written under `%LOCALAPPDATA%/SmolPC/logs/dml-export`
5. Use this playbook as the implementation checklist.

## Shared Model Baseline

Default shared model for onboarding:

1. `qwen2.5-1.5b-instruct`

Higher-capability supported model:

1. `qwen3-4b`

Shared model root (recommended):

1. `%LOCALAPPDATA%/SmolPC/models`
2. `SMOLPC_MODELS_DIR` should point to the same path for all apps.

## Contract Boundary

Your app should depend only on:

1. Engine HTTP API contract.
2. `smolpc-engine-client` crate (preferred for Rust apps).
3. `backend_status` payload for diagnostics/UX.

Do not depend on:

1. Internal logs.
2. Internal engine module paths.
3. Branch-head behavior not captured in contract docs.

## Integration Paths

## Path A: Rust app (preferred)

1. Add dependency on `smolpc-engine-client`.
2. Build `EngineConnectOptions`.
3. Call `connect_or_spawn(options)`.
4. Use typed calls:
   1. `load_model`
   2. `generate_text` or `generate_stream`
   3. `status`
   4. `cancel`

## Path B: Non-Rust app (HTTP)

1. Get bearer token from:
   - `%LOCALAPPDATA%/SmolPC/engine-runtime/engine-token.txt`
2. Call engine:
   - Base URL: `http://127.0.0.1:19432`
   - Header: `Authorization: Bearer <token>`
3. Required flow:
   1. `GET /engine/meta`
   2. `POST /engine/load` with the selected shared model id (for example `qwen2.5-1.5b-instruct`)
   3. `POST /v1/chat/completions`

## Minimum Onboarding Checklist

Every app integration must pass all checks below.

1. Connectivity:
   - `GET /engine/health` returns `ok: true`.
   - `GET /engine/meta` returns protocol major `1`.
2. Model lifecycle:
   - `POST /engine/load` succeeds.
   - `GET /engine/status` shows expected `current_model`.
3. Non-stream generation:
   - `POST /v1/chat/completions` (`stream=false`) succeeds.
   - Response includes `smolpc_metrics`.
4. Stream generation:
   - `POST /v1/chat/completions` (`stream=true`) emits token chunks.
   - Stream emits one `chat.completion.metrics` event.
   - Stream ends with `[DONE]`.
5. Cancellation:
   - `POST /engine/cancel` cancels active generation.
   - Client handles `INFERENCE_GENERATION_CANCELLED` properly.
6. Queue behavior:
   - Client handles `429` (queue full).
   - Client handles `504` (queue timeout).
7. Backend diagnostics:
   - App can surface/log `active_backend`, `runtime_engine`, `selection_reason`, and `backend_status.lanes.*`.
   - For targeted validation, verify the backend expected for the selected model and runtime mode. Example: `qwen3-4b` with `SMOLPC_FORCE_EP=openvino_npu` should report `active_backend=openvino_npu`.

## Required Error Handling

Treat these as expected operational states:

1. `429` queue full: retry with jitter.
2. `504` queue timeout: report timeout and allow user retry.
3. Stream error event:
   - `error.code=INFERENCE_GENERATION_CANCELLED` -> user/system cancel.
   - `error.code=ENGINE_STREAM_ERROR` -> runtime failure path.
4. Protocol mismatch from client: stop and report host/app version mismatch.

## Known Current Limitation

1. Automatic selection now prefers `openvino_npu -> directml -> cpu` when the OpenVINO lane passes preflight.
   - For targeted debugging, force `SMOLPC_FORCE_EP=openvino_npu`, `dml`, or `cpu` and inspect `/engine/status`.
2. OpenVINO CPU and OpenVINO NPU must use structured chat history for normal chat requests.
   - Only explicit legacy single-message ChatML payloads stay on the prompt compatibility path.
3. `qwen3-4b` is OpenVINO non-thinking only in the current supported baseline.
   - Internal OpenVINO defaults follow the upstream non-thinking guidance: `temperature=0.7`, `top_p=0.8`, `top_k=20`, `presence_penalty=1.5`.
4. The current Windows PyPI `openvino-genai` wheel is not a valid native runtime bundle for this repo's OpenVINO adapter.
   - Windows provisioning now uses the official archive path instead. The app-local bundle must include `openvino_genai_c.dll`, and the current working NPU defaults on this PC are `MAX_PROMPT_LEN=512` and `MIN_RESPONSE_LEN=1024`.
   - `MIN_RESPONSE_LEN=1024` is the current NPU `StaticLLMPipeline` allocation constraint on this PC, not a product-level preference for padding short answers.
   - For targeted debugging, `SMOLPC_OPENVINO_NPU_MAX_PROMPT_LEN` and `SMOLPC_OPENVINO_NPU_MIN_RESPONSE_LEN` can override those defaults. Invalid values block the OpenVINO lane before a false-ready status is reported.
5. `qwen3-4b` DirectML stays unified on the same public model id.
   - Do not introduce backend-specific large-model ids in app UX or app config.
   - The setup script keeps `self_build` as the supported default DML source mode; `fallback_snapshot` is a manual recovery path only when the self-build path is broken on a developer machine.

## Definition of Done (Per App)

Integration is complete when all are true:

1. App uses shared engine contract without internal engine coupling.
2. Onboarding checklist passes on target machine(s).
3. App captures and reports backend status fields in diagnostics.
4. Integration issue report template below is in place.

## Integration Issue Report Template

Use this exact payload when reporting onboarding blockers:

1. App name + app version.
2. OS + hardware (CPU/GPU/NPU, driver versions).
3. Request payload (redacted if needed).
4. HTTP status + response body.
5. `/engine/status` snapshot.
6. `GET /engine/meta` snapshot.
7. Whether `SMOLPC_FORCE_EP` or `SMOLPC_DML_DEVICE_ID` was set.

## AI Session Starter (Copy/Paste)

Use this prompt when asking an AI agent to do app onboarding:

```text
Integrate this app with the SmolPC shared engine using contract-first integration.

Requirements:
1. Use only documented engine contract endpoints (/engine/*, /v1/*) or smolpc-engine-client for Rust.
2. Do not depend on engine internals or parse logs as API.
3. Implement load -> generate (stream + non-stream) -> cancel -> status flow.
4. Handle 429 queue full, 504 queue timeout, stream error events, and protocol mismatch.
5. Surface backend diagnostics: active_backend, runtime_engine, selection_reason, and backend_status.lanes.
6. Provide final verification evidence with request/response examples and engine status snapshot.

Reference docs:
- docs/APP_ONBOARDING_PLAYBOOK.md
- docs/ENGINE_API.md
- docs/SMOLPC_SUITE_INTEGRATION.md
```
