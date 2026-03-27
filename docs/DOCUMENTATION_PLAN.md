# Documentation Plan — SmolPC Code Helper

Master plan for the complete documentation rewrite. Each session should read this file first to understand scope, dependencies, and what to explore.

## Context

- **Project:** SmolPC Code Helper — offline AI coding assistant for secondary school students (ages 11-18)
- **Partners:** Intel, UCL
- **Stack:** Rust engine (Axum HTTP server) + Tauri 2 + Svelte 5 + Tailwind 4
- **Assessment:** COMP0016 Systems Engineering. Website report = 40% of grade. Source code = 50%.
- **Dual purpose:** Repo docs serve developers AND provide raw material for every website section.
- **Branch:** `docs/fresh-rewrite`

## Approach

- One fresh session per module (or session grouping)
- Sessions execute in order — later docs reference earlier ones
- Each session explores the relevant codebase area deeply, then writes the doc
- User reviews each doc before the next session starts
- Website-specific content (user testing, UI design, benchmarks, partner feedback, dev blog, team page, Gantt chart) is added directly to the website, not repo docs

---

## Module Index

| # | File | Session | Status |
|---|------|---------|--------|
| 1 | `README.md` (repo root) | 1 | Complete |
| 2 | `docs/ARCHITECTURE.md` | 2 | Complete |
| 3 | `docs/ENGINE.md` | 3 | Complete |
| 4 | `docs/ENGINE_API.md` | 3 | Complete |
| 5 | `docs/ENGINE_LIFECYCLE.md` | 4 | Complete |
| 6 | `docs/HARDWARE_AND_MODELS.md` | 5 | Complete |
| 7 | `docs/INFERENCE_DEEP_DIVE.md` | 6 | Complete |
| 8 | `docs/CONNECTOR_GUIDE.md` | 7 | Complete |
| 9 | `docs/MODE_CAPABILITIES.md` | 7 | Complete |
| 10 | `docs/SEPARATION_GUIDE.md` | 8 | Complete |
| 11 | `docs/SETUP_AND_DEPLOYMENT.md` | 8 | Complete |
| 12 | `docs/TESTING.md` | 9 | Complete |
| 13 | `docs/BENCHMARKS.md` | 10 | Complete |
| 14 | `docs/NPU_GUIDE.md` | 10 | Complete |
| 15 | `docs/WINDOWS_GOTCHAS.md` | 11 | Complete |
| 16 | `docs/SECURITY_AND_PRIVACY.md` | 11 | Complete |
| 17 | `docs/DESIGN_DECISIONS.md` | 11 | Complete |
| 18 | `docs/DEVELOPMENT_WORKFLOW.md` | 11 | Complete |

---

## Session 1: `README.md`

**File:** `README.md` (repo root)
**Feeds website:** Home (abstract source), Appendices (deployment manual)

### Scope

- Project title and one-paragraph description
- Prerequisites: Rust 1.88+, Node 18+, Windows 11, 8GB+ RAM
- Build instructions for each component:
  - Engine: `cargo run -p smolpc-engine-host`
  - Tauri app: `npm run tauri:dev` (from `app/`)
  - TTS sidecar: `cargo build --manifest-path engine/crates/smolpc-tts-server/Cargo.toml`
  - Frontend only: `npm run dev` (from `app/`)
- How to run in production (NSIS installer, offline USB)
- Environment variables reference table (all `SMOLPC_*` vars)
- Quick-start: "run the app and get inference working in 5 minutes"
- Link index to all other docs in `docs/`
- License (MIT)

### Codebase to explore

- `Cargo.toml` (workspace root) — members, MSRV, dependencies
- `package.json` (root) — npm scripts
- `app/package.json` — app-specific scripts
- `app/src-tauri/tauri.conf.json` — app config, bundling
- `installers/README.md` — existing installer docs
- `.github/workflows/ci.yml` — what CI expects (validates build instructions)

---

## Session 2: `docs/ARCHITECTURE.md`

**File:** `docs/ARCHITECTURE.md`
**Feeds website:** System Design (architecture diagram, sequence diagrams, design patterns, packages/APIs)

### Scope

- High-level architecture diagram showing 3 zones: Engine, App, Connectors
- Component descriptions for every crate in the workspace (11 crates + TTS sidecar)
- Data flow narrative: user types message → Svelte store → Tauri invoke → command handler → engine client HTTP → engine host → backend FFI → model → response → SSE stream → Tauri channel → Svelte store → rendered markdown
- Sequence diagrams:
  - Chat completion (streaming)
  - Model loading
  - App startup + engine spawn
  - Backend selection
- Named design patterns with justification:
  - Actor pattern (EngineSupervisor — mpsc+watch channels, no Mutex)
  - Trait-object polymorphism (ToolProvider — connectors as Arc<dyn ToolProvider>)
  - RAII guards (TransitionGuard for model_transition_in_progress)
  - Centralized DLL loading (single-point-of-truth in runtime_loading.rs, CI-enforced)
  - Breadcrumb pattern (installer-source.txt for USB source discovery)
  - Atomic extraction (temp dir + rename for crash-safe provisioning)
  - Singleton mutex (Windows Global\ mutex for concurrent provisioning prevention)
