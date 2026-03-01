# Codex Working Issues

Last updated: 2026-03-01
Base branch: `codex/directml-inferencing`
Last known good commit: `7460015`

---

## Current Risk Register

### 1) Production model-path resolution for packaged app

- Status: Open
- Severity: High (shipping readiness)
- Introduced in: historical model-loader defaults (pre-existing)
- Context:
  - `ModelLoader` default path is still dev-oriented (`CARGO_MANIFEST_DIR/models`) unless overridden.
  - Packaged executable should resolve model storage under runtime app data paths.
- Mitigation:
  - Move default model root to app data directory.
  - Add first-run copy/sync flow from bundled assets (if bundling starter model).
  - Validate model load/generation on clean installed build.

### 2) Windows installer/runtime validation matrix incomplete

- Status: Open
- Severity: High (release confidence)
- Introduced in: N/A (validation gap)
- Context:
  - DirectML GenAI runtime path is implemented and locally validated in dev.
  - Need clean-machine verification for bundled DLLs and runtime behavior.
- Mitigation:
  - Run matrix:
    - Windows 10 20H1+
    - Windows 11
    - iGPU-only and hybrid GPU systems
  - Verify:
    - `SMOLPC_ENABLE_DML_GENAI=1` path
    - forced DML behavior
    - fallback/demotion behavior

### 3) Backend diagnostics not yet surfaced in frontend

- Status: Open
- Severity: Medium
- Introduced in: directml backend diagnostics phase
- Context:
  - Backend exposes `get_inference_backend_status` with runtime engine/gate/probe/failure info.
  - UI does not yet display this status.
- Mitigation:
  - Add TypeScript status model and store integration.
  - Show active runtime (`genai_dml` vs `ort_cpu`) and fallback reason in UI.

### 4) OpenVINO acceleration path decision pending

- Status: Open
- Severity: Medium
- Introduced in: post-DML planning
- Context:
  - DML path is GenAI C-FFI.
  - OpenVINO can be added via ORT EP or via GenAI (likely build-from-source/runtime packaging complexity).
- Mitigation:
  - Choose implementation track:
    1. ORT OpenVINO EP first (faster delivery)
    2. GenAI OpenVINO path (heavier integration/build work)
  - Define artifact/runtime packaging contract before coding.

### 5) Auto-selection can false-negative to CPU due to startup probe gating

- Status: Open
- Severity: High (performance + default UX regression)
- Introduced in: startup-probe-based backend gating in shared host selection flow
- Context:
  - In auto mode, some DirectML-capable machines are being classified as `cpu-only` during host startup probe.
  - That result can be persisted in backend decisions, causing repeated CPU selection in later runs.
  - This conflicts with desired behavior: app should select best available backend by default (`NPU/OpenVINO` > `GPU/DirectML` > `CPU`) with seamless fallback.
- Proposed approach:
  1. Keep probe as a ranking signal, not a hard capability gate.
  2. Build an ordered candidate list from policy + artifact availability.
  3. Validate candidates via runtime init + minimal preflight, then select first healthy backend.
  4. Persist only validated runtime decisions; do not persist `cpu-only` results from probe timeout/unknown states.
  5. Add explicit selection reasons: `startup_probe_timeout`, `probe_recovery_attempt`, `probe_recovery_failed`.
  6. Surface selected backend and reason in UI status indicator.
- Acceptance criteria:
  1. On DirectML-capable Windows machine, auto mode selects DirectML without manual env overrides.
  2. If probe is delayed or misses detection, host still attempts runtime validation and can recover to DirectML.
  3. CPU fallback remains deterministic when runtime validation fails.
  4. Decision store no longer locks future runs to CPU from probe-only false negatives.
  5. `/engine/status` clearly explains selection path and fallback reason.

---

## Completed in This Branch

- DirectML inferencing operational via ONNX Runtime GenAI C-FFI
- CPU fallback and demotion safeguards preserved
- Runtime adapter abstraction in place
- DML export/tooling scripts added
- Dead-code cleanup completed (`cargo check --all-targets` warning-clean)
- Handoff docs refreshed (`CURRENT_STATE`, session log, DML rundown)
