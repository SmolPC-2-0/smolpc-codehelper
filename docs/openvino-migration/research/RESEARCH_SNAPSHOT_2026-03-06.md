# Research Snapshot (2026-03-06)

Captured on: 2026-03-06  
Scope: OpenVINO-first EP rollout in SmolPC, with ORT EP landscape and runtime policy constraints.

## Key Verified Claims

1. ONNX Runtime upstream latest release is `v1.24.3` (`2026-03-05`).
   - Source: https://github.com/microsoft/onnxruntime/releases

2. Intel OpenVINO EP release `v5.9` (`2026-02-25`) is explicitly based on:
   - OpenVINO `2025.4.1`
   - ONNX Runtime `1.24.1`
   - Source: https://github.com/intel/onnxruntime/releases/tag/v5.9

3. OpenVINO toolkit release notes include `2026.0` (`2026-02-23`), which is newer than the OpenVINO EP tuple above.
   - Source: https://docs.openvino.ai/nightly/about-openvino/release-notes-openvino.html

4. ORT OpenVINO EP docs still show compatibility table entries including:
   - ORT `1.24.1` with OpenVINO `2025.4.1`
   - Source: https://onnxruntime.ai/docs/execution-providers/OpenVINO-ExecutionProvider.html

5. `ort` crate current release is `2.0.0-rc.12` (published `2026-03-05`), with multiversion support across ORT minor versions (`1.17` to `1.24`) via `api-*` features.
   - Sources:
     - https://crates.io/crates/ort/2.0.0-rc.12
     - https://github.com/pykeio/ort/releases/tag/v2.0.0-rc.12
     - https://ort.pyke.io/setup/multiversion

6. `ort` execution provider behavior defaults matter:
   - EPs are registered in the order provided.
   - By default, failures can silently fall back to CPU unless `.error_on_failure()` is used.
   - Sources:
     - https://ort.pyke.io/perf/execution-providers
     - https://docs.rs/ort/2.0.0-rc.12/ort/session/builder/struct.SessionBuilder.html

7. `ort` now supports automatic device routing APIs:
   - `SessionBuilder::with_auto_device(...)`
   - `SessionBuilder::with_devices(...)`
   - `AutoDevicePolicy` includes `PreferNPU` and policy aliases (`MaxEfficiency`/`MinPower` currently map to NPU preference behavior).
   - Sources:
     - https://docs.rs/ort/2.0.0-rc.12/ort/session/builder/struct.SessionBuilder.html
     - https://docs.rs/ort/2.0.0-rc.12/ort/session/builder/enum.AutoDevicePolicy.html

8. `ort` setup/linking guidance favors runtime control:
   - `load-dynamic` is recommended over compile-time dynamic linking.
   - Runtime path can be controlled via `ORT_DYLIB_PATH`.
   - Source: https://ort.pyke.io/setup/linking

9. `ort` download channel constraints:
   - Prebuilt combo behavior is constrained (doc examples call out limited combinations).
   - Current `dist.txt` snapshot references `ms@1.24.2` and does not expose an OpenVINO prebuilt artifact lane.
   - Sources:
     - https://ort.pyke.io/perf/execution-providers
     - https://github.com/pykeio/ort/blob/main/ort-sys/build/download/dist.txt

10. DirectML and ROCm status:
   - DirectML is in sustained engineering (new feature development moved to WinML for Windows deployments).
   - ROCm EP removed since ORT `1.23`; migration path is MIGraphX EP.
   - Sources:
     - https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html
     - https://onnxruntime.ai/docs/execution-providers/ROCm-ExecutionProvider.html

## Implications For SmolPC

- Use engine-host as strict EP authority and avoid silent fallback semantics in selection logic.
- Ship OpenVINO through a pinned runtime tuple and package validation (do not rely on opportunistic auto-downloaded binaries).
- Keep CPU fallback deterministic and observable, but explicit.
- Revalidate tuple compatibility before each phase gate.

## Recommended Safety Baseline

- Production tuple gate for OpenVINO Phase 1:
  - Intel OpenVINO EP `v5.9` baseline (`ORT 1.24.1 + OpenVINO 2025.4.1`)
- Selector behavior:
  - ordered candidates
  - `.error_on_failure()` for non-CPU candidates
  - explicit reason codes and telemetry for fallback/demotion

