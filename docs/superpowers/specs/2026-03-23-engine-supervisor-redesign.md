# Engine Supervisor Redesign

## Purpose

Replace the current engine lifecycle management (15 Tauri commands competing through `resolve_client` with 4 shared Mutexes) with a single-owner supervisor task using the actor/handle pattern. This eliminates the Mutex contention deadlock that prevents engine reconnection after external kill, fixes the double/triple spawn at startup, and provides a production-ready engine lifecycle with automatic restart, exponential backoff, and explicit state machine transitions.

**Companion document:** `docs/ENGINE_LIFECYCLE_AUDIT.md` contains the full analysis of the current architecture's flaws.

---

## Architecture Overview

### Current (Broken)

```
  15+ Tauri commands ──→ resolve_client() ──→ connect_or_spawn()
                          ├─ client Mutex (contention on death)
                          ├─ connect_lock Mutex (30s convoy)
                          └─ 2-3 engine spawns at startup
```

### Proposed

```
  15+ Tauri commands ──→ supervisor_handle.get_client()
                          └─ waits on watch channel (no locks)

  EngineSupervisor task (single owner):
    ├─ owns EngineClient (no Mutex needed)
    ├─ owns RuntimeConfig, DesiredModel
    ├─ internal health timer (10s, Rust-side)
    ├─ auto-restart (backoff 1s→2s→4s, max 3 in 5min)
    └─ broadcasts EngineLifecycleState via watch channel
```

### Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Lifecycle ownership | Single tokio task (supervisor) | Eliminates all Mutex contention — one task, no shared mutable state |
| State broadcasting | `tokio::sync::watch` channel | Lock-free reads for all consumers. Only latest state matters. |
| Command delivery | `tokio::sync::mpsc` channel | Decouples Tauri IPC handlers from lifecycle mutations |
| Health polling | Rust-side supervisor timer | Eliminates frontend setTimeout chain and IPC round-trips |
| Restart policy | Auto-restart, exponential backoff (1s/2s/4s), max 3 in 5min | Recovers from transient crashes; surfaces persistent failures to user |
| Command behavior when engine not ready | Wait with timeout (60s default) | Simplifies frontend — no retry logic needed |
| Engine client crate | Refactor `connect_or_spawn` into composable primitives | Supervisor orchestrates spawn/health/policy directly |
| Launcher integration | Shares same supervisor handle | Eliminates duplicate caching pattern |

---

## State Machine

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum EngineLifecycleState {
    /// No engine process. Initial state.
    Idle,
    /// Engine binary is being spawned.
    Starting,
    /// Process spawned, waiting for HTTP health endpoint to respond.
    WaitingForHealth,
    /// Health passed, engine is running. Model may or may not be loaded.
    Running {
        backend: Option<String>,
        model_id: Option<String>,
    },
    /// Engine process died unexpectedly. Will auto-restart if under limit.
    Crashed {
        message: String,
        restart_count: u32,
    },
    /// Too many crashes or unrecoverable error. User must click Retry.
    Failed {
        message: String,
    },
}
```

**Note:** `EngineClient` is `#[derive(Clone)]` with an Arc-based `reqwest::Client` internally. `get_client()` returns a cheap clone — there is no ownership transfer.

**Note:** The `Stopped` state from the initial draft is removed. On app exit, the supervisor task shuts down the engine and exits — no consumer needs to observe a `Stopped` state.

### Valid Transitions

```
Idle              → Starting              (startup requested)
Starting          → WaitingForHealth      (process spawned successfully)
Starting          → Failed                (spawn error — binary not found, etc.)
WaitingForHealth  → Running               (health check passed)
WaitingForHealth  → Crashed               (health timeout OR process exited before health)
Running           → Running               (status refresh — model loaded/unloaded, backend info updated)
Running           → Crashed               (health check failed)
Crashed           → Starting              (auto-restart, restart_count ≤ 3 within 5min)
Crashed           → Failed                (restart limit exceeded)
Failed            → Starting              (user-initiated retry via Retry button)
```

