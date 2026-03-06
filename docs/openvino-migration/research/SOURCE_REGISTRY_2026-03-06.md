# Source Registry (2026-03-06)

Primary-source registry for the OpenVINO migration planning baseline.

| Topic | Source | Why it matters |
|---|---|---|
| ORT upstream latest releases | https://github.com/microsoft/onnxruntime/releases | Canonical ORT release/date truth |
| ORT roadmap page | https://onnxruntime.ai/roadmap | Detect stale roadmap vs actual releases |
| Intel OpenVINO EP release | https://github.com/intel/onnxruntime/releases/tag/v5.9 | Canonical OpenVINO EP tuple details |
| ORT OpenVINO EP docs | https://onnxruntime.ai/docs/execution-providers/OpenVINO-ExecutionProvider.html | Official compatibility/config guidance |
| OpenVINO release notes | https://docs.openvino.ai/nightly/about-openvino/release-notes-openvino.html | Detect toolkit version drift vs EP support |
| `ort` EP behavior docs | https://ort.pyke.io/perf/execution-providers | Rust binding EP semantics and fallback notes |
| `ort` linking docs | https://ort.pyke.io/setup/linking | Runtime linking strategy and env control |
| `ort` cargo feature docs | https://ort.pyke.io/setup/cargo-features | Build/runtime feature dependencies |
| `ort` multiversion docs | https://ort.pyke.io/setup/multiversion | API-version gating behavior |
| `ort` crate metadata | https://crates.io/crates/ort/2.0.0-rc.12 | Release timestamp/version pin |
| `ort` release notes | https://github.com/pykeio/ort/releases/tag/v2.0.0-rc.12 | Breaking/behavior changes |
| `ort` docs.rs SessionBuilder | https://docs.rs/ort/2.0.0-rc.12/ort/session/builder/struct.SessionBuilder.html | API-level EP configuration methods |
| `ort` docs.rs AutoDevicePolicy | https://docs.rs/ort/2.0.0-rc.12/ort/session/builder/enum.AutoDevicePolicy.html | Auto policy semantics |
| `ort` source (`impl_options`) | https://docs.rs/ort/2.0.0-rc.12/src/ort/session/builder/impl_options.rs.html | Exact method behavior and doc comments |
| `ort` dist matrix | https://github.com/pykeio/ort/blob/main/ort-sys/build/download/dist.txt | Prebuilt artifact availability reality |
| ORT DirectML EP docs | https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html | Windows fallback EP status/requirements |
| ORT ROCm EP docs | https://onnxruntime.ai/docs/execution-providers/ROCm-ExecutionProvider.html | ROCm deprecation/removal guidance |
| ORT MIGraphX EP docs | https://onnxruntime.ai/docs/execution-providers/MIGraphX-ExecutionProvider.html | AMD replacement path |
| ORT CUDA EP docs | https://onnxruntime.ai/docs/execution-providers/CUDA-ExecutionProvider.html | NVIDIA CUDA requirements |
| ORT TensorRT EP docs | https://onnxruntime.ai/docs/execution-providers/TensorRT-ExecutionProvider.html | NVIDIA TensorRT requirements |
| ORT CoreML EP docs | https://onnxruntime.ai/docs/execution-providers/CoreML-ExecutionProvider.html | Apple EP support path |
| ORT plugin EP usage | https://onnxruntime.ai/docs/execution-providers/plugin-ep-libraries/usage.html | Long-term pluggable EP architecture |
| ORT plugin EP packaging | https://onnxruntime.ai/docs/execution-providers/plugin-ep-libraries/packaging.html | Packaging boundary constraints |
| ORT high-level design | https://onnxruntime.ai/docs/reference/high-level-design.html | EP partitioning/fallback model |
| Windows DLL security | https://learn.microsoft.com/en-us/windows/win32/dlls/dynamic-link-library-security | Secure runtime loading policy |

## Note

All volatile entries (release versions, compatibility tables, and package matrices) must be rechecked before implementation or merge gates.