- Crate dependency graph (what imports what)
- API boundary contracts: HTTP between app↔engine, in-process between app↔connectors
- Port assignments: engine = 19432, TTS sidecar = 19433

### Codebase to explore

- `Cargo.toml` — workspace members and dependency graph
- `app/src-tauri/src/engine/supervisor.rs` — actor pattern
- `app/src-tauri/src/engine/handle.rs` — channel design
- `app/src-tauri/src/modes/registry.rs` — ToolProvider routing
- `crates/smolpc-connector-common/src/provider.rs` — ToolProvider trait
- `engine/crates/smolpc-engine-host/src/routes.rs` — HTTP endpoints
- `engine/crates/smolpc-engine-host/src/state.rs` — AppState, atomic flags
- `engine/crates/smolpc-engine-core/src/inference/runtime_loading.rs` — centralized DLL loading
- `app/src-tauri/src/provisioning/mod.rs` — atomic extraction, singleton
- `app/src-tauri/src/provisioning/source.rs` — breadcrumb pattern
- `app/src/lib/api/unified.ts` — frontend API layer
- `app/src/lib/stores/` — all stores (data flow endpoints)

---

## Session 3: `docs/ENGINE.md` + `docs/ENGINE_API.md`

**Files:** `docs/ENGINE.md`, `docs/ENGINE_API.md`
**Feeds website:** Implementation (engine subsection), System Design (APIs)

### ENGINE.md scope

- What the engine is: standalone local inference HTTP server
- Why it's a separate process (survives app restarts, shared across apps, isolates FFI crashes)
- Crate breakdown with one-paragraph descriptions:
  - `smolpc-engine-core` — hardware detection, backend abstraction, model registry, FFI bindings
  - `smolpc-engine-host` — Axum HTTP server, routing, startup probe, model lifecycle, auth
  - `smolpc-engine-client` — spawn/connect/health library, used by any consumer
  - `smolpc-tts-server` — standalone TTS sidecar (own workspace due to ort version conflict)
  - `smolpc-benchmark` — performance testing CLI
- How to build and run standalone: commands, expected output, what to check
- Configuration: CLI args (data_dir, resource_dir, queue size, timeouts)
- Environment variables: SMOLPC_ENGINE_TOKEN, SMOLPC_FORCE_EP, SMOLPC_MODELS_DIR, SMOLPC_ORT_BUNDLE_ROOT, SMOLPC_OPENVINO_BUNDLE_ROOT, SMOLPC_TTS_PORT
- Data directories on Windows (%LOCALAPPDATA%\SmolPC\...)
- Auth: token-based, auto-generated UUID, constant-time comparison
- Relationship to Tauri app: engine runs independently, app connects via engine-client

### ENGINE_API.md scope

- Auth header format: `Authorization: Bearer <token>`
- Every endpoint documented:
  - `GET /engine/health` — readiness check, response schema
  - `GET /engine/meta` — version, PID, uptime
  - `GET /engine/status` — full readiness payload + backend status + model info
  - `POST /engine/ensure_started` — startup with policy enum (AutoDetect/ForceCpu/ForceDml/ForceNpu), request/response
  - `POST /engine/chat/completions` — streaming SSE and non-streaming, request schema (messages, model, stream, max_tokens, temperature), SSE event format, thinking mode handling, token counting
  - `POST /engine/models/load` — load by model ID, timeout (600s)
  - `POST /engine/models/unload` — unload current model
  - `POST /engine/audio/transcriptions` — Whisper STT, multipart audio upload, Windows-only
  - `POST /engine/audio/speech` — TTS proxy to sidecar, request schema, Windows-only
- Error response format: `{ "error": { "message": "...", "type": "...", "code": ... } }`
- Queue semantics: semaphore-based with configurable timeout
- Concurrency: one generation at a time (atomic flag), queue for waiting requests
- Example curl commands for every endpoint (copy-pasteable)

### Codebase to explore

- `engine/crates/smolpc-engine-host/src/main.rs` — server setup, router
- `engine/crates/smolpc-engine-host/src/routes.rs` — all endpoint handlers
- `engine/crates/smolpc-engine-host/src/config.rs` — CLI args
- `engine/crates/smolpc-engine-host/src/auth.rs` — token validation
- `engine/crates/smolpc-engine-host/src/chat.rs` — chat request/response handling
- `engine/crates/smolpc-engine-host/src/state.rs` — AppState structure
- `engine/crates/smolpc-engine-host/src/startup.rs` — readiness state machine
- `engine/crates/smolpc-engine-host/src/types.rs` — request/response types
- `engine/crates/smolpc-engine-host/src/tts_sidecar.rs` — TTS proxy
- `engine/crates/smolpc-engine-host/src/adapters.rs` — format adapters
- `engine/crates/smolpc-engine-client/src/lib.rs` — constants (ports, timeouts, env vars)
- `engine/crates/smolpc-engine-client/src/client.rs` — EngineClient methods (shows all API calls)
- `engine/crates/smolpc-tts-server/src/main.rs` — TTS server setup

---

## Session 4: `docs/ENGINE_LIFECYCLE.md`

