# Session Log

This file tracks progress across Claude Code sessions for SmolPC Code Helper.

---

## 2026-02-24 (Session 8) - Hardware Identity Enrichment (Milestone 3)

**Focus**: Implement Milestone 3 from DirectML integration plan

**Branch**: `codex/directml-inferencing`

**Completed**:
- Extended Rust GPU IPC type in `src-tauri/src/hardware/types.rs`:
  - Added `driver_version: Option<String>`
  - Added `pci_device_id: Option<String>`
- Updated GPU conversion in `src-tauri/src/hardware/detector.rs`:
  - Populates new fields from `hardware-query` GPU metadata
  - Normalizes empty strings to `None`
- Synced frontend type in `src/lib/types/hardware.ts`:
  - Added optional `driver_version` + `pci_device_id` fields in `GpuInfo`

**Quality Gates**:
- `cargo check` (Rust 1.88 toolchain): ✅ pass
- Targeted tests (`cargo test hardware --lib`): ✅ pass (0 filtered tests, compile gate clean)

**Next Session / Next Commit Target**:
1. Milestone 4: backend-aware session builder (`Cpu` + `DirectML`) and same-flow fallback
2. Milestone 5: first-load benchmark selector, persistence key wiring, forced override env, demotion after 3 failures
3. Milestone 6: structured backend diagnostics logs + `get_inference_backend_status` command

**Last Known Good Commit**: `b7a8f1f` (Milestone 2)
**Resume From Step**: Milestone 4

## 2026-02-24 (Session 7) - DirectML Backend Domain + Persistence (Milestone 2)

**Focus**: Implement Milestone 2 from DirectML integration plan

**Branch**: `codex/directml-inferencing`

**Completed**:
- Added `src-tauri/src/inference/backend.rs`:
  - Backend enums (`InferenceBackend`), decision metadata (`BackendDecision`, `DecisionReason`)
  - Benchmark policy constants (`+30%` decode speedup, `+15%` TTFT regression cap, 2s budget)
  - Failure counter model with demotion threshold handling (`3` consecutive DirectML failures)
- Added `src-tauri/src/inference/backend_store.rs`:
  - Versioned backend decision store schema (`backend_decisions.v1.json`)
  - Key fingerprint persistence on `model + adapter + driver + app version + ORT version`
  - Atomic write path (`tmp` file + replace) and stale-record invalidation for key changes
- Exported backend domain types via `src-tauri/src/inference/mod.rs`
- Added unit tests for backend gate logic, demotion threshold, store round-trip, stale-key invalidation, and invalid JSON recovery

**Quality Gates**:
- `cargo check` (Rust 1.88 toolchain): ✅ pass
- Targeted tests (`cargo test backend --lib`): ✅ 6 passed

**Next Session / Next Commit Target**:
1. Milestone 3: hardware identity enrichment (`driver_version`, `pci_device_id`) across Rust + TS IPC types
2. Milestone 4: backend-aware session builder with DirectML + CPU fallback in same load flow
3. Milestone 5: first-load benchmark-gated selector + persistent decision application + 3-failure demotion wiring

**Last Known Good Commit**: `5f8cf76` (Milestone 1)
**Resume From Step**: Milestone 3

## 2026-02-24 (Session 6) - DirectML Plan Implementation Start (Milestone 1)

**Focus**: Execute Milestone 1 (toolchain + runtime packaging) from DirectML integration plan

**Branch**: `codex/directml-inferencing`

**Completed**:
- Bumped Rust MSRV in `src-tauri/Cargo.toml` to `1.88`
- Upgraded ONNX wrapper from `ort = 2.0.0-rc.10` to `ort = 2.0.0-rc.11`
- Added repo-level `rust-toolchain.toml` pinned to `1.88.0`
- Rewrote `scripts/setup-libs.sh`:
  - Windows now installs DirectML-capable runtime from NuGet
  - Bundles `onnxruntime.dll`, `onnxruntime_providers_shared.dll`, `DirectML.dll`
  - Adds SHA256 verification for all runtime package/archive downloads
  - Supports `windows-x64`, `windows-arm64`, `macos-arm64`, `macos-x64`, `linux-x64`, `linux-arm64`
- Updated `src-tauri/libs/README.md` with bundled runtime file expectations
- Applied ORT rc.11 compatibility fixes:
  - Session metadata access now uses `inputs()/outputs()` and `name()`
  - ORT init now handles `init_from(...)->EnvironmentBuilder` before `commit()`
  - Aligned local `ndarray` crate to `0.17` for `Value::from_array` compatibility
