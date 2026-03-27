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
- **Testable transitions.** The state machine is explicit: `Idle → Starting → WaitingForHealth → Running → Crashed → Failed`. Invalid transitions are logged and rejected.

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
- **Order-dependent loading.** OpenVINO requires 14 DLLs loaded in a specific sequence. Loading them out of order causes "DLL not found" errors that name the wrong DLL — the error says `openvino_genai_c.dll` but the actual missing dependency is `tbb12.dll`.
- **Search path control.** We use `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32` to restrict where Windows looks for dependencies. Without these flags, a wrong-version DLL on the user's PATH can be picked up instead of our bundled version.

Centralizing this in one file makes the ordering reviewable, the search flags consistent, and violations CI-detectable. The source-invariant test has caught accidental `Library::new()` calls in new code multiple times.

## Why DXGI, Not WMI, for GPU Detection

**Decision:** GPU enumeration uses the DXGI `IDXGIFactory6` API, not WMI (`Get-WmiObject`, `Get-CimInstance`).

**What went wrong with WMI:** On some Windows machines — particularly ones with unusual driver configurations or pending updates — WMI queries hang for 60+ seconds. The hang is in the WMI service itself and cannot be timed out from the caller. This blocked engine startup on affected machines.

**Why DXGI:**
- **Fast.** DXGI adapter enumeration completes in ~14ms, consistently.
- **Accurate.** DXGI is the same API DirectX uses, so it reports exactly what DirectML will see.
- **No external process.** WMI queries spawn a PowerShell/WMI provider process. DXGI is a direct COM call.

The same reasoning applies to `taskkill.exe`, which can also hang for 60+ seconds on some machines. PID liveness checks use Toolhelp32 (`OpenProcess`) instead.

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

**Why stderr to file, never null:**
- `Stdio::null()` makes FFI crashes invisible. When a student reports "it stopped working," the only diagnostic is the stderr log.
- Logs go to `engine-spawn.log` in the engine runtime directory.

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
- The token file uses restrictive permissions (mode 0o600 on Unix).

The tradeoff: the Tauri app must re-read the token file after each engine restart. This is handled by the supervisor, which reads the token before broadcasting the `EngineClient` via the watch channel.
