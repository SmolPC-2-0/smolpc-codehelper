# PRD: Engine Supervisor Redesign

## Introduction

The SmolPC Code Helper engine process (smolpc-engine-host) runs as a separate HTTP server managing AI inference. When this process dies — whether from a crash, external kill, or resource exhaustion — the app enters a permanently broken state: the "engine disconnected" banner shows but the engine never relaunches. Students must restart the entire app to recover.

The root cause is architectural: 15 Tauri commands compete through a single `resolve_client()` function protected by 4 shared `Arc<Mutex<T>>` fields. When the engine dies, concurrent Tauri tasks deadlock on Mutex contention. Additionally, startup triggers 2-3 redundant engine spawns due to race conditions.

This redesign replaces the entire lifecycle management with a single-owner supervisor task using the actor/handle pattern — zero Mutexes, zero lock contention, automatic restart with backoff.

**Full technical spec:** `docs/superpowers/specs/2026-03-23-engine-supervisor-redesign.md`
**Architecture audit:** `docs/ENGINE_LIFECYCLE_AUDIT.md`

## Goals

- Automatic engine recovery within 10 seconds of death, with exponential backoff (1s/2s/4s) and max 3 restarts in 5 minutes
- Exactly one engine spawn during app startup (currently 2-3)
- Zero Mutex contention — replace all 4 `Arc<Mutex<T>>` fields with message passing
- Unified engine management — Code Helper and launcher share the same supervisor handle
- Production parity — works identically in NSIS-installed builds and dev builds
- Rust-side health polling (no frontend setTimeout chain or IPC round-trips)

## User Stories

### US-001: Refactor engine-client crate into composable primitives
**Description:** As the supervisor, I need composable spawn/health/policy primitives so I can orchestrate the engine lifecycle with fine-grained control instead of calling the monolithic `connect_or_spawn`.

**Acceptance Criteria:**
- [ ] `spawn_engine(options) -> Result<u32>` — spawns detached process, returns PID, does NOT wait for health
- [ ] `wait_for_healthy(client, timeout) -> Result<()>` — polls health endpoint until responsive
- [ ] `check_running_policy(client, force_override, force_respawn) -> Result<RunningHostPolicyDecision>` — policy check
- [ ] `shutdown_and_wait(client, timeout) -> Result<()>` — graceful shutdown with wait
- [ ] `kill_stale_processes()` — kills all smolpc-engine-host by name
- [ ] `with_spawn_lock(dir, f) -> Result<T>` — filesystem spawn lock wrapper
- [ ] `connect_or_spawn` still exists as a convenience wrapper calling these primitives
- [ ] `cargo test -p smolpc-engine-client` passes
- [ ] `cargo check --workspace` clean

### US-002: Build supervisor state machine and types
**Description:** As a developer, I need the `EngineLifecycleState` enum and `EngineCommand` types so the supervisor can model lifecycle transitions explicitly.

**Acceptance Criteria:**
- [ ] `EngineLifecycleState` enum: `Idle`, `Starting`, `WaitingForHealth`, `Running { backend, model_id }`, `Crashed { message, restart_count }`, `Failed { message }`
- [ ] `#[derive(serde::Serialize)]` with `#[serde(tag = "state", rename_all = "snake_case")]` for Tauri event emission
- [ ] Transition validation method `can_transition_to(&self, next) -> bool`
- [ ] `EngineCommand` enum: `Start`, `SetRuntimeMode`, `SetDesiredModel`, `RefreshStatus`, `Shutdown`
- [ ] `StartupConfig` struct: `runtime_mode`, `dml_device_id`, `default_model_id`, `startup_mode`
- [ ] Files: `apps/codehelper/src-tauri/src/engine/mod.rs`
- [ ] `cargo check --workspace` clean

### US-003: Build EngineSupervisor task
**Description:** As the app, I need a background tokio task that owns the engine lifecycle — spawning, health monitoring, auto-restart, and state broadcasting — so no Tauri command ever needs to compete for Mutexes.

**Acceptance Criteria:**
- [ ] Single tokio task spawned in `Builder::setup()`
- [ ] Owns `EngineClient`, `RuntimeConfig`, `DesiredModel` (no Mutex)
- [ ] Core loop uses `tokio::select!` with: `cmd_rx.recv()`, `health_interval.tick()` (10s), conditional `sleep(restart_delay)`
- [ ] Health check: PID alive check + HTTP `client.health()` + `client.status()` for Running state refresh
- [ ] On health failure: transition to `Crashed`, schedule restart with exponential backoff
- [ ] On `Crashed → Starting`: delete old token, regenerate, construct new EngineClient, kill stale processes, spawn
- [ ] On reaching `Running`: restore `desired_model` via `client.load_model()` if set
- [ ] After 3 restarts in 5 minutes: transition to `Failed`
- [ ] Broadcasts state via `watch::Sender<EngineLifecycleState>`
- [ ] Emits Tauri events via `app_handle.emit("engine-state-changed", &state)`
- [ ] File: `apps/codehelper/src-tauri/src/engine/supervisor.rs`
- [ ] `cargo check --workspace` clean

