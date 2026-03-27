# Engine Lifecycle

This document covers how the engine process is spawned, monitored, recovered from crashes, and shut down. The lifecycle is owned by the `EngineSupervisor` actor task in the Tauri app — no Tauri command touches the engine process directly.

## Overview

The engine is a separate OS process (`smolpc-engine-host.exe`) that runs as a detached, windowless background service. The Tauri app communicates with it over HTTP on `localhost:19432`. A supervisor actor manages the full lifecycle: spawn, health monitoring, automatic restart on crash, and graceful shutdown.

---

## State Machine

```
Idle ──Start──► Starting ──spawn──► WaitingForHealth ──healthy──► Running
                                                                     │
                                                          health loop (10s)
                                                          PID alive + HTTP
                                                                     │
                                                         3 failures ──► Crashed
                                                                          │
                                                              backoff restart
                                                              1s → 2s → 4s
                                                                          │
                                                         max 3 per 5min ──► Failed
```

### States

| State | Description |
|-------|-------------|
| `Idle` | No engine running. Waiting for a `Start` command. |
| `Starting` | Spawn sequence in progress. |
| `WaitingForHealth` | Engine process launched, polling `/engine/health`. |
| `Running` | Engine is healthy and serving requests. Carries current backend and model ID. |
| `Crashed` | Engine process died or became unresponsive. Carries error message and restart count. |
| `Failed` | Max restarts exceeded. Terminal state — requires manual intervention (app restart). |

Transitions are validated — invalid transitions (e.g., `Idle → Running`) are rejected with an error log.

---

## Supervisor Architecture

The `EngineSupervisor` is a Tokio task spawned during Tauri app initialization. It runs a `select!` loop that handles three event sources concurrently:

1. **Command channel** (`mpsc`, buffer 16) — receives commands from Tauri command handlers
2. **Health check timer** (10-second interval) — fires only when in `Running` state
3. **Restart delay timer** — fires after backoff period when a restart is pending

### Commands

| Command | Description | Response |
|---------|-------------|----------|
| `Start { config }` | Begin spawn sequence with runtime mode and desired model | `oneshot` Ok/Err |
| `SetRuntimeMode { mode }` | Change backend (triggers engine restart) | `oneshot` Ok/Err |
| `SetDesiredModel { model_id }` | Set the model to load (or None to clear) | Fire-and-forget |
| `RefreshStatus` | Re-read engine status and update state | Fire-and-forget |
| `Shutdown` | Graceful shutdown | `oneshot` Ok/Err |

### Watch Channels

External code reads engine state through watch channels (no locks, no contention):

| Channel | Type | Purpose |
|---------|------|---------|
| `state_tx` | `EngineLifecycleState` | Current lifecycle state |
| `client_tx` | `Option<EngineClient>` | Connected HTTP client or None |
| `pid_tx` | `Option<u32>` | Engine process ID or None |

### Frontend Integration

Every state transition does two things:
1. Broadcasts the new state via the watch channel
2. Emits a Tauri event `"engine-state-changed"` with the serialized state

The frontend listens for this event and updates `inferenceStore.lifecycleState`, which drives the UI (loading screens, error states, ready indicator).

---

## Spawn Sequence

When the supervisor receives a `Start` command, `do_spawn_sequence()` executes these steps:

1. **Resolve paths** — port, data directory, resource directory, app version
2. **Create directories** — ensure runtime and data directories exist
3. **Generate auth token** — delete old token file, generate a fresh 48-character alphanumeric token via OS random generator, write to `engine-token.txt`
4. **Create HTTP client** — construct `EngineClient` with the fresh token and `http://127.0.0.1:19432`
5. **Kill stale processes** — run `taskkill /F /IM smolpc-engine-host.exe` with `CREATE_NO_WINDOW` to clean up orphans
6. **Spawn engine process** — launch `smolpc-engine-host.exe` as a detached process:
   - **Process flags (Windows):** `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`
   - **CLI args:** `--port`, `--data-dir`, `--app-version`, optionally `--resource-dir`
   - **Environment:** `SMOLPC_ENGINE_TOKEN`, `RUST_LOG=info`, optionally `SMOLPC_FORCE_EP` and `SMOLPC_DML_DEVICE_ID` based on runtime mode
   - **Stderr:** redirected to `engine-spawn.log` (append mode) — never `Stdio::null()`
   - **PID file:** written to `engine.pid` in the shared runtime directory
