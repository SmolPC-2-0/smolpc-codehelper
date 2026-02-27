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
2. Workspace target binary (`target/debug/smolpc-engine-host` in dev)
3. Sidecar path next to current executable

## Next Steps

- Bundle `smolpc-engine-host` as a sidecar for production packaging.
- Migrate Blender and LibreOffice to this client contract.
- Replace remaining Ollama-specific benchmark path.
