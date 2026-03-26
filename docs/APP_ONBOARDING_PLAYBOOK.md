# Shared Engine App Onboarding Playbook

Use this playbook when onboarding another app (Blender helper, GIMP helper, etc.) to the shared SmolPC inference engine.

This is the fastest path for both developers and AI-assisted integration sessions.

Keep app UX/tool logic in your app, and integrate inference only through the shared engine contract or client.

## Scope

1. Windows-first integration.
2. Stable contract surface only (`/engine/*`, `/v1/*`).
3. No dependency on engine internals.

## Start Here

1. Read [ENGINE_API.md](./ENGINE_API.md).
2. Skim current repo examples if you want working integration patterns:
   - [app/src-tauri/src/engine/supervisor.rs](../app/src-tauri/src/engine/supervisor.rs)
   - [app/src-tauri/src/commands/inference.rs](../app/src-tauri/src/commands/inference.rs)
   - [crates/smolpc-connector-common/src/text_generation.rs](../crates/smolpc-connector-common/src/text_generation.rs)
3. Run shared model bootstrap once on the machine for the supported shared baseline:
   - `npm run runtime:setup:openvino`
   - `npm run model:setup:qwen25-instruct`
   - `npm run model:setup:qwen3-4b`
   - `qwen3-4b` now self-builds its DirectML artifact from `Qwen/Qwen3-4B` in an isolated Python `3.14` venv; export logs are written under `%LOCALAPPDATA%/SmolPC/logs/dml-export`
4. Use this playbook as the implementation checklist.

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

What not to depend on:

1. Internal logs.
2. Internal engine module paths.
3. Branch-head behavior not captured in contract docs.
4. Stale app-specific repo paths from older layouts.

## Integration Paths

## Path A: Rust app (preferred)

1. Add dependency on `smolpc-engine-client`.
2. Build `EngineConnectOptions`.
3. Call `connect_or_spawn(options)`.
4. Use typed calls such as `status`, `load_model`, `generate_stream_messages`, and `cancel`.

Minimal example:

```rust
use smolpc_engine_client::{
    connect_or_spawn, EngineChatMessage, EngineConnectOptions, RuntimeModePreference,
};
use std::path::PathBuf;

async fn run_engine_flow() -> Result<(), Box<dyn std::error::Error>> {
    let local_app_data = std::env::var("LOCALAPPDATA")?;
    let smolpc_root = PathBuf::from(local_app_data).join("SmolPC");
    let shared_runtime_dir = smolpc_root.join("engine-runtime");
    let data_dir = shared_runtime_dir.join("host-data");
    let models_dir = smolpc_root.join("models");

    let options = EngineConnectOptions {
        port: 19432,
        app_version: "dev-onboarding".to_string(),
        shared_runtime_dir,
        data_dir,
        resource_dir: None,
        models_dir: Some(models_dir),
        host_binary: None,
        runtime_mode: RuntimeModePreference::Auto,
        dml_device_id: None,
        force_respawn: false,
    };

    let client = connect_or_spawn(options).await?;

    let status = client.status().await?;
    println!("Engine ready: {}", status.ready);

    client.load_model("qwen2.5-1.5b-instruct").await?;

    let messages = vec![
        EngineChatMessage {
            role: "system".to_string(),
            content: "You are a concise local assistant.".to_string(),
        },
        EngineChatMessage {
            role: "user".to_string(),
            content: "Explain what this app needs to do to onboard successfully.".to_string(),
        },
    ];

    let metrics = client
        .generate_stream_messages(&messages, None, |token| {
            print!("{token}");
        })
        .await?;
    println!("\nMetrics: {:?}", metrics);

    Ok(())
}
```

Call `client.cancel().await?` from your UI or cancellation token when the user aborts an in-flight generation request.

For app-style startup policy flows, `EngineClient::ensure_started(...)` is also available. The current Tauri app wraps that call behind its engine supervisor instead of calling it directly from every command handler.

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

This path assumes the engine host is already running, either because your app started it or because a companion launcher/wrapper did.

Minimal PowerShell example:

```powershell
$baseUrl = "http://127.0.0.1:19432"
$tokenPath = Join-Path $env:LOCALAPPDATA "SmolPC\engine-runtime\engine-token.txt"
$token = (Get-Content $tokenPath -Raw).Trim()
$authHeader = "Authorization: Bearer $token"

curl.exe -sS `
  -H $authHeader `
  "$baseUrl/engine/meta"

$loadBody = '{"model_id":"qwen2.5-1.5b-instruct"}'
curl.exe -sS `
  -X POST `
  -H $authHeader `
  -H "Content-Type: application/json" `
  --data $loadBody `
  "$baseUrl/engine/load"

$chatBody = @'
{
  "model": "smolpc-engine",
  "messages": [
    { "role": "user", "content": "Give me a short onboarding checklist." }
  ],
  "stream": false
}
'@

curl.exe -sS `
  -X POST `
  -H $authHeader `
  -H "Content-Type: application/json" `
  --data $chatBody `
  "$baseUrl/v1/chat/completions"
```

Use the full request and streaming details in [ENGINE_API.md](./ENGINE_API.md) as the source of truth. The `/engine/load` step chooses the shared model; the `model` field in `/v1/chat/completions` is just the OpenAI-compatible request field.

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
- app/src-tauri/src/engine/supervisor.rs
- app/src-tauri/src/commands/inference.rs
- crates/smolpc-connector-common/src/text_generation.rs
```
