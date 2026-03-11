# Official Docs Index

Checked on: 2026-03-09
Principle: primary sources only.

## OpenVINO-native sources

| Source | Why it matters |
|---|---|
| https://github.com/openvinotoolkit/openvino/releases/tag/2026.0.0 | release-level truth for current OpenVINO capabilities |
| https://docs.openvino.ai/nightly/about-openvino/release-notes-openvino.html | release notes and support changes |
| https://openvinotoolkit.github.io/openvino.genai/docs/getting-started/installation/ | GenAI/OpenVINO/Tokenizers version coupling |
| https://openvinotoolkit.github.io/openvino.genai/docs/samples/ | confirms native sample surface, including `C` `text_generation` |
| https://docs.openvino.ai/2025/openvino-workflow/running-inference/inference-devices-and-modes/npu-device.html | NPU driver requirement and supported Windows platform |
| https://docs.openvino.ai/2024/api/c_cpp_api/group__ov__runtime__npu__prop__cpp__api.html | NPU driver-version and device-memory property surface |
| https://docs.openvino.ai/2025/openvino-workflow-generative/inference-with-genai/inference-with-genai-on-npu.html | native NPU export/perf/troubleshooting guidance |
| https://docs.openvino.ai/nightly/about-openvino/release-notes-openvino/system-requirements.html | OS, driver, and redistributable requirements |
| https://docs.openvino.ai/2025/openvino-workflow/deployment-locally/local-distribution-libraries.html | exact local-distribution library rules |

## ORT / DirectML fallback sources

| Source | Why it matters |
|---|---|
| https://github.com/microsoft/onnxruntime/releases/tag/v1.24.3 | current ORT release for fallback lanes |
| https://github.com/pykeio/ort/releases/tag/v2.0.0-rc.12 | current Rust wrapper release used by existing ORT-backed lanes |
| https://onnxruntime.ai/docs/genai/howto/build-from-source.html | confirms current first-class ORT GenAI build lanes |
| https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html | DirectML status and behavior notes |

## Model sources

| Source | Why it matters |
|---|---|
| https://huggingface.co/OpenVINO/Qwen2.5-1.5B-Instruct-int4-ov | primary native OpenVINO bring-up model |
| https://huggingface.co/OpenVINO/Qwen2.5-Coder-1.5B-Instruct-int4-ov | coding-oriented native OpenVINO backup |
| https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct | upstream base model reference |

## Repo-local references

| File | Why it matters |
|---|---|
| `Cargo.toml` | current ORT pin and features |
| `docs/ENGINE_API.md` | current status contract that needs to be generalized |
| `crates/smolpc-engine-host/src/main.rs` | current selector, fallback, persistence, and probe behavior |
| `crates/smolpc-engine-core/src/inference/runtime_adapter.rs` | current runtime lane abstraction |
| `crates/smolpc-engine-core/src/inference/backend.rs` | current backend enums, reason codes, and persistence key |
| `crates/smolpc-engine-core/src/models/loader.rs` | current backend artifact layout logic |
| `crates/smolpc-engine-core/src/inference/genai/directml.rs` | current DirectML GenAI runtime lane |

## Deliberately Excluded

- ORT OpenVINO EP docs
- Intel OpenVINO EP release stream

Those sources were useful while comparing strategies, but they are not part of the chosen implementation path anymore.
