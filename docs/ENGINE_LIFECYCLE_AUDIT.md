# Engine Lifecycle Audit

## Purpose
This document captures the architectural analysis of the engine lifecycle management in the SmolPC Code Helper Tauri app. It identifies fundamental design flaws that cause engine reconnection to fail after external kill, and serves as the basis for the redesign in the accompanying spec.

## Current Architecture

### Components
- **InferenceState** (`apps/codehelper/src-tauri/src/commands/inference.rs`) — Tauri managed state holding cached engine client, connect lock, runtime config, and desired model. Four `Arc<Mutex<T>>` fields.
- **resolve_client()** (same file, ~line 356) — The single entry point for all 16 Tauri commands to get an engine client. Implements double-checked locking with `connect_lock` serializing spawn/connect attempts.
- **connect_or_spawn()** (`engine/crates/smolpc-engine-client/src/spawn.rs`) — The engine client crate's spawn logic. Manages file-based spawn lock, token regeneration, process spawning, and health wait loop.
- **EngineClient** (`engine/crates/smolpc-engine-client/src/client.rs`) — HTTP client wrapper. Clone-friendly (Arc-based reqwest::Client). All methods are stateless HTTP calls.
- **Health polling** (`apps/codehelper/src/App.svelte`, lines 804-874) — Frontend setTimeout chain polling `engine_health_only` every 10s. Triggers reconnection via `reestablishEngineSession()` when engine dies.
- **Launcher orchestrator** (`apps/codehelper/src-tauri/src/launcher/orchestrator.rs`) — Duplicate engine client caching pattern for the launcher shell.

### Lock Architecture

| Lock | Type | Protects | Held Across Await? | Duration |
|------|------|----------|-------------------|----------|
| `InferenceState.client` | `Arc<Mutex<Option<EngineClient>>>` | Cached client swap | No (clone-then-release) | <1ms |
| `InferenceState.connect_lock` | `Arc<Mutex<()>>` | Serializes spawn/connect | YES — entire connect_or_spawn | 30-60s |
| `InferenceState.runtime_config` | `Arc<Mutex<RuntimeClientConfig>>` | Mode preference | No | <1ms |
| `InferenceState.desired_model` | `Arc<Mutex<Option<String>>>` | Model to restore | No | <1ms |
| `engine-spawn.lock` (file) | Filesystem lock | Inter-process spawn serialization | N/A | 30-60s |

### Tauri Commands That Call resolve_client

16 commands go through resolve_client, meaning ANY of them can trigger engine spawn:
- `list_models`, `load_model`, `unload_model`, `get_current_model`
- `inference_generate`, `inference_generate_messages`, `inference_cancel`
- `is_generating`, `check_model_readiness`, `check_model_exists`
- `get_inference_backend_status`, `set_inference_runtime_mode`
- `engine_status`, `engine_ensure_started`
- `evaluate_memory_pressure` (changed to use `cached_healthy_client`, but was resolve_client)
- `assistant_send` (via `resolve_generation_client`)

### Startup Sequence (observed from logs)

1. `listModels()` fires first → `resolve_client` → `connect_or_spawn` → spawns engine on AUTO mode
2. `reestablishEngineSession()` fires → `engine_ensure_started` → `apply_runtime_mode_preference(npu)` → clears client → `resolve_client(force_respawn=true)` → KILLS auto engine → respawns on NPU
3. NPU model loading takes 30-60s (HTTP call blocked in `client.ensure_started()`)
4. Meanwhile 10+ other Tauri commands queue on `connect_lock` (thundering herd)
5. When `connect_lock` releases, queued commands drain rapidly
6. A THIRD force_respawn sometimes fires from queued commands, killing and respawning AGAIN

**Result:** 2-3 engine spawns during a single app startup.

### Engine Death & Failed Reconnection

When the engine is killed externally (`taskkill /F /IM smolpc-engine-host.exe`):

1. The initial `engine_ensure_started` from startup is STILL running (waiting for model load HTTP)
2. HTTP gets connection reset → error handler calls `refreshBackendStatus()` → `resolve_client`
3. Health poll detects death → `reestablishEngineSession()` → `resolve_client`
4. Two concurrent `resolve_client` calls contend on `state.client.lock()` at line 367
5. Neither proceeds to `connect_lock` (which would serialize them)
6. Flow is stuck — engine never relaunches

## Identified Flaws

### Flaw 1: Every Command Is a Potential Spawner
resolve_client is called by 16 Tauri commands. Each can trigger connect_or_spawn. Commands like list_models and get_inference_backend_status should be pure queries, not spawn triggers.

### Flaw 2: No Lifecycle Owner
Nobody owns the engine lifecycle. Spawning, mode switching, and reconnection are all side effects of whichever command runs first. No state machine governs transitions.

### Flaw 3: connect_lock Creates a Convoy
When engine is starting (30s+), every command queues on connect_lock. Releases create thundering herd. Mode preference can trigger ANOTHER 30s block.

### Flaw 4: Double/Triple Spawn at Startup
listModels() spawns on auto before ensureStarted applies NPU preference. Then force_respawn kills and respawns. Queued commands can trigger a third respawn.

### Flaw 5: Long-Running HTTP Calls Outlive Their Purpose
client.ensure_started() blocks for 60+ seconds (NPU model compilation). When engine dies, the error cascade from this call creates concurrent resolve_client tasks that deadlock.

### Flaw 6: Launcher Duplicates the Pattern
launcher/orchestrator.rs has its own resolve_engine_client with separate caching. No coordination with InferenceState.

### Flaw 7: No Reconnection Was Ever Designed
The engine-client crate was designed for one-shot connect_or_spawn. Reconnection was bolted on by having resolve_client re-call connect_or_spawn when the cache is stale. This doesn't work with 16 concurrent callers.

## Impact

- Engine death leaves app permanently stuck (banner visible, no recovery)
- Students would have to restart the entire app to recover
- NPU mode startup takes 2-3x longer than necessary due to double-spawn
- Thundering herd at startup wastes time and creates log noise

## Recommendation

A fundamental redesign is needed. The engine lifecycle should be owned by a single background task (supervisor pattern), with Tauri commands requesting the client via a channel rather than through shared Mutex state. See the accompanying design spec for the proposed architecture.