**File:** `docs/ENGINE_LIFECYCLE.md`
**Feeds website:** Implementation (lifecycle subsection), System Design (sequence diagrams)

### Scope

- Full state machine diagram: Idle → Starting → WaitingForHealth → Running → Crashed → Failed
- State transition rules and guards
- EngineSupervisor actor task:
  - Main select! loop structure
  - Command channel (mpsc): Start, SetRuntimeMode, SetDesiredModel, RefreshStatus, Shutdown
  - Watch channels: state_tx (EngineLifecycleState), client_tx (Option<EngineClient>), pid_tx (Option<u32>)
  - How Tauri commands interact via EngineSupervisorHandle (never directly with process)
- Engine process spawning:
  - `spawn_engine()` from engine-client: PID file, spawn lock, detached process, CREATE_NO_WINDOW
  - Auth token: `load_or_create_token()` — UUID persisted to disk
  - Environment setup: SMOLPC_ENGINE_TOKEN, SMOLPC_FORCE_EP passthrough
  - Stderr → log file (never Stdio::null)
- Health check loop:
  - 10s interval
  - PID liveness check + HTTP health endpoint
  - 3 consecutive failures → Crashed state
- Auto-restart:
  - Exponential backoff: 1s → 2s → 4s
  - Max 3 restarts within 5-minute window
  - Exceeding max → Failed (requires manual intervention)
  - Model auto-reload after restart
- Graceful shutdown:
  - Shutdown command → POST /engine/shutdown → wait for process exit
  - Force-kill fallback with timeout
  - PID identity verification before kill (PIDs are reused on Windows)
- TTS sidecar lifecycle:
  - Spawned by engine host on startup (port 19433)
  - Independent health monitoring
  - Killed when engine shuts down
- Frontend integration:
  - Supervisor emits Tauri event: `engine-state-changed`
  - Frontend listens and updates stores reactively
  - Replaces old poll-based health loop

### Codebase to explore

- `app/src-tauri/src/engine/supervisor.rs` — full supervisor implementation
- `app/src-tauri/src/engine/handle.rs` — EngineSupervisorHandle, command enum, watch channels
- `app/src-tauri/src/engine/mod.rs` — module setup, state types
- `engine/crates/smolpc-engine-client/src/spawn.rs` — spawn_engine, wait_for_healthy, kill_stale_processes, with_spawn_lock
- `engine/crates/smolpc-engine-client/src/token.rs` — token management
- `engine/crates/smolpc-engine-host/src/startup.rs` — readiness state machine (Starting → Probing → Ready/Failed)
- `engine/crates/smolpc-engine-host/src/tts_sidecar.rs` — sidecar lifecycle
- `app/src-tauri/src/commands/inference.rs` — how commands use the handle
- `.claude/rules/windows-process-lifecycle.md` — process lifecycle rules
- `.claude/rules/tauri-patterns.md` — supervisor ownership rules

---

## Session 5: `docs/HARDWARE_AND_MODELS.md`

**File:** `docs/HARDWARE_AND_MODELS.md`
**Feeds website:** Algorithms (model selection, hardware adaptation), Implementation (hardware subsection)

### Scope

- Hardware detection pipeline:
  - DXGI GPU probe via IDXGIFactory6 (~14ms) — adapter name, dedicated VRAM, vendor ID
  - Why DXGI not WMI (WMI hangs on some machines for 60+ seconds)
  - NPU detection via driver/device presence
  - CPU detection via hardware-query crate
  - RAM/storage detection via sysinfo (pinned =0.32.1)
  - App-side GPU detection disabled (sysinfo v0.32.1 lacks GPU support) — engine probe is authoritative
- Backend selection algorithm:
  - Priority: DirectML (discrete GPU only) → OpenVINO NPU → CPU
  - Gate chain per backend: hardware_detected → artifact_available → preflight_validated
  - DirectML gates: DXGI finds discrete GPU + DirectML DLLs present + ORT validates session
  - OpenVINO NPU gates: NPU driver present + OpenVINO DLLs loaded + IR artifacts exist
  - CPU: always available fallback
  - Preflight validation: 30s timeout via spawn_blocking (protects against hung GPU drivers)
  - Benchmark comparison: optional, 2000ms budget, decode speedup threshold >1.30
  - Decision persistence: prior good records reused, timeout = temporary_fallback (never persisted as negative)
- RAM-based model selection:
  - 16GB+ RAM → qwen3-4b (INT8_SYM for NPU, INT4 for DirectML/CPU)
  - <16GB RAM → qwen2.5-1.5b-instruct (INT4 for all backends)
  - Minimum RAM thresholds per model (qwen2.5-1.5b: 3GB, qwen3-4b: 16GB)
- Model artifact format:
  - OpenVINO: IR format (.xml + .bin), manifest.json as readiness gate
  - DirectML: ONNX format
  - Directory structure: %LOCALAPPDATA%\SmolPC\models\{model_id}\{backend}\
- Frontend runtime mode switching:
  - User selects mode in InferenceModeSelector component
  - Calls set_inference_runtime_mode Tauri command
  - Flows through supervisor → SetRuntimeMode command → engine restart with new SMOLPC_FORCE_EP
  - Model reload after backend switch