`WaitingForHealth → Crashed` covers both timeout (engine started but never became healthy) and early process exit (engine crashed during initialization). Both are transient and eligible for auto-restart. After 3 restarts within 5 minutes, the supervisor transitions to `Failed`.

### Restart Behavior: Crashed → Starting

When transitioning from `Crashed` to `Starting`, the supervisor performs:
1. Delete the old token file (`engine-token.txt`)
2. Generate a fresh token via `load_or_create_token()`
3. Construct a new `EngineClient` with the fresh token (replacing the stale cached client)
4. Kill stale engine processes by name (`taskkill /F /IM smolpc-engine-host.exe`)
5. Spawn the engine binary with the fresh token
6. Transition to `WaitingForHealth`

This matches the existing `connect_or_spawn` behavior but is now owned by the supervisor — not scattered across `resolve_client` calls.

### Model Restoration After Restart

The supervisor owns `desired_model: Option<String>`. When the engine reaches `Running` state after a restart:
1. Supervisor checks if `desired_model` is set
2. If set, calls `client.load_model(desired_model)`
3. Updates the `Running` state with the loaded `model_id`
4. If model load fails, remains in `Running { model_id: None }` — the user can manually reload

This means the `Running` state guarantee is: **engine process is alive and healthy**. Model availability is reflected in the `model_id` field. Frontend shows the loading screen until `model_id.is_some()`.

### Status Refresh (Running → Running)

The supervisor learns about model/backend changes via periodic status polling (every health check cycle, 10s). After the health check passes, the supervisor calls `client.status()` and updates the `Running` state's `backend` and `model_id` fields if they changed. This replaces the frontend's `syncStatus()` polling.

Additionally, after any command that mutates engine state (`load_model`, `unload_model`, `ensure_started`), the command handler sends a `RefreshStatus` command to the supervisor, which immediately re-polls the engine status and broadcasts the updated state.

### PID Monitoring

Between health polls (every 10s), the supervisor also checks if the engine PID is still alive using a fast OS-level check (`OpenProcess` on Windows, `kill(pid, 0)` on Unix). This detects crashes faster than waiting for the next HTTP health poll — especially when the engine segfaults and the TCP connection hangs. PID checking adds <1ms per cycle.

---

## Components

### 1. EngineSupervisor (new: single tokio task)

**Location:** New file `apps/codehelper/src-tauri/src/engine/supervisor.rs`

**Responsibilities:**
- Owns the `EngineClient` instance (no Mutex — single owner)
- Owns `RuntimeConfig` (mode preference, DML device)
- Owns `DesiredModel` (model to restore after restart)
- Runs a `tokio::time::interval(10s)` health check loop with PID monitoring
- Receives commands via `mpsc::Receiver<EngineCommand>`
- Broadcasts state via `watch::Sender<EngineLifecycleState>`
- Emits Tauri events via `app_handle.emit("engine-state-changed", &state)` for frontend reactivity
- Auto-restarts on crash with exponential backoff (1s, 2s, 4s), max 3 within 5 minutes
- Restores `desired_model` after successful restart
- Cancels stale operations via `tokio::select!` when state changes

**Core loop (Rust sketch):**
```rust
loop {
    tokio::select! {
        cmd = cmd_rx.recv() => {
            match cmd {
                Some(cmd) => self.handle_command(cmd).await,
                None => break, // All senders dropped — app is shutting down
            }
        },
        _ = health_interval.tick(), if self.state.is_running() => {
            if !self.is_engine_pid_alive() || !self.client_health().await {
                self.transition_to_crashed("Engine health check failed");
                self.schedule_restart();
            } else {
                self.refresh_running_status().await;
            }
        },
        _ = tokio::time::sleep(self.restart_delay), if self.restart_pending => {
            self.restart_pending = false;
            self.do_spawn_sequence().await;
        },
    }
}
```

