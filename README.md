# SmolPC Code Helper

Offline AI coding assistant for secondary school students, built on a shared inference engine that other SmolPC desktop apps can consume.

The inference engine runs locally as a localhost HTTP daemon with bearer-token auth. Apps connect to it (or spawn it) and use an OpenAI-compatible API surface for chat completions. Backend selection (CPU / DirectML) is automatic with manual override support.

## Repo Structure

```
smolpc-codehelper/
  crates/
    smolpc-engine-core/       # model registry, backend selection, runtime logic
    smolpc-engine-host/       # localhost HTTP/SSE inference daemon
    smolpc-engine-client/     # typed Rust client (connect-or-spawn lifecycle)
  src-tauri/                  # CodeHelper desktop app (Tauri 2 / Rust)
  src/                        # CodeHelper frontend (Svelte 5)
  docs/
    ENGINE_API.md             # full API contract
    APP_ONBOARDING_PLAYBOOK.md # integration checklist for app teams
    SMOLPC_SUITE_INTEGRATION.md
```

## Prerequisites

- Node.js 20+
- Rust stable toolchain (1.88+)
- Windows runtime libraries in `src-tauri/libs` (`onnxruntime*.dll`, `DirectML.dll`)
- Python 3.10+ with `huggingface_hub`, `onnx`, `onnxruntime-genai` (for model setup)

## Setup

```bash
npm install
cargo check --workspace
```

Set up the shared model directory (Qwen3, used by all SmolPC apps):

```bash
npm run model:setup:qwen3
```

This downloads and validates model artifacts into `%LOCALAPPDATA%/SmolPC/models/qwen3-4b-instruct-2507/`.

## Running

```bash
npm run tauri:dev       # dev mode, auto backend selection
npm run tauri:dml       # force DirectML (recommended for demos)
```

The dev launcher rebuilds the engine host before app startup and shuts down any existing host so overrides apply cleanly.

## Engine API

The shared engine listens on `http://127.0.0.1:19432` with bearer-token auth.

**Control endpoints:**

| Endpoint | Purpose |
|---|---|
| `GET /engine/health` | Liveness check |
| `GET /engine/meta` | Protocol version, capabilities |
| `GET /engine/status` | Backend info, model state, diagnostics |
| `POST /engine/load` | Load a model |
| `POST /engine/unload` | Unload current model |
| `POST /engine/cancel` | Cancel active generation |
| `POST /engine/shutdown` | Shut down the host |

**Inference endpoints (OpenAI-compatible):**

| Endpoint | Purpose |
|---|---|
| `GET /v1/models` | List available models |
| `POST /v1/chat/completions` | Generate (streaming and non-streaming) |

Full contract details: [docs/ENGINE_API.md](docs/ENGINE_API.md)

## Integrating Another App

If you're wiring a new SmolPC app (Blender helper, GIMP helper, etc.) to the shared engine, start here:

**[docs/APP_ONBOARDING_PLAYBOOK.md](docs/APP_ONBOARDING_PLAYBOOK.md)** — integration checklist, validation criteria, error handling requirements, and an AI session starter prompt.

The short version:

- **Rust apps**: depend on `smolpc-engine-client`, call `connect_or_spawn()`, use typed methods (`load_model`, `generate_stream`, `status`, `cancel`). The client handles host discovery, token auth, spawn locking, and protocol checks.
- **Non-Rust apps**: read the bearer token from `%LOCALAPPDATA%/SmolPC/engine-runtime/engine-token.txt`, hit the HTTP endpoints directly.

Integrate against the documented contract, not engine internals or branch-head behavior.

## Environment Variables

| Variable | Purpose |
|---|---|
| `SMOLPC_MODELS_DIR` | Override model root (default: `%LOCALAPPDATA%/SmolPC/models`) |
| `SMOLPC_ENGINE_PORT` | Override host port (default: `19432`) |
| `SMOLPC_FORCE_EP` | Force backend: `cpu` or `dml` |
| `SMOLPC_DML_DEVICE_ID` | Select specific DirectML device |

## Troubleshooting

**Engine running on CPU when GPU is available:**
Check `/engine/status` — look at `active_backend`, `selection_reason`, and `dml_gate_state`. Ensure DirectML model artifacts exist (`dml/model.onnx`, `dml/genai_config.json`) and runtime DLLs are in `src-tauri/libs`.

**Stale engine process blocking startup:**
Kill any running `smolpc-engine-host.exe` and retry. The dev launcher handles this automatically in most cases.

**Reporting integration issues:**
Include: app name/version, OS + hardware, request payload, HTTP status + response body, `/engine/status` snapshot, and any `SMOLPC_FORCE_EP` settings. See the full template in [APP_ONBOARDING_PLAYBOOK.md](docs/APP_ONBOARDING_PLAYBOOK.md#integration-issue-report-template).
