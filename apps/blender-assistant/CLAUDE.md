# SmolPC Blender Helper Notes

Reference notes for coding sessions and repository maintenance.

Last updated: 2026-03-09  
Version: 7.0.0

## Current Architecture

- Frontend: Svelte (`src/`)
- Backend orchestration: Rust/Tauri (`src-tauri/src/`)
- Blender bridge API: Axum server on `127.0.0.1:5179`
- Primary generation backend: shared engine sidecar (`shared_engine`)
- Optional fallback backend: Ollama (`ollama`)
- Retrieval: local metadata index in Rust (`src-tauri/src/rag`)

## Runtime Flow

1. App starts and initializes logging + RAG index.
2. App resolves resources and ensures shared engine is running.
3. App attempts model autoload (`SHARED_ENGINE_MODEL_ID` preferred).
4. App starts scene bridge for Blender addon compatibility.
5. Frontend polls status and scene snapshot via Tauri commands.
6. Chat requests stream tokens through Rust generation commands.

## Tauri Command Surface

- `assistant_stream_ask`
- `inference_cancel`
- `is_generating`
- `assistant_ask`
- `assistant_analyze_scene`
- `retrieve_rag_context`
- `assistant_status`
- `scene_current`
- `scene_update`
- `set_generation_backend`
- `get_generation_backend`
- `open_logs`

## Important Paths

```text
src-tauri/src/main.rs
src-tauri/src/shared_engine.rs
src-tauri/src/scene_bridge.rs
src-tauri/src/commands/assistant.rs
src-tauri/src/commands/generation.rs
src-tauri/src/state.rs
blender_addon/blender_helper_http.py
```

## Model and Engine Asset Notes

- Bundled engine host location: `src-tauri/binaries/`
- Bundled runtime libraries: `src-tauri/libs/`
- Bundled model staging directory: `src-tauri/resources/models/`
- Shared runtime token directory: `%LOCALAPPDATA%/SmolPC/engine-runtime`

Model/bootstrap scripts:

- `npm run model:setup:qwen2_5`
- `npm run model:setup:qwen3`
- `npm run bundle:stage:model`

## Environment Variables

- `BLENDER_HELPER_BACKEND`
- `BLENDER_HELPER_ALLOW_OLLAMA_FALLBACK`
- `SHARED_ENGINE_MODEL_ID`
- `SMOLPC_MODELS_DIR`
- `SMOLPC_ENGINE_HOST_BIN`
- `ENGINE_BASE_URL`
- `OLLAMA_MODEL`

## Known Constraints

- Self-contained packaging with very large model data files can exceed MSI/NSIS tooling limits.
- Bridge requires the desktop app to be running; addon alone is not a standalone server.
- Ollama fallback is opt-in via environment variable.