### 2. EngineSupervisorHandle (new: Tauri managed state)

**Location:** New file `apps/codehelper/src-tauri/src/engine/handle.rs`

**Struct:**
```rust
#[derive(Clone)]
pub struct EngineSupervisorHandle {
    cmd_tx: mpsc::Sender<EngineCommand>,
    state_rx: watch::Receiver<EngineLifecycleState>,
    /// Cached clone of the current EngineClient, updated by a background
    /// watcher task whenever the supervisor broadcasts a new Running state.
    /// This allows get_client_if_ready() to be synchronous.
    cached_client: Arc<std::sync::Mutex<Option<EngineClient>>>,
}
```

**Public API:**
```rust
impl EngineSupervisorHandle {
    /// Instant snapshot of engine state. No network, no lock.
    pub fn current_state(&self) -> EngineLifecycleState;

    /// Wait until engine reaches Running state or timeout.
    /// Returns a clone of EngineClient (Arc-based, cheap to clone).
    /// Used by all Tauri commands that need the engine.
    pub async fn get_client(&self, timeout: Duration) -> Result<EngineClient, String>;

    /// Non-blocking: return client clone only if engine is currently Running.
    /// Used by evaluate_memory_pressure and assistant_cancel.
    pub fn get_client_if_ready(&self) -> Option<EngineClient>;

    /// Request engine startup with config. Waits for Running or Failed.
    pub async fn ensure_started(&self, config: StartupConfig) -> Result<(), String>;

    /// Request runtime mode change. Supervisor handles restart internally.
    pub async fn set_runtime_mode(&self, mode: RuntimeModePreference) -> Result<(), String>;

    /// Request graceful shutdown.
    pub async fn shutdown(&self) -> Result<(), String>;

    /// Tell supervisor to re-poll engine status (after load_model, etc.)
    pub async fn refresh_status(&self);

    /// Subscribe to state changes.
    pub fn subscribe(&self) -> watch::Receiver<EngineLifecycleState>;
}
```

### 3. EngineCommand (new: message types)

**Location:** `apps/codehelper/src-tauri/src/engine/mod.rs`

```rust
pub enum EngineCommand {
    Start {
        config: StartupConfig,
        respond_to: oneshot::Sender<Result<(), String>>,
    },
    SetRuntimeMode {
        mode: RuntimeModePreference,
        respond_to: oneshot::Sender<Result<(), String>>,
    },
    SetDesiredModel {
        model_id: Option<String>,
    },
    RefreshStatus,
    Shutdown {
        respond_to: oneshot::Sender<Result<(), String>>,
    },
}

pub struct StartupConfig {
    pub runtime_mode: RuntimeModePreference,
    pub dml_device_id: Option<i32>,
    pub default_model_id: Option<String>,
    pub startup_mode: StartupMode,
}
```

### 4. smolpc-engine-client crate refactor

**Current monolithic function:** `connect_or_spawn()` (does health check, policy enforcement, spawn lock, process kill, token regen, spawn, health wait — all in one)

**Proposed split into composable primitives:**

```rust
/// Spawn the engine host binary as a detached process.
/// Does NOT wait for health. Returns the child PID.
pub fn spawn_engine(options: &SpawnOptions) -> Result<u32, EngineClientError>;

/// Poll health endpoint until responsive or timeout.
pub async fn wait_for_healthy(
    client: &EngineClient,
    timeout: Duration,
) -> Result<(), EngineClientError>;

/// Check if a running engine satisfies runtime policy.
/// Returns Reuse, Restart, or Reject.
pub async fn check_running_policy(
    client: &EngineClient,
    force_override: Option<&str>,
    force_respawn: bool,
) -> Result<RunningHostPolicyDecision, EngineClientError>;

/// Gracefully shut down a running engine and wait for it to stop.
pub async fn shutdown_and_wait(
    client: &EngineClient,
    timeout: Duration,
) -> Result<(), EngineClientError>;

/// Kill all smolpc-engine-host processes by name (Windows).
pub fn kill_stale_processes();

/// Manage the filesystem spawn lock.
pub async fn with_spawn_lock<F, Fut, T>(
    shared_runtime_dir: &Path,
    f: F,
) -> Result<T, EngineClientError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T, EngineClientError>>;
```

