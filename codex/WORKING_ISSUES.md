# Working Issues

Last updated: 2026-03-20
Base branch: `main`
Last known good commit: `38607a4` (Qwen3-4B NPU INT8 + template patch, PR #106 merged)

---

## Current Risk Register

### 1) OpenVINO NPU — Qwen3-4B INT8 quantization resolves NPU accuracy

- Status: **RESOLVED** — INT8_SYM per-channel quantization fixes NPU output quality
- Severity: ~~Medium~~ → Resolved
- Root cause (template): Qwen3 `chat_template.jinja` requires `enable_thinking` to be explicitly defined for non-thinking mode. Engine now auto-patches at load time (branch `fix/qwen3-npu`).
- Root cause (accuracy): Qwen3-4B INT4 on NPU produces 0-1 content tokens then EOS — upstream INT4 quantization is too aggressive for NPU inference. INT8_SYM per-channel (3.75 GB, 50% of FP16) restores full accuracy.
- Live test results (2026-03-20, INT8_SYM on NPU):
  - **Qwen3-4B NPU (INT8)**: 34 tokens, coherent, proper EOS — **WORKING** (8.1 tok/s, TTFT 1.4s)
  - **Qwen3-4B CPU (INT4)**: 27 tokens, coherent — WORKING
  - **Qwen2.5-1.5B NPU (INT4)**: 24 tokens, coherent — WORKING (no regression)
- Previous INT4 results (2026-03-19): 2 tokens (" endeavour" + stop) — BROKEN
- Resolution: Requantized FP16 → INT8_SYM using `nncf.compress_weights()`. FP16 (7.49 GB) too large for StaticLLMPipeline; INT4 (2.2 GB) had garbage output; INT8 (3.75 GB) is the sweet spot.
- Quantization method: `nncf.compress_weights(model, mode=nncf.CompressWeightsMode.INT8_SYM)` — symmetric, per-channel, all 253 layers.
- Artifact layout:
  - `openvino/` — INT8_SYM (active, 3.75 GB)
  - `openvino-fp16/` — FP16 (backup, 7.49 GB)
  - `openvino-int4/` — INT4 (backup, 2.2 GB)

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

### 4) Engine health check reports "unhealthy" after idle, client reconnect never recovers

- Status: Open
- Severity: Low-Medium (restart resolves; user-facing annoyance)
- Context:
  - After extended idle time with a loaded NPU model, the engine health/cache check transitions to an unhealthy state.
  - The client displays a "reconnecting" status that never resolves until the engine is restarted.
  - Observed during Qwen3-4B INT8 NPU testing (2026-03-20). Manual restart of the engine fully recovers.
- Likely causes:
  - NPU StaticLLMPipeline may not respond to keepalive/health pings after idle timeout.
  - SSE connection or health poll could be hitting a stale cached state that isn't cleared on reconnect.
- Next steps:
  - Investigate engine-side health check behavior when NPU pipeline is idle.
  - Consider adding an automatic engine-side recovery or client-side forced reconnect after repeated health failures.

### 5) Backend diagnostics not fully surfaced in frontend

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

- **Qwen3-4B NPU inference — WORKING via INT8** (2026-03-20, `fix/qwen3-npu-int8`): INT4 produced garbage on NPU; FP16 too large for StaticLLMPipeline. Requantized FP16→INT8_SYM per-channel (3.75 GB) using `nncf.compress_weights()`. Live-tested: 34 coherent tokens, 8.1 tok/s, proper EOS. Template patch from `fix/qwen3-npu` is a prerequisite.
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
