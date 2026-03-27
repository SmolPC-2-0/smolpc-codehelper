# Design Decisions

This document records the major architectural decisions in SmolPC Code Helper, the alternatives considered, and the reasoning behind each choice. It is intended to serve as a decision log for future maintainers and as source material for the Research and System Design sections of the COMP0016 assessment.

## Why an Actor Pattern, Not Mutexes

**Decision:** The engine lifecycle is managed by `EngineSupervisor`, a single background Tokio task that owns all engine process state. External code communicates through channels: an mpsc command channel for requests, and watch channels for state and client observation.

**What we tried first:** The original design used 4 `Mutex`-guarded fields shared across Tauri commands. Multiple commands could read and write engine state concurrently.

**What went wrong:** Deadlocks and race conditions. A health check could run while a model load was in progress, both holding different mutexes but needing each other's. The engine disconnection bug — where the UI showed "disconnected" while the engine was actually running — was caused by a stale client reference surviving a state transition because two code paths held overlapping locks.

**Why the actor pattern works:**
- **Exclusive ownership.** Only the supervisor touches the engine process. No concurrent mutation.
- **Ordered processing.** Commands arrive through an mpsc channel and are processed one at a time.
- **Observable state.** Watch channels let any number of readers observe the current state without blocking the supervisor.
- **Testable transitions.** The state machine is an explicit graph with cycles, not a linear chain. Valid transitions (from `can_transition_to` in `mod.rs`):
  ```
  Idle ──> Starting ──> WaitingForHealth ──> Running
                 ^   \                         │  │ │
                 │    └──> Failed <──┐         │  │ │
                 │                   │         │  │ │
                 ├───────────────────┼─────────┘  │ │  (user restart / mode change)
                 │                   │            │ │
                 │    Crashed <──────┼────────────┘ │
                 │      │           │               │
                 │      └──> Starting               │
                 │      └──> Failed                 │
                 │                                  │
                 └── Running ───────────────────────┘  (status update: Running -> Running)
                 └── Failed ──> Starting
  ```
  Key cycles: `Running -> Starting` (user-initiated restart or mode change), `Running -> Running` (status update with new backend/model info), `Crashed -> Starting` (auto-restart), `Failed -> Starting` (user retry). Invalid transitions are logged and rejected.

The `EngineSupervisorHandle` avoids double-wrapping in `Arc` because Tauri's managed state already provides it. Inner fields (mpsc::Sender, watch::Receiver) are cheaply cloneable by design.

## Why an HTTP Server, Not In-Process FFI

**Decision:** The inference engine runs as a separate process (`smolpc-engine-host`) communicating over HTTP on `localhost:19432`.

**Alternatives considered:**
- In-process FFI calls from the Tauri app directly to OpenVINO/ORT
- IPC via Unix domain sockets or named pipes
- gRPC

**Why HTTP in a separate process:**
- **FFI crash isolation.** OpenVINO and ONNX Runtime are C libraries loaded at runtime. A segfault in the FFI layer kills the process. If that process is the Tauri app, the student loses their UI. With a separate process, the supervisor detects the crash and auto-restarts the engine while the UI remains responsive.
- **Survives app restarts.** The engine process persists after the app closes. Reopening the app reconnects instantly — no model reload needed.
- **Shareable.** Multiple consumers can hit the same engine. This matters for the future launcher architecture where Blender, GIMP, and Code Helper apps share one engine.
- **Debuggable.** Any HTTP client (curl, Postman, a test script) can talk to the engine. No need for Rust to debug inference issues.
- **Standard protocol.** HTTP + SSE is the same wire format as the OpenAI API, which makes the engine a drop-in backend for any client that speaks that protocol.

gRPC was rejected because it adds a build dependency (protobuf compiler) and complexity for no benefit — the engine is localhost-only, latency is negligible, and the SSE streaming model maps naturally to HTTP chunked responses.

## Why Centralized DLL Loading

**Decision:** All `Library::new()` and `load_with_flags()` calls are confined to `runtime_loading.rs`. A CI test scans every `.rs` file in the workspace and fails if DLL loading appears anywhere else.