- Known constraints and gotchas:
  - DirectML on Intel iGPU: probe succeeds but output is garbage → only discrete GPUs accepted
  - Qwen3-4B INT4 on NPU: crashes → only INT8_SYM works
  - NPU fixed context window: MAX_PROMPT_LEN (2048) + MIN_RESPONSE_LEN (1024), exceeding crashes
  - SMOLPC_FORCE_EP in shell does NOT reach engine — supervisor controls env via cmd.env()

### Codebase to explore

- `engine/crates/smolpc-engine-host/src/probe.rs` — DXGI GPU detection, hardware probes
- `engine/crates/smolpc-engine-host/src/selection.rs` — backend decision logic, persistence
- `engine/crates/smolpc-engine-host/src/model_loading.rs` — model loading, preflight validation
- `engine/crates/smolpc-engine-host/src/artifacts.rs` — manifest validation, fingerprinting
- `engine/crates/smolpc-engine-core/src/hardware/` — detector.rs, types.rs
- `engine/crates/smolpc-engine-core/src/inference/backend.rs` — InferenceBackend enum, thresholds
- `engine/crates/smolpc-engine-core/src/inference/backend_store.rs` — decision history
- `engine/crates/smolpc-engine-core/src/models/registry.rs` — ModelDefinition, ModelRegistry, RAM thresholds
- `engine/crates/smolpc-engine-core/src/models/loader.rs` — ModelLoader trait
- `app/src/lib/components/InferenceModeSelector.svelte` — frontend mode UI
- `app/src/lib/stores/inference.svelte.ts` — inference state store
- `.claude/rules/npu-constraints.md` — NPU-specific rules
- `.claude/rules/engine-models.md` — model-specific rules

---

## Session 6: `docs/INFERENCE_DEEP_DIVE.md`

**File:** `docs/INFERENCE_DEEP_DIVE.md`
**Feeds website:** Algorithms (entire page), Implementation (inference subsection), Research (technology decisions)

This is the most technically dense doc. The session should spend significant time reading the FFI wrapper code.

### Scope

- Why custom FFI wrappers: no Rust bindings exist for OpenVINO GenAI C API or ONNX Runtime GenAI
- Centralized DLL loading architecture:
  - All Library::new() calls in runtime_loading.rs — CI test enforces this
  - OpenVINO DLL load order (14 DLLs in dependency order): tbb12 → tbbbind → ... → openvino_genai_c
  - Why order matters (Windows implicit dependency resolution)
  - Load-with-flags approach for explicit dependency management
- **OpenVINO GenAI wrapper (CPU + NPU):**
  - C API function signatures bound in openvino_ffi.rs
  - OpenVinoGenAiGenerator: pipeline creation, config params, streaming callback
  - OpenVinoPipelineConfig: how generation params map to C API calls
  - NPU-specific codepath:
    - StaticLLMPipeline (compiled, cached)
    - Greedy decoding only (do_sample=false)
    - Fixed context window: MAX_PROMPT_LEN=2048, MIN_RESPONSE_LEN=1024
    - Compilation blob caching (CACHE_DIR env var)
    - Template patching for Qwen3 non-thinking mode
  - CPU codepath differences
  - Streaming: callback function receives tokens, pushes to channel
- **DirectML wrapper (discrete GPU):**
  - ONNX Runtime GenAI C API bindings in directml_ffi.rs
  - GenAiDirectMlGenerator: session creation, GPU device selection by DXGI adapter index
  - DML-specific generation config
  - Preflight: spawn_blocking with 30s timeout (hung driver protection)
- **Whisper STT wrapper:**
  - OpenVINO GenAI Whisper pipeline (whisper_ffi.rs)
  - Audio file → transcription flow
  - Model: Whisper via OpenVINO IR
  - Windows-only (depends on OpenVINO GenAI)
- Token counting strategy:
  - Host-side counting before sending to NPU (crash prevention)
  - No tokenizer exposed via C API → use tokenizers crate with model's tokenizer.json
  - Character heuristic fallback (~3.5 chars/token for Qwen)
- Qwen model specifics:
  - Qwen2.5: two stop tokens — <|endoftext|> (151643) + <|im_end|> (151645)
  - Qwen3: non-thinking mode default, /nothink in system message
  - Chat template handling: string OR array of {name, template} objects
  - OpenVINO 2026.0.0 bug: min_new_tokens >= 1 suppresses EOS → runaway generation
- Performance characteristics:
  - TTFT vs decode speed tradeoffs across backends
  - NPU: fast TTFT, slower decode
  - DirectML: variable TTFT, fast decode on discrete GPU
  - CPU: slowest but most reliable

### Codebase to explore (read thoroughly)