### US-004: Build EngineSupervisorHandle
**Description:** As a Tauri command handler, I need a Clone-able handle to communicate with the supervisor — requesting the engine client, sending commands, and observing state — without any Mutex access.

**Acceptance Criteria:**
- [ ] `EngineSupervisorHandle` struct with `mpsc::Sender<EngineCommand>` + `watch::Receiver<EngineLifecycleState>`
- [ ] `current_state() -> EngineLifecycleState` — instant, no network, no lock
- [ ] `get_client(timeout) -> Result<EngineClient, String>` — waits on watch until Running, returns clone
- [ ] `get_client_if_ready() -> Option<EngineClient>` — non-blocking, for polling paths
- [ ] `ensure_started(config) -> Result<(), String>` — sends Start command, waits for Running/Failed
- [ ] `set_runtime_mode(mode) -> Result<(), String>` — sends SetRuntimeMode, waits for Running/Failed
- [ ] `shutdown() -> Result<(), String>` — sends Shutdown command
- [ ] `refresh_status()` — sends RefreshStatus command (after load_model, etc.)
- [ ] Registered as Tauri managed state: `app.manage(handle)`
- [ ] File: `apps/codehelper/src-tauri/src/engine/handle.rs`
- [ ] `cargo check --workspace` clean

### US-005: Migrate state-mutating Tauri commands (Group A)
**Description:** As the app, I need the state-mutating commands to go through the supervisor first, so the old `resolve_client` path can't kill engines the supervisor manages.

**Acceptance Criteria:**
- [ ] `engine_ensure_started` → `supervisor.ensure_started(config).await`
- [ ] `set_inference_runtime_mode` → `supervisor.set_runtime_mode(mode).await`
- [ ] `load_model` → `supervisor.get_client(60s).await` + `supervisor.refresh_status()` after
- [ ] `unload_model` → `supervisor.get_client(60s).await` + `supervisor.refresh_status()` after
- [ ] `cargo check --workspace` clean
- [ ] `cargo clippy --workspace` clean

### US-006: Migrate read-only Tauri commands (Group B)
**Description:** As the app, I need read-only commands to use the supervisor handle instead of `resolve_client`.

**Acceptance Criteria:**
- [ ] `list_models` → `supervisor.get_client(60s).await`
- [ ] `get_current_model` → `supervisor.get_client(60s).await`
- [ ] `get_inference_backend_status` → `supervisor.get_client(60s).await`
- [ ] `engine_status` → `supervisor.get_client(60s).await`
- [ ] `check_model_readiness` → `supervisor.get_client(60s).await`
- [ ] `check_model_exists` → `supervisor.get_client(60s).await`
- [ ] `is_generating` → `supervisor.get_client(60s).await`
- [ ] `cargo check --workspace` clean

### US-007: Migrate generation and assistant Tauri commands (Group C)
**Description:** As the app, I need generation commands to use the supervisor handle.

**Acceptance Criteria:**
- [ ] `inference_generate` → `supervisor.get_client(60s).await`
- [ ] `inference_generate_messages` → `supervisor.get_client(60s).await`
- [ ] `inference_cancel` → `supervisor.get_client(60s).await`
- [ ] `assistant_send` (3 match arms) → `supervisor.get_client(60s).await`
- [ ] `assistant_cancel` → `supervisor.get_client_if_ready()` (non-blocking)
- [ ] `cargo check --workspace` clean

### US-008: Migrate special-case Tauri commands (Group D)
**Description:** As the app, I need the health and memory pressure commands to use the supervisor's lock-free state access.

**Acceptance Criteria:**
- [ ] `engine_health_only` → `supervisor.current_state().is_running()` (instant, no HTTP)
- [ ] `evaluate_memory_pressure` → `supervisor.get_client_if_ready()` (non-blocking)
- [ ] `cargo check --workspace` clean

### US-009: Delete old InferenceState and resolve_client
**Description:** As a developer, I need the old Mutex-based code removed so there's no confusion about which pattern to use.

**Acceptance Criteria:**
- [ ] `InferenceState` struct deleted
- [ ] `resolve_client()` deleted
- [ ] `resolve_generation_client()` deleted
- [ ] `cached_generation_client()` deleted
- [ ] `ensure_desired_model_loaded()` deleted
- [ ] `apply_runtime_mode_preference()` deleted
- [ ] All `Arc<Mutex<>>` imports cleaned up
- [ ] `cargo check --workspace` clean
- [ ] `cargo clippy --workspace` clean (no dead code warnings)

