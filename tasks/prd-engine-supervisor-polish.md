# PRD: Engine Supervisor Polish & Debug Pass

## Introduction

The engine supervisor (Ralph implementation, 12 user stories) provides the correct architecture — single-owner task, watch channels, message passing, no Mutexes. However, live testing revealed three critical bugs and multiple code quality issues that prevent production readiness. This pass fixes all bugs structurally (not patches), removes dead/legacy code, and polishes for production.

**Reference docs:**
- Design spec: `docs/superpowers/specs/2026-03-23-engine-supervisor-redesign.md`
- Architecture audit: `docs/ENGINE_LIFECYCLE_AUDIT.md`
- Ralph progress log: `scripts/ralph/progress.txt`

## Goals

- Fix the 60-second startup delay caused by `listModels()` blocking before engine start
- Eliminate double-spawn by passing runtime mode preference in the initial Start command
- Fix model auto-load after engine restart
- Complete missing implementation (transition validation, dead code removal)
- Polish all supervisor code for production quality
- Remove ALL legacy InferenceState remnants

## User Stories

### US-001: Fix startup ordering — ensureStarted before listModels
**Description:** As a student, I want the app to start the engine immediately on launch, not wait 60 seconds.

**Acceptance Criteria:**
- [ ] In `App.svelte` `initInference()`: call `performStartup()` BEFORE `listModels()`
- [ ] `listModels()` must NOT be the first command that hits `supervisor.get_client()`
- [ ] Engine starts within 2s of app launch (not 60s)
- [ ] `listModels()` still populates available models after engine is running
- [ ] `cargo check --workspace` clean
- [ ] `svelte-check` 0 errors

### US-002: Pass runtime mode preference in Start command — eliminate double spawn
**Description:** As a student, I want the engine to start on my preferred backend (NPU/CPU/DML) in ONE spawn, not start on auto then restart for my preference.

