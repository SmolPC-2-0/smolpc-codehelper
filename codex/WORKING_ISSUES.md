# Working Issues

Last updated: 2026-03-21
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

### 3) Benchmark CLI release run skips expected model combos on this laptop

- Status: Open
- Severity: Medium
- Context:
  - On `feat/benchmark` commit `c8a8662`, `cargo run --release -p smolpc-benchmark -- --machine igpu-32gb --resource-dir "%LOCALAPPDATA%\\SmolPC Code Helper"` completed but skipped 3/4 combos.
  - Fast repro (no generation): `cargo run --release -p smolpc-benchmark -- --machine igpu-32gb-cpu-two-model-smoke --backends cpu --models qwen2.5-1.5b-instruct,qwen3-4b --runs 0 --warmup 0 --cooldown 1 --resource-dir "%LOCALAPPDATA%\\SmolPC Code Helper"` reproduces the `qwen3-4b` conflict in ~20s.
  - CPU lane ran `qwen2.5-1.5b-instruct` successfully, but `qwen3-4b` failed with `HTTP 409 STARTUP_POLICY_CONFLICT` (`Engine is already ready under a different startup mode/policy`).
  - Auto-detection included `openvino_npu`, then both NPU combos failed with `HTTP 503 STARTUP_MODEL_LOAD_FAILED` (`NPU hardware was detected, but OpenVINO did not expose an NPU device`).
  - Result file: `benchmark-results/benchmark-igpu-32gb-2026-03-21.json`.
- Next steps:
  - Reproduce with explicit backends (`--backends cpu`) to unblock benchmark practice sessions.
  - Fix benchmark lifecycle for per-model CPU runs (likely needs explicit model load/switch semantics between combos).
  - Tighten backend auto-detect or preflight gating so unsupported NPU lanes are skipped before scheduling combos.

---

## Recently Resolved

- **Engine production readiness — 20 fixes** (2026-03-20, PR #107): Logger init, idle unload bug, DirectML>NPU>CPU priority, health endpoint, probe timeouts, preflight timeouts, panic removal, template patch errors, SSE dedup, crash detection, race guards, shutdown cancellation. Live-tested: idle stability confirmed at 90s, DirectML auto-selection working.
- **Qwen3-4B NPU inference — INT8** (2026-03-20, PR #106): INT4 garbage on NPU. Requantized FP16→INT8_SYM (3.75 GB). 34 coherent tokens, 8.1 tok/s.
- **Qwen2.5-1.5B NPU inference** (2026-03-19): greedy decoding enforced, presence_penalty skipped, structured messages on NPU.
- **OpenVINO CPU infinite loop** — fixed via structured chat history and stop token enforcement.
- **DirectML qwen3-4b export** — completed.
- **Auto-selection false-negative** — fixed: probe timeout no longer hard-blocks backend selection.
- **Frontend Prettier drift** — fixed: repo-wide formatting pass (2026-03-19).
