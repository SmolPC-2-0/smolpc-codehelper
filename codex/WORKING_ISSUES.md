# Working Issues

Last updated: 2026-03-19
Base branch: `main`
Last known good commit: `1b3bbb4`

---

## Current Risk Register

### 1) OpenVINO NPU backend not yet finalized

- Status: Open
- Severity: High (core feature)
- Context:
  - OpenVINO CPU inference works (infinite loop bug fixed).
  - OpenVINO NPU is the primary acceleration target but not yet validated end-to-end.
  - NPU compilation is slow on first load; `CACHE_DIR` enables compiled blob reuse.
  - INT4 quantization required for broad NPU compatibility (NF4 only works on Core Ultra Series 2+).
- Next steps:
  - Validate NPU lane with qwen2.5-1.5b-instruct and qwen3-4b IR artifacts.
  - Verify DLL load order for NPU plugin (`openvino_intel_npu_plugin`).
  - Test structured chat history path on NPU.
  - Confirm stop token detection (both Qwen2.5 and Qwen3).

### 2) Production model-path resolution for packaged app

- Status: Open (deferred to packaging phase)
- Severity: High (shipping readiness)
- Context:
  - `ModelLoader` default path is still dev-oriented (`CARGO_MANIFEST_DIR/models`) unless overridden.
  - Packaged executable should resolve model storage under runtime app data paths.
  - Packaging approach established on `feat/windows-dml-packaging` branch.
- Next steps:
  - Move default model root to app data directory.
  - Validate model load/generation on clean installed build.

### 3) Windows installer/runtime validation matrix incomplete

- Status: Open (deferred to packaging phase)
- Severity: High (release confidence)
- Context:
  - DirectML and OpenVINO CPU paths are locally validated in dev.
  - Need clean-machine verification for bundled DLLs and runtime behavior.
- Next steps:
  - Test matrix: Windows 10 20H1+, Windows 11, iGPU-only and hybrid GPU systems.
  - Verify all three backends (OpenVINO NPU, DirectML, CPU) in packaged form.

### 4) Backend diagnostics not fully surfaced in frontend

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

- Qwen2.5 OpenVINO artifacts — complete: `openvino_config.json` and `chat_template.jinja` restored locally; manifest now resolves all 15 required files (`codex/qwen25-openvino-artifact`)
- OpenVINO CPU infinite loop — fixed via structured chat history and stop token enforcement
- DirectML qwen3-4b export — completed
- OpenVINO acceleration path — decided: GenAI C-FFI (not ORT EP)
- Auto-selection false-negative from startup probe — fixed: probe timeout no longer hard-blocks backend selection
- Frontend Prettier drift — fixed: repo-wide formatting pass applied (2026-03-19)
- Tauri bundle staging — hardened (commit 1b3bbb4)
- DML background probe non-blocking — fixed (commit 89f3146)