**Why this matters on Windows:**
- **Implicit dependency resolution.** When you load `openvino_genai_c.dll`, Windows tries to resolve its dependencies (like `tbb12.dll`) using the default search order: application directory, system directory, PATH. If TBB hasn't been loaded yet, the load fails silently.
- **Order-dependent loading.** OpenVINO requires 13 DLLs for CPU-only operation, or 15 DLLs when NPU support is included (the two NPU-specific DLLs are `openvino_intel_npu_compiler` and `openvino_intel_npu_plugin`). Loading them out of order causes "DLL not found" errors that name the wrong DLL — the error says `openvino_genai_c.dll` but the actual missing dependency is `tbb12.dll`.
- **Search path control.** We use `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32` to restrict where Windows looks for dependencies. Without these flags, a wrong-version DLL on the user's PATH can be picked up instead of our bundled version.

Centralizing this in one file makes the ordering reviewable, the search flags consistent, and violations CI-detectable. The source-invariant test has caught accidental `Library::new()` calls in new code multiple times.

## Why DXGI, Not WMI, for GPU Detection

**Decision:** GPU enumeration uses the DXGI `IDXGIFactory6` API, not WMI (`Get-WmiObject`, `Get-CimInstance`).

**What went wrong with WMI:** On some Windows machines — particularly ones with unusual driver configurations or pending updates — WMI queries hang for 60+ seconds. The hang is in the WMI service itself and cannot be timed out from the caller. This blocked engine startup on affected machines.

**Why DXGI:**
- **Fast.** DXGI adapter enumeration completes in ~14ms, consistently.
- **Accurate.** DXGI is the same API DirectX uses, so it reports exactly what DirectML will see.
- **No external process.** WMI queries spawn a PowerShell/WMI provider process. DXGI is a direct COM call.

The same reasoning applies to `taskkill.exe`, which can also hang for 60+ seconds on some machines. PID liveness checks use `OpenProcess` + `GetExitCodeProcess` (standard Windows process query APIs) — checking `STILL_ACTIVE` (259) exit code to determine if the process is alive. This is unrelated to the Toolhelp32 snapshot API; it is a direct process handle query.

## Why Tauri 2, Not Electron

**Decision:** The desktop app uses Tauri 2 with a Rust backend and Svelte 5 frontend, not Electron.

**The constraint:** Target hardware is 8 GB RAM machines in secondary schools. The LLM itself needs 3-6 GB of RAM. The app framework cannot consume another 300-500 MB.

**Why Tauri:**
- **Memory footprint.** Tauri uses the system WebView2 (already installed on Windows 11). No bundled Chromium. Baseline memory is ~50 MB vs. Electron's ~200-400 MB.
- **Rust backend.** All engine lifecycle, FFI, and connector code is Rust. Tauri's Rust backend is native, not a Node.js bridge.
- **Small installer.** The Tauri NSIS installer is smaller because it doesn't bundle a browser engine.
- **IPC performance.** Tauri's invoke mechanism and Channel primitive provide typed, ordered message passing without serialization overhead.

The tradeoff: Tauri has a smaller ecosystem than Electron, WebView2 rendering differences across Windows versions require testing, and Tauri 2 was relatively new when we adopted it. We accepted these tradeoffs because the memory constraint was non-negotiable.

## Why Svelte 5, Not React

**Decision:** The frontend uses Svelte 5 with runes (`$state`, `$derived`, `$effect`) and Tailwind 4.

**Why Svelte:**
- **Bundle size.** Svelte compiles to vanilla JS with no runtime. The frontend bundle is ~150 KB vs. React's ~300 KB+ baseline. On 8 GB machines, every MB counts.
- **Runes model.** Svelte 5's runes provide fine-grained reactivity without `useEffect` dependency arrays or stale closure bugs. The store pattern maps naturally to our state shape (inference status, chat history, mode selection).
- **No virtual DOM.** Direct DOM updates reduce CPU overhead during token streaming, where the UI updates ~20 times per second.
- **Tailwind 4 integration.** Svelte's template syntax works cleanly with utility classes (no `className`, no JSX escaping).

The tradeoff: smaller community, fewer UI component libraries, and team members needed to learn Svelte. The performance benefits justified this for our target hardware.

## Why OpenVINO as the Primary Backend

**Decision:** OpenVINO is the primary acceleration target, with DirectML as an alternative for discrete GPUs and CPU as the universal fallback.