7. **Broadcast PID** — send PID to watch channel
8. **Transition to WaitingForHealth**
9. **Wait for healthy** — poll `/engine/health` every 100ms with a 30-second timeout
10. **Transition to Running** — reset failure count, broadcast client
11. **Refresh status** — read `/engine/status` to populate backend and model info
12. **Restore desired model** — if a model was previously set, call `load_model()` with one retry after 2 seconds on failure

---

## Health Check Loop

While in `Running` state, the supervisor runs a health check every 10 seconds:

1. **Check PID liveness** — verify the engine process is still alive via `OpenProcess` (Windows) or `kill(pid, 0)` (Unix)
2. **HTTP health check** — call `GET /engine/health`
3. **On success:** reset failure counter to 0
4. **On failure:** increment failure counter
5. **After 3 consecutive failures:** transition to `Crashed`

The PID check catches cases where the process died but the port hasn't been released yet. The HTTP check catches cases where the process is alive but hung.

---

## Automatic Restart

When the engine crashes, the supervisor attempts automatic recovery:

### Restart Policy

| Parameter | Value |
|-----------|-------|
| Max restarts per window | 3 |
| Window duration | 5 minutes |
| Backoff delays | 1s, 2s, 4s (indexed by restart count) |

### Restart Flow

1. Engine transitions to `Crashed` with error message
2. Supervisor checks if within the restart window:
   - If window has expired (>5 minutes since first restart): reset counter, start fresh
   - If under max restarts: schedule restart with backoff delay
   - If at max restarts: transition to `Failed` (terminal)
3. After the backoff delay, `do_spawn_sequence()` runs again
4. The desired model is automatically restored after successful restart

### Model Auto-Reload

On every spawn sequence completion, the supervisor checks `desired_model`:
- If set, attempts `load_model()` immediately
- On first failure, waits 2 seconds and retries once
- On second failure, logs the error but remains in `Running` state without a model — the engine is still usable, just without a loaded model

---

## Auth Token Management

The engine and app share a token for HTTP authentication:

1. **Generation:** 48-character alphanumeric string via `OsRng` (cryptographically secure)
2. **Persistence:** written to `engine-token.txt` in the shared runtime directory
3. **Race handling:** uses `create_new` file mode to prevent concurrent writes. If a file already exists (another instance created it), reads the existing token instead
4. **Refresh on spawn:** token is deleted and regenerated on each spawn sequence to prevent stale token reuse
5. **Unix permissions:** file created with mode `0o600` (owner read/write only)

---

## Spawn Lock

A file-based lock prevents multiple app instances from spawning the engine simultaneously:

1. **Lock file:** `engine-spawn.lock` in the shared runtime directory
2. **Acquisition:** try `create_new` — if file exists, check if lock holder is dead
3. **Stale detection:** if lock file is older than 30 seconds, delete and retry
4. **Wait timeout:** if lock isn't acquired within 10 seconds, force-delete and try once more
5. **PID identity:** lock file contains `pid=<owner_pid>`, verified via `OpenProcess` (Windows) or `kill(pid, 0)` (Unix)
6. **Cleanup:** lock file is automatically deleted when the `SpawnLockGuard` is dropped

---

## Graceful Shutdown

### App Exit Path

When the Tauri app receives `ExitRequested`:

1. **Stop audio** — halt recording and playback synchronously (prevents Windows WASAPI session corruption)
2. **Snapshot PID** — capture current engine PID before shutdown clears it
3. **Send shutdown command** — `supervisor_handle.shutdown()` with 8-second outer timeout
4. **Supervisor handles it:**
   - Sends `POST /engine/shutdown` to the engine with 5-second timeout
   - Engine receives shutdown, notifies its internal shutdown channel, stops accepting connections
   - Supervisor clears client and PID, broadcasts None
5. **On success:** clean up PID file
6. **On timeout or error:** force-kill via PID as fallback