- Compile gate passed with Rust 1.88 toolchain (`cargo check`)

**Key Discoveries**:
- Local workstation default Cargo/Rustc path still points to Homebrew Rust `1.87.0`
- Explicit `RUSTC` override to Rustup 1.88 toolchain is currently required for local checks

**Next Session / Next Commit Target**:
1. Milestone 2: add backend domain model (`inference/backend.rs`)
2. Milestone 2: add persistent backend decision store (`inference/backend_store.rs`)
3. Wire minimum status surface for backend state needed by upcoming selector flow

**Blockers**: None for implementation; local toolchain path quirk is documented in `codex/WORKING_ISSUES.md`

## 2026-02-09 (Session 5) - Stop Token Fix + ChatML + Repetition Penalty

**Focus**: Fix runaway generation (model producing self-Q&A training data patterns)

**Branch**: `fix/stop-token-chatml` (PR → `feature/ort_setup`)

**Completed**:
- Diagnosed root cause: two bugs working together
  1. Only checked `<|im_end|>` (151645) stop token, but model generates `<|endoftext|>` (151643) without ChatML
  2. Raw text prompts caused model to fall into pretraining-data patterns
- Fixed tokenizer: `eos_token_id: u32` → `stop_token_ids: Vec<u32>` with both 151643 and 151645
- Added `is_stop_token(token_id)` method replacing `eos_token_id()` getter
- Updated generator stop checks (prefill + decode loop) to use `is_stop_token()`
- Changed `encode(prompt, true)` → `encode(prompt, false)` (ChatML tokens already in prompt)
- Replaced `buildContextPrompt()` with `buildChatMLPrompt()` using proper ChatML template
- Fixed message duplication bug (`slice(0, -2)` excludes just-added messages)
- Added repetition penalty (sign-aware, configurable window) to generation pipeline
- Synced `repetition_penalty` fields across Rust types, TS types, and inference store
- Fixed KV cache `dead_code` warnings with `#[allow(dead_code)]` + doc notes
- All checks pass: `cargo check`, `cargo clippy`, `cargo test`, `npm run check`

**Key Discoveries**:
- Qwen2.5-Coder special tokens: 151643 (`<|endoftext|>`), 151644 (`<|im_start|>`), 151645 (`<|im_end|>`)
- Without ChatML, model never generates `<|im_end|>` — only `<|endoftext|>` which passed through as literal text
- `buildContextPrompt()` was called AFTER adding user message to store, causing duplication

**Files Changed**:
- `src-tauri/src/inference/tokenizer.rs` (stop token set)
- `src-tauri/src/inference/generator.rs` (is_stop_token, encode false, repetition penalty)
- `src-tauri/src/inference/types.rs` (repetition_penalty fields)
- `src-tauri/src/inference/kv_cache.rs` (dead_code fixes)
- `src-tauri/src/inference/benchmark.rs` (Default::default fixes)
- `src/App.svelte` (ChatML template, config)
- `src/lib/types/inference.ts` (repetition_penalty fields)
- `src/lib/stores/inference.svelte.ts` (repetition_penalty passthrough)

**Next Session**:
1. Merge `fix/stop-token-chatml` PR
2. Verify on Windows with model loaded — test clean stop, no self-Q&A
3. Begin Phase 2: Execution Provider abstraction

**Blockers**: Must verify on Windows with model loaded

---

## 2026-02-07 (Session 4) - ONNX Runtime Bundling + Code Review Fixes

**Focus**: Bundle ONNX Runtime into app, comprehensive code review, fix 4 issues