**Why OpenVINO:**
- **Intel partnership.** The project's hardware partner is Intel. Target devices are Core Ultra laptops with NPUs. OpenVINO is Intel's inference framework and the only way to access the NPU.
- **NPU support.** No other framework supports Intel's NPU hardware. Without OpenVINO, the NPU — the most interesting hardware feature of the target devices — would be unused.
- **CPU fallback.** OpenVINO also provides a high-quality CPU backend, so machines without an NPU still benefit from OpenVINO's optimized CPU inference.

**Why also DirectML:**
- DirectML supports any DirectX 12 GPU via ONNX Runtime. Some school machines have NVIDIA or AMD discrete GPUs that are faster than CPU inference.
- It provides a fallback for machines where the NPU driver is missing or broken.
- The ONNX model format (used by DirectML) is more portable than OpenVINO IR.

**Backend priority:** DirectML (discrete GPU only) → OpenVINO NPU → OpenVINO CPU. Intel integrated GPUs are rejected for DirectML because they produce garbage logits (detected by the preflight NaN/Inf check).

## Why a Separate TTS Sidecar

**Decision:** `smolpc-tts-server` runs as a standalone process on port 19433 with its own workspace root, not as part of the main engine.

**The constraint:** The TTS server depends on `ort` (ONNX Runtime Rust crate) version X. The engine depends on `ort` version Y. Cargo does not allow two versions of the same crate in one workspace when they have conflicting native library requirements.

**Why a separate workspace:**
- Separate `Cargo.toml` workspace root allows an independent `ort` version.
- The TTS server is a simple HTTP proxy — it doesn't need access to engine internals.
- Process isolation means a TTS crash doesn't affect inference.

**Why not fix the version conflict:** The `ort` crate versions differ in their native library expectations. Upgrading one breaks the other. The conflict will resolve when both libraries converge on the same ORT version, at which point the sidecar can rejoin the workspace.

Build with `--manifest-path`, not `-p`:
```bash
cargo build --manifest-path engine/crates/smolpc-tts-server/Cargo.toml
```

## Why a Detached Engine Process

**Decision:** The engine is spawned as a detached process (`DETACHED_PROCESS` on Windows) with a PID file, stderr log redirection, and `CREATE_NO_WINDOW`.

**Why detached:**
- **Survives app crashes.** If the Tauri app panics, the engine keeps running. When the app restarts, it reconnects to the existing engine.
- **Shared across apps.** The future launcher architecture has multiple apps (Code Helper, Blender connector, GIMP connector) sharing one engine. A child process tied to one app's lifetime would die when that app closes.
- **Clean restart.** The supervisor can kill and respawn the engine without restarting the entire app.

**Why PID file + identity verification:**
- Windows reuses PIDs aggressively. A stale PID file could point to an unrelated process.
- Before killing via a stale PID, the supervisor checks the process name/path matches the engine binary.
- The PID file is written on spawn and cleaned up on both graceful exit and force-kill.

**Why stderr to file (with a pragmatic fallback):**
- Stderr is redirected to `engine-spawn.log` in the engine runtime directory. When a student reports "it stopped working," the only diagnostic is this log.
- If the log file cannot be opened (e.g., permissions, disk full), `spawn.rs` lines 68-71 fall back to `Stdio::null()` rather than failing the spawn entirely. This is a conscious tradeoff: a running engine with no crash log is better than no engine at all. The risk is that if the fallback triggers, FFI crashes become invisible. The spawn diagnostics header (written before the fallback decision) records whether the log file was successfully opened, so the absence of engine stderr in the log is itself a diagnostic signal.

**Why `CREATE_NO_WINDOW`:**
- Without this flag, spawning a console application on Windows briefly flashes a black console window. Students would see a flickering black rectangle every time the engine starts.

## Why Breadcrumb-Based Source Discovery

