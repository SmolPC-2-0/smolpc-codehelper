# Engine Supervisor Redesign — Ralph Loop Implementation Prompt

## Your Mission

You are implementing the Engine Supervisor redesign for SmolPC Code Helper. This replaces the broken `resolve_client()` + 4-Mutex `InferenceState` pattern with a single-owner supervisor task using the actor/handle pattern.

**Read these documents before starting:**
- `docs/superpowers/specs/2026-03-23-engine-supervisor-redesign.md` — Full design spec
- `docs/ENGINE_LIFECYCLE_AUDIT.md` — Current architecture audit and flaws
- `docs/superpowers/specs/2026-03-23-engine-supervisor-PRD.md` — Product requirements
- `CLAUDE.md` — Project conventions and learnings

## Implementation Phases

Work through these phases IN ORDER. Each phase must compile and pass tests before moving to the next. Commit after each phase.

### Phase 0: Refactor Engine Client Crate Primitives

**Files:** `engine/crates/smolpc-engine-client/src/spawn.rs`

Split the monolithic `connect_or_spawn()` into composable primitives:
- `pub fn spawn_engine(options: &SpawnOptions) -> Result<u32, EngineClientError>` — spawn only, no health wait
- `pub async fn wait_for_healthy(client: &EngineClient, timeout: Duration) -> Result<(), EngineClientError>` — poll health
- `pub async fn check_running_policy(client: &EngineClient, force_override: Option<&str>, force_respawn: bool) -> Result<RunningHostPolicyDecision, EngineClientError>` — policy check
- `pub async fn shutdown_and_wait(client: &EngineClient, timeout: Duration) -> Result<(), EngineClientError>` — graceful shutdown
- `pub fn kill_stale_processes()` — kill by name
- `pub async fn with_spawn_lock<F, Fut, T>(shared_runtime_dir: &Path, f: F) -> Result<T, EngineClientError>` — spawn lock wrapper

Keep `connect_or_spawn` as a convenience wrapper calling these primitives. No existing behavior changes.

**Verify:** `cargo check --workspace && cargo test -p smolpc-engine-client`

### Phase 1: Supervisor Core

**New files:**
- `apps/codehelper/src-tauri/src/engine/mod.rs` — module root, EngineCommand, StartupConfig, EngineLifecycleState
- `apps/codehelper/src-tauri/src/engine/supervisor.rs` — EngineSupervisor task
- `apps/codehelper/src-tauri/src/engine/handle.rs` — EngineSupervisorHandle

**Wire up in:** `apps/codehelper/src-tauri/src/lib.rs`
- Add `mod engine;`
- In `Builder::setup()`, create supervisor channels, spawn supervisor task, `app.manage(handle)`
- Supervisor runs alongside existing InferenceState (both registered as managed state)

**State machine:** `Idle → Starting → WaitingForHealth → Running → Crashed → (restart ≤3) → Starting` / `→ Failed`

**Supervisor loop uses `tokio::select!`:**
- `cmd_rx.recv()` — handle Start, SetRuntimeMode, SetDesiredModel, RefreshStatus, Shutdown
- `health_interval.tick()` (10s) — PID check + HTTP health + status refresh
- `sleep(restart_delay)` with conditional guard — exponential backoff restart

**Key behaviors:**
- On `Crashed → Starting`: delete old token, regenerate, construct new EngineClient, kill stale processes, spawn
- On reaching `Running`: restore `desired_model` via `client.load_model()` if set
- Broadcast state via `watch::Sender` AND `app_handle.emit("engine-state-changed", &state)`

