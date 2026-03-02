# Shared Engine Integration Notes

This branch introduces a shared engine architecture with three crates:

- `crates/smolpc-engine-core`
- `crates/smolpc-engine-host`
- `crates/smolpc-engine-client`

## CodeHelper Wiring

`src-tauri/src/commands/inference.rs` now routes inference commands through `smolpc-engine-client`.

Frontend command names are unchanged:

- `load_model`
- `unload_model`
- `generate_text`
- `inference_generate`
- `inference_cancel`
- `is_generating`
- `list_models`
- `get_current_model`
- `check_model_exists`
- `get_inference_backend_status`

## Runtime Directories

- Shared runtime root: `%LOCALAPPDATA%/SmolPC/engine-runtime` (or platform equivalent)
- Host data dir: `<runtime-root>/host-data`
- Token file: `<runtime-root>/engine-token.txt`
- Shared model root (recommended): `%LOCALAPPDATA%/SmolPC/models`

## Host Discovery

Resolution order:

1. `SMOLPC_ENGINE_HOST_BIN`
2. Resource sidecar directories (`<resource-dir>` and `<resource-dir>/binaries`)
3. Sidecar path next to current executable
4. Workspace target binary (`target/debug/smolpc-engine-host` then `target/release/smolpc-engine-host`)

The client also coordinates spawn with a short-lived lock file:

- `<runtime-root>/engine-spawn.lock`

This avoids duplicate spawn races when two apps connect at the same time.

## Dev Runtime Policy

Recommended local launch:

- `npm run tauri:dev`
- `npm run tauri:dml` (forced DirectML)

`run-tauri-dev.ps1` now:

1. Builds `smolpc-engine-host` before app launch.
2. Sets `SMOLPC_ENGINE_DEV_FORCE_RESPAWN=1` for deterministic host reuse behavior.
3. Requests host shutdown pre-launch so force overrides (`SMOLPC_FORCE_EP`) apply cleanly.

Shared Qwen3 bootstrap:

- `npm run model:setup:qwen3`
- Creates `qwen3-4b-instruct-2507` under shared models root for cross-app reuse.

## Automatic Backend Selection (Current)

At startup, the host runs an async capability probe and then applies capability-first selection on load:

1. Detect available backends (`cpu`, optional `directml`).
2. Rank DirectML candidates on multi-GPU systems (prefer discrete GPU, then higher VRAM).
3. Select backend with sticky decision support and failure counters from backend store.

Behavior details:

- Startup remains non-blocking for UI; model load waits up to ~1.5s for probe completion.
- In auto mode, DirectML is preferred when available and artifact exists.
- On DirectML init/runtime failure, host falls back to CPU for current session flow without requiring app restart.
- Force overrides remain supported for diagnostics:
  - `SMOLPC_FORCE_EP=cpu|dml`
  - `SMOLPC_DML_DEVICE_ID=<id>`

Current model priority in registry:

1. `qwen3-4b-instruct-2507`
2. `qwen2.5-coder-1.5b`

## Packaging

Tauri bundles `smolpc-engine-host` as a packaged resource via:

- `src-tauri/tauri.conf.json -> bundle.resources` (`binaries/*`)
- `src-tauri/binaries/` target-triple sidecar naming

Release workflow stages sidecar binaries before running `tauri-action`.

## Streaming and Metrics Contract

- Stream errors are structured SSE error events (not token text).
- Cancellation uses code `INFERENCE_GENERATION_CANCELLED`.
- Runtime stream errors use code `ENGINE_STREAM_ERROR`.
- `smolpc_metrics` is emitted for stream completion and non-stream responses.

Backend status payloads now use canonical backend strings:

- `cpu`
- `directml`

Additional status fields used by consumers/UI:

- `available_backends`
- `selection_state`
- `selection_reason`
- `selected_device_id`
- `selected_device_name`

## Next Steps

- Migrate Blender and LibreOffice to this client contract.
- Replace remaining Ollama-specific benchmark path.