**Decision:** The NSIS installer writes `installer-source.txt` to `%LOCALAPPDATA%\SmolPC 2.0\` containing the directory the installer was launched from.

**The problem:** The offline USB deployment needs the app to find model archives after installation. But the USB drive letter can change between computers (E: on one machine, F: on another), and the installer copies the app to `%LOCALAPPDATA%` — far from the USB.

**Why a breadcrumb:**
- The NSIS post-install hook knows `$EXEDIR` (where the installer was launched). Writing this to a file is trivial and reliable.
- On first launch, the app reads the breadcrumb and checks if model archives exist at that path.
- If the drive letter changed (USB was E: at install time but is now F:), the app tries the same relative path on other drive letters.

This is simpler than registry keys, environment variables, or installer arguments — and it survives across app updates because the breadcrumb persists in `%LOCALAPPDATA%`.

## Why Atomic Extraction for Model Provisioning

**Decision:** Model archives are extracted to a temporary directory (`target.with_extension("extracting")`), then atomically renamed to the final path.

**The problem:** Extraction of a 3 GB model archive takes 30-60 seconds. If the process crashes mid-extraction (power loss, user kills the app, antivirus interference), the final directory contains partial data. The engine would try to load a corrupted model.

**Why atomic rename:**
- The final path never contains partial data. It is either absent (not yet extracted) or complete (rename succeeded).
- If a crash occurs, the `.extracting` temp directory is cleaned up on next launch.
- SHA-256 verification before extraction ensures the archive itself is not corrupted.

The same pattern is used for cache file writes (setup state, host detection cache): write to `.tmp`, backup existing to `.bak`, then rename.

## Why Token Rotation Per Engine Spawn

**Decision:** The auth token is deleted and regenerated each time the engine is spawned. The old token file is removed before the new one is written.

**Why not a persistent token:**
- A persistent token survives across engine restarts. If the token file is discovered (e.g., by another user on a shared machine), it provides permanent access to the engine.
- Fresh tokens ensure that each engine instance has a unique secret. Restarting the engine invalidates all previous tokens.
- On Unix, the token file uses restrictive permissions (mode `0o600`). On Windows, ACL restriction is not yet implemented — there is a `TODO` in `token.rs` to harden Windows ACLs so only the current user can read the file. This is a known gap; the current mitigation is that the file resides in a per-user `%LOCALAPPDATA%` directory.

The tradeoff: the Tauri app must re-read the token file after each engine restart. This is handled by the supervisor, which reads the token before broadcasting the `EngineClient` via the watch channel.

## Security Architecture

**Decision:** Defense-in-depth security for a local-only application targeting shared school machines.

**Path validation with canonicalization:**
- All file paths from the frontend pass through `validate_path()` in `app/src-tauri/src/security/mod.rs`. The function canonicalizes the path (resolving symlinks and `../` sequences) and verifies the canonical path starts with an allowed base directory (app data, app cache, or app local data).
- This prevents symlink escape attacks: a student cannot create a symlink inside the allowed directory that points to `C:\Windows\System32` and trick the app into reading or writing outside its sandbox.
- The allowlist approach (only approved directories pass) is safer than a denylist (blocking known-bad directories) because it fails closed on unexpected inputs.

**File size limit (10 MB):**
- `MAX_FILE_SIZE` is `10 * 1024 * 1024` bytes (10 MB). Both reads (`validate_file_size`) and writes (`validate_content_size`) enforce this limit.
- The target hardware has 8 GB RAM. A malicious or accidental multi-gigabyte file read would exhaust available memory and crash the app or the OS. The 10 MB cap keeps file I/O well within budget.

**CSP locked to `'self'`:**
- The Content Security Policy in `tauri.conf.json` is: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'`.
- This prevents external script injection. Even if an attacker could inject HTML into the WebView, no external scripts or connections would load. `object-src 'none'` blocks plugin-based attacks. `style-src 'unsafe-inline'` is required for Svelte's scoped styles.

**Constant-time token comparison:**
- Token validation in `auth.rs` uses a hand-rolled `constant_time_eq()` function that XORs all bytes and accumulates differences, returning equality only if the accumulated diff is zero.
- This prevents timing attacks where an attacker could determine how many bytes of a guessed token match by measuring response time. On localhost this is a low-probability attack, but the cost of constant-time comparison is negligible and it eliminates the vector entirely.

## Why spawn_blocking for All Backend Preflights

**Decision:** Every backend preflight (the initial model load and validation) uses `tokio::task::spawn_blocking()` wrapped in `tokio::time::timeout()`.

