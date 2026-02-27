# Codex Working Issues

Last updated: 2026-02-27
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

---

## Completed in This Branch

- DirectML inferencing operational via ONNX Runtime GenAI C-FFI
- CPU fallback and demotion safeguards preserved
- Runtime adapter abstraction in place
- DML export/tooling scripts added
- Dead-code cleanup completed (`cargo check --all-targets` warning-clean)
- Handoff docs refreshed (`CURRENT_STATE`, session log, DML rundown)