**`connect_or_spawn` is kept as a convenience wrapper** that calls these primitives in sequence. Existing callers (e.g., tests, launcher compat) can keep using it. The supervisor calls the primitives directly.

### 5. Tauri Command Layer Changes

**Deleted:**
- `resolve_client()` — replaced by `handle.get_client()`
- `InferenceState` struct — replaced by `EngineSupervisorHandle`
- `apply_runtime_mode_preference()` — mode changes go through supervisor command
- `connect_lock`, `client` Mutex, `runtime_config` Mutex, `desired_model` Mutex — all eliminated
- `resolve_generation_client()` — replaced by `handle.get_client()`
- `cached_generation_client()` — replaced by `handle.get_client_if_ready()`
- `ensure_desired_model_loaded()` — supervisor handles model restoration

**Commands that migrate from resolve_client to supervisor.get_client():**

| # | Command | Current entry point | New entry point | Notes |
|---|---------|-------------------|-----------------|-------|
| 1 | `list_models` | `resolve_client(false)` | `get_client(60s)` | |
| 2 | `load_model` | `resolve_client(false)` | `get_client(60s)` + `refresh_status()` after | |
| 3 | `unload_model` | `resolve_client(false)` | `get_client(60s)` + `refresh_status()` after | |
| 4 | `get_current_model` | `resolve_client(false)` | `get_client(60s)` | |
| 5 | `get_inference_backend_status` | `resolve_client(false)` | `get_client(60s)` | |
| 6 | `set_inference_runtime_mode` | `resolve_client(true)` + rollback | `supervisor.set_runtime_mode()` | Supervisor handles restart |
| 7 | `check_model_readiness` | `resolve_client(false)` | `get_client(60s)` | |
| 8 | `check_model_exists` | `resolve_client(false)` | `get_client(60s)` | |
| 9 | `inference_generate` | `resolve_client(false)` | `get_client(60s)` | |
| 10 | `inference_generate_messages` | `resolve_client(false)` | `get_client(60s)` | |
| 11 | `inference_cancel` | `resolve_client(false)` | `get_client(60s)` | |
| 12 | `is_generating` | `resolve_client(false)` | `get_client(60s)` | |
| 13 | `engine_status` | `resolve_client(false)` | `get_client(60s)` | |
| 14 | `engine_ensure_started` | `resolve_client(mode_changed)` | `supervisor.ensure_started()` | Supervisor handles mode+spawn |
| 15 | `assistant_send` | `resolve_generation_client()` | `get_client(60s)` | 3 match arms, same change |

**Special cases (no resolve_client, but use Mutex-based client access):**

| Command | Current | New | Notes |
|---------|---------|-----|-------|
| `engine_health_only` | `state.check_health()` | `supervisor.current_state().is_running()` | Instant, no HTTP |
| `evaluate_memory_pressure` | `state.cached_healthy_client()` | `supervisor.get_client_if_ready()` | Non-blocking |
| `assistant_cancel` | `cached_generation_client()` | `supervisor.get_client_if_ready()` | Non-blocking for cancel path |

**Out-of-scope commands (do NOT touch the engine):**
`mode_undo`, `launcher_list_apps`, `launcher_launch_or_focus`, `list_modes`, `mode_status`, `mode_refresh_tools`, `mode_open_host_app`, `setup_status`, `setup_prepare`, `detect_hardware`, `get_cached_hardware`, `read`, `write`, `save_code`, `get_benchmarks_directory`, `open_benchmarks_folder`, `run_benchmark` (currently a no-op stub).

