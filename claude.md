# CLAUDE.md

This file guides Claude Code sessions working on SmolPC Code Helper.

**Last Updated:** 2026-03-12
**Current Phase:** Launcher MVP Complete → Engine integration testing
**Branch:** `feature/blenderhelper`

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

**Phase**: Launcher MVP complete → Engine integration testing
**Branch**: `feature/blenderhelper`
**Last Session**: 2026-03-12 - Launcher app scaffold + build + DLL bundling
**Apps**: CodeHelper (working), Blender Assistant (standalone), GIMP/LibreOffice (skeleton)

**What's Working**:

- ONNX inference via `ort` crate with Qwen2.5-Coder-1.5B (CodeHelper)
- KV Cache with Attention Sinks (~8 tok/s on CPU)
- Streaming generation via Tauri Channels (race condition fixed)
- ChatML prompt template + multi-stop-token detection
- **Launcher app** (`launcher/`) — standalone Tauri app at `com.smolpc.launcher`
  - Svelte 5 frontend: app list, running status dots, engine status bar
  - Rust backend: extracted from CodeHelper's launcher modules
  - Loads apps from `apps.manifest.json`, shows running state via sysinfo polling
  - Engine demand-started on first app click (not on launcher open)
  - Bundled ONNX Runtime DLLs (v1.23) — auto-configures `ORT_DYLIB_PATH` + `SMOLPC_GENAI_DYLIB`
  - Builds to NSIS/MSI installers via `tauri build`
  - 5 Rust unit tests passing, 0 TypeScript errors

**Next Up**:

1. End-to-end test: launcher → engine host → CodeHelper launch flow
2. Engine host startup issues (model asset paths for fresh machines)
3. Bundle engine host as Tauri sidecar (currently found via workspace `target/` path)
4. Phase 2: Execution Provider abstraction (trait-based)
5. Intel NPU detection + OpenVINO EP

**Blockers**:
- Engine host finds DLLs via exe-relative paths; launcher sets env vars but engine host must be co-located with `libs/` or receive correct `--resource-dir`
- `tauri build` raw exe has `cfg(dev)=true` when run outside `tauri build` — use installer or `tauri dev`
- Stale `smolpc-engine-host.exe` processes hold port 19432 — must kill before testing

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
   - Phase work → Read relevant `docs/new_onnx_plan/PHASE-X.md`
   - Bug fix → Read relevant source files
   - New feature → Read `CURRENT_STATE.md` for full context
2. If task is non-trivial (>2 steps), enter plan mode
3. Proceed with workflow

### During Session

- Record mistakes and corrections in the Learnings section below
- Use task list for multi-step work
- Enter plan mode for non-trivial tasks (>2 steps)

### Session End

When user says **"end session"**, **"wrap up"**, **"that's all"**, or similar:

1. **Update Current State Summary** above with new status
2. **Update SESSION_LOG.md** - Add session entry
3. **Update Learnings** below if any corrections were made
4. **Update MEMORY.md** - Key facts for future reference
5. **Ask about committing** if changes were made

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
   └─► voltagent-research:research-analyst: External docs
   └─► context7: Library documentation

4. IMPLEMENT (parallel where appropriate)
   └─► voltagent-lang:rust-engineer: Rust/Tauri backend
   └─► voltagent-lang:typescript-pro: TypeScript frontend
   └─► code-reviewer: Review as code is produced
   └─► test-automator: Unit tests for new code

5. VERIFY
   └─► Run tests, check compilation
   └─► Update state files