- `engine/crates/smolpc-engine-core/src/inference/runtime_loading.rs` — DLL loading (critical)
- `engine/crates/smolpc-engine-core/src/inference/genai/openvino.rs` — OpenVINO wrapper
- `engine/crates/smolpc-engine-core/src/inference/genai/openvino_ffi.rs` — C API bindings
- `engine/crates/smolpc-engine-core/src/inference/genai/directml.rs` — DirectML wrapper
- `engine/crates/smolpc-engine-core/src/inference/genai/directml_ffi.rs` — ORT GenAI bindings
- `engine/crates/smolpc-engine-core/src/inference/genai/whisper.rs` — Whisper wrapper
- `engine/crates/smolpc-engine-core/src/inference/genai/whisper_ffi.rs` — Whisper C bindings
- `engine/crates/smolpc-engine-core/src/inference/genai/mod.rs` — module router
- `engine/crates/smolpc-engine-core/src/inference/runtime_adapter.rs` — adapter layer
- `engine/crates/smolpc-engine-core/src/inference/types.rs` — GenerationConfig, GenerationMetrics
- `engine/crates/smolpc-engine-host/src/openvino.rs` — OpenVINO setup, CACHE_DIR, compilation
- `engine/crates/smolpc-engine-host/src/chat.rs` — thinking mode, token counting
- `engine/crates/smolpc-engine-host/src/runtime_bundles.rs` — runtime detection
- `.claude/rules/engine-openvino.md` — OpenVINO-specific rules
- `.claude/rules/engine-models.md` — model-specific rules
- `.claude/rules/npu-constraints.md` — NPU constraints

---

## Session 7: `docs/CONNECTOR_GUIDE.md` + `docs/MODE_CAPABILITIES.md`

**Files:** `docs/CONNECTOR_GUIDE.md`, `docs/MODE_CAPABILITIES.md`
**Feeds website:** Implementation (connector subsection), System Design (extensibility)

### CONNECTOR_GUIDE.md scope

- The ToolProvider trait contract:
  - `execute(&self, request: ToolRequest) -> Result<ToolResponse>`
  - `refresh_tools(&self) -> Result<Vec<ToolDefinition>>`
  - `status(&self) -> ProviderStatus`
- Shared infrastructure from smolpc-connector-common:
  - CancellationToken for graceful generation cancellation
  - TextStreamer / EngineTextStreamer for SSE integration
  - Host app detection utilities
  - Python runtime resolution
  - Manifest parsing for addon/plugin verification
- Step-by-step guide to creating a new connector:
  1. Create crate in connectors/ with standard module structure
  2. Implement ToolProvider trait
  3. Add AppMode variant in smolpc-assistant-types
  4. Register in ModeProviderRegistry (app/src-tauri/src/modes/registry.rs)
  5. Add resources directory with manifest.json
  6. Add setup.rs for host app detection and addon installation
  7. Bundle resources in tauri.conf.json
  8. Add frontend mode entry (store, component, dropdown)
- Worked example: Blender connector end-to-end:
  - User sends task → connector gets Blender context → RAG retrieval → engine generates Python → addon executes in Blender → result returned
- Pattern comparison table: Blender (addon IPC), GIMP (Python-Fu server), LibreOffice (macro execution)

### MODE_CAPABILITIES.md scope

- Capability matrix table (mode vs. feature)
- Per-mode detailed breakdown:
  - **Code:** streaming chat, multi-turn conversation, code generation, syntax highlighting, keyboard shortcuts, voice I/O, context preservation
  - **Blender:** Python code generation, API RAG, addon execution, scene context, selection awareness. Limits: requires Blender running, no viewport preview
  - **GIMP:** PDB operations, Python-Fu/Scheme, layer awareness, image manipulation. Limits: requires GIMP + Python-Fu server
  - **Writer:** document content generation, editing assistance. Limits: requires LibreOffice
  - **Impress:** presentation generation. Limits: requires LibreOffice
- Voice I/O: Windows-only, Whisper STT via /audio/transcriptions, TTS via sidecar /audio/speech
- Shared across modes: engine, chat history, model selection, hardware detection, setup wizard
- What's mode-unique: system prompts, tool definitions, host app IPC, bundled resources

### Codebase to explore

- `crates/smolpc-connector-common/src/` — all files (provider.rs, cancellation.rs, text_generation.rs, launch.rs, manifests.rs, host_apps.rs, python.rs)
- `crates/smolpc-assistant-types/src/` — AppMode, ToolDefinitionDto, ModeCapabilitiesDto
- `connectors/blender/src/` — all files (provider.rs, executor.rs, bridge.rs, setup.rs, state.rs, prompts.rs, rag.rs, response.rs)
- `connectors/gimp/src/` — all files (provider.rs, executor.rs, transport.rs, runtime.rs, planner.rs, heuristics.rs, setup.rs)
- `connectors/libreoffice/src/` — all files
- `app/src-tauri/src/modes/registry.rs` — ModeProviderRegistry
- `app/src-tauri/src/modes/code.rs` — built-in Code provider
- `app/src-tauri/src/commands/modes.rs` — mode commands
- `app/src/lib/stores/mode.svelte.ts` — frontend mode state
- `app/src/lib/components/layout/AppModeDropdown.svelte` — mode switcher UI

---

## Session 8: `docs/SEPARATION_GUIDE.md` + `docs/SETUP_AND_DEPLOYMENT.md`

**Files:** `docs/SEPARATION_GUIDE.md`, `docs/SETUP_AND_DEPLOYMENT.md`
**Feeds website:** Appendices (deployment manual, user manual)

### SEPARATION_GUIDE.md scope

