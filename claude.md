# CLAUDE.md

## Project Overview

SmolPC Code Helper is an **offline AI coding assistant** for secondary school students (ages 11-18).

**Principles:** offline-first (no cloud/telemetry), privacy-first (GDPR/FERPA), budget hardware (8GB RAM min), Intel NPU partnership (OpenVINO is primary acceleration target on Windows).

**Stack:** Rust engine (`axum` HTTP server) + Tauri 2 + Svelte 5 + Tailwind 4

**Architecture:** `engine/` contains the shared inference server (`smolpc-engine-host` + `smolpc-engine-core`). `apps/` contains product apps (Code Helper is the first). `launcher/` is the suite shell. The engine runs as a local HTTP server; Tauri apps connect via `smolpc-engine-client` over HTTP + SSE.

**Backend selection priority:** `directml` > `openvino_npu` > `cpu`

**Current runtime status:** the supported shared model baseline is `qwen2.5-1.5b-instruct` (default) plus `qwen3-4b`. OpenVINO CPU and OpenVINO NPU now use structured chat history for normal chat requests; only explicit legacy ChatML payloads stay on the prompt-compatibility path.

---

## Quick Reference

```bash
# Engine (from repo root)
cargo run -p smolpc-engine-host        # Start engine server
cargo check --workspace                # Check all crates compile
cargo clippy --workspace               # Lint all crates
cargo test -p smolpc-engine-core       # Core tests
cargo test -p smolpc-engine-host       # Host tests

# Tauri app (from apps/codehelper/)
npm run tauri:dev                      # Full app with hot reload and shared-engine cleanup
npm run dev                            # Frontend only
npm run check                          # TypeScript check
npm run lint                           # Lint

# Debug-only env overrides
SMOLPC_ORT_BUNDLE_ROOT=/path/to/libs
SMOLPC_OPENVINO_BUNDLE_ROOT=/path/to/libs
SMOLPC_MODELS_DIR=/path/to/models
```