```

### Relevant Agents for This Project

| Purpose                 | Agent                                   |
| ----------------------- | --------------------------------------- |
| Codebase exploration    | `Explore`                               |
| Implementation planning | `Plan`                                  |
| Rust backend            | `voltagent-lang:rust-engineer`          |
| TypeScript/Svelte       | `voltagent-lang:typescript-pro`         |
| UI components           | `voltagent-core-dev:frontend-developer` |
| Code review             | `code-reviewer`                         |
| Tests                   | `test-automator`                        |
| External research       | `voltagent-research:research-analyst`   |

---

## Git Workflow

### Branch Strategy

**Before starting any fix or feature:**
1. Create a new branch from current working branch
2. Use naming convention: `fix/short-description` or `feature/short-description`

```bash
git checkout -b fix/event-race-condition
```

### During Work

**Commit consistently** - Don't wait until the end:
- Commit after completing each logical step
- Commit before making risky changes (easy rollback)
- Use conventional commit messages

```bash
git add <specific-files>
git commit -m "fix: set up listeners before invoke call"
```

### Session End

**Open a PR at the end of the session** (if work is ready for review):
1. Push branch to remote
2. Create PR with summary of changes
3. Reference any related issues or plans

```bash
git push -u origin fix/event-race-condition
gh pr create --title "fix: resolve event listener race condition" --body "..."
```

**If work is incomplete:**
- Commit current progress with `WIP:` prefix
- Note in SESSION_LOG.md what's left to do
- Don't open PR until ready

### Branch Naming

| Type | Pattern | Example |
|------|---------|---------|
| Bug fix | `fix/description` | `fix/event-race-condition` |
| Feature | `feature/description` | `feature/openvino-ep` |
| Refactor | `refactor/description` | `refactor/cache-abstraction` |
| Docs | `docs/description` | `docs/phase2-update` |

---

## Project Overview

SmolPC Code Helper is an **offline AI coding assistant** for secondary school students (ages 11-18).

**Key Principles:**

- **Offline-First**: No cloud, no telemetry
- **Privacy-First**: Student data stays local (GDPR/FERPA)
- **Budget Hardware**: Must run on 8GB RAM minimum
- **Partnership Requirements**: Intel NPU (OpenVINO), Windows primary

### Current Architecture

```
┌───────────────────────────────────────────────────────────┐
│              SmolPC Launcher (Tauri, port 1421)            │
│  App list + engine status bar + launch/focus orchestration │
│  Bundles: onnxruntime.dll, DirectML.dll, genai DLL        │
│  Sets ORT_DYLIB_PATH + SMOLPC_GENAI_DYLIB at startup      │
└──────────┬──────────────────────┬─────────────────────────┘
           │ spawns (first click) │ launches
           ▼                      ▼
