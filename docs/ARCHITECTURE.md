# Architecture

SmolPC 2.0 is a local-first AI assistant platform. All inference runs on the student's machine — there is no cloud backend, no telemetry, and no internet requirement after initial setup.

The system is split into three zones: a **shared inference engine** (Rust, Axum HTTP server), a **Tauri 2 desktop app** (Rust backend + Svelte 5 frontend), and **self-contained connectors** for third-party applications (Blender, GIMP, LibreOffice). A set of **shared crates** provide common types and the connector trait interface.

---

## System Architecture

```
┌───────────────────────────────────────────────────────────┐
│                    Tauri 2 Desktop App                     │
│                                                           │
│  ┌─────────────────────┐    ┌──────────────────────────┐ │
│  │    Svelte 5 + TW4   │    │      Rust Backend        │ │
│  │                     │    │                          │ │
│  │  Stores:            │    │  EngineSupervisor        │ │
│  │   inference         │◄──►│   (actor, mpsc+watch)    │ │
│  │   chats             │IPC │                          │ │
│  │   mode              │    │  Commands:               │ │
│  │   setup             │    │   assistant, inference,   │ │
│  │   provisioning      │    │   audio, hardware,       │ │
│  │   hardware          │    │   modes, setup           │ │
│  │   voice             │    │                          │ │
│  │   settings          │    │  ModeProviderRegistry    │ │
│  │   ui                │    │   Code (built-in)        │ │
│  │                     │    │   Blender (connector)    │ │
│  │  API: unified.ts    │    │   GIMP (connector)       │ │
│  │   (Tauri invoke)    │    │   LibreOffice (connector)│ │
│  └─────────────────────┘    └────────────┬─────────────┘ │
└──────────────────────────────────────────┼───────────────┘
                                           │
                              HTTP (localhost:19432)
                              Auth: Bearer token
                                           │
┌──────────────────────────────────────────┼───────────────┐
│                   Inference Engine                        │
│                                                          │
│  ┌──────────────────────────────────────────────────┐   │
│  │            smolpc-engine-host (Axum)              │   │
│  │                                                    │   │
│  │  /engine/health, /engine/status, /engine/meta     │   │
│  │  /engine/ensure-started, /engine/load, /unload    │   │
│  │  /v1/chat/completions (SSE streaming)             │   │
│  │  /v1/audio/transcriptions (Whisper STT)           │   │
│  │  /v1/audio/speech (TTS proxy)                     │   │
│  └────────────────────┬─────────────────────────────┘   │
│                       │                                   │
│  ┌────────────────────┴─────────────────────────────┐   │
│  │            smolpc-engine-core                      │   │
│  │                                                    │   │
│  │  Hardware detection (DXGI, NPU, CPU, RAM)         │   │
│  │  Backend abstraction (CPU, DirectML, OpenVINO)    │   │
│  │  FFI wrappers (OpenVINO GenAI C API, ORT GenAI)   │   │
│  │  Centralized DLL loading (runtime_loading.rs)     │   │
│  │  Model registry (Qwen 2.5, Qwen 3)               │   │
│  └──────────────────────────────────────────────────┘   │
│                                                          │
│  TTS Sidecar (:19433) — standalone process               │
└──────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────┐
│                      Connectors                           │
│                                                           │
│  ┌─────────────┐  ┌─────────────┐  ┌──────────────────┐ │
│  │  Blender    │  │    GIMP     │  │   LibreOffice    │ │
│  │  (addon     │  │  (Python-Fu │  │   (Writer +      │ │
│  │   IPC)      │  │   server)   │  │    Impress)      │ │
│  └──────┬──────┘  └──────┬──────┘  └────────┬─────────┘ │
│         │                │                   │            │
│         └────────────────┴───────────────────┘            │
│                          │                                │
│              smolpc-connector-common                       │
│              (ToolProvider trait, utilities)                │
└──────────────────────────────────────────────────────────┘
```

---

## Zone 1: Inference Engine

The engine runs as a standalone HTTP server on `localhost:19432`. It is a separate process that survives app restarts, can be shared across multiple consumers, and isolates FFI crashes from the desktop app.

### smolpc-engine-core

Library crate — no binary. Provides the foundations that the engine host builds on.

