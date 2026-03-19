# Working Issues

Last updated: 2026-03-19
Base branch: `main`
Last known good commit: `b383a1a` (Qwen2.5 NPU working, PR #105 merged)

---

## Current Risk Register

### 1) OpenVINO NPU — Qwen3-4B produces minimal output

- Status: **Upstream limitation confirmed** — template patch necessary but not sufficient
- Severity: Medium (Qwen3-4B NPU is not viable with current OpenVINO; Qwen2.5 NPU and Qwen3 CPU work)
- Root cause (template): Qwen3 `chat_template.jinja` requires `enable_thinking` to be explicitly defined for non-thinking mode. Engine now auto-patches at load time (branch `fix/qwen3-npu`).
- Root cause (accuracy): Even with template fix, Qwen3-4B INT4 on NPU produces 0-1 content tokens then EOS. This matches the OpenVINO 2025.3 release note: "reduced accuracy in chat scenarios" for Qwen3 architecture. The HuggingFace `Qwen3-4B-int4-ov` model card does NOT list NPU as a target device.
- Live test results (2026-03-19):
  - **Qwen3-4B NPU**: 2 tokens (" endeavour" + stop) — BROKEN (upstream limitation)
  - **Qwen3-4B CPU**: 27 tokens, coherent — WORKING
  - **Qwen2.5-1.5B NPU**: 24 tokens, coherent — WORKING (no regression)
  - **Qwen2.5-1.5B CPU**: 29 tokens, coherent — WORKING (no regression)
- Decision: Qwen3-4B should fall back to CPU or DirectML. The template patch is kept as a necessary correctness fix (self-healing for fresh downloads). NPU remains viable only for Qwen2.5.
- Next steps:
  - Ensure backend selection does not attempt NPU for Qwen3-4B (or accept graceful degradation).
  - Monitor future OpenVINO releases for Qwen3 NPU accuracy improvements.

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

- **Qwen2.5-1.5B NPU inference — WORKING** (2026-03-19, `fix/npu-inference`): greedy decoding enforced, presence_penalty skipped, structured messages work correctly on NPU. Live-tested: 24 coherent tokens, proper EOS.
- **NPU sampling fix** (2026-03-19, `fix/npu-inference`): `do_sample=false` forced for NPU; `presence_penalty` skipped; `extra_context` replaced with `/nothink` system message injection.
- Qwen2.5 OpenVINO artifacts — complete: `openvino_config.json` and `chat_template.jinja` restored locally; manifest now resolves all 15 required files (`codex/qwen25-openvino-artifact`)
- OpenVINO CPU infinite loop — fixed via structured chat history and stop token enforcement
- DirectML qwen3-4b export — completed
- OpenVINO acceleration path — decided: GenAI C-FFI (not ORT EP)
- Auto-selection false-negative from startup probe — fixed: probe timeout no longer hard-blocks backend selection
- Frontend Prettier drift — fixed: repo-wide formatting pass applied (2026-03-19)
- Tauri bundle staging — hardened (commit 1b3bbb4)
- DML background probe non-blocking — fixed (commit 89f3146)