┌──────────────────┐   ┌──────────────────────────────────┐
│  Engine Host     │   │  Helper Apps                     │
│  (HTTP, :19432)  │   │  ├─ Code Helper (Tauri, :1420)   │
│  /engine/*       │   │  ├─ Blender Assistant (standalone)│
│  /v1/chat/*      │   │  ├─ GIMP Assistant (planned)      │
│  ONNX inference  │   │  └─ LibreOffice Assistant (plan.) │
└──────────────────┘   └──────────────────────────────────┘
```

**Current Performance:** ~8 tok/s on CPU with KV cache

### Detailed Documentation

| Topic               | File                                       |
| ------------------- | ------------------------------------------ |
| Full PRD            | `docs/new_onnx_plan/PRD.md`                |
| Current state       | `docs/new_onnx_plan/CURRENT_STATE.md`      |
| Phase 2 (GPU/NPU)   | `docs/new_onnx_plan/PHASE-2.md`            |
| KV cache benchmarks | `docs/new_onnx_plan/KV_CACHE_BENCHMARK.md` |

---

## Quick Reference

### Commands

```bash
# Development
npm run tauri:dev                              # CodeHelper with hot reload
cd launcher && npx tauri dev                   # Launcher with hot reload
npm run launcher:dev                           # Launcher (from root)

# Build (production)
cd launcher && npx tauri build                 # Launcher → NSIS/MSI installers
cargo build --release -p smolpc-engine-host    # Engine host (must build separately)

# Checks
npm run check                                  # CodeHelper TypeScript
npm run check --workspace launcher             # Launcher TypeScript
cargo check -p smolpc-launcher                 # Launcher Rust
cargo clippy -p smolpc-launcher                # Launcher lint
cargo test -p smolpc-launcher                  # Launcher tests (5 unit tests)

# Kill stale engine (important before re-testing)
taskkill /F /IM smolpc-engine-host.exe
```

### Key File Locations

**Launcher (Tauri app at `launcher/`):**

- `launcher/src-tauri/src/lib.rs` - Tauri setup, DLL env config, command registration
- `launcher/src-tauri/src/launcher/orchestrator.rs` - Engine spawn, app launch/focus
- `launcher/src-tauri/src/launcher/catalog.rs` - Manifest loading/validation
- `launcher/src-tauri/src/launcher/types.rs` - DTOs (icon + is_running fields)
- `launcher/src-tauri/src/commands/launcher.rs` - List apps, launch/focus commands
- `launcher/src-tauri/src/commands/engine.rs` - Engine health polling
- `launcher/src-tauri/resources/launcher/apps.manifest.json` - Dev app registry
- `launcher/src-tauri/libs/` - Bundled DLLs (gitignored, copy from blender-assistant)
- `launcher/src/App.svelte` - Root UI
- `launcher/src/lib/stores/launcher.svelte.ts` - Svelte 5 runes store (polling, launch)
- `launcher/src/lib/components/` - AppCard, AppList, EngineStatusBar

**CodeHelper (Tauri app at `apps/codehelper/`):**

- `apps/codehelper/src-tauri/src/lib.rs` - Tauri setup, command registration
- `apps/codehelper/src-tauri/src/commands/inference.rs` - ONNX inference commands
- `apps/codehelper/src-tauri/src/inference/` - Core inference engine
- `apps/codehelper/src/App.svelte` - Main app, uses inferenceStore

**Engine:**

- `engine/crates/smolpc-engine-host/` - HTTP server (:19432), ONNX inference
- `engine/crates/smolpc-engine-client/` - Rust client, connect_or_spawn logic
- `engine/crates/smolpc-engine-core/` - Inference, model registry, DLL loading

---

## Critical Conventions

### Type Synchronization (Rust ↔ TypeScript)

Types must match exactly:

- Rust: `src-tauri/src/inference/types.rs`
- TypeScript: `src/lib/types/inference.ts`
- `Option<T>` in Rust = `T | null` in TypeScript

### Svelte 5 Runes (NOT Svelte 4 Stores)

```typescript
// Correct - Svelte 5
let data = $state<T>(initial);
export const store = {
	get data() {
		return data;
	},
	method() {
		data = newValue;
	}
};

// Wrong - Svelte 4
import { writable } from 'svelte/store'; // DON'T USE
```

### Tailwind 4

- **DO NOT use `@apply`** - Not supported in Tailwind 4
- Use utility classes directly in templates

### Tauri Streaming Pattern (Channels)

```rust
// Backend: accept Channel param, send tokens through it, return result directly
#[tauri::command]
async fn inference_generate(
    prompt: String,
    on_token: Channel<String>,
    state: State<'_, InferenceState>,
) -> Result<GenerationMetrics, String> {
    // ... send tokens via on_token.send(token)
    Ok(metrics)
}
```

```typescript
// Frontend: create Channel, pass to invoke, await result
const onTokenChannel = new Channel<string>();
onTokenChannel.onmessage = (token) => { /* handle token */ };
const metrics = await invoke<GenerationMetrics>('inference_generate', {
    prompt, onToken: onTokenChannel
});
```

---

## Learnings

Corrections and patterns discovered during development. Categorized for easy reference.

### Rust/Tauri

- **Use Channels over Events for streaming**: `tauri::ipc::Channel<T>` is command-scoped and ordered. Events are global broadcast with no lifecycle tie to `invoke()`. Channels eliminate listener race conditions by design.
- **Use `OnceLock<Result>` over `Once` for fallible init**: `Once::call_once` doesn't return values. If you need to cache and return a result from one-time init, use `OnceLock::get_or_init()`.
- **Fatal init in production, non-fatal in dev**: `if !cfg!(debug_assertions) { return Err(...) }` in Tauri's `.setup()` — lets dev work continue without model files.
- **Use dedicated `AtomicBool` over `try_lock()` for state tracking**: `try_lock()` creates TOCTOU races. An explicit flag set/cleared in the function lifecycle is reliable.

### Frontend/Svelte

- **Tauri Channel pattern**: Create `new Channel<T>()`, set `onmessage`, pass to `invoke()`. The `invoke()` promise resolves with the command's return value only after all channel messages are delivered. No manual listener cleanup needed.
- **Single source of truth for state**: Don't duplicate reactive state between component and store. If the store tracks `isGenerating` with proper `finally` cleanup, the component should read from the store — not maintain its own copy.

### ONNX/Inference

- **ONNX Runtime version split**: v1.22.1 only ships Windows builds. Use v1.22.0 for macOS/Linux.
- **Tauri `resources` glob must match files**: `bundle.resources: ["libs/*"]` fails at compile time if no files match. Use a README.md as glob satisfier with `.gitignore` exception.
- **Qwen2.5 has TWO stop tokens**: `<|endoftext|>` (151643) for raw completion EOS and `<|im_end|>` (151645) for ChatML turn end. Without ChatML the model only emits 151643, so you must check both.
- **ChatML is mandatory for chat behavior**: Without `<|im_start|>system/user/assistant<|im_end|>` formatting, Qwen falls into pretraining-data completion patterns (self-Q&A, training data regurgitation).
- **Set `add_special_tokens: false` when embedding special tokens in prompt**: If the prompt already contains ChatML tokens, don't let the tokenizer add its own — they'd be duplicated or wrong.

### Launcher / Distribution

- **System32 onnxruntime.dll (v1.17) breaks `ort`**: Windows ships an old `onnxruntime.dll` in System32. The `ort` crate finds it before project-local copies. Fix: set `ORT_DYLIB_PATH` to the bundled v1.23 DLL at launcher startup before spawning the engine host.
- **Engine host needs ALL runtime DLLs**: `onnxruntime.dll`, `onnxruntime_providers_shared.dll`, `DirectML.dll`, AND `onnxruntime-genai.dll`. Missing any one causes 503 from `/engine/ensure-started`. Set both `ORT_DYLIB_PATH` and `SMOLPC_GENAI_DYLIB` env vars.
- **Stale engine host holds port 19432**: The engine host is a detached process that outlives the launcher. Always `taskkill /F /IM smolpc-engine-host.exe` before re-testing.
- **`tauri build` raw exe has `cfg(dev)=true`**: Running `target/release/smolpc-launcher.exe` directly shows "localhost refused to connect" because the Tauri build.rs sets `cfg(dev)` unless invoked via `tauri build`. Use the NSIS/MSI installer for production, `tauri dev` for development.
- **DLLs are gitignored (~40MB)**: Copy from `apps/blender-assistant/src-tauri/libs/` to `launcher/src-tauri/libs/`. A `README.md` in the dir explains what's needed.
- **`resolve_host_binary_path` depth matters**: CodeHelper is 3 dirs deep (`apps/codehelper/src-tauri`), launcher is 2 (`launcher/src-tauri`). The `..` traversal to reach workspace `target/` must match.

### Workflow

- **Parallel specialist reviews catch cascading issues**: Running Rust, Frontend, and Architecture reviews in parallel, then fixing and re-reviewing, is effective for thorough audits.

---

## Common Pitfalls

1. **Forgetting type sync** - Change Rust type → must update TypeScript
2. **Using Svelte 4 patterns** - This project uses Svelte 5 runes only
3. **Using `@apply`** - Tailwind 4 doesn't support it
4. **Using Events for streaming** - Use Tauri Channels instead; they auto-cleanup and prevent race conditions
5. **Blocking the main thread** - All inference is async with Tokio
6. **Sending raw prompts without ChatML** - Model will generate pretraining patterns, not chat responses
7. **Only checking one stop token** - Qwen uses different EOS tokens depending on prompt format
8. **Stale engine host on port 19432** - Kill before re-testing or you'll talk to an old binary
9. **System32 onnxruntime.dll** - v1.17 breaks `ort` v2.0.0-rc.11; launcher must set `ORT_DYLIB_PATH`
10. **Running raw `tauri build` exe** - Has `cfg(dev)`, won't load embedded frontend; use installer or `tauri dev`
11. **Missing onnxruntime-genai.dll** - Engine host returns 503; must bundle all 4 DLLs in `launcher/src-tauri/libs/`

---

## Before Committing

```bash
# CodeHelper
npm run check                                  # TypeScript compiles
npm run lint                                   # No lint errors
cd apps/codehelper/src-tauri && cargo check && cargo clippy

# Launcher
npm run check --workspace launcher             # TypeScript compiles
cargo check -p smolpc-launcher && cargo clippy -p smolpc-launcher
cargo test -p smolpc-launcher                  # 5 unit tests
```

Commit message format (Conventional Commits):

```
feat: add KV cache with Attention Sinks
fix: resolve memory leak in generator
docs: update CURRENT_STATE after Phase 1
```

---

## Resources

- [Tauri 2 Docs](https://v2.tauri.app/)
- [Svelte 5 Runes](https://svelte.dev/docs/svelte/what-are-runes)
- [ONNX Runtime Rust (ort)](https://docs.rs/ort/)
- [Tailwind CSS 4](https://tailwindcss.com/docs)