- **Hardware detection:** DXGI adapter enumeration for GPUs (~14ms), NPU driver probing, CPU/RAM/storage detection via the `hardware-query` crate and `sysinfo`
- **Inference backends:** `InferenceBackend` enum with three variants: `Cpu`, `DirectML`, `OpenVinoNpu`. Each backend has FFI bindings to the underlying C APIs
- **FFI wrappers:** `OpenVinoGenAiGenerator` (OpenVINO GenAI C API), `GenAiDirectMlGenerator` (ONNX Runtime GenAI), `WhisperPipeline` (OpenVINO Whisper)
- **Centralized DLL loading:** All `Library::new()` calls live in `runtime_loading.rs`. A CI test scans every `.rs` file in the workspace and fails if DLL loading appears anywhere else. OpenVINO requires 14 DLLs loaded in strict dependency order; ORT requires 2
- **Model registry:** `ModelDefinition` structs with RAM thresholds, artifact paths, and backend compatibility. RAM-based auto-selection: 16 GB+ gets Qwen3-4B, <16 GB gets Qwen2.5-1.5B

Has no workspace crate dependencies — it is the foundation layer.

### smolpc-engine-host

The binary. An Axum HTTP server with token-based authentication on every endpoint.

**Endpoints:**

| Path | Method | Purpose |
|------|--------|---------|
| `/engine/health` | GET | Readiness check |
| `/engine/meta` | GET | Version, PID, uptime |
| `/engine/status` | GET | Full readiness + backend status + model info |
| `/engine/ensure-started` | POST | Start engine with backend policy (AutoDetect, ForceCpu, ForceDml, ForceNpu) |
| `/engine/load` | POST | Load model by ID |
| `/engine/unload` | POST | Unload current model |
| `/engine/cancel` | POST | Cancel active generation |
| `/engine/shutdown` | POST | Graceful shutdown |
| `/engine/check-model` | POST | Check model artifact availability |
| `/v1/models` | GET | List available models |
| `/v1/chat/completions` | POST | Streaming (SSE) or non-streaming generation |
| `/v1/audio/transcriptions` | POST | Whisper STT (Windows only) |
| `/v1/audio/speech` | POST | TTS via sidecar proxy (Windows only) |

**Concurrency model:** A generation semaphore (capacity 1) ensures only one LLM generation runs at a time. A queue semaphore (configurable, default 3) limits waiting requests. A voice semaphore (capacity 1) serializes STT/TTS. All handlers update an atomic activity timestamp for idle timeout tracking.

**Background tasks:** The server spawns a TTS sidecar process on startup and runs an idle timeout loop (checks every 30s) for optional model unload and process exit.

Depends on: `smolpc-engine-core`.

### smolpc-engine-client

Library crate consumed by the Tauri app and any other engine consumer. Handles the messy process lifecycle:

- `spawn_engine()` — launches the engine as a detached process with PID file, spawn lock, stderr logging, and `CREATE_NO_WINDOW`
- `wait_for_healthy()` — polls `/engine/health` with configurable timeout (default 60s) and 250ms interval
- `kill_stale_processes()` — cleans up orphaned engine processes with PID identity verification
- `EngineClient` — HTTP client wrapper with streaming SSE support for chat completions
- `RuntimeModePreference` — enum (Auto, Cpu, Dml, Npu) for overriding backend selection
- Version negotiation for protocol/API compatibility

Depends on: `smolpc-engine-core`.

### smolpc-tts-server

Standalone TTS sidecar binary on port 19433. Lives in its own `[workspace]` (separate `Cargo.toml` workspace root) because of an `ort` crate version conflict with the main workspace. Must be built with `--manifest-path`, not `-p`.

Spawned and monitored by the engine host. Runs as a detached, windowless process.

### smolpc-benchmark

CLI tool for performance testing. Measures TTFT, tokens/sec, memory usage across backends. Supports multi-backend comparison, warmup/cooldown, and CSV/JSON output.

Depends on: `smolpc-engine-client`, `smolpc-engine-core`.

---

## Zone 2: Desktop App

A Tauri 2 application with a Rust backend and Svelte 5 frontend. The Rust backend manages engine lifecycle, modes, setup, and provisioning. The frontend renders the chat UI and reacts to state changes.

### Rust Backend

#### Engine Supervisor (actor pattern)

The `EngineSupervisor` is a long-running Tokio task that owns the engine process lifecycle. No Tauri command touches the engine process directly — all interaction goes through the `EngineSupervisorHandle` via channels.

**Channels:**
- Command channel (`mpsc`, buffer 16): `Start`, `SetRuntimeMode`, `SetDesiredModel`, `RefreshStatus`, `Shutdown`
- State broadcast (`watch`): `EngineLifecycleState` — current lifecycle state
- Client broadcast (`watch`): `Option<EngineClient>` — connected client or None
- PID broadcast (`watch`): `Option<u32>` — engine process ID or None