**The problem:** Backend preflights call into C libraries via FFI. These calls can hang indefinitely:
- **OpenVINO NPU:** First-time model compilation can take 3-5 minutes. The NPU driver has no internal timeout.
- **OpenVINO CPU:** Normally fast (~10s), but can hang on misconfigured systems.
- **DirectML/GPU:** Broken GPU drivers can block the D3D12 device creation call forever.

If these calls ran on a Tokio worker thread, they would starve the async runtime — health checks, HTTP responses, and shutdown commands would all block.

**Timeout budgets** (from `types.rs`):
- OpenVINO NPU: 300s (`OPENVINO_PREFLIGHT_BUDGET`) — accommodates first-time NPU compilation
- OpenVINO CPU: 30s (`CPU_PREFLIGHT_BUDGET`)
- DirectML: 60s (`DIRECTML_PREFLIGHT_BUDGET`)

**Pattern** (from `model_loading.rs`):
```rust
let task = tokio::task::spawn_blocking(move || run_openvino_preflight(...));
match timeout(OPENVINO_PREFLIGHT_BUDGET, task).await { ... }
```

`spawn_blocking` moves the FFI call to a dedicated thread pool. `timeout` ensures that even if `spawn_blocking`'s thread hangs, the Tokio runtime reclaims control after the budget expires. Any new backend added to the engine MUST use this pattern — a bare FFI call on an async task is a latent deadlock.

## Why RAII Drop Guards for State Flags

**Decision:** State flags that must be cleaned up on all exit paths use RAII drop guards: `TransitionGuard` and `GenerationPermit` (defined in `engine/crates/smolpc-engine-host/src/types.rs`).

**The pattern:**
```rust
pub(crate) struct TransitionGuard(pub(crate) Arc<AtomicBool>);
impl Drop for TransitionGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
```

`TransitionGuard` wraps an `AtomicBool` (e.g., `model_transition_in_progress`). When the guard is dropped — whether by normal return, early `?` error return, or panic — it sets the flag to `false`.

`GenerationPermit` does the same for the `generating` flag and also clears the active cancellation token on drop.

**Why this exists:** Before these guards, multi-path functions (model loading, generation) had manual `flag.store(false, ...)` calls on each exit path. Bugs occurred when a new error path was added without the cleanup call, leaving the flag permanently set. With RAII, the cleanup is automatic and cannot be forgotten.

## Why Model Idle Unload Defaults to Disabled

**Decision:** The `model_idle_unload` timeout defaults to `None` (disabled). The engine keeps the loaded model in memory indefinitely until explicitly unloaded or the process exits.

**Source:** `config.rs` calls `parse_idle_timeout_secs("SMOLPC_ENGINE_MODEL_IDLE_UNLOAD_SECS", None, 30)` — the `None` second argument is the default when the env var is unset, meaning no timeout.

**Why not enable it:** Enabling a timeout causes the "unhealthy after idle" bug. The sequence:
1. Student stops chatting for N minutes.
2. Idle timer fires, model is unloaded.
3. Supervisor's health check polls `/status` — the engine reports no model loaded.
4. The supervisor (or frontend) interprets this as unhealthy and triggers a restart cycle.

The fix would require the health check to distinguish "idle-unloaded" from "crashed," but the current status contract does not expose this state. Until that contract change is made, idle unload stays disabled.

**Warning for future developers:** Adding a default timeout here will re-introduce the bug. The env var (`SMOLPC_ENGINE_MODEL_IDLE_UNLOAD_SECS`) exists for manual testing but must not be set in production defaults.

## Why DLL Load Order Is a Hard Constraint

**Decision:** The DLL load sequence in `runtime_loading.rs` is a deliberate ordering, not an arbitrary list.

**The order for OpenVINO** (from `ensure_initialized_internal`):
1. TBB family: `tbb12`, `tbbbind`, `tbbmalloc`, `tbbmalloc_proxy`
2. OpenVINO core: `openvino`, `openvino_c`
3. OpenVINO IR frontend: `openvino_ir_frontend`
4. CPU plugin: `openvino_intel_cpu_plugin`
5. (NPU only): `openvino_intel_npu_compiler`, `openvino_intel_npu_plugin`
6. ICU: `icudt`, `icuuc`
7. Tokenizers + GenAI: `openvino_tokenizers`, `openvino_genai`, `openvino_genai_c`

