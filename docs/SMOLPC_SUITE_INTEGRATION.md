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

## Host Discovery

Resolution order:

1. `SMOLPC_ENGINE_HOST_BIN`
2. Resource sidecar directories (`<resource-dir>` and `<resource-dir>/binaries`)
3. Sidecar path next to current executable
4. Workspace target binary (`target/debug/smolpc-engine-host` then `target/release/smolpc-engine-host`)

The client also coordinates spawn with a short-lived lock file:

- `<runtime-root>/engine-spawn.lock`

This avoids duplicate spawn races when two apps connect at the same time.

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

## Next Steps

- Migrate Blender and LibreOffice to this client contract.
- Replace remaining Ollama-specific benchmark path.
