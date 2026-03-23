# Engine Supervisor — Lifecycle Controller

## Overview

The engine supervisor is a single background Tokio task that owns the complete lifecycle of the `smolpc-engine-host` process — spawning, health monitoring, automatic restart on crash, and graceful shutdown. It replaces the previous `InferenceState` pattern where 15+ concurrent Tauri commands competed through a shared `resolve_client()` function protected by 4 Mutexes.

**Key property:** Zero shared mutable state. The supervisor owns all engine state exclusively. External consumers interact only via message passing (mpsc channels for commands, watch channels for state observation).

**Module location:** `apps/codehelper/src-tauri/src/engine/`

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Tauri IPC Commands                                             │
│  (engine_ensure_started, load_model, inference_generate, etc.)  │
│                                                                 │
│  supervisor.get_client(timeout) ──→ waits on watch channel      │
│  supervisor.ensure_started(config) ──→ sends Start via mpsc     │
└───────────────┬─────────────────────────────────┬───────────────┘
                │ mpsc::Sender<EngineCommand>      │ watch::Receiver
                ▼                                  │
┌───────────────────────────────────┐              │
│  EngineSupervisor (single task)   │              │
│                                   │              │
│  Owns:                            │  Broadcasts: │
│  ├─ EngineClient                  │  ├─ EngineLifecycleState (watch)
│  ├─ RuntimeConfig (mode, device)  │  ├─ Option<EngineClient> (watch)
│  ├─ DesiredModel                  │  └─ Tauri events (for frontend)
│  ├─ engine PID                    │              │
│  └─ restart policy state          │              │
│                                   │              │
│  Loop:                            │              │
│  ├─ recv command (mpsc)           │              │
│  ├─ health check (10s timer)      │              │
│  └─ restart delay (conditional)   │              │
└───────────────────────────────────┘              │
                                                   ▼
┌─────────────────────────────────────────────────────────────────┐
│  Frontend (Svelte)                                              │
│                                                                 │
│  listen('engine-state-changed') ──→ updateLifecycleState()      │
│  Maps to: engineHealthy, readiness, error, isReady              │
│  Drives: banner, loading screen, generation gates               │
└─────────────────────────────────────────────────────────────────┘
```

---

## State Machine

```
  Idle ──→ Starting ──→ WaitingForHealth ──→ Running
                              │                  │
                              ▼                  ▼
                           Crashed ←─────────────┘
                              │
                         (restart ≤ 3)
                              │
                              ▼
                           Starting ──→ ...
                              │
                         (restart > 3)
                              │
                              ▼
                           Failed ──→ (user retry) ──→ Starting
```

### States

| State | Description | `engineHealthy` | `isReady` |
|-------|-------------|:---------------:|:---------:|
| `Idle` | No engine process. Initial state. | false | false |
| `Starting` | Engine binary being spawned. | false | false |
| `WaitingForHealth` | Process spawned, waiting for HTTP health endpoint. | false | false |
| `Running` | Engine healthy. `model_id` may be None (loading) or Some (ready). | true | model_id.is_some() |
| `Crashed` | Engine died unexpectedly. Auto-restart scheduled. | false | false |
| `Failed` | Too many crashes or unrecoverable error. User must retry. | false | false |

### Restart Policy

- **Max restarts:** 3 within a 5-minute window
- **Backoff delays:** 1s, 2s, 4s (exponential)
- **On 4th crash:** Transition to `Failed` — user sees error with Retry button
- **On user retry (Failed → Starting):** Restart counter resets

### Desired Model Restoration

After each restart, the supervisor checks `desired_model`:
- If set: calls `client.load_model(model_id)` on the new engine
- If first attempt fails: retries once after 2 seconds
- If retry also fails: stays in `Running { model_id: None }` (engine usable, no model)

`desired_model` is set by:
- `engine_ensure_started` — when the initial startup loads a model via startup policy
- `load_model` command — when user explicitly loads a model
- `unload_model` command — clears to None
- `set_inference_runtime_mode` — when mode switch loads a model on the new backend

---

## Components

### `engine/mod.rs` — Types

| Type | Kind | Purpose |
|------|------|---------|
| `EngineLifecycleState` | Enum (serde tagged) | Lifecycle states broadcast via watch channel and Tauri events |
| `EngineCommand` | Enum | Commands sent to supervisor via mpsc channel |
| `StartupConfig` | Struct | Configuration for engine startup (runtime mode, device, default model) |

`EngineLifecycleState` derives `Serialize` with `#[serde(tag = "state", rename_all = "snake_case")]` so it serializes as `{ "state": "running", "backend": "openvino_npu", "model_id": "qwen2.5" }` — directly consumable by the frontend TypeScript discriminated union.