**Verify:** `cargo check --workspace` (supervisor starts but isn't used by commands yet)

### Phase 2: Migrate Tauri Commands

**Files:** `apps/codehelper/src-tauri/src/commands/inference.rs`, `engine_client_adapter.rs`, `assistant.rs`

Migrate in atomic groups:

**Group A (state-mutating — migrate together):**
- `engine_ensure_started` → `supervisor.ensure_started(config).await`
- `set_inference_runtime_mode` → `supervisor.set_runtime_mode(mode).await`
- `load_model`, `unload_model` → `supervisor.get_client(60s).await` + `supervisor.refresh_status().await` after

**Group B (read-only):**
- `list_models`, `get_current_model`, `get_inference_backend_status`, `engine_status`
- `check_model_readiness`, `check_model_exists`, `is_generating`
- All become: `supervisor.get_client(60s).await?` then use client

**Group C (generation):**
- `inference_generate`, `inference_generate_messages`, `inference_cancel`
- `assistant_send` (3 match arms via resolve_generation_client)
- All become: `supervisor.get_client(60s).await?` then use client
- `assistant_cancel` → `supervisor.get_client_if_ready()` (non-blocking)

**Group D (special):**
- `engine_health_only` → `supervisor.current_state().is_running()` (instant)
- `evaluate_memory_pressure` → `supervisor.get_client_if_ready()` (non-blocking)

After all groups: delete `resolve_client`, `resolve_generation_client`, `cached_generation_client`, `ensure_desired_model_loaded`, `apply_runtime_mode_preference`, `InferenceState`.

**Verify:** `cargo check --workspace && cargo clippy --workspace`

### Phase 3: Migrate Frontend

**Files:** `apps/codehelper/src/App.svelte`, `apps/codehelper/src/lib/stores/inference.svelte.ts`, `apps/codehelper/src/lib/types/inference.ts`

**Delete from App.svelte:**
- The health polling `$effect` (setTimeout chain)
- `reestablishEngineSession()` function
- `reconnectingEngineSession` state variable

**Add to App.svelte onMount:**
```typescript
const unlisten = await listen<EngineLifecycleState>('engine-state-changed', (event) => {
    inferenceStore.updateLifecycleState(event.payload);
});
```

**Add TypeScript type:**
```typescript
type EngineLifecycleState =
    | { state: 'idle' }
    | { state: 'starting' }
    | { state: 'waiting_for_health' }
    | { state: 'running'; backend: string | null; model_id: string | null }
    | { state: 'crashed'; message: string; restart_count: number }
    | { state: 'failed'; message: string };
```

**Update inference.svelte.ts:**
- Add `lifecycleState` reactive field
- Add `updateLifecycleState(state)` method that maps to `engineHealthy`, `error`, derived states
- `forceResetGenerationState()` fires on `crashed`/`failed` transitions
- Keep `ensureStarted()` — it now just invokes `engine_ensure_started` which waits on supervisor

**Mapping:**
- `running(model_id=some)` → `engineHealthy=true, readiness.state='ready', isReady=true`
- `running(model_id=null)` → `engineHealthy=true, readiness.state='loading_model', isReady=false`
- `crashed/failed` → `engineHealthy=false, error=message`
- `starting/waiting_for_health` → `engineHealthy=false, readiness.state='starting'`

**Verify:** `cd apps/codehelper && npm run check && npm run lint`

### Phase 4: Migrate Launcher

**Files:** `apps/codehelper/src-tauri/src/launcher/orchestrator.rs`

Replace `resolve_engine_client()` with `supervisor.get_client()`. Delete the launcher's separate `EngineClient` cache, connect lock, and runtime config.

**Verify:** `cargo check --workspace`

### Phase 5: Cleanup

Delete dead code:
- `InferenceState` struct and all methods
- `resolve_client()`, `resolve_generation_client()`, `cached_generation_client()`
- `ensure_desired_model_loaded()`, `apply_runtime_mode_preference()`
- `connect_lock`, `client` Mutex, `runtime_config` Mutex, `desired_model` Mutex
- Unused imports

**Verify:**
```bash
cargo check --workspace && cargo clippy --workspace
cargo test -p smolpc-engine-core && cargo test -p smolpc-engine-host
cd apps/codehelper && npm run check && npm run lint
```

## Conventions

- **Conventional commits:** `feat(engine):`, `refactor(engine):`, etc.
- **Svelte 5 runes only:** `$state`, `$derived`, `$effect` — no `writable`/`readable`
- **Tailwind 4:** Utility classes only, no `@apply`
- **Tauri Channels for streaming:** `tauri::ipc::Channel<T>`, not global Events
- **Pre-commit:** `cargo check --workspace && cargo clippy --workspace && cd apps/codehelper && npm run check`

## Success Criteria

When ALL of these are true, output `<promise>ENGINE SUPERVISOR COMPLETE</promise>`:

1. `taskkill /F /IM smolpc-engine-host.exe` → engine restarts within 10s, banner clears
2. App startup produces exactly ONE "Engine client connected/spawned successfully" log
3. No `connect_lock` or `client Mutex` anywhere in the codebase
4. `cargo check --workspace` clean, `cargo clippy --workspace` clean
5. `svelte-check` 0 errors, `eslint` 0 errors on changed files
6. All existing tests pass
7. Launcher uses the same supervisor handle (no duplicate client cache)

## Important Notes for AI Implementers

- `EngineClient` is `#[derive(Clone)]` with an Arc-based `reqwest::Client`. Cloning is cheap.
- The supervisor owns the ONLY mutable copy. Commands receive clones via the handle.
- `tokio::sync::watch` is the broadcast mechanism. Consumers call `.borrow()` (instant, no lock).
- Tauri already wraps managed state in `Arc`. Don't add redundant `Arc` inside the handle.
- On Windows, `DETACHED_PROCESS` means `child.wait()` doesn't work. Use HTTP health polls + PID checks.
- Token regeneration on restart is CRITICAL — stale tokens are the #1 cause of "unhealthy after idle".
- The `engine-spawn.lock` file mechanism is kept for inter-process safety.
- `run_benchmark` is currently a no-op stub — don't try to migrate it.