**State machine:**

```
Idle ──Start──► Starting ──spawn──► WaitingForHealth ──healthy──► Running
                                                                     │
                                                          health loop (10s)
                                                          PID check + HTTP
                                                                     │
                                                         3 failures ──► Crashed
                                                                          │
                                                              restart (backoff)
                                                              1s → 2s → 4s
                                                                          │
                                                         max 3 per 5min ──► Failed
```

The `Running` state carries the current backend name and model ID. The `Crashed` state carries the error message and restart count. `Failed` is terminal — requires manual intervention.

On state change, the supervisor emits a Tauri event (`engine-state-changed`) that the frontend listens to.

#### Commands

Tauri IPC command handlers organized by domain:

- **assistant** — `assistant_send` (streaming chat with Tauri Channel), `assistant_cancel`, `mode_undo`
- **inference** — `inference_generate`, `load_model`, `unload_model`, `set_inference_runtime_mode`, `ensure_started`, `list_models`
- **audio** — voice input/output commands (Windows only)
- **hardware** — hardware detection with caching
- **modes** — `list_modes`, `mode_status`, `mode_refresh_tools`, `mode_open_host_app`
- **setup** — `setup_status`, `setup_prepare`

All commands that need the engine access it through the managed `EngineSupervisorHandle` — they send a command and either await a oneshot response or read the watch channel.

#### Mode Provider Registry

Routes `AppMode` variants to `ToolProvider` implementations:

| AppMode | Provider | Type |
|---------|----------|------|
| `Code` | `CodeProvider` | Built-in |
| `Blender` | `BlenderProvider` | Connector crate |
| `Gimp` | `GimpProvider` | Connector crate |
| `Writer` | `LibreOfficeProvider` | Connector crate |
| `Impress` | `LibreOfficeProvider` | Connector crate (shared) |

Writer and Impress map to the same provider family.

#### Setup and Provisioning

First-run setup orchestrates host app detection (Blender, GIMP, LibreOffice), Python runtime staging, and model provisioning.

Model provisioning discovers sources in priority order:
1. **Breadcrumb** — `installer-source.txt` left by the NSIS installer
2. **Drive scan** — all Windows drives (C:-Z:) with 3s timeout per drive
3. **Internet** — HuggingFace (if connectivity detected)

Extraction is crash-safe: archives are verified (SHA-256), extracted to a temp directory, then atomically renamed to the final path. A Windows global mutex prevents concurrent provisioning across app instances.

#### Managed State

Registered via `app.manage()` during initialization:
- `EngineSupervisorHandle` — engine lifecycle commands and state
- `AssistantState` — multi-turn conversation state
- `HardwareCache` — cached hardware detection results
- `ProvisioningCancel` — atomic bool for cancellation
- `AudioState` — voice input/output state

### Svelte 5 Frontend

Runes-only (`$state`, `$derived`, `$effect`). No legacy `writable`/`readable` stores. Tailwind 4 for styling (no `@apply`).

**Stores:**

| Store | Key State |
|-------|-----------|
| `inference` | Engine readiness, generating flag, current model, backend status, lifecycle state, runtime mode, memory pressure |
| `chats` | Chat history per mode, current chat, message CRUD |
| `mode` | Active mode, mode configs, provider status per mode |
| `setup` | Setup phase state |
| `provisioning` | Download/extraction progress, source list |
| `hardware` | CPU, GPU, NPU, RAM detection results |
| `voice` | Voice input/output state |
| `settings` | User preferences |
| `ui` | Sidebar state, theme |
| `composerDraft` | Input draft persistence |

**API layer:** `app/src/lib/api/unified.ts` exports typed async functions that wrap `tauri.invoke()` calls. Every frontend → backend call goes through this layer.

**Streaming:** Chat responses stream via Tauri Channels (command-scoped, ordered delivery), not global Tauri Events.

---

## Zone 3: Connectors

Each connector is a self-contained Rust crate that integrates with one host application. All implement the `ToolProvider` trait from `smolpc-connector-common`.

### ToolProvider Trait