- Current workspace structure: 11 crates + TTS sidecar (own workspace)
- Dependency graph: what depends on what
- How to build Code Helper only (no connectors):
  - Remove connector crates from workspace members
  - Remove connector dependencies from smolpc-desktop
  - Remove ProviderFamily variants and registry entries
  - Remove connector resource bundles from tauri.conf.json
- How to build a single connector app (GIMP-only, Blender-only):
  - Keep only that connector + common + types + engine crates
  - Simplify ModeProviderRegistry
- Engine as standalone service:
  - Already works: `cargo run -p smolpc-engine-host -- --data-dir <path>`
  - Any HTTP client can consume the API
  - engine-client crate provides Rust consumer library
- NSIS installer customization: what to include/exclude in bundle.resources
- Offline bundle variants: lite (qwen2.5 only), standard (qwen3-4b), full (all models)

### SETUP_AND_DEPLOYMENT.md scope

- NSIS installer:
  - No admin required (currentUser install mode)
  - Offline WebView2 bundled
  - VC++ Redistributable check (NSIS hook)
  - Install path: %LOCALAPPDATA%\Programs\SmolPC Code Helper\
  - Breadcrumb: writes installer-source.txt to %LOCALAPPDATA%\SmolPC\
- First-run setup wizard:
  - Boot state check: models_provisioned? portable mode?
  - Source detection: breadcrumb → USB drive scan (3s timeout per drive) → internet
  - SourceSelector UI: shows available sources with hardware recommendation
  - Model provisioning: singleton mutex → manifest parse → disk space check → SHA-256 verify → extract to temp → atomic move
  - Streaming progress events via Tauri Channel
  - Cancel/retry/skip support
- Model source manifest format (model-archives.json)
- USB offline deployment:
  - Bundle creation via package-offline-bundle.ps1
  - Bundle structure: installer .exe + Install-CodeHelper.cmd + Install-CodeHelper.ps1 + models/
  - Double-click Install-CodeHelper.cmd → silent NSIS install → model extraction → launch
  - No admin, no internet
- Portable mode: .portable sentinel file skips setup
- Runtime provisioning: OpenVINO DLLs, DirectML DLLs, Python runtime
- Troubleshooting:
  - Missing models: re-run setup wizard or manual model download
  - Backend fallback: check engine-spawn.log for probe results
  - GPU not detected: verify discrete GPU, check DXGI probe output
  - Engine won't start: check PID file staleness, port conflicts

### Codebase to explore

- `app/src-tauri/src/setup/` — all files
- `app/src-tauri/src/provisioning/` — all files (mod.rs, source.rs, downloader.rs, extractor.rs, manifest.rs, singleton.rs, types.rs)
- `app/src-tauri/src/commands/setup.rs` — setup commands
- `app/src-tauri/tauri.conf.json` — bundle config, NSIS config
- `app/src/lib/components/setup/` — SetupWizard.svelte, SourceSelector.svelte, ProgressPanel.svelte
- `app/src/lib/stores/setup.svelte.ts` — setup state
- `app/src/lib/stores/provisioning.svelte.ts` — provisioning state
- `app/src/App.svelte` — boot sequence
- `installers/` — README.md, model bundle structure
- `app/package.json` — model/runtime setup scripts

---

## Session 9: `docs/TESTING.md`

**File:** `docs/TESTING.md`
**Feeds website:** Testing (entire page)

### Scope

- Testing philosophy: unit tests validate logic, live curl tests validate hardware paths
- Unit test inventory:
  - Engine host tests (tests.rs, ~1123 lines): ChatML, request parsing, thinking filter, auth, state machine, backend selection
  - Type contract tests (smolpc-assistant-types): serialization format guarantees
  - Security tests: path validation, file size limits
  - Connector tests: bridge communication, executor, RAG, setup
  - Engine client test utilities: RuntimeEnvGuard, thread-safe env isolation
  - Statistics tests (benchmark crate): mean, median, percentiles
  - Total: 150+ test functions, 66+ async (tokio::test)
- Integration testing approach:
  - Start engine with SMOLPC_FORCE_EP=<backend>
  - Curl streaming chat completion
  - Verify output quality, not just "no crash"
  - Required after any change to generation config, backend selection, or model loading
- CI pipeline (8 jobs in ci.yml):
  - frontend-quality: TypeScript check, Svelte check, npm audit
  - boundary-enforcement: check-boundaries.ps1 (9 architectural rules)
  - engine-tests-msrv: Rust 1.88.0 test suite
  - engine-tests-stable: latest stable test suite
  - tauri-build-check: cargo check -p smolpc-desktop
  - incremental-style-gates: Prettier, ESLint, rustfmt on changed files only
  - rust-security-audit: cargo audit
- Boundary enforcement details: what each rule prevents and why
- Release pipeline validation:
  - 8 required artifacts verified (DLLs, sidecar binary, Python)
  - Size gate: bundle must be >50MB (catches missing content)
- Compatibility testing:
  - Three test machines: Core Ultra with NPU, Intel CPU (no GPU), i5 + RTX 2000 (note: verify current test hardware from memory)
  - Windows 11, 8GB-16GB RAM configurations
  - Backend coverage: CPU, DirectML, OpenVINO NPU
