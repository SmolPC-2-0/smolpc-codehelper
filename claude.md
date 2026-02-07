# CLAUDE.md

This file guides Claude Code sessions working on SmolPC Code Helper.

**Last Updated:** 2026-02-07
**Current Phase:** 1.5 Complete (Frontend Integration) → Phase 2 (GPU/NPU Acceleration)
**Branch:** `feature/ort_setup`

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

**Phase**: 1.5 Complete → Phase 2 (GPU/NPU Acceleration)
**Branch**: `feature/ort_setup`
**Last Session**: 2026-02-07 (Session 3) - Migrated streaming to Tauri Channels

**What's Working**:

- ONNX inference via `ort` crate with Qwen2.5-Coder-1.5B
- KV Cache with Attention Sinks (~8 tok/s on CPU)
- Streaming generation via Tauri Channels (race condition fixed)
- Frontend integrated (chat UI uses ONNX, not Ollama)

**Next Up**:

1. Phase 2: Execution Provider abstraction (trait-based)
2. Intel NPU detection + OpenVINO EP
3. NVIDIA GPU detection + CUDA EP

**Blockers**: None currently

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

### Current Architecture (ONNX-Based)

```
┌─────────────────────────────────────────────────────────┐
│                Code Helper (Tauri 2.6.2)                │
│  ┌─────────────────────────────────────────────────┐   │
│  │           Frontend (Svelte 5)                    │   │
│  │  inference.svelte.ts → Tauri events              │   │
│  └─────────────────────┬───────────────────────────┘   │
│                        │ IPC                            │
│  ┌─────────────────────┴───────────────────────────┐   │
│  │              Backend (Rust)                      │   │
│  │  commands/inference.rs → inference/ module       │   │
│  │                                                  │   │
│  │  inference/                                      │   │
│  │  ├── generator.rs    (autoregressive loop)      │   │
│  │  ├── kv_cache.rs     (Attention Sinks)          │   │
│  │  ├── session.rs      (ONNX session wrapper)     │   │
│  │  └── tokenizer.rs    (HuggingFace tokenizers)   │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
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
npm run tauri dev          # Full app with hot reload
npm run dev                # Frontend only

# Checks
npm run check              # TypeScript
npm run lint               # Lint
cd src-tauri && cargo check && cargo clippy  # Rust

# Tests
cd src-tauri && cargo test -- --ignored --nocapture  # Inference tests
```

### Key File Locations

**Backend (Rust):**

- `src-tauri/src/lib.rs` - Tauri setup, command registration
- `src-tauri/src/commands/inference.rs` - ONNX inference commands
- `src-tauri/src/inference/` - Core inference engine
- `src-tauri/src/models/` - Model registry and loading

**Frontend (TypeScript/Svelte):**

- `src/App.svelte` - Main app, uses inferenceStore
- `src/lib/stores/inference.svelte.ts` - ONNX inference state
- `src/lib/types/inference.ts` - Type definitions

**Legacy (To Be Removed):**

- `src-tauri/src/commands/ollama.rs` - Old Ollama integration
- `src/lib/stores/ollama.svelte.ts` - Old Ollama store

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

### Frontend/Svelte

- **Tauri Channel pattern**: Create `new Channel<T>()`, set `onmessage`, pass to `invoke()`. The `invoke()` promise resolves with the command's return value only after all channel messages are delivered. No manual listener cleanup needed.

### ONNX/Inference

_(None recorded yet)_

### Workflow

_(None recorded yet)_

---

## Common Pitfalls

1. **Forgetting type sync** - Change Rust type → must update TypeScript
2. **Using Svelte 4 patterns** - This project uses Svelte 5 runes only
3. **Using `@apply`** - Tailwind 4 doesn't support it
4. **Using Events for streaming** - Use Tauri Channels instead; they auto-cleanup and prevent race conditions
5. **Blocking the main thread** - All inference is async with Tokio

---

## Before Committing

```bash
npm run check              # TypeScript compiles
npm run lint               # No lint errors
cd src-tauri && cargo check && cargo clippy  # Rust compiles
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
