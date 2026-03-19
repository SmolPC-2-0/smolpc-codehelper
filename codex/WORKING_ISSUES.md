# Working Issues

Last updated: 2026-03-19
Base branch: `main`
Last known good commit: `b383a1a` (Qwen2.5 NPU working, PR #105 merged)

---

## Current Risk Register

### 1) OpenVINO NPU — Qwen3-4B produces minimal output

- Status: Open (investigation needed)
- Severity: High (core feature — Qwen2.5 NPU works, Qwen3 does not)
- Context:
  - **Qwen2.5-1.5B NPU: WORKING** — coherent output, clean EOS, validated 2026-03-19.
  - **Qwen3-4B NPU: BROKEN** — generates 1-2 tokens then hits EOS under greedy decoding.
  - Root causes fixed so far (branch `fix/npu-inference`):
    - Forced `do_sample=false` on NPU (was enabling multinomial sampling on greedy-only hardware).
    - Skipped `presence_penalty` on NPU (incompatible with greedy decoding).
    - Injected `/nothink` into system message instead of using `extra_context` API (NPU does not support it; caused immediate EOS).
  - Remaining Qwen3-4B issue: greedy decoding selects EOS after 1-2 tokens. Possible causes:
    - `MAX_PROMPT_LEN=512` may be too small for Qwen3's templated prompt (needs investigation).
    - Qwen3-4B INT4 quantization may interact differently with NPU greedy decoding.
    - Model architecture differences vs Qwen2.5 (thinking mode, longer chat template).
- Next steps:
  - Research OpenVINO GenAI StaticLLMPipeline constraints for Qwen3 architecture.
  - Test with increased `MAX_PROMPT_LEN` (env var override exists).
  - Compare prompt token counts between Qwen2.5 and Qwen3 for the same input.
  - Consider whether Qwen3-4B NPU is feasible or if it should be CPU/DirectML only.

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