- Benchmark infrastructure: smolpc-benchmark CLI, measurement methodology, metrics

### Codebase to explore

- `engine/crates/smolpc-engine-host/src/tests.rs` — full test file
- `crates/smolpc-assistant-types/tests/contracts.rs` — type contract tests
- `app/src-tauri/src/security/tests.rs` — security tests
- `engine/crates/smolpc-engine-client/src/test_utils.rs` — test utilities
- `engine/crates/smolpc-benchmark/src/` — all benchmark modules
- `.github/workflows/ci.yml` — full CI config
- `.github/workflows/release.yml` — release validation
- `scripts/check-boundaries.ps1` — boundary enforcement rules
- Connector test modules in each connector crate

---

## Session 10: `docs/BENCHMARKS.md` + `docs/NPU_GUIDE.md`

**Files:** `docs/BENCHMARKS.md`, `docs/NPU_GUIDE.md`
**Feeds website:** Algorithms (experiments, performance), Testing (performance section)

### BENCHMARKS.md scope

- Benchmark CLI usage and configuration
- Measurement methodology:
  - Token counting: native metadata (eval_count)
  - Timing: nanosecond-precision (TTFT, total_time, tokens_per_sec)
  - Non-streaming mode for metadata accuracy
  - Model warmup before measurement
  - Process-specific resource monitoring (CPU%, memory) via sysinfo
  - Sampling at 50ms intervals
- Metrics collected:
  - first_token_ms, total_time_ms, tokens_per_sec, avg_token_ms
  - Memory: before, during, after, peak
  - CPU utilization percentage
- Output formats: CSV (data + summary + metadata sections), JSON reports
- Hardware configurations tested (fill in from benchmark results when available)
- Backend comparison methodology:
  - Same prompts across all backends
  - Configurable warmup/cooldown
  - Thermal considerations (cooldown between GPU backend switches)
  - Statistical analysis: mean, median, p90, p95, std_dev

### NPU_GUIDE.md scope

- What the Intel NPU is (Neural Processing Unit on Core Ultra processors)
- Why NPU matters for this project (low power, built-in, no discrete GPU needed)
- OpenVINO GenAI on NPU: how it works
  - StaticLLMPipeline: compiled model graph, cached blobs
  - Compilation caching: CACHE_DIR environment variable, avoids recompilation
  - First-run compilation time vs. subsequent cached startup
- NPU constraints (consolidated from .claude/rules/npu-constraints.md):
  - Greedy decoding ONLY (do_sample=false, no temperature/top_p/top_k)
  - presence_penalty incompatible (causes runaway generation)
  - Fixed context window: MAX_PROMPT_LEN=2048 + MIN_RESPONSE_LEN=1024
  - Exceeding MAX_PROMPT_LEN crashes with "unknown exception" (no graceful error)
  - Token counting must happen host-side before sending
  - No tokenizer exposed via C API — use tokenizers crate with model's tokenizer.json
  - Character heuristic fallback: ~3.5 chars/token for Qwen
- Quantization:
  - INT8_SYM is the only working quantization for Qwen3-4B on NPU
  - INT4 crashes NPU pipeline
  - FP16 too large for NPU memory
  - Qwen2.5-1.5B works with INT4 on NPU
- Chat template patching:
  - Qwen3 template must default to non-thinking when enable_thinking is undefined
  - Template format: can be string OR array of {name, template} objects
- Driver requirements: driver .3717 known-good
- Recovery from DEVICE_LOST: destroy and recreate pipeline
- Performance characteristics: fast TTFT, slower decode vs. GPU

### Codebase to explore

- `engine/crates/smolpc-benchmark/src/` — all files (main.rs, runner.rs, compare.rs, output.rs, stats.rs, resource_sampler.rs, config.rs, prompts.rs)
- `engine/crates/smolpc-engine-core/src/inference/genai/openvino.rs` — NPU codepath
- `engine/crates/smolpc-engine-host/src/openvino.rs` — CACHE_DIR, compilation setup
- `engine/crates/smolpc-engine-host/src/chat.rs` — token counting, context window management
- `.claude/rules/npu-constraints.md` — all NPU rules
- `.claude/rules/engine-openvino.md` — OpenVINO version pinning, tokenization
- `.claude/rules/engine-models.md` — Qwen stop tokens, thinking mode

---

## Session 11: Supporting Docs

**Files:** `docs/WINDOWS_GOTCHAS.md`, `docs/SECURITY_AND_PRIVACY.md`, `docs/DESIGN_DECISIONS.md`, `docs/DEVELOPMENT_WORKFLOW.md`
**Feeds website:** Implementation, System Design, Appendices, Research

### WINDOWS_GOTCHAS.md scope