### `engine/handle.rs` — Handle (Tauri Managed State)

`EngineSupervisorHandle` is the public API. It is `Clone` and registered as Tauri managed state. Every Tauri command receives it via `State<'_, EngineSupervisorHandle>`.

| Method | Blocking? | Purpose |
|--------|-----------|---------|
| `get_client(timeout)` | async, waits | Returns `EngineClient` clone when engine reaches Running. Watches both state and client channels via `tokio::select!`. |
| `get_client_if_ready()` | non-blocking | Returns `Some(EngineClient)` if engine is Running, `None` otherwise. Used by `evaluate_memory_pressure` and `assistant_cancel`. |
| `ensure_started(config)` | async, waits | Sends `Start` command, waits for supervisor response via oneshot. |
| `set_runtime_mode(mode)` | async, waits | Sends `SetRuntimeMode` command, waits for response. Supervisor handles engine restart internally. |
| `shutdown()` | async, waits | Sends `Shutdown` command, waits for response. |
| `refresh_status()` | async, fire-and-forget | Tells supervisor to re-poll engine status. |
| `set_desired_model(model_id)` | async, fire-and-forget | Updates the model the supervisor should restore after crash. |

**Client cache race protection:** `get_client()` uses `tokio::select!` on both the state watch and client watch channels. The supervisor always broadcasts the client BEFORE broadcasting the Running state, guaranteeing the client is available when any consumer observes Running.

### `engine/supervisor.rs` — Supervisor Task

The supervisor is a `struct EngineSupervisor<R: Runtime>` with a `pub async fn run(mut self)` method that consumes self and runs until all senders are dropped (app exit).

**Core loop:**
```rust
loop {
    tokio::select! {
        cmd = self.cmd_rx.recv() => { /* handle command */ },
        _ = health_interval.tick(), if self.state.is_running() => { /* health check */ },
        _ = sleep(self.restart_delay), if self.restart_pending => { /* restart */ },
    }
}
```

**Health check (every 10s when Running):**
1. PID alive check (OS-level, <1ms)
2. HTTP health endpoint ping
3. If either fails: transition to `Crashed`, schedule restart
4. If healthy: refresh running status (backend, model_id)

**Spawn sequence (9 steps):**
1. Resolve paths (app data, runtime, resource, models, host binary, port)
2. Delete old auth token
3. Regenerate auth token via `load_or_create_token()`
4. Create new `EngineClient` with fresh token
5. Kill stale engine processes (`taskkill /F /IM smolpc-engine-host.exe`)
6. Spawn engine binary as detached process
7. Wait for HTTP health endpoint (100ms poll, 30s timeout)
8. Transition to `Running`, broadcast client
9. Restore desired model (if set, with retry-once)

---

## Frontend Integration

### Event Listener

The frontend listens for supervisor state changes via Tauri events:

```typescript
listen<EngineLifecycleState>('engine-state-changed', (event) => {
    inferenceStore.updateLifecycleState(event.payload);
});
```

### TypeScript Type

```typescript
type EngineLifecycleState =
    | { state: 'idle' }
    | { state: 'starting' }
    | { state: 'waiting_for_health' }
    | { state: 'running'; backend: string | null; model_id: string | null }
    | { state: 'crashed'; message: string; restart_count: number }
    | { state: 'failed'; message: string };
```

### State Mapping

| Lifecycle State | `engineHealthy` | `readiness.state` | `isReady` | UI |
|:---------------|:---------------:|:------------------:|:---------:|:---|
| `idle` | false | `idle` | false | Loading screen |
| `starting` | false | `starting` | false | Loading screen |
| `waiting_for_health` | false | `starting` | false | Loading screen |
| `running` (model_id=null) | true | `loading_model` | false | Loading screen |
| `running` (model_id=some) | true | `ready` | true | Chat ready |
| `crashed` | false | `failed` | false | Auto-restart banner |
| `failed` | false | `failed` | false | Error + Retry button |

### Startup Sequence

```
App.svelte onMount:
  1. Set up event listener for 'engine-state-changed'
  2. initInference():
     a. performStartup()    ← starts engine, applies runtime mode, loads model
     b. listModels()        ← populates model selector (engine already running)
     c. pollMemoryPressure()
```

`performStartup()` passes the user's runtime mode preference in the `ensureStarted()` request, so the engine spawns on the correct backend (NPU/CPU/DirectML) in a **single spawn** — no double-startup.

---

## Tauri Command Integration

