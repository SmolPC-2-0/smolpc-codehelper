# OpenVINO EP Acceleration Strategy (Amended)

Date: 2026-03-06  
Owner: EP planning/research stream  
Scope: OpenVINO-first rollout with future multi-EP expansion

## Executive Recommendation

Amend the plan to treat `ort.pyke.io` as the Rust integration authority and use it to harden runtime behavior:

- keep EP choice in engine-host
- disable implicit behavior
- ship OpenVINO via a pinned custom runtime tuple (not `ort` auto-downloaded binaries)

Best safe starting tuple: Intel OpenVINO EP `v5.9` (`2026-02-25`) aligned to ORT `1.24.1` + OpenVINO `2025.4.1`.

---

## What Changed After Reviewing `ort.pyke.io`

1. `ort` currently **silently falls back to CPU** when EP registration fails unless `.error_on_failure()` is used.
2. `ort` now has **automatic device selection** (`with_auto_device`, `with_devices`, `api-22+`), including NPU preference paths.
3. `ort` docs recommend **`load-dynamic`** runtime linking over compile-time dynamic linking for operational control.
4. `ort` prebuilt download matrix is constrained; current `dist.txt` points to **ms@1.24.2**, and does **not** provide a prebuilt OpenVINO package path.
5. Therefore, OpenVINO-first in production should use **custom ORT/OpenVINO packaging**, not `download-binaries`.

---

## 1) EP Landscape Matrix (Amended)

| EP | Product Fit | `ort` Practical Packaging | Risk |
|---|---|---|---|
| CPU baseline | Required fallback | Built-in/default | Low |
| OpenVINO (priority) | Best Intel path incl. NPU | **Custom build/package required** | Medium |
| DirectML | Good Windows GPU fallback | Available broadly; sustained engineering mode upstream | Medium |
| CUDA/TensorRT | Strong NVIDIA perf | Prebuilt combos exist in `ort` ecosystem; still driver/CUDA complexity | Medium-High |
| ROCm (legacy) | Not recommended new rollout | ROCm EP removed upstream since 1.23; use MIGraphX | High |
| MIGraphX | AMD path | Separate EP track | Medium-High |
| CoreML | Apple track | Supported for macOS/iOS path | Medium |
| WebGPU/NVRTX (alternative) | Niche future experiments | Available in `ort` prebuilt channels; WebGPU marked experimental | Medium-High |

---

## 2) Target Architecture Ownership

### Decision ownership

- Setup/launcher: capture user inference intent once (for example `auto_accelerated`) and cache it.
- Engine-client/apps: read status/contracts only; no EP decision logic.
- Engine-host: final EP selector and validator.

### Engine-host session creation rules (hardened)

- register EPs in explicit order
- use `.error_on_failure()` for non-CPU EP candidates
- avoid environment-driven EP drift (`with_no_environment_execution_providers` and sanitized env strategy)
- never rely on silent ORT fallback for policy decisions

---

## 3) OpenVINO Implementation (Amended)

### Integration path

- Keep `ort` crate (current observed: `2.0.0-rc.12`, published `2026-03-05`).
- Disable production dependency on `download-binaries`.
- Use `load-dynamic` + explicit host-managed runtime package.
- Package pinned tuple: ORT `1.24.1` + OpenVINO `2025.4.1` (Intel EP `v5.9` basis).

### Compatibility gates

- tuple allowlist check
- Intel device + driver/runtime check
- EP preflight startup and first-token validation

### Fallback chain

- `openvino_npu -> openvino_gpu -> directml (Windows) -> cpu`

### Telemetry/events

- `ep_registration_failed`
- `ep_error_on_failure_triggered`
- `ep_selected`
- `ep_demoted`
- `ep_tuple`

---

## 4) Phased Rollout (Amended)

### Phase 0

- Runtime hardening first: remove implicit fallback behavior from selector path.
- Switch production packaging to host-managed runtime artifacts.

### Phase 1

- OpenVINO canary on Intel SKUs with pinned tuple and strict telemetry.

### Phase 2

- Expand pluggable packages (CUDA/TensorRT, MIGraphX, CoreML), same selector contract.

---

## 5) Validation Plan (Amended)

Add explicit tests for:

- EP registration failure must surface (no silent CPU in selector path)
- `with_execution_providers` ordering behavior
- environment EP override suppression
- dynamic library path policy (expected path only)

Benchmarks:

- include effective acceleration checks (node placement/offload evidence), not only throughput

---

## 6) High-Impact Open Questions

1. Approve replacing `ort` `download-binaries` for production with fully host-managed runtime packaging?
2. Should host forbid runtime env overrides in production (`ORT_DYLIB_PATH`, `ORT_CUDA_VERSION`, etc.) except signed internal config?
3. Which Intel launch SKUs and driver baselines are in scope for OpenVINO Phase 1?
4. Is OpenVINO go/no-go metric throughput, power efficiency, or both?

---

## Stale/Conflicting Guidance Called Out

- ONNX Runtime latest is `v1.24.3` (`2026-03-05`), but `ort` prebuilt dist currently tracks `1.24.2`.
- ONNXRuntime OpenVINO EP page references "latest v5.8", but Intel release is `v5.9` (`2026-02-25`).
- OpenVINO toolkit has `2026.0` (`2026-02-23`), but Intel OpenVINO EP compatibility is explicitly tied to `2025.4.1 + ORT 1.24.1`.

Safest choice: ship OpenVINO on validated Intel tuple first, then uplift versions behind canary gates.

---

## Prioritized Action Checklist

1. Freeze OpenVINO Phase 1 tuple policy (Intel `v5.9` basis).
2. Approve production switch to host-managed runtime packages (`load-dynamic` path).
3. Add host selector hardening: ordered EP registration + `.error_on_failure()` + explicit fallback reasons.
4. Implement package validation (hash/signature + tuple compatibility).
5. Run Intel canary matrix and only then broaden rollout.

---

## Primary Sources

- `ort` EP docs: https://ort.pyke.io/perf/execution-providers
- `ort` cargo features: https://ort.pyke.io/setup/cargo-features
- `ort` linking: https://ort.pyke.io/setup/linking
- `ort` multiversioning: https://ort.pyke.io/setup/multiversion
- `ort` release `v2.0.0-rc.12` (`2026-03-05`): https://github.com/pykeio/ort/releases/tag/v2.0.0-rc.12
- `ort` dist matrix (`ms@1.24.2`): https://github.com/pykeio/ort/blob/main/ort-sys/build/download/dist.txt
- `ort` crate versions: https://crates.io/crates/ort/2.0.0-rc.12
- ONNX Runtime releases: https://github.com/microsoft/onnxruntime/releases
- Intel OpenVINO EP `v5.9`: https://github.com/intel/onnxruntime/releases/tag/v5.9
- ONNX Runtime OpenVINO EP doc: https://onnxruntime.ai/docs/execution-providers/OpenVINO-ExecutionProvider.html
- ONNX Runtime DirectML EP doc: https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html
- ONNX Runtime ROCm EP doc: https://onnxruntime.ai/docs/execution-providers/ROCm-ExecutionProvider.html
- OpenVINO release notes (`2026.0`): https://docs.openvino.ai/nightly/about-openvino/release-notes-openvino.html