**Why order matters:** Windows resolves implicit DLL dependencies at load time using the default search order. When `openvino_genai_c.dll` is loaded, it expects `openvino.dll` to already be in the process. If TBB hasn't been loaded first, `openvino.dll` itself fails to load. The error message names the DLL you tried to load (`openvino_genai_c.dll`), not the actual missing transitive dependency (`tbb12.dll`), making debugging without this knowledge extremely difficult.

Reordering any entry in this list can produce confusing "DLL not found" errors naming the wrong DLL. The CI source-invariant test enforces that all loading stays in `runtime_loading.rs`, but cannot enforce ordering — that is protected by this documentation and review.

## Error Propagation Strategy

**Decision:** Errors use different representations at different layers, by design.

**Library crates (`smolpc-engine-client`):** Typed errors via `thiserror` with an `EngineClientError` enum:
```rust
#[derive(Debug, thiserror::Error)]
pub enum EngineClientError {
    #[error("{0}")]
    Message(String),
    #[error("Engine process crashed or is unreachable: {0}")]
    EngineCrashed(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
```
This gives callers `match`-able variants and automatic `From` conversions for common error types.

**Engine host:** String errors with `map_err(|e| format!(...))`. The host assembles errors from many sources (FFI, config, probing, model loading) where a unified enum would be unwieldy. String errors with context prefixes are sufficient for logging and the status API.

**Tauri commands:** `Result<T, String>` for IPC serialization. Tauri's invoke mechanism serializes errors to JSON for the frontend. Custom error enums would need `serde::Serialize` and add complexity for no UX benefit — the frontend displays the error message string.

This layering is intentional: type safety where it aids callers (library boundary), pragmatic strings where errors are diverse and consumed by humans (host internals), and serialization-friendly strings at the IPC boundary.

## Why a Filesystem Spawn Lock

**Decision:** Engine spawning is protected by a filesystem-based lock (`engine-spawn.lock`) in the shared runtime directory.

**The problem:** Multiple apps (Code Helper, Blender connector, GIMP connector) can try to spawn the engine simultaneously on first launch. Without coordination, two engine processes start on the same port, and one crashes.

**How the lock works** (from `spawn.rs`):
- **Atomic creation:** `OpenOptions::new().create_new(true)` — the OS guarantees only one process succeeds.
- **PID recording:** The lock file contains `pid=<holder_pid>` so other processes can check if the holder is still alive.
- **Dead-holder detection:** If the recorded PID is no longer running (`OpenProcess` returns null on Windows, `kill(pid, 0)` fails on Unix), the lock is removed and re-acquired.
- **Stale age timeout:** If the lock file is older than 30s (`SPAWN_LOCK_STALE_AGE`), it is considered stale and removed. This handles cases where the holder crashed without cleanup.
- **Force-acquire timeout:** After 10s (`SPAWN_LOCK_WAIT`) of waiting, the lock is force-removed and one final acquisition attempt is made. If that also fails, the spawn is abandoned.

The `SpawnLockGuard` RAII struct removes the lock file on drop, ensuring cleanup on all exit paths.

## Why Preflight Timeout Is a Temporary Fallback

**Decision:** When a backend preflight times out, the result is classified as `temporary_fallback` and is never persisted as a negative decision to the backend decision store.

**Source:** In `model_loading.rs`, when `OpenVinoPreflightResult::Timeout` is returned, the outcome sets `suppress_store_update = true` and `persistence_state_override = Some(DecisionPersistenceState::TemporaryFallback)`. The same applies when the OpenVINO startup probe is still pending.

**Why this matters:** The backend decision store persists selections across engine restarts to avoid re-probing on every launch. If a timeout were persisted as "OpenVINO NPU failed," the machine would permanently skip the NPU on all future startups — even though NPU first-time compilation is expected to be slow and would succeed on the second attempt (using the compiled blob cache).

**The invariant:** A preflight timeout must NOT be persisted as a negative decision. It stays `temporary_fallback` and does not overwrite a prior good record. On next startup, the engine re-runs the preflight. If the compiled blob cache is now warm, the preflight succeeds within budget and the positive result is persisted.

Violating this invariant causes a permanent performance regression: a machine with a working NPU falls back to CPU forever after a single slow startup.