- DLL loading on Windows: why order matters, implicit dependency resolution, load-with-flags
- Process lifecycle: detached processes, PID file management, identity verification before kill
- taskkill.exe: can hang 60+ seconds, use Toolhelp32 for PID liveness checks instead
- CREATE_NO_WINDOW: required for background sidecars (prevents console flash)
- WMI queries: hang on some machines — use DXGI for GPU detection
- Stderr capture: never Stdio::null() for detached daemons — redirect to log file
- tar.exe: use $env:SystemRoot\System32\tar.exe, not Git Bash tar (can't handle Windows paths)
- PowerShell: Compress-Archive has 2GB limit — use tar.exe for large archives
- NSIS installer: kebab-cases output names (don't hardcode)
- File locking: investigate lock holders before deleting lock files

### SECURITY_AND_PRIVACY.md scope

- Privacy-by-design: offline-first, no cloud telemetry, no user data leaves the device
- GDPR/FERPA compliance: what this means for a school-deployed app
- Content Security Policy: what's allowed and why (no inline scripts, data: only for images)
- Path validation: canonicalization-based traversal prevention, whitelist approach
- File size validation: 10MB limit to prevent memory exhaustion
- Auth token: auto-generated UUID, constant-time comparison, per-installation
- No network access: engine listens on localhost only (127.0.0.1:19432)
- Model data: no personal data in model artifacts, no fine-tuning on user data

### DESIGN_DECISIONS.md scope

Document key architectural decisions with the "why" behind each:
- Why actor pattern over mutexes (4-Mutex state caused the engine disconnection bug)
- Why HTTP server not IPC/FFI (engine survives app restarts, shared across apps, isolates FFI crashes)
- Why centralized DLL loading (Windows dependency ordering, single enforcement point, CI-testable)
- Why DXGI not WMI for GPU detection (WMI hangs, DXGI is 1000x faster)
- Why Tauri not Electron (memory footprint for 8GB machines, Rust backend, no bundled Chrome)
- Why Svelte 5 not React (smaller bundle, runes model fits reactive state, Tailwind 4 integration)
- Why OpenVINO as primary (Intel NPU partnership, Core Ultra is target hardware)
- Why separate TTS sidecar (ort version conflict prevents workspace membership)
- Why detached engine process (survives app crashes, shared across future apps)

### DEVELOPMENT_WORKFLOW.md scope

- Development environment setup: Rust toolchain, Node, VS Code extensions
- Pre-commit checks: cargo check + clippy + test + npm check + lint
- CI pipeline overview (reference TESTING.md for details)
- Architectural boundaries: what the rules are and how they're enforced
- MSRV policy: Rust 1.88.0, why it's pinned
- Debug environment variables: SMOLPC_FORCE_EP, SMOLPC_MODELS_DIR, etc.
- Dev scripts: npm run tauri:dev, npm run tauri:dml
- Live hardware testing: how to validate backend changes with curl
- Git conventions: conventional commits with scope (feat(engine):, fix(openvino):)
- Branch strategy: main = known-good, branch for investigation, merge when live-tested

### Codebase to explore

- `.claude/rules/` — all 6 rule files
- `app/src-tauri/src/security/` — mod.rs, tests.rs
- `app/src-tauri/tauri.conf.json` — CSP header
- `engine/crates/smolpc-engine-host/src/auth.rs` — token auth
- `.github/workflows/ci.yml` — CI details
- `scripts/check-boundaries.ps1` — boundary rules
- `CLAUDE.md` — conventions section (source of truth for workflow)
- `AGENTS.md` — Codex conventions

---

## Website-Only Content (not repo docs)

These are written directly during website creation sessions, drawing from repo docs:

| Website Section | Source Material | Notes |
|-----------------|---------------|-------|
| **Home** | README.md + user-provided abstract, video, team info, Gantt chart | 3-paragraph abstract required |
| **Requirements** | User-provided: partner intro, goals, requirement gathering, personas, use cases, MoSCoW list | Must include use case diagram |
| **Research** | DESIGN_DECISIONS.md + INFERENCE_DEEP_DIVE.md + user research | Technology comparisons with IEEE refs |
| **Algorithms** | INFERENCE_DEEP_DIVE.md + HARDWARE_AND_MODELS.md + BENCHMARKS.md + NPU_GUIDE.md | Models, experiments, quantified results |
| **UI Design** | User-provided: sketches, Figma prototypes, design principles | Iteration evidence needed |
| **System Design** | ARCHITECTURE.md + ENGINE_API.md | Architecture diagram, sequence diagrams, design patterns |
| **Implementation** | All docs — each becomes a subsection | ENGINE, INFERENCE, CONNECTORS, SETUP, WINDOWS |
| **Testing** | TESTING.md + BENCHMARKS.md + user-provided UAT results | Unit, integration, UAT, compatibility |
| **Evaluation** | User-provided: MoSCoW satisfaction, self-critique, future work + BENCHMARKS.md | Requirements traceability matrix |
| **Appendices** | SETUP_AND_DEPLOYMENT.md + SECURITY_AND_PRIVACY.md + user manual screenshots + dev blog + team contributions | GDPR, deployment manual, dependencies |

---

## Quality Checklist (per doc)

Before marking a doc complete, verify:

- [ ] All referenced code files actually exist (grep/glob check)
- [ ] All function/struct/enum names match current code
- [ ] All file paths are correct
- [ ] No stale information from deleted code
- [ ] Consistent terminology with other completed docs
- [ ] Code snippets compile (or are clearly marked as pseudocode)
- [ ] Diagrams are described in enough detail for website rendering
- [ ] Website section mapping is clear
