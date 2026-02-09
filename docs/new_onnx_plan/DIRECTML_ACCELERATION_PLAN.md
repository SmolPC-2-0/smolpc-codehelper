# DirectML Hardware-Agnostic Acceleration Plan

Last updated: 2026-02-09  
Branch: `plan/directml-hardware-agnostic-acceleration`  
Status: Planning only (no implementation in this branch yet)

## Follow-Ups Asked and Current Defaults

Questions asked before planning:
1. Minimum performance win target on weak laptops?
2. Acceptable TTFT tradeoff vs throughput gains?
3. Minimum supported Windows version?
4. Should GPU be used only when meaningfully faster?
5. Allow hidden debug override for backend forcing?
6. Persist backend choice per device?

Planning defaults until confirmed otherwise:
- Target at least +30% decode throughput (`tokens/sec`) on D3D12-capable devices.
- Allow up to +15% TTFT regression only if decode throughput win is >= +30%.
- Support Windows 10 20H1+ and Windows 11.
- Require a measured win threshold before choosing DirectML by default.
- Allow hidden debug override (`SMOLPC_FORCE_EP=cpu|dml`) with no UI setup.
- Persist selected backend per device fingerprint and model.

## Current State Snapshot

1. Inference is CPU-only in `src-tauri/src/inference/session.rs`.
2. Current bundled ORT DLL (`src-tauri/libs/onnxruntime.dll`) does not export `OrtSessionOptionsAppendExecutionProvider_DML`.
3. `Microsoft.ML.OnnxRuntime.DirectML.1.22.1` native runtime DLL does export `OrtSessionOptionsAppendExecutionProvider_DML`.
4. Generation loop is KV-cache based and currently rebuilds CPU tensors per decode step (`src-tauri/src/inference/generator.rs`, `src-tauri/src/inference/kv_cache.rs`).
5. Hardware detection is already present and can be reused for backend ranking (`src-tauri/src/hardware`, `src-tauri/src/commands/hardware.rs`).

## Goals

1. Enable DirectML acceleration automatically with no user setup.
2. Keep behavior hardware agnostic with robust fallback to CPU.
3. Deliver true performance gains, not just "GPU enabled" status.
4. Preserve stability of existing generation, cancellation, and model-loading flows.

## Non-Goals (for this phase)

1. CUDA/OpenVINO/QNN/CoreML implementation.
2. New model variants beyond `qwen2.5-coder-1.5b`.
3. User-facing backend configuration UI.

## Success Criteria

Functional:
1. On compatible Windows hardware, app auto-selects DirectML when healthy and beneficial.
2. On unsupported or failing hardware, app auto-falls back to CPU without user action.
3. No regression in cancellation, single-flight generation, or model lifecycle correctness.

Performance:
1. On at least one iGPU-class weak laptop: >= 30% decode throughput improvement or fallback to CPU if not met.
2. On unsupported/weak GPU paths: no major regression because selector chooses CPU.
3. Backend decision completes quickly (startup or load overhead bounded; target under 2 seconds additional decision time after warm cache).

## Key Technical Constraints

1. DirectML EP works best with stable/static dimensions and sequential execution.
2. Current loop uses dynamic sequence lengths and CPU-side KV marshaling every step.
3. Naive DirectML registration alone may not produce real gains on this loop.
4. ORT runtime package must include a DML-capable `onnxruntime.dll` and required shared provider DLLs.

## Architecture Plan

### 1) Execution Backend Abstraction

Add explicit backend model:
- `InferenceBackend`: `Cpu`, `DirectMl { device_id }`.
- `BackendDecision`: selected backend, reason, benchmark snapshot, fallback info.
- `BackendSelector`: ranks candidates from hardware detection and prior cache.

Primary file targets:
- `src-tauri/src/inference/session.rs`
- new `src-tauri/src/inference/backend.rs`
- `src-tauri/src/commands/inference.rs` (status plumbing only)

### 2) Runtime Packaging for DirectML

Use a DML-capable ORT distribution:
- Source: `Microsoft.ML.OnnxRuntime.DirectML` native runtime assets.
- Bundle required files in `src-tauri/libs/` for x64:
  - `onnxruntime.dll` (DML-capable)
  - `onnxruntime_providers_shared.dll`

Installer/build updates:
- Extend setup/download script with checksum verification for DML runtime package.
- Keep Windows fat installer path unchanged for user experience.

Primary file targets:
- `scripts/setup-libs.sh` or Windows equivalent acquisition script
- `scripts/build-windows-offline-installer.ps1`
- `src-tauri/tauri.conf.json`
- `src-tauri/src/inference/mod.rs` (library resolution checks/logging)

### 3) Session Creation and EP Registration

Session creation strategy:
1. Try DirectML session with device candidate.
2. If init fails, log reason and create CPU session.
3. If `SMOLPC_FORCE_EP` is set, honor it (debug only).

DirectML session options to apply:
1. `with_execution_providers([DirectMLExecutionProvider::default().with_device_id(id).build()])`
2. Use sequential execution mode (`with_parallel_execution(false)`) for DML sessions.
3. Keep optimization level high (`Level3`) and tune thread settings for CPU fallback sessions separately.

### 4) Performance-Critical Loop Strategy (True Gains)

Phase A (minimum viable DirectML):
1. Enable EP selection and runtime fallback with current generator.
2. Add quick benchmark gating so slow DML does not get selected.