```rust
#[async_trait]
pub trait ToolProvider: Send + Sync {
    async fn connect_if_needed(&self, mode: AppMode) -> Result<ProviderStateDto, String>;
    async fn status(&self, mode: AppMode) -> Result<ProviderStateDto, String>;
    async fn list_tools(&self, mode: AppMode) -> Result<Vec<ToolDefinitionDto>, String>;
    async fn execute_tool(
        &self, mode: AppMode, name: &str, arguments: Value,
    ) -> Result<ToolExecutionResultDto, String>;
    async fn undo_last_action(&self, mode: AppMode) -> Result<(), String>;
    async fn disconnect_if_needed(&self, mode: AppMode) -> Result<(), String>;
}
```

Every connector implements this interface. The `ModeProviderRegistry` holds `Arc<dyn ToolProvider>` instances and routes by `AppMode`.

### Blender Connector

IPC bridge to Blender via a bundled Python addon. Retrieves scene context (open document, selection), augments the prompt with Blender API documentation via RAG, sends to the engine for Python code generation, and executes the result in Blender via the addon.

**Modules:** provider, executor, bridge (IPC), setup (addon install), state, prompts, rag, response
**Resources:** Python addon, RAG index, manifest

### GIMP Connector

Connects to GIMP's Python-Fu server. Plans operations via the engine (what PDB calls to make), then executes Scheme/Python commands via IPC. Layer-aware and context-sensitive.

**Modules:** provider, executor, transport (IPC), runtime (Python-Fu), planner, heuristics, setup, response
**Resources:** Plugin runtime, C/Python bridge, upstream API references, manifest

### LibreOffice Connector

Integrates with LibreOffice Writer (document editing) and Impress (presentations) through a shared provider.

**Modules:** provider, executor
**Resources:** Manifest

---

## Shared Crates

### smolpc-assistant-types

Zero-dependency type definitions shared across the entire workspace:

- `AppMode` enum: `Code`, `Gimp`, `Blender`, `Writer`, `Impress`
- `ModeConfigDto`, `ModeCapabilitiesDto` — mode metadata and feature flags
- `ProviderStateDto`, `ToolDefinitionDto`, `ToolExecutionResultDto` — provider interface types
- `AssistantStreamEventDto`, `AssistantResponseDto` — streaming response types
- `SetupStatusDto` — setup state wire format

All DTOs use `camelCase` serialization for frontend compatibility.

### smolpc-connector-common

Shared connector infrastructure:

- `ToolProvider` trait (the core interface)
- `CancellationToken` for graceful generation cancellation
- `TextStreamer` / `EngineTextStreamer` for SSE integration with the engine
- Host app detection (find Blender, GIMP, LibreOffice installations)
- Python runtime resolution
- Setup manifest parsing and addon/plugin verification

Depends on: `smolpc-assistant-types`, `smolpc-engine-core`, `smolpc-engine-client`.

---

## Data Flow: Chat Completion

How a user message becomes an AI response:

1. User types a message in `ChatInput.svelte` and presses Enter
2. `chatsStore.addMessage()` appends the user message to the current chat
3. `assistantSend()` in `unified.ts` calls `tauri.invoke('assistant_send')` with the message and a Tauri Channel for streaming
4. The `assistant_send` Tauri command looks up the active mode's `ToolProvider` via `ModeProviderRegistry`
5. For Code mode: the command builds a message array from chat history and calls the engine via `EngineSupervisorHandle` to get the `EngineClient`
6. `EngineClient.chat_completions_stream()` sends `POST /v1/chat/completions` to the engine with `stream: true`
7. The engine host acquires the queue semaphore (waits if full), then the generation semaphore (capacity 1)
8. The engine loads the prompt into the active backend (OpenVINO, DirectML, or CPU) via the `InferenceRuntimeAdapter`
9. The backend's FFI wrapper calls into the native library (OpenVINO GenAI C API or ORT GenAI) and registers a streaming callback
10. As tokens are generated, the callback sends them as SSE `data:` events over the HTTP response
11. `EngineClient` parses the SSE stream and forwards tokens to the Tauri Channel
12. The frontend receives each token via the channel callback, appends it to the assistant message in `chatsStore`, and renders incrementally

For connector modes (Blender, GIMP, LibreOffice), step 5 differs: the connector retrieves context from the host app, augments the prompt, calls the engine, then executes the generated code in the host app before returning the result.

## Data Flow: App Startup

How the app goes from launch to ready:

1. Tauri initializes: resolves paths, creates managed state, spawns the `EngineSupervisor` task
2. `App.svelte` calls `get_boot_state` — checks if models are provisioned and if running in portable mode
3. If models are missing and not portable: the `SetupWizard` is shown
4. SetupWizard detects model sources (breadcrumb → drive scan → internet), user selects one
5. `provision_models` acquires the singleton mutex, verifies archives (SHA-256), extracts atomically, streams progress events to the UI
6. On completion, `App.svelte` calls `performStartup()`
7. `performStartup` sends a `Start` command to the supervisor via the handle
8. The supervisor spawns the engine process (`spawn_engine`), writes PID file, waits for healthy
9. The engine starts its probe: detects hardware (DXGI for GPU, driver check for NPU), selects backend (DirectML → NPU → CPU), loads the default model
10. Once `/engine/health` returns ready, the supervisor transitions to `Running` and broadcasts via watch channels
11. The frontend receives the `engine-state-changed` event, updates `inferenceStore.lifecycleState`, and the UI shows the chat interface
12. The health check loop begins (10s interval) — if the engine dies, the supervisor auto-restarts with backoff

---

## Design Patterns

### Actor Pattern — Engine Supervisor

The `EngineSupervisor` is a Tokio task that owns all engine process state. External code (Tauri commands) communicates exclusively through channels: an mpsc command channel for requests, and watch channels for state observation. This replaced an earlier design with 4 `Mutex`-guarded fields that caused deadlocks and race conditions.

### Trait-Object Polymorphism — ToolProvider

All connector modes implement `ToolProvider` and are stored as `Arc<dyn ToolProvider>` in the `ModeProviderRegistry`. Adding a new mode means implementing the trait and registering it — no changes to the command layer or frontend routing.

### RAII Guards — State Transitions

`TransitionGuard` (and similar patterns) ensure that state flags like `model_transition_in_progress` are always reset, even if a load/unload operation panics or returns early. The guard sets the flag on creation and clears it on drop.

### Centralized DLL Loading

All FFI `Library::new()` calls are confined to `runtime_loading.rs` in engine-core. A source-invariant CI test scans every `.rs` file in the workspace and fails if DLL loading appears anywhere else. This ensures Windows DLL dependency ordering is maintained in exactly one place.

### Breadcrumb Pattern — Source Discovery

The NSIS installer writes `installer-source.txt` to `%LOCALAPPDATA%\SmolPC 2.0\` with the install source path. On first run, the provisioning system reads this breadcrumb to find model archives without scanning every drive. If the path is stale (USB drive letter changed), it tries other drive letters.

### Atomic Extraction — Crash-Safe Provisioning

Model archives are extracted to a temporary directory (`target.with_extension("extracting")`), then atomically renamed to the final path. If the process crashes mid-extraction, the temp directory is cleaned up on next run — the final path never contains partial data.

### Singleton Mutex — Concurrent Provisioning

A Windows global mutex (`Global\SmolPC-Provisioning`) prevents multiple app instances from provisioning simultaneously. The guard is held for the duration of the provisioning operation and released on drop.

---

## Crate Dependency Graph

```
smolpc-assistant-types (no dependencies)
        │
        ▼
smolpc-engine-core (no workspace dependencies)
        │
        ├──────────────────────┐
        ▼                      ▼
smolpc-engine-client    smolpc-engine-host (binary)
        │
        ├───────────────────────────────────┐
        ▼                                   ▼
smolpc-connector-common              smolpc-benchmark
        │
        ├──────────────┬────────────────────┐
        ▼              ▼                    ▼
smolpc-connector-  smolpc-connector-  smolpc-connector-
  blender            gimp              libreoffice
        │              │                    │
        └──────────────┴────────────────────┘
                       │
                       ▼
              smolpc-desktop (Tauri app binary)
              imports ALL workspace crates
```

`smolpc-assistant-types` is the leaf — no dependencies. `smolpc-engine-core` is the foundation. Everything else builds upward. The desktop app (`smolpc-desktop`) is the top-level binary that imports everything.

`smolpc-engine-host` is a separate binary that only depends on `smolpc-engine-core` — it does not import connectors or the desktop app.

---

## API Boundaries

| Boundary | Transport | Auth |
|----------|-----------|------|
| Frontend ↔ Rust Backend | Tauri IPC (`invoke` + Channels) | In-process (same app) |
| Rust Backend ↔ Engine | HTTP (localhost:19432) | Bearer token (auto-generated UUID) |
| Engine ↔ TTS Sidecar | HTTP (localhost:19433) | None (localhost only) |
| Connectors ↔ Host Apps | IPC (varies per connector) | None |

The engine listens on `127.0.0.1` only — it is not accessible from other machines. The auth token is auto-generated on first run and shared between the app and engine via file (`SMOLPC_ENGINE_TOKEN` env var).
