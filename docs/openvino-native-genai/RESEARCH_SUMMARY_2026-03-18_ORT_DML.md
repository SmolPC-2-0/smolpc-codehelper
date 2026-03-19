# ORT / DirectML Research Summary

Checked on: 2026-03-18
Validated on this PC: 2026-03-18 through 2026-03-19 overnight
Scope: unified `qwen3-4b` DirectML path on `fix/openvino-supported-qwen-baseline`

## Official Sources Rechecked

- ORT GenAI build-model docs:
  - https://onnxruntime.ai/docs/genai/howto/build-model.html
- ORT model-builder guide:
  - https://github.com/microsoft/onnxruntime-genai/blob/main/src/python/py/models/README.md
- PyPI package truth:
  - https://pypi.org/project/onnxruntime-genai/
  - https://pypi.org/project/onnxruntime-genai-directml/
- Windows runtime package truth:
  - https://www.nuget.org/packages/Microsoft.ML.OnnxRuntimeGenAI.DirectML
- Maintained fallback ONNX snapshot:
  - https://huggingface.co/onnx-community/Qwen3-4B-ONNX

## Findings

- The current official Python package pair is `onnxruntime-genai==0.12.2` and `onnxruntime-genai-directml==0.12.2`.
- The `0.12.2` Python line installs successfully in an isolated Python `3.14` venv on this machine.
- The validated builder tuple on this branch is:
  - `onnxruntime==1.24.2`
  - `onnxruntime-directml==1.24.2`
  - `onnxruntime-genai==0.12.2`
  - `onnxruntime-genai-directml==0.12.2`
- The official Windows NuGet runtime line is not the same as the Python builder line.
  - `Microsoft.ML.OnnxRuntimeGenAI.DirectML` `0.12.2` still depends on `Microsoft.ML.OnnxRuntime.DirectML` `1.23.0`.
  - App-local runtime staging should therefore align to `0.12.2` GenAI plus `1.23.0` ORT DirectML on Windows.
- `Qwen/Qwen3-4B` self-build to DirectML `int4` succeeds on this PC.
  - Validated artifact files: `model.onnx`, `model.onnx.data`, `genai_config.json`, `tokenizer.json`
  - The ONNX external-data reference resolves cleanly to `model.onnx.data`.
- Forced DirectML runtime validation succeeds on this branch after the self-build artifact is staged.
  - `POST /engine/load` for `qwen3-4b` activates `active_backend=directml`
  - `backend_status.runtime_engine=genai_dml`
  - short streamed generation succeeded
  - longer streamed generation succeeded and terminated normally
- The shared DirectML path still works for `qwen2.5-1.5b-instruct`.
  - forced `directml` load succeeded
  - short streamed generation succeeded
- The maintained fallback snapshot is structurally usable as a recovery source.
  - `onnx-community/Qwen3-4B-ONNX` exposes `onnx/model_q4f16.onnx` with matching external-data files plus tokenizer/config assets
  - this path should stay manual and explicit through `config_only=true`; it was not needed for the supported path on this PC

## Decisions

- Keep strict model unification.
  - supported public ids stay `qwen2.5-1.5b-instruct` and `qwen3-4b`
  - do not reintroduce `qwen3-4b-instruct` or `qwen3-4b-instruct-2507`
- Keep `self_build` as the default and supported `qwen3-4b` DirectML source mode.
- Keep `fallback_snapshot` only as an explicit recovery mode in the setup script.
- Keep DirectML export logs under `%LOCALAPPDATA%/SmolPC/logs/dml-export/`.
- Keep Windows runtime staging aligned to the official NuGet dependency pair even though the Python builder env validates on a newer ORT tuple.

## Validation Notes

- `qwen3-4b` short DirectML generation:
  - output: `Hello!`
  - metrics: `time_to_first_token_ms=389`, `tokens_per_second=9.49`, `total_time_ms=632`, `total_tokens=6`
- `qwen3-4b` longer DirectML generation:
  - output started with: `Rain forms when water vapor in the air rises and cools.`
  - metrics: `time_to_first_token_ms=848`, `tokens_per_second=29.81`, `total_time_ms=1979`, `total_tokens=59`
- `qwen2.5-1.5b-instruct` short DirectML generation:
  - output: `Hello! How can I help you today?`
  - metrics: `time_to_first_token_ms=523`, `tokens_per_second=12.59`, `total_time_ms=715`, `total_tokens=9`