**Acceptance Criteria:**
- [ ] `performStartup()` in App.svelte passes `runtime_mode_preference` in the ensureStarted request (same field as the old PR #197 approach)
- [ ] Remove the separate `setRuntimeMode()` call after `ensureStarted()` in `performStartup()`
- [ ] `engine_ensure_started` Tauri command extracts the preference and includes it in the `StartupConfig` sent to the supervisor
- [ ] Supervisor's `handle_start()` applies the runtime mode BEFORE spawning the engine (sets `SMOLPC_FORCE_EP` env var)
- [ ] Only ONE "Supervisor: engine spawned with PID" message in logs during startup
- [ ] Only ONE "Engine client connected/spawned successfully" equivalent in logs
- [ ] `cargo check --workspace` clean
- [ ] `svelte-check` 0 errors

### US-003: Fix model auto-load after engine restart
**Description:** As a student, I want my previously loaded model to be automatically restored when the engine restarts after a crash.

**Acceptance Criteria:**
- [ ] Supervisor stores `desired_model` when frontend calls `load_model` (via SetDesiredModel command or inferred from load_model success)
- [ ] After transitioning to Running state (post-restart), supervisor checks `desired_model` and calls `client.load_model()` if set
- [ ] If model load fails on first attempt, retry once after 2 seconds
- [ ] If retry also fails, log error and stay in Running with `model_id: None` — don't crash the supervisor
- [ ] After successful model restoration, broadcast updated Running state with `model_id` populated
- [ ] `cargo check --workspace` clean

### US-004: Add transition validation to supervisor state machine
**Description:** As a developer, I want invalid state transitions to be caught and logged so bugs in the supervisor logic are visible.

**Acceptance Criteria:**
- [ ] `supervisor.rs` `transition()` method calls `can_transition_to()` before applying the transition
- [ ] If transition is invalid, log an error with current state and attempted state, and DO NOT apply the transition
- [ ] If transition is valid, apply it and broadcast
- [ ] Remove `#[warn(dead_code)]` on `can_transition_to` — it's now used
- [ ] `cargo check --workspace` clean

### US-005: Fix readiness state on engine death
**Description:** As a student, I want the UI to correctly show "engine crashed" when the engine dies, not show stale "ready" state.

**Acceptance Criteria:**
- [ ] In `inference.svelte.ts` `updateLifecycleState()`: when state is `crashed` or `failed`, update `readiness` to reflect the failure (set `readiness.state = 'failed'`, `readiness.error_message = message`)
- [ ] When state is `starting` or `waiting_for_health`, set `readiness.state = 'starting'`
- [ ] When state is `idle`, set `readiness` to null or idle state
- [ ] StartupLoadingScreen correctly shows "Starting engine..." for `starting`/`waiting_for_health` states
- [ ] Banner correctly shows error for `crashed`/`failed` states
- [ ] `svelte-check` 0 errors

### US-006: Fix client cache race condition
**Description:** As a developer, I need the supervisor to guarantee the client is available when Running state is broadcast, so commands don't get None from `get_client_if_ready()` during the transition.

**Acceptance Criteria:**
- [ ] In `supervisor.rs`: broadcast the client via `client_tx` BEFORE broadcasting Running state via `state_tx`
- [ ] This ensures the client cache watcher updates `cached_client` before any consumer sees the Running state and tries `get_client_if_ready()`
- [ ] Alternatively: combine the channels so state and client are broadcast atomically
- [ ] Add a log message when client is broadcast to help debug timing issues
- [ ] `cargo check --workspace` clean

### US-007: Remove dead code and unused fields
**Description:** As a developer, I want all dead code removed so the codebase is clean and the compiler has zero warnings.

**Acceptance Criteria:**
- [ ] Remove `startup_mode` field from `StartupConfig` (never read by supervisor)
- [ ] Remove `current_state()` method from handle IF not needed, OR wire it into `engine_health_only` command
- [ ] Remove `subscribe()` method from handle IF not needed (no current consumers)
- [ ] Remove `checkHealth()` method from `inference.svelte.ts` (supervisor handles health)
- [ ] Remove any remaining `InferenceState` references, imports, or comments
- [ ] Remove any remaining `resolve_client` references in comments
- [ ] `cargo check --workspace` produces ZERO warnings (not just zero errors)
- [ ] `cargo clippy --workspace` clean
- [ ] `svelte-check` 0 errors
- [ ] `eslint` 0 errors on changed files

### US-008: Run cargo fmt and fix all formatting
**Description:** As a developer, I need all Rust code formatted consistently.

**Acceptance Criteria:**
- [ ] `cd apps/codehelper/src-tauri && cargo fmt` applied
- [ ] `cargo fmt -- --check` returns no diff
- [ ] `prettier --check` passes on all changed frontend files
- [ ] Commit as `style: cargo fmt + prettier on supervisor code`

### US-009: Reduce log verbosity — suppress reqwest DEBUG noise
**Description:** As a developer, I want clean logs that show supervisor lifecycle events without hundreds of HTTP pool messages.

**Acceptance Criteria:**
- [ ] Set default log filter to suppress `hyper_util` and `reqwest` DEBUG messages
- [ ] Use `RUST_LOG=info,hyper_util=warn,reqwest=warn` or equivalent filter in the engine host spawn
- [ ] Supervisor lifecycle logs (state transitions, spawn, health check results) remain visible at INFO level
- [ ] `cargo check --workspace` clean

### US-010: Update CLAUDE.md with supervisor learnings
**Description:** As a future developer/AI, I need the project conventions updated to reflect the supervisor architecture.

**Acceptance Criteria:**
- [ ] Remove learnings about `resolve_client`, `InferenceState`, `connect_lock` that are no longer applicable
- [ ] Add learnings: "Engine lifecycle is owned by the EngineSupervisor task — do not spawn or kill engine processes from Tauri commands"
- [ ] Add learning: "Use `tauri::async_runtime::spawn` (not `tokio::spawn`) for tasks created during Builder::setup()"
- [ ] Add learning: "Tauri commands get the engine client via `supervisor.get_client(timeout)` — this waits on a watch channel, no Mutex"
- [ ] Add learning: "Runtime mode preference must be passed in the Start command — never apply mode after spawn"
- [ ] Update the Quick Reference section if any commands changed

## Functional Requirements

- FR-1: Engine MUST start within 5 seconds of app launch (not 60)
- FR-2: Engine MUST spawn exactly once during startup (not 2-3 times)
- FR-3: Desired model MUST be restored after engine restart
- FR-4: State transitions MUST be validated via `can_transition_to()`
- FR-5: Frontend readiness state MUST reflect supervisor lifecycle state
- FR-6: Client cache MUST be populated before Running state is observable
- FR-7: Compiler MUST produce zero warnings
- FR-8: All Rust code MUST pass `cargo fmt --check`
- FR-9: Logs MUST be readable without hundreds of pool/connection debug messages

## Non-Goals

- No new features — this is purely bug fixes, dead code removal, and polish
- No changes to the engine host binary or its HTTP API
- No changes to the smolpc-engine-client crate (Phase 0 primitives are fine)
- No frontend UI redesign — only state mapping fixes

## Technical Considerations

- **Startup ordering:** The fix is in App.svelte's `initInference()` — swap `listModels` and `performStartup`. `listModels` should be called AFTER `ensureStarted` returns.
- **Runtime mode in Start:** The `StartupConfig` already has `runtime_mode: RuntimeModePreference`. The supervisor's `do_spawn_sequence()` already reads it for `SMOLPC_FORCE_EP`. The issue is that the frontend passes `mode: 'auto'` and applies the preference separately.
- **Client cache ordering:** In supervisor.rs, the `broadcast_client()` call must come before `transition()` for Running states. Currently both are called but ordering may not guarantee atomicity.
- **Log filtering:** The engine host already sets `RUST_LOG=info` when spawning. The Tauri app itself needs a similar filter for its own runtime.

## Success Metrics

| Metric | Before (Ralph v1) | After (Polish Pass) |
|--------|-------------------|---------------------|
| Startup delay | ~60s | <5s |
| Engine spawns at startup | 2 | 1 |
| Model auto-load after restart | No | Yes |
| Compiler warnings | 3 | 0 |
| rustfmt clean | No | Yes |
| Log lines per startup | ~500+ DEBUG | ~20 INFO |

## Open Questions

None — all issues identified from live testing and code audit.
