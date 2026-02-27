# SmolPC CodeHelper

Offline desktop coding assistant for low-spec school hardware.

This repository now includes a shared inference engine architecture so other SmolPC apps can reuse one local daemon session.

## What Changed

- Added a Rust workspace at repo root.
- Added shared crates:
  - `crates/smolpc-engine-core`
  - `crates/smolpc-engine-host`
  - `crates/smolpc-engine-client`
- Switched CodeHelper inference commands to route through `smolpc-engine-client`.
- Preserved existing frontend Tauri command names to avoid UI breakage.

## Workspace Layout

```text
smolpc-codehelper/
  Cargo.toml                # workspace root
  Cargo.lock                # workspace lockfile
  crates/
    smolpc-engine-core/     # reusable inference/model/runtime code
    smolpc-engine-host/     # localhost HTTP/SSE daemon
    smolpc-engine-client/   # connect-or-spawn + typed client
  src-tauri/                # CodeHelper desktop app
  src/                      # Svelte frontend
```

## Local Engine API

Base URL: `http://127.0.0.1:19432`

Auth: `Authorization: Bearer <token>`

Core endpoints:

- `GET /engine/health`
- `GET /engine/meta`
- `GET /engine/status`
- `POST /engine/load`
- `POST /engine/unload`
- `POST /engine/cancel`
- `POST /engine/check-model`
- `POST /engine/shutdown`
- `GET /v1/models`
- `POST /v1/chat/completions` (streaming and non-streaming)

Full endpoint notes: `docs/ENGINE_API.md`.

## Development

### Prerequisites

- Node.js 20+
- Rust stable toolchain
- ONNX Runtime/DirectML runtime files available under app resources (`src-tauri/libs` in dev, bundle resources in release)
- Local model files under `src-tauri/models` (or set `SMOLPC_MODELS_DIR`)

### Build/Check

```bash
cargo check --workspace
npm install
npm run tauri dev
```

### Engine Host Binary Resolution

When CodeHelper starts inference, `smolpc-engine-client` resolves host binary in this order:

1. `SMOLPC_ENGINE_HOST_BIN`
2. `target/debug/smolpc-engine-host` (dev)
3. sidecar next to app executable

### Shared Runtime Paths

Default shared runtime root:

- Windows: `%LOCALAPPDATA%/SmolPC/engine-runtime`
- Other OS: platform local data dir equivalent

Files:

- token: `engine-token.txt`
- host data: `host-data/`

## Command Surface Used by Frontend

The frontend still uses these Tauri commands:

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

## Integration Docs

- API contract: `docs/ENGINE_API.md`
- suite integration notes: `docs/SMOLPC_SUITE_INTEGRATION.md`

## Status

CodeHelper now runs inference through a reusable daemon/client architecture in this branch. Blender/LibreOffice migration can use the same client contract and shared session model.