**Branch**: `fix/channel-migration` (PR #24 → `feature/ort_setup`)

**Completed**:
- Implemented ONNX Runtime bundling plan (5 steps):
  - Created `scripts/setup-libs.sh` — cross-platform download script
  - Configured `tauri.conf.json` with `resources: ["libs/*"]`
  - Refactored `init_onnx_runtime()` to accept Tauri resource dir
  - Updated `lib.rs` to resolve and pass resource dir
  - Updated `.gitignore` for `src-tauri/libs/`
- Ran 3-way parallel code review (Rust, Frontend, Architecture)
- Fixed 4 issues identified by review:
  1. `Once` → `OnceLock<Result>` for proper error propagation
  2. Fatal ONNX init in production, non-fatal in dev
  3. `try_lock()` TOCTOU → dedicated `AtomicBool` for `is_generating`
  4. Removed duplicate `isGenerating` state from `App.svelte`
- Re-ran frontend review — all MUST FIX items resolved

**Key Discoveries**:
- ONNX Runtime v1.22.1 only ships Windows builds; v1.22.0 needed for macOS/Linux
- Tauri `resources` glob fails at compile time if no files match
- BSD `tar` on macOS doesn't extract single files same as GNU tar

**Files Changed**:
- `scripts/setup-libs.sh` (NEW)
- `src-tauri/libs/README.md` (NEW — glob satisfier)
- `src-tauri/tauri.conf.json` (resources config)
- `src-tauri/src/inference/mod.rs` (OnceLock, dylib search)
- `src-tauri/src/lib.rs` (resource dir, fatal init)
- `src-tauri/src/commands/inference.rs` (AtomicBool)
- `src-tauri/src/inference/generator.rs` (test call sites)
- `src-tauri/src/inference/benchmark.rs` (test call sites)
- `src-tauri/src/inference/session.rs` (test call sites)
- `src/App.svelte` (single source of truth for isGenerating)
- `.gitignore` (libs directory)

**Review Findings (SHOULD FIX — deferred to future sessions)**:
- GenerationConfig defaults mismatch (frontend temp=0.7 vs backend temp=1.0)
- AvailableModel TS type missing fields from Rust ModelDefinition
- Missing error display on auto-load failure
- Windows DLL search priority could be shadowed by user-placed DLL
- macOS code signing needed for bundled dylib (Phase 2 concern)

**Next Session (Windows laptop)**:
1. Check if Git Bash is available (needed for setup-libs.sh)
2. Merge PR #24 into `feature/ort_setup`
3. Run `scripts/setup-libs.sh` OR verify legacy `ort-extracted/` path works
4. Run full verification: `cargo check`, `cargo clippy`, `cargo test -- --ignored`, `npm run tauri dev`
5. Verify streaming generation works end-to-end with model loaded
6. If all passes → begin Phase 2

**Blockers**: Must verify on Windows with model loaded before Phase 2

---

## 2026-02-05 (Session 2) - Event Race Condition Fix Planning

**Focus**: Fix event listener race condition in `inference.svelte.ts`

**Completed**:
- Analyzed race condition in `generateStream()` method
- Created implementation plan: `docs/new_onnx_plan/FIX-EVENT-RACE-CONDITION.md`
- Identified Promise-wrapper pattern from `src/main.js` as solution

**Problem Identified**:
- `generateStream()` awaits `invoke()` which returns when Rust function returns
- `finally` block cleans up listeners BEFORE `inference_done` event arrives
- Results in: lost tokens, `isGenerating` stuck true, null metrics

**Solution** (NOT YET IMPLEMENTED):
- Set up listeners FIRST
- Create Promise that resolves on `inference_done` / rejects on `inference_error`
- Fire-and-forget `invoke()` (don't await it)
- AWAIT the completion Promise
- Clean up listeners in `finally` AFTER Promise settles

**Next Steps**:
- Implement the fix in `src/lib/stores/inference.svelte.ts`
- Test streaming generation end-to-end
- Verify no listener leaks

**Blockers**: None - session ended before implementation

---

## 2026-02-05 - CLAUDE.md Overhaul

**Focus**: Overhauling the CLAUDE.md and establishing session workflow

**Completed**:
- Reviewed current project state (Phase 1.5 complete)
- Reviewed docs/new_onnx_plan/ documentation
- Created new lean CLAUDE.md (~280 lines vs ~800 before)
- Established session startup/end protocols
- Defined task workflow with specialist agents
- Created SESSION_LOG.md (this file)
- Created .claude/plans/ directory structure
- Initialized MEMORY.md

**Key Findings**:
- Old CLAUDE.md had extensive Ollama references (now replaced by ONNX)
- Old roadmap was outdated (mentioned llama.cpp, not ONNX)
- Current inference engine: ~8 tok/s on CPU with KV cache + Attention Sinks
- Next phase: GPU/NPU acceleration (Phase 2)

**Next Steps**:
- Begin Phase 2 implementation (EP abstraction, hardware detection)
- Test on macOS for cross-platform validation
- Consider removing Ollama legacy code

**Blockers**: None

---

*Previous sessions can be found in git history and docs/new_onnx_plan/CURRENT_STATE.md*