Phase B (required for stronger gains):
1. Reduce host-device transfer overhead around KV path.
2. Introduce pre-allocated tensors and reuse patterns where feasible.
3. Prototype `IoBinding` path for stable inputs/outputs to reduce repeated copies.

Phase C (shape stability tuning):
1. Add fixed-shape decode bucket(s) for DML-friendly execution.
2. Evaluate dimension override strategy only if model graph names permit safe use.
3. Keep CPU path unchanged as correctness baseline.

Important note:
- `ort` docs for `IoBinding` say gains are limited when every input changes each run.
- For this LLM loop, only some data is stable; expected wins require measurement, not assumptions.
- Selection logic must be benchmark-driven.

### 5) Hardware-Agnostic Auto Selection and Fallback

Selection policy:
1. Build candidate list from detected hardware:
   - DirectML candidate if DX12-capable GPU is present.
   - CPU always available fallback.
2. Attempt candidate initialization in order.
3. Run short warm benchmark on first use:
   - Prompt prefill + fixed decode sample.
4. Choose backend by policy thresholds:
   - Prefer DirectML only if throughput and TTFT thresholds are met.
5. Persist decision with fingerprint key:
   - model id + adapter info + driver version + app version + ORT version.
6. If runtime failures occur repeatedly, demote DirectML and fail over to CPU automatically.

Persistence target:
- New small JSON cache in app data, similar to hardware cache behavior.

### 6) Observability and UX (No Setup Required)

No user setup screens. Required transparency:
1. Log backend decision and reason.
2. Expose active backend string via command/status API for diagnostics.
3. Optional lightweight indicator text in existing status component later (non-blocking).

## Work Breakdown by Milestone

### Milestone 0: Baseline and Instrumentation
1. Add backend metrics struct for TTFT, tok/s, decode ms/token.
2. Add repeatable mini benchmark command for selector use.
3. Capture CPU baseline on first run.

Exit criteria:
1. Stable baseline metrics captured for model/device.

### Milestone 1: DirectML Enablement and Fallback Safety
1. Package DML-capable ORT runtime.
2. Add EP registration path and fallback to CPU.
3. Add env override and backend decision logging.

Exit criteria:
1. App runs on DML-capable hardware and CPU-only hardware with no user intervention.

### Milestone 2: Auto-Selection and Persistence
1. Implement selector thresholds and candidate ranking.
2. Add decision persistence and invalidation rules.
3. Add failure demotion policy.

Exit criteria:
1. Backend chosen automatically and consistently across launches.

### Milestone 3: Performance Optimization Pass
1. Profile copy hotspots in decode path.
2. Prototype tensor reuse and targeted `IoBinding` improvements.
3. Add shape-bucket experiments for DML path.
4. Keep hard rollback to CPU for any non-beneficial scenario.

Exit criteria:
1. Demonstrated net gains on weak-laptop target class, or policy correctly keeps CPU.

## Testing Matrix

Required test environments:
1. Weak Windows laptop with integrated GPU (primary target).
2. CPU-only or incompatible GPU Windows laptop.
3. Stronger discrete GPU Windows machine (sanity only).

Test categories:
1. Session init and fallback tests (unit/integration with mocked selector inputs).
2. Cancellation and single-flight regression tests.
3. Benchmark comparisons across backends.
4. Fresh installer test with DirectML runtime bundled.

## Risks and Mitigations

1. Risk: DML slower than CPU on some weak GPUs.
   - Mitigation: benchmark-gated selection and persistent CPU preference.
2. Risk: ORT runtime packaging mismatch.
   - Mitigation: strict checksum verification and startup runtime sanity checks.
3. Risk: Copy overhead erases GPU gains.
   - Mitigation: optimization milestone focused on transfer reduction and tensor reuse.
4. Risk: Driver/device instability.
   - Mitigation: failure counters, auto demotion, robust CPU fallback.

## File Map for Future Implementation

Primary backend/EP changes:
- `src-tauri/src/inference/session.rs`
- `src-tauri/src/inference/mod.rs`
- `src-tauri/src/inference/generator.rs`
- `src-tauri/src/inference/kv_cache.rs`
- new `src-tauri/src/inference/backend.rs`

Command/status wiring:
- `src-tauri/src/commands/inference.rs`
- `src-tauri/src/lib.rs`
- optional frontend status wiring in `src/lib/stores/inference.svelte.ts`

Packaging and build:
- `src-tauri/tauri.conf.json`
- `scripts/setup-libs.sh`
- `scripts/build-windows-offline-installer.ps1`

## Open Questions to Resolve Before Implementation Starts

1. Confirm exact performance threshold policy (+30% target and +15% TTFT tolerance).
2. Confirm whether backend status should be visible in UI now or only in logs.
3. Confirm whether first-run backend benchmark can add small startup delay.
4. Confirm whether persistence should be per model only or per model+device fingerprint.

## References

1. ONNX Runtime DirectML EP docs: https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html
2. ONNX Runtime IOBinding docs: https://onnxruntime.ai/docs/performance/tune-performance/iobinding.html
3. `ort` crate DirectML EP implementation (`2.0.0-rc.10`): `.../ort-2.0.0-rc.10/src/execution_providers/directml.rs`
4. `ort` crate IOBinding notes (`2.0.0-rc.10`): `.../ort-2.0.0-rc.10/src/io_binding.rs`
5. Existing session code: `src-tauri/src/inference/session.rs`
6. Existing generator and KV cache code: `src-tauri/src/inference/generator.rs`, `src-tauri/src/inference/kv_cache.rs`