### 6. Frontend Changes

**Deleted:**
- Health polling `$effect` (setTimeout chain) — supervisor owns health polling now
- `reestablishEngineSession()` — supervisor handles reconnection
- `reconnectingEngineSession` state — supervisor broadcasts state transitions
- `checkHealth()` Tauri invoke — replaced by event listener

**New: Tauri event listener**
```typescript
// In App.svelte onMount:
const unlisten = await listen<EngineLifecycleState>('engine-state-changed', (event) => {
    inferenceStore.updateLifecycleState(event.payload);
});
```

**TypeScript type for event payload:**
```typescript
type EngineLifecycleState =
    | { state: 'idle' }
    | { state: 'starting' }
    | { state: 'waiting_for_health' }
    | { state: 'running'; backend: string | null; model_id: string | null }
    | { state: 'crashed'; message: string; restart_count: number }
    | { state: 'failed'; message: string };
```

**Mapping to existing store fields:**

| Lifecycle State | `engineHealthy` | `readiness.state` | `error` | `isReady` |
|----------------|-----------------|-------------------|---------|-----------|
| `idle` | false | `'idle'` | null | false |
| `starting` | false | `'starting'` | null | false |
| `waiting_for_health` | false | `'starting'` | null | false |
| `running` (model_id=null) | true | `'loading_model'` | null | false |
| `running` (model_id=some) | true | `'ready'` | null | true |
| `crashed` | false | `'failed'` | message | false |
| `failed` | false | `'failed'` | message | false |

**`EngineReadinessDto` is kept** for the return value of `engine_ensure_started` and `engine_status` Tauri commands — these still return detailed readiness info from the engine HTTP API. The lifecycle state is a higher-level abstraction that the frontend uses for UI rendering. Both coexist: lifecycle state drives the banner/loading screen, readiness DTO drives the status panel details.

**inference.svelte.ts changes:**
- New `lifecycleState` reactive field
- `updateLifecycleState(state)` — called from event listener, updates `engineHealthy`, `error`, derived states
- `forceResetGenerationState()` fires when `lifecycleState` transitions to `crashed` or `failed`
- `ensureStarted()` still invokes `engine_ensure_started` which now waits on the supervisor internally

### 7. Launcher Integration

**Current:** `launcher/orchestrator.rs` has its own `resolve_engine_client` with a separate `EngineClient` cache and its own `Mutex<Option<EngineClient>>`.

**After:** The launcher receives the same `EngineSupervisorHandle` via Tauri managed state. All `resolve_engine_client` calls become `supervisor.get_client()`. Delete the launcher's separate client cache, connect lock, and runtime config.

---

## Production Deployment Considerations

### Dev vs Production Path Resolution

| Item | Development | Production (NSIS currentUser) |
|------|-------------|-------------------------------|
| App binary | `target/debug/smolpc-code-helper.exe` | `%LOCALAPPDATA%/SmolPC Code Helper/smolpc-code-helper.exe` |
| Engine binary | `target/debug/smolpc-engine-host.exe` (workspace fallback) | `%LOCALAPPDATA%/SmolPC Code Helper/resources/binaries/smolpc-engine-host.exe` |
| Bundled resources | `apps/codehelper/src-tauri/resources/` | `%LOCALAPPDATA%/SmolPC Code Helper/resources/` |
| Engine state files | `%LOCALAPPDATA%/SmolPC/engine-runtime/` | Same |
| Models | `%LOCALAPPDATA%/SmolPC/models/` or bundled | Same |
| Engine port | 19432 (or `SMOLPC_ENGINE_PORT`) | Same |

### What the Supervisor Must Handle for Production

1. **Binary resolution:** The supervisor resolves the engine binary path using the SAME fallback chain as current `resolve_host_binary_path()` + `resolve_host_binary()`. In dev, this finds `target/debug/smolpc-engine-host.exe`. In production, it finds the bundled binary via `app_handle.path().resource_dir()`. **No change to resolution logic — just moved into the supervisor's spawn method.**

