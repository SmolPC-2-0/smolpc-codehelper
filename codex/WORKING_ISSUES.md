# Working Issues

Last updated: 2026-03-20
Base branch: `main`
Last known good commit: `3d7460e` (Engine production readiness — 20 fixes, PR #107 merged)

---

## Current Risk Register

### 1) Packaging — model-path resolution and installer validation

- Status: Open (next up — Roadmap Step 2)
- Severity: High (shipping readiness)
- Context:
  - `ModelLoader` release-mode path fixed: uses `current_exe().parent()/models`. `%LOCALAPPDATA%\SmolPC\models` preferred via env override.
  - Prior work on `feat/windows-dml-packaging` branch (7 commits, 2 working paths).
  - Need clean-machine verification for bundled DLLs and runtime behavior.
- Next steps:
  - Extend packaging to include OpenVINO DLLs alongside DirectML.
  - Test matrix: Windows 10 20H1+, Windows 11 across 3 hardware targets.
  - Verify all three backends (OpenVINO NPU, DirectML, CPU) in packaged form.

### 2) Backend diagnostics not fully surfaced in frontend

- Status: Open
- Severity: Medium
- Context:
  - Backend exposes `get_inference_backend_status` with runtime engine/gate/probe/failure info.
  - UI partially displays status but full diagnostics are not exposed.
- Next steps:
  - Verify current frontend status display coverage.
  - Add missing status fields if needed for handoff demo.

---

## Recently Resolved

- **Engine production readiness — 20 fixes** (2026-03-20, PR #107): Logger init, idle unload bug, DirectML>NPU>CPU priority, health endpoint, probe timeouts, preflight timeouts, panic removal, template patch errors, SSE dedup, crash detection, race guards, shutdown cancellation. Live-tested: idle stability confirmed at 90s, DirectML auto-selection working.
- **Qwen3-4B NPU inference — INT8** (2026-03-20, PR #106): INT4 garbage on NPU. Requantized FP16→INT8_SYM (3.75 GB). 34 coherent tokens, 8.1 tok/s.
- **Qwen2.5-1.5B NPU inference** (2026-03-19): greedy decoding enforced, presence_penalty skipped, structured messages on NPU.
- **OpenVINO CPU infinite loop** — fixed via structured chat history and stop token enforcement.
- **DirectML qwen3-4b export** — completed.
- **Auto-selection false-negative** — fixed: probe timeout no longer hard-blocks backend selection.
- **Frontend Prettier drift** — fixed: repo-wide formatting pass (2026-03-19).
