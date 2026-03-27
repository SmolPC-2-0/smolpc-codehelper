# Development Workflow

## Environment Setup

### Prerequisites

- **Rust 1.88.0+** — the workspace MSRV. Install via [rustup](https://rustup.rs/).
- **Node.js 18+** — for the Svelte frontend. Node 23 is used in CI.
- **Windows 11** — the engine's FFI layer and NPU support are Windows-only.
- **8 GB+ RAM** — 16 GB recommended for running the engine alongside an IDE.

### First-Time Setup

```bash
# Clone the repo
git clone <repo-url>
cd CodeHelper

# Install frontend dependencies
cd app && npm ci && cd ..

# Verify everything compiles
cargo check --workspace

# Run the frontend dev server (frontend only, no engine)
cd app && npm run dev
```

To run the full app with engine:

```bash
cd app && npm run tauri:dev
```

This starts the Svelte dev server with hot reload and spawns the Tauri app, which in turn spawns the engine. The dev script handles engine cleanup on exit.

### Runtime DLLs (Development)

The engine needs native DLLs (OpenVINO, ORT, DirectML) to load models. In development, point to them via environment variables:

```powershell
$env:SMOLPC_ORT_BUNDLE_ROOT = "C:\path\to\ort-libs"
$env:SMOLPC_OPENVINO_BUNDLE_ROOT = "C:\path\to\openvino-libs"
```

These are only checked in development mode (`cfg!(debug_assertions)`). In production, DLLs are resolved from the app's `libs/` directory.

### Models (Development)

Override the model directory:

```powershell
$env:SMOLPC_MODELS_DIR = "C:\path\to\models"
```

Each model lives in `{SMOLPC_MODELS_DIR}/{model_id}/{backend}/` (e.g., `qwen2.5-1.5b-instruct/openvino/`).

## Pre-Commit Checks

Run these before pushing:

```bash
# Rust: compile check + lint + tests
cargo check --workspace && cargo clippy --workspace
cargo test -p smolpc-engine-core && cargo test -p smolpc-engine-host

# Frontend: type check + lint
cd app && npm run check && npm run lint
```

CI runs these same checks, so catching issues locally saves a round-trip.

## Debug Environment Variables

| Variable | Purpose | Example |
|---|---|---|
| `SMOLPC_FORCE_EP` | Force a specific backend | `cpu`, `directml`, `openvino_npu` |
| `SMOLPC_MODELS_DIR` | Override model directory | `C:\models` |
| `SMOLPC_ORT_BUNDLE_ROOT` | ORT DLL directory (dev only) | `C:\libs\ort` |
| `SMOLPC_OPENVINO_BUNDLE_ROOT` | OpenVINO DLL directory (dev only) | `C:\libs\openvino` |
| `SMOLPC_ENGINE_TOKEN` | Override engine auth token | `my-dev-token` |
| `SMOLPC_OPENVINO_NPU_MAX_PROMPT_LEN` | NPU input budget | `2048` |
| `SMOLPC_OPENVINO_NPU_MIN_RESPONSE_LEN` | NPU output budget | `1024` |

Note: `SMOLPC_FORCE_EP` set in the shell does NOT reach the engine when spawned by the Tauri app — the supervisor controls the engine's environment. Use the dev script's `-ForceEp` parameter or the frontend's runtime mode preference instead. Setting it only works when running the engine directly with `cargo run -p smolpc-engine-host`.

## Dev Scripts

| Script | Purpose |
|---|---|
| `npm run tauri:dev` | Full app with hot reload and engine cleanup |
| `npm run dev` | Frontend only (no engine, no Tauri) |
| `npm run check` | TypeScript + Svelte type checking |
| `npm run lint` | ESLint + Prettier |
| `cargo run -p smolpc-engine-host` | Engine standalone (for testing with curl) |

## Live Hardware Testing

After changing generation config, backend selection, or model loading, validate on real hardware:

```powershell
# Start engine with forced backend
$env:SMOLPC_FORCE_EP = "openvino_npu"
$env:SMOLPC_ENGINE_TOKEN = "test-token"
cargo run -p smolpc-engine-host

# In another terminal, send a test request
curl -s http://localhost:19432/v1/chat/completions `
  -H "Authorization: Bearer test-token" `
  -H "Content-Type: application/json" `
  -d '{"messages":[{"role":"user","content":"Write a Python function that checks if a number is prime"}],"stream":true,"max_tokens":256}'
```

Verify:
- Tokens stream incrementally (not a single dump)
- Output is coherent code
- Generation stops naturally (not runaway)
- No "unknown exception" or crash errors

Repeat for each backend that was affected by the change.

## Git Conventions

### Conventional Commits

All commits use the conventional commit format with a scope:

```
feat(engine): add NPU compilation caching
fix(openvino): skip min_new_tokens on 2026.0.0
docs: update ARCHITECTURE.md with connector patterns
refactor(gimp): extract fast path heuristics to module
test(blender): add RAG retrieval unit tests
chore(ci): add Rust security audit job
```

Common scopes: `engine`, `openvino`, `directml`, `app`, `gimp`, `blender`, `libreoffice`, `ci`, `setup`, `tts`.

### Branch Strategy

- **`main`** is the known-good branch. It should always compile, pass CI, and work on all test machines.
- **Feature/fix branches** are created for investigation and development. Name them descriptively: `feat/npu-compilation-cache`, `fix/directml-igpu-fallback`.
- **Merge when live-tested.** A branch is merged to main only after the change has been validated on real hardware (for engine changes) or with the full pre-commit check suite (for frontend/connector changes).
- **Don't dismiss broken checks as pre-existing.** If verification fails in your branch, fix it before merging — even if the issue existed before your change.

### No AI Attribution

Do not add `Co-Authored-By` lines to commits. Do not add "Generated with Claude Code" or similar attribution to PR descriptions.

## MSRV Policy

The workspace minimum supported Rust version is **1.88.0**. CI tests on both MSRV and latest stable.

Why it is pinned:
- Ensures the codebase compiles on the version installed on development machines
- Prevents accidental use of newer language features that would break CI
- The MSRV is updated deliberately, not by accident

## Architectural Boundaries

9 boundary rules are enforced in CI via `scripts/check-boundaries.ps1`:

**Files that must not exist** (legacy code that was intentionally removed):
- `app/src-tauri/src/inference` — app must not own inference
- `app/src-tauri/src/models` — app must not own model management
- `app/src-tauri/src/commands/ollama.rs` — Ollama integration removed
- `app/src/lib/stores/ollama.svelte.ts` — Ollama frontend removed
- `app/src/lib/types/ollama.ts` — Ollama types removed

**Content that must not appear:**
- `commands/mod.rs` must not contain `ollama` — no Ollama references in command routing
- `app/src-tauri/Cargo.toml` must not depend on `smolpc-engine-host` — the app uses `smolpc-engine-client` only
- `app/src-tauri/src/**/*.rs` must not import `smolpc_engine_host` — process isolation boundary

The engine-host boundary is the most important: if the app imports engine-host directly, it defeats the purpose of running the engine as a separate process. An FFI crash in engine-host would take down the app.

## Coding Conventions

### Rust

- **Svelte 5 runes only** in the frontend. No `writable` / `readable` from `svelte/store`. Use `$state`, `$derived`, `$effect`.
- **Tailwind 4.** No `@apply` — use utility classes directly in templates.
- **Tauri Channels for streaming.** Use `tauri::ipc::Channel<T>` (command-scoped, ordered delivery), not global Tauri Events.
- **Engine host owns all backend policy.** The Tauri app and launcher consume engine status only — they must not rank backends or override engine selection.
- **Engine lifecycle owned by EngineSupervisor.** Tauri commands never spawn or kill the engine directly — they send commands via mpsc channel and read state via watch channel.
- **DLL loading is centralized.** All `Library::new()` calls in `runtime_loading.rs`. A CI test enforces this.
- **OpenVINO models must be IR format.** The OpenVINO lane expects `.xml` + `.bin` artifacts, not ONNX. The `openvino/manifest.json` is the artifact readiness gate.

### Frontend

- **camelCase for wire format.** All DTOs use `#[serde(rename_all = "camelCase")]`. The type contract tests in `smolpc-assistant-types` enforce this.
- **API layer in `unified.ts`.** Every frontend → backend call goes through `app/src/lib/api/unified.ts`. No direct `tauri.invoke()` calls in components.
- **Stores per domain.** Separate stores for inference, chats, mode, setup, provisioning, hardware, voice, settings, UI.

## CI Pipeline Overview

7 jobs run on every push (see [TESTING.md](TESTING.md) for details):

1. **Frontend Quality** — TypeScript check, Svelte check, npm audit
2. **Boundary Enforcement** — architectural rule validation
3. **Engine Tests (MSRV)** — Rust 1.88.0 test suite (7 crates)
4. **Engine Tests (Stable)** — latest Rust test suite
5. **Tauri Build Check** — `cargo check -p smolpc-desktop`
6. **Incremental Style Gates** — Prettier, ESLint, rustfmt on changed files only
7. **Rust Security Audit** — `cargo audit`

The release pipeline additionally validates 8 required DLL/binary artifacts and enforces a minimum installer size (>200 MB).
