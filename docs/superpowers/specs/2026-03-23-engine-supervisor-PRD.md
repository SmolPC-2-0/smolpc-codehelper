# PRD: Engine Supervisor Redesign

## Problem Statement

When the SmolPC Code Helper engine process dies (crash, external kill, or resource exhaustion), the app enters a permanently broken state — the "engine disconnected" banner shows but the engine never relaunches. Students must restart the entire app to recover. This is caused by Mutex contention deadlocks in the engine lifecycle management, where 15+ concurrent Tauri commands compete through a single `resolve_client()` function protected by shared Mutexes. Additionally, app startup triggers 2-3 redundant engine spawns due to race conditions between commands.

## Goals

1. **Automatic recovery from engine death** — engine restarts within 10s of being killed, with exponential backoff and a 3-restart limit before surfacing an error to the user
2. **Single engine spawn at startup** — eliminate the double/triple spawn caused by `listModels()` racing with `ensureStarted()` and mode preference application
3. **Zero Mutex contention** — replace the 4-Mutex `InferenceState` with a single-owner supervisor task using message passing (actor pattern)
4. **Unified engine management** — both Code Helper and the launcher share the same supervisor, eliminating the duplicated client cache in `launcher/orchestrator.rs`
5. **Production parity** — the supervisor works identically in dev builds and NSIS-installed production builds

## Non-Goals

- Changing the engine host binary or its HTTP API
- Adding multi-instance support (multiple app windows sharing one engine)
- Changing the model loading pipeline or backend selection logic
- Redesigning the frontend UI components (only their data source changes)

## User Stories

### US-1: Engine Crash Recovery
**As a** student using Code Helper on a budget laptop,
**I want** the engine to automatically restart when it crashes,
**So that** I can continue my work without restarting the app.

**Acceptance criteria:**
- Kill engine with `taskkill /F /IM smolpc-engine-host.exe` → engine restarts within 10s, banner clears
- If engine crashes 3 times within 5 minutes → show error with "Retry" button (not infinite loop)
- Generation state (isGenerating, cancelState) is cleared immediately on crash — UI is never stuck
- Previously loaded model is restored after successful restart

### US-2: Clean Single Startup
**As a** student starting the app for the first time,
**I want** the engine to start once on my preferred backend (NPU/CPU/DirectML),
**So that** the app loads faster and I don't see multiple loading screens.

**Acceptance criteria:**
- Exactly ONE "Engine client connected/spawned successfully" message in logs during startup
- Runtime mode preference (from settings) is applied at spawn time, not via post-startup restart
- No thundering herd: other Tauri commands wait for startup to complete, don't trigger parallel spawns

### US-3: Runtime Mode Switching
**As a** student switching from NPU to CPU mode via the InferenceModeSelector,
**I want** the switch to complete cleanly without double-spawning,
**So that** mode switching is fast and doesn't leave the app in a broken state.

**Acceptance criteria:**
- `set_inference_runtime_mode` sends a command to the supervisor, which handles restart internally
- No `resolve_client` contention during the switch
- If the mode switch fails (e.g., NPU not available), previous mode is preserved

### US-4: Graceful App Exit
**As a** student closing the app,
**I want** the engine to shut down cleanly,
**So that** no orphan processes are left running on my laptop.

**Acceptance criteria:**
- App exit → engine receives shutdown command → process terminates
- `engine.pid` file is cleaned up
- If engine doesn't respond to shutdown within 5s, force-kill via PID

### US-5: Production Deployment
**As a** student running the installed app (not dev mode),
**I want** the same engine lifecycle behavior as in development,
**So that** the app works reliably on my machine.

**Acceptance criteria:**
- Engine binary resolves from `%LOCALAPPDATA%\SmolPC Code Helper\resources\binaries\`
- Token, PID, spawn log, and spawn lock files live in `%LOCALAPPDATA%\SmolPC\engine-runtime\`
- Stderr redirects to spawn log (crash diagnostics available)
- Stale PID from previous crash doesn't kill wrong process

## Technical Architecture

See full design spec: `docs/superpowers/specs/2026-03-23-engine-supervisor-redesign.md`

**Key components:**
- `EngineSupervisor` — single tokio task owning engine lifecycle (state machine: Idle → Starting → WaitingForHealth → Running → Crashed → restart loop)
- `EngineSupervisorHandle` — Clone-able handle with `mpsc::Sender` for commands and `watch::Receiver` for state
- `EngineLifecycleState` — enum broadcast via `tokio::sync::watch`, consumed by frontend via Tauri events
- Composable primitives in `smolpc-engine-client` crate (`spawn_engine`, `wait_for_healthy`, etc.)

## Metrics & Success Thresholds

| Metric | Current | Target |
|--------|---------|--------|
| Engine recovery after taskkill | Never (permanent broken state) | <10s automatic recovery |
| Engine spawns during startup | 2-3 | Exactly 1 |
| Mutex lock contentions on death | Deadlock (permanent) | 0 (no Mutexes) |
| Frontend health poll IPC round-trips | 6/min (10s interval × Tauri invoke) | 0 (event-driven from Rust) |
| Time from app launch to "Ready" | ~30s (double spawn + NPU compile) | ~15s (single spawn) |

## Scope & Timeline

**Phase 0:** Refactor engine client crate into composable primitives (low risk, additive)
**Phase 1:** Build supervisor core — task, handle, state machine, commands (low risk, additive)
**Phase 2:** Migrate Tauri commands in atomic groups — state-mutating first, then read-only, then generation (medium risk)
**Phase 3:** Migrate frontend — event listener replaces health polling (medium risk)
**Phase 4:** Migrate launcher — share supervisor handle (low risk)
**Phase 5:** Cleanup — delete InferenceState, resolve_client, dead code (low risk)

Each phase is independently deployable and testable. Rollback is possible at any phase boundary.

## Risks

| Risk | Mitigation |
|------|-----------|
| Supervisor task panics → no recovery | Use `catch_unwind` or spawn a monitor task that restarts the supervisor |
| Token mismatch after restart → auth failure | Supervisor regenerates token before each spawn (existing pattern) |
| Health poll too slow to detect crash | Add PID-alive check between HTTP polls (<1ms, OS-level) |
| Migration Phase 2 inconsistent state | Safety net: supervisor's health loop detects stale client from old-path kills |
| Production path resolution differs from dev | Same fallback chain, tested in production checklist |

## Dependencies

- `tokio::sync::{watch, mpsc, oneshot}` — already in dependency tree via Tauri
- `smolpc-engine-client` crate — refactored but API-compatible
- Tauri 2 event system (`app_handle.emit()`) — already used elsewhere in the app
- No new external crates required
