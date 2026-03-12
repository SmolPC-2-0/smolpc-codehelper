# Blender Learning Assistant

Offline Blender tutoring app built with Tauri + Svelte, with a Rust backend that runs local retrieval and local model inference through a bundled/shared engine.

Version: 7.0.0

## What Is Implemented

- Scene-aware Blender Q&A (desktop app + Blender addon)
- Token streaming with cancellation
- Local RAG retrieval from bundled Blender docs metadata
- Shared engine sidecar as the primary backend (`shared_engine`)
- Optional Ollama fallback (`ollama`) when explicitly enabled

## Runtime Architecture

- Frontend: Svelte app in `src/`
- Backend orchestration: Rust in `src-tauri/src/`
- Blender bridge HTTP API: `http://127.0.0.1:5179` (Axum, token-protected except `/health`)
- Shared engine API: `http://127.0.0.1:19432`
- Optional Ollama API: `http://127.0.0.1:11434`

The app does not require a Python Flask server at runtime.

## Development Quick Start

### Prerequisites

- Node.js 18+
- Rust stable toolchain
- Tauri prerequisites for your platform

### Run From Monorepo Root

```bash
npm install
npm run runtime:setup:blender
npm run model:setup:qwen2_5:blender
npm run tauri:dev:blender
```

`runtime:setup` uses `scripts/setup-libs.ps1` on Windows. A shell-script alternative remains available via `runtime:setup:sh`.
`npm run tauri:dev` and `npm run tauri dev` both route through Blender wrappers that set `CARGO_TARGET_DIR`.

Other useful root commands:

```bash
npm run check:blender
npm run model:setup:qwen2_5:blender
npm run model:setup:qwen3:blender
npm run tauri:dml:blender
```

### Run

```bash
npm install
npm run runtime:setup
npm run model:setup:qwen2_5
npm run tauri:dev
```

### Build

```bash
npm run tauri:build
```

## Blender Addon

`blender_addon/blender_helper_http.py` is auto-synced into detected Blender user addon folders when the desktop app starts.

Then in Blender:

1. Blender -> Edit -> Preferences -> Add-ons
2. Search for `Blender Learning Assistant`
3. Enable the addon

The addon talks to the local bridge (`127.0.0.1:5179`) and reads the auth token written by the desktop app.

If Blender is not installed yet (or no Blender version folder is found), use manual install from `blender_addon/blender_helper_http.py`.

## Self-Contained Build (Engine + Model Resources)

For release packaging that includes engine/runtime/model assets:

1. (Optional) Download/update model artifacts:

```powershell
npm run model:setup:qwen2_5
# or
npm run model:setup:qwen3
# or
npm run model:setup:qwen3_5
```

2. Stage model artifacts into `src-tauri/resources/models/`:

```powershell
npm run bundle:stage:model
# or (for qwen3.5)
npm run bundle:stage:model:qwen3_5
```

3. Build:

```powershell
npm run tauri:build:self-contained
```

Detailed bundling notes are in `Docs/ENGINE_BUNDLING_SETUP.md`.

## Backend Selection and Fallback

- Default backend: `shared_engine`
- Fallback to Ollama is disabled unless `BLENDER_HELPER_ALLOW_OLLAMA_FALLBACK` is set
- UI backend toggle switches between `shared_engine` and `ollama`

## Environment Variables

- `BLENDER_HELPER_BACKEND`: startup backend (`shared_engine` or `ollama`)
- `BLENDER_HELPER_ALLOW_OLLAMA_FALLBACK`: enable fallback (`1|true|yes|on`)
- `SHARED_ENGINE_MODEL_ID`: preferred shared engine model id
- `SMOLPC_MODELS_DIR`: override model root directory
- `SMOLPC_ENGINE_HOST_BIN`: explicit engine host binary path
- `ENGINE_BASE_URL`: override shared engine base URL
- `OLLAMA_MODEL`: model id used only when running on Ollama backend

## Repository Layout

```text
src/                            Svelte UI
src-tauri/src/                  Rust backend (commands, bridge, RAG, engine client)
src-tauri/binaries/             Engine host binary placeholder/bundle dir
src-tauri/libs/                 ONNX Runtime library bundle dir
src-tauri/resources/models/     Staged model artifacts for packaging
src-tauri/resources/rag_system/ Bundled retrieval metadata assets
blender_addon/                  Blender addon bridge client
scripts/                        Model/runtime setup and staging scripts
```