All 15+ Tauri commands that need the engine use the supervisor handle:

| Pattern | Commands | Method |
|---------|----------|--------|
| Wait for engine | `list_models`, `load_model`, `inference_generate`, `inference_generate_messages`, `inference_cancel`, `engine_status`, `get_inference_backend_status`, `check_model_readiness`, etc. | `supervisor.get_client(60s)` |
| Non-blocking | `evaluate_memory_pressure`, `assistant_cancel` | `supervisor.get_client_if_ready()` |
| Lifecycle control | `engine_ensure_started` | `supervisor.ensure_started(config)` |
| Mode switch | `set_inference_runtime_mode` | `supervisor.set_runtime_mode(mode)` |

No command directly spawns, kills, or interacts with the engine process. The supervisor is the sole lifecycle owner.

---

## Configuration & Paths

### Runtime Paths

| Item | Development | Production (NSIS) |
|------|-------------|-------------------|
| Engine binary | `target/debug/smolpc-engine-host.exe` | `%LOCALAPPDATA%/SmolPC Code Helper/resources/binaries/smolpc-engine-host.exe` |
| Engine state files | `%LOCALAPPDATA%/SmolPC/engine-runtime/` | Same |
| Auth token | `engine-runtime/engine-token.txt` | Same |
| PID file | `engine-runtime/engine.pid` | Same |
| Spawn log | `engine-runtime/engine-spawn.log` | Same |
| Spawn lock | `engine-runtime/engine-spawn.lock` | Same |

### Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `HEALTH_CHECK_INTERVAL` | 10s | How often the supervisor pings the engine |
| `MAX_RESTARTS` | 3 | Maximum auto-restarts within one window |
| `RESTART_WINDOW` | 5 minutes | Window for counting restarts |
| `BACKOFF_DELAYS` | [1s, 2s, 4s] | Exponential backoff between restarts |
| `DEFAULT_ENGINE_PORT` | 19432 | HTTP port for engine API |

### Environment Overrides

| Variable | Effect |
|----------|--------|
| `SMOLPC_ENGINE_PORT` | Override engine HTTP port |
| `SMOLPC_FORCE_EP` | Force backend: `cpu`, `dml`, `openvino_npu` |
| `SMOLPC_DML_DEVICE_ID` | Force DirectML device index |
| `SMOLPC_ENGINE_HOST_BIN` | Override engine binary path |
| `SMOLPC_MODELS_DIR` | Override models directory |

---

## Design Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| Single owner task vs shared Mutexes | Single owner | Previous 4-Mutex design deadlocked on engine death |
| watch channels vs Tauri events | Both | watch for Rust consumers, Tauri events for frontend |
| mpsc + oneshot vs direct method calls | mpsc + oneshot | Decouples IPC handlers from state mutations |
| Health polling in Rust vs frontend | Rust | Eliminates IPC round-trips, faster detection |
| PID check + HTTP health | Both | PID check is instant (<1ms), HTTP confirms API readiness |
| Exponential backoff | 1s/2s/4s, max 3 | Recovers from transient crashes, stops on persistent failures |
| `tauri::async_runtime::spawn` | Required | `tokio::spawn` panics inside `Builder::setup()` |
| Client broadcast before state | Required | Prevents race where consumer sees Running but client cache is empty |

---

## Testing

### Unit Tests (Rust)

| Location | Tests | Coverage |
|----------|-------|----------|
| `engine/mod.rs` | 6 tests | State transitions, serialization, helper methods |
| `engine/handle.rs` | 10 tests | Clone, get_client timeout/terminal/race, command routing |

### Manual Testing Checklist

1. **Fast startup** — Engine starts within 2s, model loads within 10s
2. **Single spawn** — One "Supervisor: engine spawned with PID" in logs
3. **Kill recovery** — `taskkill /F /IM smolpc-engine-host.exe` → auto-restart within 10s, model restored
4. **3x crash → Failed** — Kill 3 times rapidly → error with Retry shown
5. **Mode switch** — Switch backend via InferenceModeSelector → single restart, no double-spawn
6. **App exit** — Close app → engine shuts down, no orphan process
7. **Generation during restart** — UI shows "engine disconnected", generation state cleared, retry works after recovery

---

## History

This module was designed in response to issue #196 (engine death leaves app stuck) and issue #173 (double startup). The previous `InferenceState` + `resolve_client()` architecture had fundamental Mutex contention problems documented in `docs/ENGINE_LIFECYCLE_AUDIT.md`. The full design spec is at `docs/superpowers/specs/2026-03-23-engine-supervisor-redesign.md`.
