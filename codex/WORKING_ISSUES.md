# Working Issues

Last updated: 2026-03-22
Base branch: `main` (includes unified frontend from PR #121)
Stable baseline tag: `stable/codehelper-v2.2.0`

---

## Current Risk Register

### 1) Intermittent hardware detection failure (WMI hang)

- Status: Open
- Severity: Medium (non-blocking — retry works)
- Context:
  - `hardware_query::HardwareInfo::query()` uses WMI which intermittently hangs on `CoSetProxyBlanket` in async contexts.
  - The Tauri app's detector has a `spawn_blocking` + timeout wrapper, but the hang can still occur.
  - Engine host has its own 9.5s probe timeout guard.
  - When it fails, the hardware panel shows no data and backend probing may be incomplete.
  - Subsequent app restarts usually succeed.
- Next steps:
  - Consider replacing WMI-based detection with `sysinfo` for the Tauri app (same fix applied to the benchmark CLI).
  - Investigate COM apartment model conflicts with tokio runtime.

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

- Status: Open (PR #121 merged baseline; follow-up PRs needed)
- Severity: Low (Code mode fully functional)
- Context:
  - Mode dropdown and setup panel are wired in and functional.
  - GIMP/Blender/Writer mode handlers not yet routed in App.svelte's `handleSendMessage`.
  - Per-mode chat sessions not yet implemented (using single-chat model from main).
  - Setup "Prepare" button provisioning not yet tested end-to-end.
- Next steps:
  - Add mode routing to `handleSendMessage` (if gimp → `assistantSend`; if code → existing path).
  - Test GIMP/Blender/LibreOffice MCP connectivity with host apps installed.
  - Implement per-mode chat session switching.

---

## Recently Resolved

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
