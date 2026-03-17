# Official Docs Index

Checked on: 2026-03-12
Principle: primary sources only.

## OpenVINO-native sources

| Source | Why it matters |
|---|---|
| https://github.com/openvinotoolkit/openvino?tab=readme-ov-file | OpenVINO repo root and release-level context |
| https://github.com/openvinotoolkit/openvino.genai?tab=readme-ov-file | OpenVINO GenAI repo root and package/runtime context |
| https://docs.openvino.ai/2026/index.html | canonical 2026 docs entrypoint |
| https://docs.openvino.ai/2026/get-started/install-openvino/install-openvino-genai.html | GenAI archive install path and package coupling |
| https://docs.openvino.ai/2026/openvino-workflow-generative/inference-with-genai/inference-with-genai-on-npu.html | native NPU inference, caching, and prompt-budget guidance |
| https://docs.openvino.ai/2026/openvino-workflow-generative/ov-tokenizers.html | tokenizers runtime packaging and install guidance |
| https://docs.openvino.ai/2026/about-openvino/release-notes-openvino/system-requirements.html | OS, driver, and redistributable requirements |
| https://docs.openvino.ai/2026/_static/download/OpenVINO_Quick_Start_Guide.pdf | Windows quick-start reference for the 2026 release |
| https://github.com/openvinotoolkit/openvino.genai/tree/master/samples/c | confirms the native GenAI C sample surface |

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
| https://huggingface.co/OpenVINO/Qwen2.5-Coder-1.5B-Instruct-int8-ov | coding-oriented native OpenVINO backup and current default artifact for repo testing |
| https://huggingface.co/OpenVINO/Qwen3-4B-int4-ov | current official Qwen3 OpenVINO smoke-test artifact |
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
