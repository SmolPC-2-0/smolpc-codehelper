# SmolPC Shared Engine Workspace

Local-first inference workspace for CodeHelper and other SmolPC desktop apps.

This repo contains:

1. A reusable shared inference daemon (`smolpc-engine-host`)
2. A typed Rust client with connect-or-spawn lifecycle (`smolpc-engine-client`)
3. Shared model/runtime logic (`smolpc-engine-core`)
4. The CodeHelper desktop app (`src-tauri` + `src`)

The goal is one stable engine contract that multiple apps can consume.

## Who This README Is For

This README is written for:

1. Engineers integrating another app (Blender helper, GIMP helper, etc.)
2. AI agents asked to "wire app X to the shared engine"

Assume the app team should integrate against the engine contract, not engine internals.

## Repo Layout

```text
smolpc-codehelper/
  Cargo.toml                         # workspace root
  crates/
    smolpc-engine-core/              # model/runtime/backend selection domain
    smolpc-engine-host/              # localhost HTTP/SSE daemon
    smolpc-engine-client/            # connect-or-spawn Rust client
  src-tauri/                         # CodeHelper Rust/Tauri app
  src/                               # CodeHelper Svelte frontend
  docs/ENGINE_API.md                 # API contract
  docs/SMOLPC_SUITE_INTEGRATION.md   # integration notes
```

## Quick Start (Dev)

## Prerequisites

1. Node.js 20+
2. Rust stable toolchain (workspace uses Rust 1.88)
3. Windows runtime libraries available in `src-tauri/libs` (includes `onnxruntime*.dll`, `DirectML.dll`)
4. Model assets in `src-tauri/models`

## Build and Validate

```bash
npm install
cargo check --workspace
npm run check
```

## Run CodeHelper

Automatic backend selection:

```bash
npm run tauri:dev
```

Forced DirectML:

```bash
npm run tauri:dml
```

Notes:

1. Dev launcher rebuilds `smolpc-engine-host` before app startup.
2. Dev launcher requests host shutdown before launch so overrides apply cleanly.

## Engine Contract (What Consumers Depend On)

Base URL:

`http://127.0.0.1:19432`

Auth:

`Authorization: Bearer <token>`

Core control endpoints:

1. `GET /engine/health`
2. `GET /engine/meta`
3. `GET /engine/status`
4. `POST /engine/load`
5. `POST /engine/unload`
6. `POST /engine/cancel`
7. `POST /engine/check-model`
8. `POST /engine/shutdown`

Inference surface:

1. `GET /v1/models`
2. `POST /v1/chat/completions` (streaming and non-streaming)

Authoritative API details are in [docs/ENGINE_API.md](docs/ENGINE_API.md).

## Integration Workflow (For External Apps)

Use this sequence:

1. Connect to existing engine or spawn it.
2. Check health and protocol via `/engine/meta`.
3. Load model with `/engine/load`.
4. Generate via `/v1/chat/completions`.
5. Read backend status via `/engine/status` for diagnostics and telemetry.
6. Handle cancellation, queue full (429), queue timeout (504), and reconnect paths.

Do not:

1. Depend on branch head behavior.
2. Parse internal logs as contract.
3. Assume a backend (always check `/engine/status`).

## Rust Integration (Preferred Inside SmolPC Apps)

Inside Rust app code, use `smolpc-engine-client`:

1. Build `EngineConnectOptions`
2. Call `connect_or_spawn(options)`
3. Use typed methods:
1. `load_model`
2. `generate_stream` or `generate_text`
3. `status`
4. `cancel`

The client handles:

1. Host binary discovery
2. Token auth file lifecycle
3. Spawn locking for multi-app races
4. Protocol major-version checks

## HTTP Integration (Non-Rust Apps)

For Python or other apps, treat host as localhost HTTP service with bearer auth.

Minimal non-stream example:

```bash
curl -H "Authorization: Bearer <token>" ^
  -H "Content-Type: application/json" ^
  -d "{\"model\":\"smolpc-engine\",\"stream\":false,\"messages\":[{\"role\":\"user\",\"content\":\"Hello\"}]}" ^
  http://127.0.0.1:19432/v1/chat/completions
```

Streaming example:

```bash
curl -N -H "Authorization: Bearer <token>" ^
  -H "Content-Type: application/json" ^
  -d "{\"model\":\"smolpc-engine\",\"stream\":true,\"messages\":[{\"role\":\"user\",\"content\":\"Count to 5\"}]}" ^
  http://127.0.0.1:19432/v1/chat/completions
```

## Backend Selection Behavior (Windows)

Current policy is capability-first with resilience:

1. Startup probe detects available backends and DirectML device candidates.
2. On load, host prefers DirectML when available and artifact exists.
3. If DirectML init/runtime fails in auto mode, host falls back to CPU.
4. Forced DirectML mode is strict (failure is returned as error).

Relevant status fields in `/engine/status.backend_status`:

1. `active_backend`
2. `runtime_engine`
3. `available_backends`
4. `selection_state`
5. `selection_reason`
6. `selected_device_id`
7. `selected_device_name`
8. `failure_counters`

Use these fields for UI badges, telemetry, and triage.

## Runtime Discovery and Packaging

Host binary resolution order:

1. `SMOLPC_ENGINE_HOST_BIN`
2. Resource sidecar directories
3. Sidecar near executable
4. Workspace `target/debug` then `target/release`

Shared runtime directory (Windows):

`%LOCALAPPDATA%/SmolPC/engine-runtime`

Important files:

1. `engine-token.txt`
2. `engine-spawn.lock`
3. Host data directory (`host-data`)

Release packaging includes sidecar resources via `src-tauri/tauri.conf.json`.

## Environment Variables

Common:

1. `SMOLPC_MODELS_DIR` override model root
2. `SMOLPC_ENGINE_PORT` override host port

Debug/diagnostic:

1. `SMOLPC_FORCE_EP=cpu|dml`
2. `SMOLPC_DML_DEVICE_ID=<int>`
3. `SMOLPC_ENGINE_DEV_FORCE_RESPAWN=1` (dev launcher sets this)

## Troubleshooting

If engine appears on CPU when GPU exists:

1. Check `/engine/status.backend_status.active_backend`
2. Check `selection_reason` and `dml_gate_state`
3. Ensure `dml/model.onnx` artifact exists for model
4. Confirm DirectML runtime DLLs are present in `src-tauri/libs`

If `tauri:dml` fails with binary lock:

1. Ensure no stale `smolpc-engine-host.exe` process is running
2. Re-run `npm run tauri:dml` (launcher now requests shutdown and force-kills stale host if needed)

If streaming looks chunked/slow:

1. Verify active backend in `/engine/status`
2. Inspect `smolpc_metrics` from stream/non-stream responses
3. Retry after `POST /engine/shutdown` to clear stale runtime state

## Developer Handoff Notes

When handing this repo to another team:

1. Share the release tag and `docs/ENGINE_API.md`
2. Require integration against tagged contract, not branch head
3. Ask teams to report blockers with:
1. Request payload
2. Response body/status
3. `/engine/status` snapshot
4. Exact app version and hardware
