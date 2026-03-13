# CLAUDE.md

This file guides Claude Code sessions working on SmolPC Code Helper.

**Last Updated:** 2026-03-13
**Current Phase:** Unified Assistant — Specification & Parallel Implementation
**Branch:** `docs/unified-assistant-spec` (specs), future: `feature/*` workstreams

---

## Collaboration Principles

**We are academic partners working toward a production app.** This means:

- **Think critically** - Challenge my assumptions, correct mistakes, propose alternatives
- **Never be a yes-man** - Disagreement that improves the project is valuable
- **Ask, don't assume** - If uncertain about anything, ask follow-up questions
- **State uncertainty explicitly** - If you're not sure, say so clearly
- **Focus on the goal** - Productivity over pleasing

---

## Current State Summary

_Updated at end of each session. Provides immediate context without reading external files._

**Phase**: Unified Assistant — specification complete, parallel implementation ready to begin
**Branch**: `docs/unified-assistant-spec` (this branch — specifications)
**Last Session**: 2026-03-13 — Created 12 specification documents crystallizing all research findings

**What's Done**:

- Shared engine architecture fully implemented: `smolpc-engine-host` (axum HTTP on :19432) + `smolpc-engine-core` + `smolpc-engine-client`
- Three inference backends: ORT CPU, DirectML GenAI FFI, OpenVINO GenAI NPU FFI
- Backend selection with fingerprint-based caching and lane readiness model
- GIMP assistant migration complete (PR #58 merged) — engine-client integration working
- Blender assistant production-ready (v7.0.0) with HTTP bridge + shared engine
- **12 specification documents** for unified assistant in `docs/unified-assistant-spec/`
- Research completed: model selection, MCP ecosystem, VS Code extension feasibility, packaging, model export pipeline

**What's In Progress**:

- OpenVINO DLL bundle + model artifact staging (needs Intel NPU hardware to validate)
- CPU runtime consolidation (migrating off raw `ort` crate to onnxruntime-genai or native OpenVINO)
- Final model selection (benchmarking blocked on target hardware)

**Next Up — Parallel Workstreams** (branch from this spec branch):

| Workstream | Branch | Description |
|---|---|---|
| Unified frontend | `feature/unified-frontend` | Mode dropdown, shared chat UI, per-mode config |
| VS Code extension | `feature/vscode-extension` | InlineCompletionProvider + webview chat |
| MCP client | `feature/mcp-client` | Generalized MCP client (stdio + TCP) |
| Model export | `feature/model-export` | ONNX + OpenVINO INT4 export pipeline |
| Installer | `feature/packaging` | NSIS installer, DLL bundling, uv sidecar |

**Blockers**: OpenVINO validation needs Intel Core Ultra laptop with NPU driver ≥ 32.0.100.3104

**Full specs**: See `docs/unified-assistant-spec/README.md` for reading order

---

## Session Protocol

### Trigger Phrases

When user says **"new session"**, **"initialize"**, **"start session"**, or similar:
**IMMEDIATELY execute the Session Startup steps below.**

### Session Startup (Two Stages)

**Stage 1 - Orient** (do immediately):

1. Read the Current State Summary above (already loaded)
2. Run `git log --oneline -5` to see recent changes
3. Briefly summarize current state to user
4. **Ask: "What are we focusing on this session?"**

**Stage 2 - Load Context** (after user gives goals):

1. Based on the goal, read relevant documentation:
   - OpenVINO work → `docs/openvino-native-genai/PLAN.md` + `MODEL_STRATEGY.md`
   - Benchmark work → `engine/crates/smolpc-engine-core/src/inference/backend.rs`
   - Bug fix → relevant source file directly
   - Architecture questions → `docs/openvino-native-genai/ENGINE_SURFACE_TARGET.md`
2. If task is non-trivial (>2 steps), enter plan mode
3. Proceed with workflow

### During Session

- Record mistakes and corrections in the Learnings section below
- Use task list for multi-step work
- Enter plan mode for non-trivial tasks (>2 steps)

### Session End

When user says **"end session"**, **"wrap up"**, **"that's all"**, or similar:

1. **Update Current State Summary** above with new status
2. **Update Learnings** below if any corrections were made
3. **Ask about committing** if changes were made

---

## Task Workflow

### When to Use Plan Mode

Use `EnterPlanMode` for tasks that:

- Require more than 2 steps
- Touch multiple files
- Involve architectural decisions
- Have unclear requirements

Skip plan mode for: typo fixes, single-line changes, simple additions.

### Workflow with Specialist Agents

```
1. EXPLORE (if needed)
   └─► Explore agent: Understand relevant codebase areas

2. PLAN (for non-trivial tasks)
   └─► Plan agent: Design implementation approach
   └─► Present plan with reasoning → User approves

3. RESEARCH (if needed)
   └─► research-analyst: External docs / OpenVINO API
   └─► context7: Library documentation

4. IMPLEMENT (parallel where appropriate)
   └─► rust-engineer: Rust engine work
   └─► typescript-pro: TypeScript/Svelte frontend
   └─► frontend-developer: UI components

5. VERIFY
   └─► Run tests, check compilation
   └─► Update state files
```

---

## Git Workflow

### Branch Strategy

**Before starting any fix or feature:**
1. Create a new branch from current working branch
2. Use naming convention: `fix/short-description` or `feature/short-description`

### During Work

**Commit consistently** — commit after each logical step, before risky changes.

```bash
git add <specific-files>
git commit -m "feat: extend benchmark comparison for three-way openvino/dml/cpu"
```

### Session End

**Open a PR at the end of the session** (if work is ready for review):

```bash
git push -u origin feature/openvino-benchmark
gh pr create --title "feat: three-way benchmark comparison" --body "..."
```

### Branch Naming

| Type | Pattern | Example |
|------|---------|---------|
| Bug fix | `fix/description` | `fix/openvino-preflight-timeout` |
| Feature | `feature/description` | `feature/unified-frontend` |
| Refactor | `refactor/description` | `refactor/bundle-resolution` |
| Engine | `engine/description` | `engine/openvino-runtime-activation` |
| Docs/specs | `docs/description` | `docs/unified-assistant-spec` |

**Unified assistant workstreams** (branch from `docs/unified-assistant-spec`):

| Workstream | Branch |
|---|---|
| Frontend rewrite | `feature/unified-frontend` |
| VS Code extension | `feature/vscode-extension` |
| MCP client | `feature/mcp-client` |
| Model export pipeline | `feature/model-export` |
| Installer/packaging | `feature/packaging` |

---

## Project Overview

SmolPC Code Helper is an **offline AI coding and creative assistant** for secondary school students (ages 11-18).

**Key Principles:**

- **Offline-First**: No cloud, no telemetry
- **Privacy-First**: Student data stays local (GDPR/FERPA)
- **Budget Hardware**: Must run on 8GB RAM minimum, primary KPI is weak Intel laptops
- **Partnership Requirements**: Intel NPU (OpenVINO) is the primary acceleration target on Windows

### Unified Assistant Vision

The project is expanding from a code-only assistant to a **unified assistant** with six modes:

| Mode | Integration | Status |
|---|---|---|
| Code | VS Code extension (InlineCompletionProvider + webview chat) | Spec complete |
| GIMP | `maorcc/gimp-mcp` MCP server | PR #58 merged |
| Blender | Hybrid HTTP bridge + `blender-mcp` | v7.0.0 production |
| Writer | `patrup/mcp-libre` MCP server | Spec complete |
| Calc | `patrup/mcp-libre` MCP server | Spec complete |
| Impress | `patrup/mcp-libre` MCP server | Spec complete |

Two products share one engine: a **Tauri 2 desktop app** (all modes via dropdown) and a **VS Code extension** (Code mode only). Both connect to `smolpc-engine-host` on `:19432`.

### Current Architecture (Shared Engine)

```
┌──────────────────────────────────────────────────────────────┐
│                  Code Helper (Tauri 2.6.2)                   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │              Frontend (Svelte 5)                        │  │
│  └─────────────────────────┬──────────────────────────────┘  │
│                            │ Tauri IPC (Channel streaming)   │
│  ┌─────────────────────────┴──────────────────────────────┐  │
│  │  Tauri Backend (apps/codehelper/src-tauri)              │  │
│  │  commands/ → smolpc-engine-client                       │  │
│  └─────────────────────────┬──────────────────────────────┘  │
└────────────────────────────┼─────────────────────────────────┘
                             │ HTTP + SSE
┌────────────────────────────┴─────────────────────────────────┐
│          smolpc-engine-host  (axum HTTP server)               │
│  POST /engine/load   → backend selection + preflight          │
│  POST /v1/chat/completions → streaming inference              │
│  GET  /engine/status → lane readiness + backend status        │
│                                                               │
│  Backend selection:  openvino_npu → directml → cpu            │
│                                                               │
│  ┌─────────────────────────────────────────────────────┐     │
│  │           smolpc-engine-core                         │     │
│  │  InferenceRuntimeAdapter (dispatch enum)             │     │
│  │  ├── Ort { Generator }          ← CPU via ort crate  │     │
│  │  ├── GenAiDirectMl { ... }      ← DML native FFI     │     │
│  │  └── OpenVinoGenAiNpu { ... }   ← OV GenAI native    │     │
│  └─────────────────────────────────────────────────────┘     │
└──────────────────────────────────────────────────────────────┘
```

### Runtime Bundle Locations

```
apps/codehelper/src-tauri/libs/
  onnxruntime.dll                   ← ORT CPU + DirectML
  onnxruntime_providers_shared.dll
  onnxruntime-genai.dll
  DirectML.dll
  openvino/                         ← OpenVINO (NOT YET PRESENT)
    openvino.dll
    openvino_c.dll
    openvino_intel_npu_plugin.dll
    openvino_intel_cpu_plugin.dll
    openvino_ir_frontend.dll
    openvino_genai.dll
    openvino_tokenizers.dll
    tbb12.dll
```

### Model Artifact Layout

```
%LOCALAPPDATA%/SmolPC/models/<model_id>/
  cpu/
    model.onnx
    tokenizer.json
  dml/
    model.onnx
    genai_config.json
    tokenizer.json
  openvino_npu/
    manifest.json          ← lists required files
    model.xml              ← OpenVINO IR graph
    model.bin              ← model weights
    tokenizer.json
    generation_config.json
    config.json
```

OpenVINO GenAI handles tokenization itself — no Rust tokenizer needed for the NPU lane.

### Detailed Documentation

**Unified Assistant Specs** (canonical — read these first):

| Topic | File |
|-------|------|
| Spec index & reading order | `docs/unified-assistant-spec/README.md` |
| System architecture | `docs/unified-assistant-spec/ARCHITECTURE.md` |
| Frontend (Tauri app) | `docs/unified-assistant-spec/FRONTEND_SPEC.md` |
| VS Code extension | `docs/unified-assistant-spec/VSCODE_EXTENSION_SPEC.md` |
| MCP integration | `docs/unified-assistant-spec/MCP_INTEGRATION.md` |
| Model selection & export | `docs/unified-assistant-spec/MODEL_STRATEGY.md` |
| Packaging & distribution | `docs/unified-assistant-spec/PACKAGING.md` |
| Git workflow | `docs/unified-assistant-spec/GIT_WORKFLOW.md` |
| Code conventions | `docs/unified-assistant-spec/CODE_CONVENTIONS.md` |
| Learnings (living) | `docs/unified-assistant-spec/LEARNINGS.md` |
| Current state (living) | `docs/unified-assistant-spec/CURRENT_STATE.md` |
| External resources | `docs/unified-assistant-spec/RESOURCES.md` |

**Legacy docs** (on `main` branch, for historical context only):

| Topic | File |
|-------|------|
| OpenVINO plan | `docs/openvino-native-genai/PLAN.md` |
| Engine API surface | `docs/openvino-native-genai/ENGINE_SURFACE_TARGET.md` |

---

## Quick Reference

### Commands

```bash
# Engine (run from repo root)
cargo run -p smolpc-engine-host                  # Start engine server
cargo test -p smolpc-engine-host                 # Engine tests
cargo test -p smolpc-engine-core                 # Core inference tests
cargo check --workspace                          # Check all crates
cargo clippy --workspace                         # Lint all crates

# Development overrides (engine picks these up automatically in debug builds)
SMOLPC_ORT_BUNDLE_ROOT=/path/to/ort/libs         # Override ORT DLL directory
SMOLPC_OPENVINO_BUNDLE_ROOT=/path/to/ov/libs     # Override OpenVINO DLL directory
SMOLPC_MODELS_DIR=/path/to/models                # Override models directory

# Tauri app (from apps/codehelper/)
npm run tauri dev          # Full app with hot reload
npm run dev                # Frontend only
npm run check              # TypeScript check
npm run lint               # Lint
```

### Key File Locations

**Engine Host:**

- `engine/crates/smolpc-engine-host/src/main.rs` — HTTP server, backend selection, load/inference handlers
- `engine/crates/smolpc-engine-host/src/openvino.rs` — OpenVINO startup probe, artifact check, preflight runner
- `engine/crates/smolpc-engine-host/src/runtime_bundles.rs` — DLL bundle path resolution, bundle validation

**Engine Core:**

- `engine/crates/smolpc-engine-core/src/inference/runtime_adapter.rs` — `InferenceRuntimeAdapter` dispatch enum
- `engine/crates/smolpc-engine-core/src/inference/runtime_loading.rs` — `OrtRuntimeLoader`, `OpenVinoRuntimeLoader` (all DLL loading lives here)
- `engine/crates/smolpc-engine-core/src/inference/genai/openvino.rs` — OpenVINO GenAI native FFI
- `engine/crates/smolpc-engine-core/src/inference/genai/directml.rs` — DirectML GenAI native FFI
- `engine/crates/smolpc-engine-core/src/inference/generator.rs` — ORT CPU autoregressive loop
- `engine/crates/smolpc-engine-core/src/inference/backend.rs` — `InferenceBackend`, `BackendStatus`, benchmark types
- `engine/crates/smolpc-engine-core/src/inference/backend_store.rs` — Persisted backend decision records
- `engine/crates/smolpc-engine-core/src/models/loader.rs` — Model path resolution per lane
- `engine/crates/smolpc-engine-core/src/models/registry.rs` — Model registry (qwen3-4b, qwen2.5-coder-1.5b)

**Engine Client + Tauri App:**

- `engine/crates/smolpc-engine-client/src/lib.rs` — HTTP client wrapping engine API
- `apps/codehelper/src-tauri/src/commands/engine_client_adapter.rs` — Tauri command → engine client bridge
- `apps/codehelper/src-tauri/libs/` — ORT DLL bundle (ORT present, `openvino/` missing)

**Frontend (TypeScript/Svelte):**

- `apps/codehelper/src/App.svelte` — Main app
- `apps/codehelper/src/lib/stores/inference.svelte.ts` — Inference state
- `apps/codehelper/src/lib/types/inference.ts` — Type definitions

---

## Critical Conventions

### DLL Loading Must Stay Centralized

All `Library::new()` / `load_with_flags()` calls MUST live in `runtime_loading.rs`. This is enforced by a source-invariant test (`runtime_loading_is_centralized`) that fails if any other file in the crate calls them. Do not bypass this.

### OpenVINO DLL Load Order

OpenVINO DLLs must be loaded in dependency order or Windows will fail to resolve symbols:
```
tbb12.dll → openvino.dll → openvino_c.dll → openvino_ir_frontend.dll
→ openvino_intel_cpu_plugin.dll → openvino_intel_npu_plugin.dll
→ openvino_tokenizers.dll → openvino_genai.dll
```

### Bundle Fingerprint Invalidates Cached Decisions

`RuntimeBundleFingerprint` hashes each DLL's path, existence, size, and mtime. Any DLL change (update, replace) invalidates all cached backend decisions automatically — no manual cache clearing needed.

### OpenVINO Preflight Semantics

- **Preflight timeout** → `temporary_fallback`, NOT a persisted negative decision. Does not overwrite a prior good persisted OpenVINO record.
- **Preflight failure** → `OpenVinoPreflightFailed`, falls through to DirectML/CPU.
- **Successful preflight** → generator retained in memory, `runtime_engine = "ov_genai_npu"` set, decision persisted.

### Production vs Development Bundle Resolution

- **Production** (release build): resolves DLLs from `<exe_dir>/libs/` only. Env overrides are ignored.
- **Development** (debug build): `SMOLPC_ORT_BUNDLE_ROOT` and `SMOLPC_OPENVINO_BUNDLE_ROOT` take priority if absolute paths. Falls back through workspace dev paths.

### OpenVINO Model Must Be OpenVINO IR, Not ONNX

The OpenVINO lane uses `openvino_genai.dll` directly — it expects `.xml` + `.bin` IR artifacts. Do not point it at an ONNX file. The `openvino_npu/manifest.json` must enumerate the required files.

### Svelte 5 Runes (NOT Svelte 4 Stores)

```typescript
// Correct - Svelte 5
let data = $state<T>(initial);
export const store = {
    get data() { return data; },
    method() { data = newValue; }
};

// Wrong - Svelte 4
import { writable } from 'svelte/store'; // DON'T USE
```

### Tailwind 4

- **DO NOT use `@apply`** — Not supported in Tailwind 4
- Use utility classes directly in templates

### Tauri Streaming Pattern (Channels)

The Tauri app still uses Channels for token streaming — the engine client receives SSE from the engine host and forwards tokens through a Tauri Channel to the frontend.

```rust
// Backend command: accept Channel, forward engine tokens through it
#[tauri::command]
async fn inference_generate(on_token: Channel<String>, ...) -> Result<Metrics, String> {
    engine_client.stream(prompt, |token| on_token.send(token)).await
}
```

---

## Learnings

Corrections and patterns discovered during development. Categorized for easy reference.

### Engine Architecture

- **Engine host owns all backend policy**: launcher/Tauri app must not rank backends or override engine selection logic. They consume status only.
- **OpenVINO GenAI handles its own tokenization**: the NPU lane calls `ov_genai_llm_pipeline_create` with a model directory; OpenVINO loads tokenizer files itself. The Rust `TokenizerWrapper` is not involved.
- **Selection profile `openvino_native_v1` invalidates stale records**: changing this constant in `main.rs:OPENVINO_SELECTION_PROFILE` forces full re-evaluation for all existing cached decisions.
- **Bundle fingerprint auto-invalidates on DLL change**: updating a DLL file changes its mtime, which changes the hash, which causes a new fingerprint, which forces fresh backend selection.

### OpenVINO / NPU

- **OpenVINO 2026.0.0 is the pinned tuple**: `openvino`, `openvino_genai`, and `openvino_tokenizers` versions must be kept in sync — mixing versions breaks ABI.
- **Use INT4, not NF4, for broad NPU compatibility**: NF4 quantization is only supported on Intel Core Ultra Series 2 NPU and later. INT4 works on a wider range of devices.
- **NPU driver floor 32.0.100.3104 is a troubleshooting recommendation, not a hard gate**: the engine classifies it as `openvino_npu_driver_recommended_update` (non-blocking) rather than a fatal failure.
- **NPU compilation is slow on first load, fast after caching**: `CACHE_DIR` is passed to the pipeline constructor. After the first run, subsequent loads reuse the compiled blob.
- **OpenVINO DLL load order matters on Windows**: tbb12 must be loaded before openvino core, plugins before genai. The loader in `runtime_loading.rs` already does this in the right order.

### Rust/Tauri

- **Use Channels over Events for streaming**: `tauri::ipc::Channel<T>` is command-scoped and ordered. Events are global broadcast. Channels prevent listener race conditions.
- **Use `OnceLock<Result>` over `Once` for fallible init**: `Once::call_once` doesn't return values. `OnceLock::get_or_init()` is the right pattern for cacheable fallible initialization.
- **Use dedicated `AtomicBool` over `try_lock()` for state tracking**: `try_lock()` creates TOCTOU races.

### ONNX/ORT

- **ORT crate version is pinned to `2.0.0-rc.11`** (hardcoded as `ORT_CRATE_VERSION` constant). Do not upgrade without updating the constant and re-validating the DML lane.
- **Qwen2.5 has TWO stop tokens**: `<|endoftext|>` (151643) + `<|im_end|>` (151645). Both must be checked.
- **ChatML is mandatory for chat behavior**: Without `<|im_start|>` formatting, Qwen regurgitates pretraining data.

---

## Common Pitfalls

1. **Adding DLL loading outside `runtime_loading.rs`** — the source-invariant test will fail at CI
2. **Mixing OpenVINO component versions** — `openvino`, `openvino_genai`, `openvino_tokenizers` must be the same release tuple
3. **Pointing OpenVINO lane at an ONNX file** — it needs OpenVINO IR (`.xml` + `.bin`), not ONNX
4. **Forgetting `manifest.json` in `openvino_npu/`** — the engine uses this as the artifact readiness gate
5. **Using Svelte 4 patterns** — this project uses Svelte 5 runes only
6. **Using `@apply`** — Tailwind 4 doesn't support it
7. **Changing backend policy in Tauri/launcher** — engine host owns all selection logic
8. **Persisting timeout results as final decisions** — preflight timeout must stay `temporary_fallback`

---

## Before Committing

```bash
cargo check --workspace           # All Rust crates compile
cargo clippy --workspace          # No lint errors
cargo test -p smolpc-engine-core  # Core tests pass
cargo test -p smolpc-engine-host  # Host tests pass
cd apps/codehelper && npm run check && npm run lint  # TypeScript + lint
```

Commit message format (Conventional Commits):

```
feat(engine): extend benchmark comparison for three-way openvino/dml/cpu
fix(openvino): correct DLL load order for tbb dependency
docs(engine): update PLAN.md with validation results
```

---

## Resources

**Full resource list with caveats**: See `docs/unified-assistant-spec/RESOURCES.md`

**Quick links:**

- [OpenVINO 2026.0.0 Release](https://github.com/openvinotoolkit/openvino/releases/tag/2026.0.0)
- [OpenVINO GenAI NPU Guide](https://docs.openvino.ai/2025/openvino-workflow-generative/inference-with-genai/inference-with-genai-on-npu.html)
- [OpenVINO Local Distribution](https://docs.openvino.ai/2025/openvino-workflow/deployment-locally/local-distribution-libraries.html)
- [ONNX Runtime GenAI — Build Models](https://onnxruntime.ai/docs/genai/howto/build-model.html)
- [ONNX Runtime GenAI — Config Reference](https://onnxruntime.ai/docs/genai/reference/config.html)
- [Tauri 2 Docs](https://v2.tauri.app/)
- [Tauri Windows Installer](https://v2.tauri.app/distribute/windows-installer/)
- [Tauri Code Signing](https://v2.tauri.app/distribute/sign/windows/)
- [Svelte 5 Runes](https://svelte.dev/docs/svelte/what-are-runes)
- [Tailwind CSS 4](https://tailwindcss.com/docs)
- [VS Code Extension API](https://code.visualstudio.com/api)
- [ONNX Runtime Rust (ort)](https://docs.rs/ort/)
- [GIMP MCP Server](https://github.com/maorcc/gimp-mcp)
- [Blender MCP Server](https://github.com/ahujasid/blender-mcp)
- [LibreOffice MCP Server](https://github.com/patrup/mcp-libre)
- [BFCL Leaderboard](https://gorilla.cs.berkeley.edu/leaderboard.html)
- [Azure Trusted Signing](https://azure.microsoft.com/en-in/pricing/details/trusted-signing/)