### Force Kill Fallback

If graceful shutdown fails (timeout or error):

1. **Verify PID identity** — on Windows, run `tasklist` to confirm the PID is still `smolpc-engine-host.exe` (PIDs are reused by the OS)
2. **Kill** — `taskkill /F /PID <pid>` (Windows) or `SIGKILL` (Unix)
3. **Cleanup** — delete PID file

The 8-second outer timeout intentionally exceeds the supervisor's internal 5-second timeout. If the engine responds to HTTP shutdown within 5 seconds, the supervisor reports success. If not, the 8-second timeout triggers force-kill.

---

## TTS Sidecar Lifecycle

The TTS sidecar (`smolpc-tts-server`) is a separate process managed by the engine host, not the supervisor:

1. **Spawn:** engine host launches the sidecar during startup on port 19433
   - Flags: `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`
   - Args: `--port`, `--model-dir` (kittentts-nano), optionally `--espeak-dir`
   - Env: `SMOLPC_ENGINE_TOKEN`, `RUST_LOG=info`, optionally `ESPEAK_DATA_PATH`
   - Stderr: appended to `tts-spawn.log`
   - PID: written to `tts.pid`
2. **Health check:** polled at 200ms intervals during startup, 15-second total budget, 3-second per-check timeout
3. **Proxy:** the engine host proxies `/v1/audio/speech` requests to `http://127.0.0.1:19433/synthesize`
4. **Respawn:** if a speech request finds the sidecar unhealthy, the engine attempts one respawn before returning 503
5. **Shutdown:** killed when the engine process exits (child process of engine)

---

## Process Architecture Diagram

```
┌──────────────────────┐
│    Tauri Desktop App  │
│                       │
│  EngineSupervisor     │
│  (Tokio task)         │
│    │                  │
│    │ mpsc commands    │
│    │ watch broadcasts │
│    │                  │
│  Tauri Commands       │
│  (read handle only)   │
└──────────┬────────────┘
           │
           │ HTTP (:19432)
           │ Bearer token auth
           │
┌──────────▼────────────┐         ┌─────────────────────┐
│  smolpc-engine-host   │ spawn   │  smolpc-tts-server   │
│  (detached process)   │────────►│  (detached process)  │
│                       │         │                      │
│  PID: engine.pid      │ proxy   │  PID: tts.pid        │
│  Log: engine-spawn.log│────────►│  Log: tts-spawn.log  │
│  Port: 19432          │  :19433 │  Port: 19433         │
└───────────────────────┘         └──────────────────────┘
```

Key properties:
- Both processes are detached — they survive parent exit
- Both use `CREATE_NO_WINDOW` — no console flash
- Both redirect stderr to log files — crash diagnosis is always possible
- PID files enable identity verification before force-kill
- Token is shared via environment variable, not command line (prevents `ps` exposure)

---

## Connect-or-Spawn (Client Library)

The `connect_or_spawn()` function in `smolpc-engine-client` provides a convenience wrapper for consumers that don't use the supervisor:

1. Create runtime and data directories
2. Load or create auth token from `engine-token.txt`
3. Create initial HTTP client
4. **First policy check (without lock):**
   - If engine is healthy and compatible: reuse it
   - If protocol mismatch and engine is idle: shut it down
   - If protocol mismatch and engine is busy: reject (can't interrupt active generation)
5. **Acquire spawn lock** (prevents race with other instances)
6. **Second policy check (with lock held)** — same logic, prevents TOCTOU race
7. If no engine running: kill stale processes, generate fresh token, spawn engine, wait for healthy (30s)

### Running Host Policy

When an engine is already running, the client decides what to do:

| Condition | Decision |
|-----------|----------|
| Healthy + compatible protocol | `Reuse` — connect to existing engine |
| Protocol mismatch + idle | `Restart` — shut down and respawn |
| Protocol mismatch + busy | `Reject` — can't interrupt active generation |
| Force override requested + idle | `Restart` — shut down and respawn with new config |
| Force override requested + busy | `Reject` — can't interrupt active generation |
| Not healthy | `SpawnNeeded` — start fresh |