2. **Detached process lifecycle:** On Windows, the engine is spawned with `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`. This means the engine survives app exit. The supervisor must:
   - Write `engine.pid` after spawn (already done in `spawn_host`)
   - Verify PID identity before force-killing stale processes (CLAUDE.md learning)
   - Clean up `engine.pid` on both graceful exit and after force-kill
   - Redirect stderr to `engine-spawn.log` (not `Stdio::null()`)

3. **Single-instance assumption:** Only one Code Helper instance runs at a time. The supervisor doesn't need to coordinate with other app instances. However, the file-based spawn lock (`engine-spawn.lock`) is kept for safety — it prevents races if the launcher also tries to connect.

4. **Token management in production:** The auth token lives at `%LOCALAPPDATA%/SmolPC/engine-runtime/engine-token.txt`. The supervisor follows the existing pattern: load token on startup, regenerate on engine death (stale tokens from a dead host are the #1 cause of "unhealthy after idle" per CLAUDE.md).

5. **DLL loading:** The engine host loads OpenVINO DLLs from the bundled `libs/` directory. The supervisor passes `resource_dir` to the engine so it can find them. Unchanged from current behavior.

6. **App exit cleanup:** When the Tauri app exits (`RunEvent::ExitRequested`), the supervisor receives a `Shutdown` command (via dropping the `cmd_tx` sender, which causes `cmd_rx.recv()` to return `None`). It sends `POST /engine/shutdown` to the engine, waits briefly, then the task exits. If the engine doesn't respond, the supervisor kills it via PID.

### Other Components Affected

| Component | Current Interaction | After Redesign | Migration Notes |
|-----------|-------------------|----------------|-----------------|
| **InferenceModeSelector** | Calls `inferenceStore.setRuntimeMode()` → destructive `force_respawn` | Same frontend API, but backend sends `SetRuntimeMode` command to supervisor. Supervisor handles restart internally. | Frontend unchanged. Backend command handler simplified. |
| **StartupLoadingScreen** | Reads `inferenceStore.isReady` + `readiness.state` | Reads `lifecycleState` from event stream. Shows loading for `starting`, `waiting_for_health`, `running(model_id=null)`. | Replace readiness checks with lifecycle state checks. |
| **StatusIndicator / HardwarePanel** | Reads `inferenceStore.backendStatus` via `refreshBackendStatus()` | `get_inference_backend_status` command still queries engine directly (unchanged). Supervisor's `Running.backend` field provides a cache. | Backend status fetch is unchanged — it's a read-only query. |
| **SetupBanner / SetupPanel** | Independent of engine lifecycle | Unchanged | No migration needed. |
| **Assistant modes (GIMP/Blender/LibreOffice)** | `assistant_send` → `resolve_generation_client` → `resolve_client` | `assistant_send` → `supervisor.get_client()` | One-line change. `assistant_cancel` → `get_client_if_ready()`. |
| **Memory pressure polling** | Frontend polls `evaluate_memory_pressure` every 10s | Keep frontend polling with `get_client_if_ready()` (non-blocking). | `evaluate_memory_pressure` already uses `cached_healthy_client()` — just swap to supervisor equivalent. |

### Production Testing Checklist

Before shipping the supervisor redesign, verify in a **release build** (not dev):

1. Engine binary resolves correctly from `resource_dir/binaries/`
2. Engine spawns as detached process with correct env vars
3. Engine PID file is written and cleaned up
4. Token regeneration works after engine death
5. Spawn lock prevents double-spawn during rapid restart
6. Stderr redirects to spawn log (crash diagnostics available)
7. App exit triggers graceful engine shutdown
8. Stale PID from previous crash doesn't kill wrong process
9. Auto-restart works 3 times, then surfaces error
10. Model loads correctly on preferred backend (NPU) in single spawn

---

## Migration Strategy

### Phase 0: Refactor Engine Client Crate Primitives

Split `connect_or_spawn` into composable primitives (`spawn_engine`, `wait_for_healthy`, `check_running_policy`, `shutdown_and_wait`, `kill_stale_processes`, `with_spawn_lock`). Keep `connect_or_spawn` as a convenience wrapper that calls them in sequence. All existing callers continue to work.

**Why first:** The supervisor (Phase 1) needs these primitives. Building on the monolithic `connect_or_spawn` would require refactoring it later anyway.

**Risk:** Low — additive. `connect_or_spawn` is preserved as a wrapper. No existing behavior changes.

### Phase 1: Supervisor Core (Rust-side)

Create the supervisor task, handle, state machine, and command types in a new `apps/codehelper/src-tauri/src/engine/` module. Wire up `Builder::setup()` to spawn the supervisor. The supervisor uses the Phase 0 primitives. Keep all existing Tauri commands using `InferenceState` — the supervisor runs alongside it.

**Risk:** Low — additive, no existing code changed.

### Phase 2: Migrate Tauri Commands (atomic groups)

Migrate commands in groups to avoid inconsistent state:

**Group A (state-mutating, migrate together):**
- `engine_ensure_started` → `supervisor.ensure_started()`
- `set_inference_runtime_mode` → `supervisor.set_runtime_mode()`
- `load_model`, `unload_model` → `get_client()` + `refresh_status()`

**Group B (read-only, migrate after Group A):**
- `list_models`, `get_current_model`, `get_inference_backend_status`
- `engine_status`, `check_model_readiness`, `check_model_exists`
- `is_generating`

**Group C (generation, migrate after Group B):**
- `inference_generate`, `inference_generate_messages`, `inference_cancel`
- `assistant_send`, `assistant_cancel`

**Group D (special cases):**
- `engine_health_only` → `supervisor.current_state()`
- `evaluate_memory_pressure` → `supervisor.get_client_if_ready()`

**Safety net during migration:** The supervisor's health loop detects when its cached client is stale (engine was killed by old-path code) and transitions to `Crashed`. This prevents inconsistent state during the transition period where some commands use the old path and others use the supervisor.

Delete `resolve_client`, `InferenceState`, and related functions when all groups are migrated.

**Risk:** Medium — each group is testable. State-mutating commands go first to prevent the old path from killing engines the supervisor manages.

### Phase 3: Migrate Frontend

Replace the health polling `$effect` with Tauri event listener. Replace `reestablishEngineSession` with event-driven reactivity. Delete `reconnectingEngineSession` state. Update components that read `engineHealthy`/`readiness` to use the new lifecycle state mapping.

**Risk:** Medium — changes reactive flow but behavior is equivalent.

### Phase 4: Migrate Launcher

Replace launcher's `resolve_engine_client` with the shared `EngineSupervisorHandle`. Delete the launcher's separate client cache.

**Risk:** Low — launcher is a simple consumer.

### Phase 5: Cleanup

Remove `InferenceState`, `resolve_client`, `resolve_generation_client`, `cached_generation_client`, `ensure_desired_model_loaded`, `apply_runtime_mode_preference`, all dead Mutex imports. Run full test suite. Verify production build.

---

## Success Criteria

1. `taskkill /F /IM smolpc-engine-host.exe` → engine restarts automatically within 10s, banner clears
2. App startup produces exactly ONE engine spawn (not 2-3)
3. No `connect_lock` or `client Mutex` contention in logs
4. Runtime mode switch (NPU/CPU/DML) completes without double-spawn
5. 3 consecutive crashes → error surfaced to user with Retry button
6. App exit → engine shuts down cleanly (no orphan processes)
7. Production (NSIS) build works identically to dev build
8. Launcher shares the same engine session (no separate client cache)
