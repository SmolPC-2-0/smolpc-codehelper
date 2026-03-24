# CLAUDE.md

## Project Overview

SmolPC Code Helper is an **offline AI coding assistant** for secondary school students (ages 11-18).

**Principles:** offline-first (no cloud/telemetry), privacy-first (GDPR/FERPA), budget hardware (8GB RAM min), Intel NPU partnership (OpenVINO is primary acceleration target on Windows).

**Stack:** Rust engine (`axum` HTTP server) + Tauri 2 + Svelte 5 + Tailwind 4

**Architecture:**
- `engine/` — shared inference server: `smolpc-engine-host` (axum, 17 modules), `smolpc-engine-core` (backends, hardware, models), `smolpc-engine-client` (HTTP+SSE client for apps), `smolpc-tts-server` (standalone TTS sidecar).
- `apps/codehelper/` — Tauri 2 app with modes: Code (default), Blender, GIMP, LibreOffice. Each mode has its own executor, provider, and runtime under `src-tauri/src/modes/`.
- `launcher/` — suite shell (extraction target, currently embedded in `apps/codehelper/src-tauri/src/launcher/`).
- Engine runs as a local HTTP server on port 19432; Tauri apps connect via `smolpc-engine-client`.

**Backend selection priority:** `directml` (discrete GPU only) > `openvino_npu` > `cpu`. RAM-aware model defaults: 16GB+ gets `qwen3-4b`, <16GB gets `qwen2.5-1.5b-instruct`.

**Voice I/O:** Whisper STT via OpenVINO GenAI FFI (`/v1/audio/transcriptions`), TTS via `smolpc-tts-server` sidecar (`/v1/audio/speech`). Both Windows-only.

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

**Key docs:** @docs/ARCHITECTURE.md | [`docs/ENGINE_API.md`](docs/ENGINE_API.md) (290 lines — read on demand) | @docs/CONTRIBUTING.md

---

## Conventions

**Verify before claiming done.** Run pre-commit checks, live-test hardware paths with `SMOLPC_FORCE_EP=<backend> curl`, and confirm output BEFORE claiming success. Never say "should work" — prove it works.

**DLL loading is centralized.** All `Library::new()` / `load_with_flags()` calls live in `runtime_loading.rs`. A source-invariant test enforces this — adding DLL loading elsewhere fails CI.

**OpenVINO DLL load order matters.** Windows requires dependency order: `tbb12 -> openvino -> openvino_c -> openvino_ir_frontend -> openvino_intel_cpu_plugin -> openvino_intel_npu_plugin -> openvino_tokenizers -> openvino_genai`

**OpenVINO models must be IR format.** The OpenVINO lane expects `.xml` + `.bin` artifacts, not ONNX. The `openvino/manifest.json` is the artifact readiness gate.

**Preflight timeout = temporary fallback.** A preflight timeout must NOT be persisted as a negative decision. It stays `temporary_fallback` and does not overwrite a prior good record.

**Engine host owns all backend policy.** Launcher and Tauri app consume status only — they must not rank backends or override engine selection.

**Engine lifecycle owned by EngineSupervisor.** Tauri commands never spawn or kill the engine directly — they send commands via mpsc channel and read state via watch channel. See `.claude/rules/tauri-patterns.md` for detailed Tauri conventions.

**Svelte 5 runes only.** No `writable` / `readable` from `svelte/store`. Use `$state`, `$derived`, `$effect`.

**Tailwind 4.** No `@apply` — use utility classes directly in templates.

**Tauri Channels for streaming.** Use `tauri::ipc::Channel<T>` (command-scoped, ordered), not global Events.

**Conventional Commits with scope.** `feat(engine):`, `fix(openvino):`, `docs:`, etc.

**No AI attribution in commits or PRs.** Do not add `Co-Authored-By` lines to commits. Do not add "Generated with Claude Code" or similar attribution to PR descriptions.

**Pre-commit checks:**
```bash
cargo check --workspace && cargo clippy --workspace
cargo test -p smolpc-engine-core && cargo test -p smolpc-engine-host
cd apps/codehelper && npm run check && npm run lint
```