### US-010: Migrate frontend to event-driven lifecycle
**Description:** As a student, I want the UI to reactively reflect engine state changes (starting, running, crashed, failed) without any polling delays, so the app feels responsive.

**Acceptance Criteria:**
- [ ] Health polling `$effect` (setTimeout chain) deleted from App.svelte
- [ ] `reestablishEngineSession()` deleted
- [ ] `reconnectingEngineSession` state deleted
- [ ] Tauri event listener added: `listen('engine-state-changed', handler)`
- [ ] TypeScript type `EngineLifecycleState` defined with all variants
- [ ] `inferenceStore.updateLifecycleState(state)` maps to `engineHealthy`, `error`, `isReady`
- [ ] `forceResetGenerationState()` fires on `crashed`/`failed` transitions
- [ ] `ensureStarted()` invokes `engine_ensure_started` which waits on supervisor
- [ ] `svelte-check` 0 errors
- [ ] `eslint` 0 errors on changed files

### US-011: Migrate launcher to shared supervisor
**Description:** As the launcher, I need to share the same engine supervisor handle instead of maintaining a duplicate client cache.

**Acceptance Criteria:**
- [ ] `resolve_engine_client()` in orchestrator.rs replaced with `supervisor.get_client()`
- [ ] Launcher's separate `EngineClient` cache, connect lock, and runtime config deleted
- [ ] `cargo check --workspace` clean

## Functional Requirements

- FR-1: The supervisor MUST be the sole owner of the engine lifecycle — no other code path may spawn, kill, or restart the engine
- FR-2: The supervisor MUST broadcast `EngineLifecycleState` via `tokio::sync::watch` after every state transition
- FR-3: The supervisor MUST emit `engine-state-changed` Tauri events for frontend consumption
- FR-4: The supervisor MUST auto-restart on crash with exponential backoff (1s, 2s, 4s), max 3 restarts in 5 minutes
- FR-5: The supervisor MUST regenerate the auth token before each restart (stale tokens cause "unhealthy after idle")
- FR-6: The supervisor MUST restore the desired model after successful restart
- FR-7: The supervisor MUST check PID alive between health polls for fast crash detection
- FR-8: All Tauri commands that need the engine MUST use `supervisor.get_client(timeout)` which waits on the watch channel
- FR-9: `engine_health_only` MUST read from the watch channel instantly (no HTTP, no lock)
- FR-10: `evaluate_memory_pressure` and `assistant_cancel` MUST use `get_client_if_ready()` (non-blocking)
- FR-11: The frontend MUST NOT poll for health — it MUST react to Tauri events
- FR-12: The launcher MUST share the same `EngineSupervisorHandle` (no duplicate cache)
- FR-13: App exit MUST trigger graceful engine shutdown via the supervisor
- FR-14: The `connect_or_spawn` convenience wrapper MUST be preserved for backward compatibility
- FR-15: Production builds MUST resolve engine binary from `resource_dir/binaries/`

## Non-Goals

- Changing the engine host binary or its HTTP API
- Adding multi-instance support (multiple app windows sharing one engine)
- Changing the model loading pipeline or backend selection logic
- Redesigning frontend UI components (only their data source changes)
- Adding new Tauri commands (only modifying existing ones)
- Migrating `run_benchmark` (currently a no-op stub)

## Technical Considerations

- **Tokio channels:** `watch` for state broadcast, `mpsc` for commands, `oneshot` for request-response. All already in dependency tree via Tauri.
- **Tauri managed state:** Already wraps in `Arc` — do NOT add redundant `Arc` inside the handle.
- **EngineClient is Clone:** Arc-based `reqwest::Client` internally. `get_client()` returns cheap clones.
- **Windows DETACHED_PROCESS:** `child.wait()` doesn't work. Use HTTP health polls + PID alive checks.
- **Token file:** `%LOCALAPPDATA%/SmolPC/engine-runtime/engine-token.txt` — regenerate on every restart.
- **Spawn lock:** Filesystem-based `engine-spawn.lock` — kept for inter-process safety.
- **Migration ordering:** State-mutating commands (Group A) must migrate BEFORE read-only commands to prevent the old path from killing supervisor-managed engines.

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Recovery after taskkill | Never (permanent broken state) | <10s automatic |
| Engine spawns during startup | 2-3 | Exactly 1 |
| Mutex lock contentions on death | Deadlock (permanent) | 0 (no Mutexes) |
| Frontend health poll IPC round-trips | 6/min | 0 (event-driven) |
| Time from app launch to Ready | ~30s (double spawn) | ~15s (single spawn) |

## Open Questions

None — all design decisions finalized in the approved spec.
