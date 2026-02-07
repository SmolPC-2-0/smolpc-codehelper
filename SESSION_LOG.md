# Session Log

This file tracks progress across Claude Code sessions for SmolPC Code Helper.

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