**Plugin and tool usage (mandatory):**

*Code intelligence — always use LSP over grep:*
- When tracing symbol usage: `LSP findReferences` (Rust) or TypeScript LSP (frontend)
- When finding where something is defined: `LSP goToDefinition`
- When checking what calls a function: `LSP incomingCalls`
- When removing/renaming a field or function: `LSP findReferences` first to find ALL consumers
- Subagents exploring the codebase MUST be instructed to use LSP

*Superpowers plugin — invoke via Skill tool when the trigger matches:*
- **Brainstorming** (`superpowers:brainstorming`): User says "let's brainstorm", "be creative", "what are our options", or asks to explore architectural decisions. Invoke BEFORE proposing solutions.
- **Writing plans** (`superpowers:writing-plans`): When given specs/requirements for a multi-step task, before touching code. Use instead of ad-hoc plan files.
- **Executing plans** (`superpowers:executing-plans`): When a written plan exists and is approved. Provides review checkpoints during implementation.
- **Verification before completion** (`superpowers:verification-before-completion`): BEFORE claiming work is done, fixed, or passing. Run verification commands, confirm output, THEN claim success. Never say "should work" — prove it works.
- **Receiving code review** (`superpowers:receiving-code-review`): When processing review feedback. Requires technical rigour — verify the reviewer's claims against the code before implementing fixes. Don't blindly agree.
- **Systematic debugging** (`superpowers:systematic-debugging`): When encountering bugs, test failures, or unexpected behavior. Use before proposing fixes.
- **Dispatching parallel agents** (`superpowers:dispatching-parallel-agents`): When facing 2+ independent tasks that can be parallelised.
- **Code review** (`superpowers:requesting-code-review`): When completing major features or before merging — verify work meets requirements.

---

## Learnings

Hard-won rules from development. Organized by theme. **Detailed per-subsystem rules live in `.claude/rules/`** — these are the cross-cutting principles.

*Hardware validation:*
- Unit tests validate logic; live `curl` tests validate hardware paths. After changing generation config, backend selection, or model loading: start the engine with `SMOLPC_FORCE_EP=<backend>` and curl a streaming chat completion before declaring it fixed
- When removing or replacing a dependency, trace every consumed field to its terminal behavior — a "hint" boolean can be a hard gate downstream
- DirectML on Intel integrated GPU produces garbage — only accept discrete GPUs as DirectML candidates
- DXGI adapter enumeration (`IDXGIFactory6`) is 1000x faster than WMI for GPU detection

*Rust state management:*
- Use RAII drop guards for multi-path state transitions (e.g., `TransitionGuard` for `model_transition_in_progress`)
- Use `OnceLock<Result>` over `Once` for fallible init; `AtomicBool` over `try_lock()` for flags
- CPU and DirectML preflights need timeouts via `spawn_blocking` — hung GPU drivers block the load path forever

*Windows process lifecycle:*
- Detached processes need: PID file on spawn, identity check before kill, stderr→log file, cleanup on all exit paths (graceful + force-kill)
- Use `CREATE_NO_WINDOW` for background sidecars to prevent console flash
- Force-backend env var is `SMOLPC_FORCE_EP` (not `SMOLPC_FORCE_BACKEND`)

*Config parsing:*
- HuggingFace `tokenizer_config.json` `chat_template` can be a string OR an array of `{name, template}` objects — handle both
- Use `starts_with` not `contains` for directive idempotency checks — `contains` matches user content substrings
- Model idle unload must default to disabled (`None`) — a timeout causes the "unhealthy after idle" bug

*Workflow discipline:*
- Don't dismiss broken checks as "pre-existing" — if verification fails, fix it in the current session
- Always use LSP (`findReferences`, `incomingCalls`, `goToDefinition`) when tracing symbol usage — grep misses indirect consumers. Subagents must also use LSP.

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

---

## Compaction

When compacting, always preserve: the current task and its acceptance criteria, all modified file paths, which backend/model combination is being tested, and any test commands that must be re-run.
