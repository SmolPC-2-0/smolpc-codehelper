# Working Issues

Last updated: 2026-03-22
Base branch: `main` (unified frontend PR #121 + engine module split PR #141 + clippy fixes PR #145)
Stable baseline tag: `stable/codehelper-v2.2.0`
Master roadmap: issue #143

---

## Current Risk Register

### 1) Intermittent hardware detection failure (WMI hang)

- Status: Resolved (codex/issue-123-hardware-query-hang, commit 37c79b3)
- Severity: Medium (non-blocking — retry works)
- Context:
  - Tauri hardware detection no longer calls `hardware_query::HardwareInfo::query()` or WMI/COM paths.
  - Detection now uses `sysinfo` for CPU, memory, and storage and returns safe GPU/NPU fallbacks.
  - Engine host retains its independent 9.5s probe timeout guard for startup probing.

### 2) NPU (OpenVINO) — qwen2.5 works, qwen3-4b fails

- Status: Open (partially working)
- Severity: Medium
- Verified 2026-03-22 (direct engine curl tests, `SMOLPC_FORCE_EP=openvino_npu`):
  - **qwen2.5-1.5b-instruct on NPU: PASS** — TTFT 873ms, 8.5 TPS
  - **qwen3-4b on NPU: FAIL** — `openvino_npu_compile_failed: ov_genai_llm_pipeline_create: unknown exception`
  - NPU hardware detected (Intel AI Boost, driver 1004621), startup probe passes.
  - qwen3-4b `openvino/` artifact is 3.8 GB INT8_SYM — pipeline creation fails before any generation attempt.
  - Template is already patched for non-thinking (empty `<think>` block when `enable_thinking` undefined).
- Context:
  - qwen2.5-1.5b-instruct (1.5B, INT4) loads and runs fine on NPU in dev mode.
  - qwen3-4b (4B, INT8_SYM) fails at pipeline creation — may be model size, quantization format, or NPU compiler limitation.
  - The installed app previously worked for qwen2.5 on NPU; qwen3-4b NPU was not previously confirmed working.
- Next steps:
  - Test qwen3-4b NPU with the packaged installer to rule out dev-mode DLL issues.
  - Try INT4 variant (`openvino-int4/`) on NPU despite known quality issues — may at least load.
  - Check if OpenVINO 2026.0.0 NPU compiler supports Qwen3 architecture at INT8 precision.
  - Consider filing upstream OpenVINO issue if the architecture is supported but compilation fails.

### 3) Packaging — model-path resolution and installer validation

- Status: Open (deferred — unified frontend is higher priority)
- Severity: High (shipping readiness)
- Context:
  - `ModelLoader` release-mode path fixed: uses `current_exe().parent()/models`. `%LOCALAPPDATA%\SmolPC\models` preferred via env override.
  - Prior work on `feat/windows-dml-packaging` branch (7 commits, 2 working paths).
  - Need clean-machine verification for bundled DLLs and runtime behavior.
- Next steps:
  - Extend packaging to include OpenVINO DLLs alongside DirectML.
  - Test matrix: Windows 10 20H1+, Windows 11 across 3 hardware targets.
  - Verify all three backends (OpenVINO NPU, DirectML, CPU) in packaged form.

### 4) Unified frontend — remaining wiring work

- Status: Mostly resolved
- Mode routing: ✅ PR #150
- Per-mode chat sessions: ✅ PR #151
- Host app detection: ✅ PR (app-connectivity) — GIMP per-user paths, Blender dynamic glob, GIMP 3.2 profile
- Remaining: end-to-end tool execution testing (GIMP bridge, Blender addon, LibreOffice UNO)

---

## Recently Resolved

- **Issue #156 LibreOffice helper startup timeout (Writer mode)** (2026-03-22)
  - Status: Resolved (codex/issue-156-libreoffice-helper-timeout, commit 2c203ca)
  - Scope: LibreOffice MCP runtime now resolves helper interpreter from the LibreOffice install (`program/python(.exe)`), with optional override and early-fail startup detection when helper exits before port 8765 is ready.
- **Issue #142 state module split polish** (2026-03-22)
  - Status: Resolved (codex/issue-142-state-module-split, commit d3b5c26)
  - Scope: Split `state.rs` by extracting model loading flow (`model_loading.rs`), startup orchestration (`startup.rs`), and runtime adapter builders (`adapters.rs`) while preserving behavior and checks.
- **Issue #137 memory-aware degradation (P1)** (2026-03-22)
  - Status: Resolved (codex/issue-137-memory-pressure, commit ab63120)
  - Scope: Added runtime memory-pressure polling with warnings, minimized-state critical auto-unload, memory-impact model labels, and explicit OOM fallback guidance toward smaller models.
- **Issue #137 PR review follow-ups** (2026-03-22)
  - Status: Resolved (codex/issue-137-memory-pressure, commit ea7b160)
  - Scope: Stabilized memory-pressure banner dismiss keys, documented the sysinfo unit heuristic assumption, cross-referenced heavy host-mode checks to the mode registry, and centralized startup error-code constants.
- **Issue #137 hardening follow-ups** (2026-03-22)
  - Status: Resolved (codex/issue-137-memory-pressure, commit 8a3b988)
  - Scope: Pinned sysinfo to 0.32.1, shifted auto-unload race handling to rely on host-side unload guards, and restored a legacy `path` alias in model list payloads for compatibility.
- **PR #158 follow-up review fixes** (2026-03-22)
  - Status: Resolved (codex/pr158-crash-safety-followups-v2, commit 1db963d)
  - Scope: Setup cache disk I/O moved off async worker threads, `prepare_setup` now persists only when `last_error` changes, and composer drafts are cleared when chats are archived/deleted.
- **Student work persistence and crash recovery (P1)** (2026-03-22)
  - Status: Resolved (codex/persistence-crash-safety, commit e6a6453)
  - Scope: Crash-safe chat/model/mode persistence, draft autosave, stale-stream recovery, and setup host-detection cache persistence.
- **Engine lifecycle robustness P0** (2026-03-22, PR #148): Graceful engine shutdown on app close (ExitRequested + PID fallback), cancel-on-mode-switch, 10s health polling with disconnected banner.
- **Clippy zero warnings** (2026-03-22, PR #145): Fixed all 8 workspace warnings (Box enum variants, inlined format args, derive Default, contains, needless return).
- **Engine module split** (2026-03-22, PR #141): main.rs 4,650→112 lines (10 modules), engine-client lib.rs 1,777→560 lines (4 modules), genai FFI split (2 new files each). 123 tests pass.
- **Unified frontend reconciliation v2** (2026-03-22, PR #121): Additive merge of unified mode shell onto main's stable engine. 5 commits, ~110 new files, 2 existing files modified (57 lines added). Code mode fully preserved. Pre-merge inference test matrix (all via direct engine curl, same path as frontend):
  - CPU + qwen2.5-1.5b: PASS (TTFT 112ms, 41 TPS)
  - CPU + qwen3-4b: PASS (TTFT 1194ms, 12.8 TPS)
  - DirectML + qwen2.5-1.5b: PASS (TTFT 406ms, 15.5 TPS)
  - DirectML + qwen3-4b: PASS (TTFT 332ms, 10.4 TPS)
  - NPU + qwen2.5-1.5b: PASS (TTFT 873ms, 8.5 TPS)
  - NPU + qwen3-4b: FAIL (pipeline creation exception — see issue #2)
- **Unified frontend v1 attempt** (2026-03-21, `feat/unified-frontend`): Wholesale replacement approach caused regressions (409 errors, locked inference mode, NPU support removed). Abandoned in favor of v2 additive approach.
- **Benchmark CLI** (2026-03-21, `feat/benchmark`): Interactive mode, discovery probing, model-switch fix. Parked — not blocking.
- **Engine production readiness — 20 fixes** (2026-03-20, PR #107): Logger init, idle unload bug, DirectML>NPU>CPU priority, health endpoint, probe timeouts, preflight timeouts, panic removal, template patch errors, SSE dedup, crash detection, race guards, shutdown cancellation.
- **All-backend offline packaging** (2026-03-20, PR #108): NSIS installer, DLL bundling, sidecar engine host.
- **Qwen3-4B NPU inference — INT8** (2026-03-20, PR #106): INT4 garbage on NPU. Requantized FP16→INT8_SYM (3.75 GB).
- **Qwen2.5-1.5B NPU inference** (2026-03-19): greedy decoding enforced, presence_penalty skipped, structured messages on NPU.
- **OpenVINO CPU infinite loop** — fixed via structured chat history and stop token enforcement.
- **DirectML qwen3-4b export** — completed.
- **Auto-selection false-negative** — fixed: probe timeout no longer hard-blocks backend selection.
- **Frontend Prettier drift** — fixed: repo-wide formatting pass (2026-03-19).