**Key docs:** [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | [`docs/ENGINE_API.md`](docs/ENGINE_API.md) | [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md)

---

## Conventions

**DLL loading is centralized.** All `Library::new()` / `load_with_flags()` calls live in `runtime_loading.rs`. A source-invariant test enforces this — adding DLL loading elsewhere fails CI.

**OpenVINO DLL load order matters.** Windows requires dependency order: `tbb12 -> openvino -> openvino_c -> openvino_ir_frontend -> openvino_intel_cpu_plugin -> openvino_intel_npu_plugin -> openvino_tokenizers -> openvino_genai`

**OpenVINO models must be IR format.** The OpenVINO lane expects `.xml` + `.bin` artifacts, not ONNX. The `openvino/manifest.json` is the artifact readiness gate.

**Preflight timeout = temporary fallback.** A preflight timeout must NOT be persisted as a negative decision. It stays `temporary_fallback` and does not overwrite a prior good record.

**Engine host owns all backend policy.** Launcher and Tauri app consume status only — they must not rank backends or override engine selection.

**Svelte 5 runes only.** No `writable` / `readable` from `svelte/store`. Use `$state`, `$derived`, `$effect`.

**Tailwind 4.** No `@apply` — use utility classes directly in templates.

**Tauri Channels for streaming.** Use `tauri::ipc::Channel<T>` (command-scoped, ordered), not global Events.

**Conventional Commits with scope.** `feat(engine):`, `fix(openvino):`, `docs:`, etc.

**Pre-commit checks:**
```bash
cargo check --workspace && cargo clippy --workspace
cargo test -p smolpc-engine-core && cargo test -p smolpc-engine-host
cd apps/codehelper && npm run check && npm run lint
```

---

## Learnings

Corrections discovered during development. **When you correct a mistake, append a one-line rule here.**

- OpenVINO GenAI handles its own tokenization — Rust `TokenizerWrapper` is not involved for the NPU lane
- OpenVINO 2026.0.0 is the pinned tuple — `openvino`, `openvino_genai`, `openvino_tokenizers` must match; mixing breaks ABI
- Use INT4, not NF4, for broad NPU compatibility — NF4 only works on Core Ultra Series 2+
- Qwen2.5 has TWO stop tokens: `<|endoftext|>` (151643) + `<|im_end|>` (151645) — check both
- OpenVINO GenAI chat requests must use structured message history on CPU and NPU; keep the preformatted ChatML string path only for explicit legacy compatibility
- Use `OnceLock<Result>` over `Once` for fallible init — `Once::call_once` can't return values
- Use `AtomicBool` over `try_lock()` for state tracking — `try_lock()` creates TOCTOU races
- Bundle fingerprint auto-invalidates on DLL change — mtime change forces fresh backend selection
- Don't dismiss broken checks as "pre-existing" - if verification fails, fix it in the current session
- Selection profile constant (`OPENVINO_SELECTION_PROFILE`) change forces re-evaluation of all cached decisions
- NPU compilation is slow on first load but fast after - `CACHE_DIR` enables compiled blob reuse
- Qwen3 OpenVINO support is currently non-thinking only; align temperature, top_p, top_k, and presence_penalty with the upstream non-thinking guidance
- Do NOT set `min_new_tokens` on OpenVINO GenAI 2026.0.0 — any value >= 1 permanently suppresses EOS detection, causing runaway generation
- PowerShell wrappers around native tools must coerce stderr records to plain strings before logging or `$ErrorActionPreference = 'Stop'` will treat normal tool output as a fatal error
- After a long-running model export times out at the shell layer, check for orphaned builder `python` processes before retrying or the next validation pass starts from a dirty state
- Do not hard-block DirectML mode selection on the background hardware probe; the probe can time out while DirectML runtime initialization still works with a valid staged artifact
- NPU StaticLLMPipeline only supports greedy decoding — always force do_sample=false for NPU target
- presence_penalty is incompatible with NPU greedy decoding — skip it on NPU
- NPU StaticLLMPipeline does not support extra_context API for thinking control — inject /nothink into the system message content instead
- Force-backend env var is `SMOLPC_FORCE_EP` (not `SMOLPC_FORCE_BACKEND`)
- Use `starts_with` not `contains` for directive idempotency checks — `contains` matches user content substrings
- Qwen3 chat_template.jinja defaults to thinking mode when enable_thinking is undefined — NPU requires the template condition to be patched to default to non-thinking
- Qwen3-4B INT4 produces garbage on NPU but INT8_SYM per-channel (via `nncf.compress_weights`) works — INT8 is the NPU variant, INT4 stays for CPU
- Model idle unload must default to disabled (None) — a 30s timeout causes the "unhealthy after idle" bug
- HuggingFace `tokenizer_config.json` `chat_template` can be a string OR an array of `{name, template}` objects — handle both
- Use `env_logger::init()` in the engine host main() — without it, all log::info!/warn! calls are silently discarded
- Qwen3 NPU template patch failure must be a hard error, not a warning — un-patched template defaults to thinking mode causing runaway generation
- CPU and DirectML preflights need timeouts (30s/60s) via spawn_blocking — a hung GPU driver or malformed model can block the load path forever
- Use a drop guard (TransitionGuard) for model_transition_in_progress — load_model has many early return paths
- In PowerShell packaging scripts, use `$env:SystemRoot\System32\tar.exe` not bare `tar` — Git Bash tar intercepts and can't handle Windows paths
- PowerShell `Compress-Archive` has a 2 GB limit — use Windows tar.exe for large model archives
- PowerShell array splatting (`@array`) can split flag strings — use explicit if/else for cargo build flags
- Tauri resource map format (`"libs/": "libs/"`) recursively copies directories including subdirectories — preferred over glob arrays for nested DLL layouts
- DirectML on Intel integrated GPU produces garbage (no EOS, runaway generation) — only accept discrete GPUs as DirectML candidates
- Tauri NSIS `installMode: "currentUser"` installs to `%LOCALAPPDATA%\<productName>\` and kebab-cases the binary name (e.g., `smolpc-code-helper.exe`)
- Detached engine processes must redirect stderr to a log file — `Stdio::null()` makes crash diagnosis impossible on user machines
- `npm ci` fails with EPERM on Tailwind's `.node` binary when VS Code Tailwind IntelliSense extension holds a file lock — close VS Code or disable the extension before running `npm ci`
- Tauri 2 `Builder::run()` has no RunEvent access — use `Builder::build()` + `App::run(callback)` to hook `ExitRequested` for cleanup
- Use `tauri::async_runtime::block_on()` for async cleanup in `RunEvent::ExitRequested` — the closure is sync (`FnMut`), not async
- Engine PID must be written to `engine.pid` on spawn — DETACHED_PROCESS children survive parent exit and need explicit cleanup
- Verify PID identity before force-killing — stale `engine.pid` after crash can match a reused PID from an unrelated process
- Clean up `engine.pid` on both graceful exit and after force-kill to prevent stale PID accumulation
- GIMP NSIS installer defaults to per-user install at `%LOCALAPPDATA%\Programs\GIMP 3\`, not `Program Files` — check both paths
- GIMP 3.2 creates its profile under `GIMP/3.2/`, not `GIMP/3.0/` — resolve profile version dynamically by scanning the config root
- Blender version detection must use dynamic directory enumeration, not hardcoded versions — Blender Foundation updates version numbers frequently
- When removing a dependency, trace every consumed field to its terminal behavior — a "hint" boolean can be a hard gate downstream (e.g., `npu_hardware_detected` gates the entire OpenVINO probe)
- After replacing a hardware data source, verify all backend paths with live tests (`SMOLPC_FORCE_EP=openvino_npu` curl test) — unit tests don't catch hardware path regressions
- DXGI adapter enumeration is 1000x faster than WMI for GPU detection — use `IDXGIFactory6::EnumAdapterByGpuPreference` with fallback to `IDXGIFactory1::EnumAdapters1`

---

## Resources

- [OpenVINO 2026.0.0 Release](https://github.com/openvinotoolkit/openvino/releases/tag/2026.0.0)
- [OpenVINO GenAI NPU Guide](https://docs.openvino.ai/2025/openvino-workflow-generative/inference-with-genai/inference-with-genai-on-npu.html)
- [OpenVINO Local Distribution](https://docs.openvino.ai/2025/openvino-workflow/deployment-locally/local-distribution-libraries.html)
- [Tauri 2 Docs](https://v2.tauri.app/)
- [Svelte 5 Runes](https://svelte.dev/docs/svelte/what-are-runes)
- [Tailwind CSS 4](https://tailwindcss.com/docs)
- [HuggingFace: Qwen2.5-1.5B-Instruct-int4-ov](https://huggingface.co/OpenVINO/Qwen2.5-1.5B-Instruct-int4-ov)
- [HuggingFace: Qwen3-4B-int4-ov](https://huggingface.co/OpenVINO/Qwen3-4B-int4-ov)
