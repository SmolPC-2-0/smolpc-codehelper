# Model Strategy

Checked on: 2026-03-20
Primary KPI: best practical performance on weak Intel laptops.

## Selection Rules

- prefer small official or maintainer-backed models
- prefer native OpenVINO NPU-ready artifacts when they exist
- keep a separate DirectML artifact lane for fallback benchmarking
- choose bring-up models that reduce export ambiguity

## Primary Bring-Up Model

`OpenVINO/Qwen2.5-1.5B-Instruct-int4-ov`

Why:

- Intel explicitly calls out `Qwen2.5-1.5B-Instruct` as supported on NPU in OpenVINO `2026.0.0`
- size is more realistic for weak laptops than larger 4B+ models
- the OpenVINO-hosted artifact aligns with the chosen native runtime lane

Sources:

- https://github.com/openvinotoolkit/openvino/releases/tag/2026.0.0
- https://huggingface.co/OpenVINO/Qwen2.5-1.5B-Instruct-int4-ov

## Higher-Capability Supported Model

`OpenVINO/Qwen3-4B-int4-ov` (CPU) / locally-quantized INT8_SYM (NPU)

Why:

- an official OpenVINO-hosted Qwen3 4B artifact exists and can back the shared `qwen3-4b` model id across OpenVINO CPU, OpenVINO NPU, and DirectML
- the current supported OpenVINO pass runs it in non-thinking mode only so stopping and answer quality stay aligned with upstream guidance
- **NPU variant**: the upstream INT4 artifact produces garbage on NPU. The FP16 model is requantized locally to INT8_SYM per-channel (3.75 GB, 50% of FP16) using `nncf.compress_weights()`. INT8 delivers coherent output on NPU at 8.1 tok/s.
- **CPU variant**: the upstream INT4 artifact works correctly on OpenVINO CPU

Sources:

- https://huggingface.co/OpenVINO/Qwen3-4B-int4-ov
- Quantization script: `scripts/quantize_int8.py`

## DirectML Source Of Truth

- keep the public large-model id unified as plain `qwen3-4b` across OpenVINO CPU, OpenVINO NPU, and DirectML
- the supported DirectML source mode is `self_build` from `Qwen/Qwen3-4B`, not `qwen3-4b-instruct` or `qwen3-4b-instruct-2507`
- the current validated builder tuple on this branch is:
  - Python `3.14`
  - `onnxruntime==1.24.2`
  - `onnxruntime-directml==1.24.2`
  - `onnxruntime-genai==0.12.2`
  - `onnxruntime-genai-directml==0.12.2`
- the DirectML staging script must validate `model.onnx`, `genai_config.json`, `tokenizer.json`, and all ONNX external-data references; the validated `qwen3-4b` self-build on this PC references `model.onnx.data`
- DirectML export logs live under `%LOCALAPPDATA%/SmolPC/logs/dml-export/`
- `fallback_snapshot` is available only as an explicit recovery mode using `onnx-community/Qwen3-4B-ONNX` plus `config_only=true`; it is not the default shipping path
- Windows app-local runtime staging stays aligned to the official NuGet dependency pair:
  - `Microsoft.ML.OnnxRuntimeGenAI.DirectML` `0.12.2`
  - `Microsoft.ML.OnnxRuntime.DirectML` `1.23.0`

## Export Rules For Native OpenVINO NPU

### Qwen2.5-1.5B (INT4 works on NPU)

Use Optimum Intel / OpenVINO NPU guidance:

- `--sym`
- `--weight-format int4`
- `--ratio 1.0`
- `--group-size -1` or `128` depending on model size

### Qwen3-4B (INT4 broken on NPU — use INT8)

INT4 produces garbage on NPU for Qwen3 architecture. Use `nncf.compress_weights()` to requantize the FP16 IR to INT8_SYM:

```python
import openvino as ov, nncf
model = ov.Core().read_model("openvino_model.xml")
compressed = nncf.compress_weights(model, mode=nncf.CompressWeightsMode.INT8_SYM)
ov.save_model(compressed, "openvino_model.xml")
```

- INT8_SYM per-channel: 3.75 GB (50% of FP16), coherent NPU output at 8.1 tok/s
- INT4: 2.2 GB but produces 0-1 content tokens on NPU
- FP16: 7.49 GB, too large for StaticLLMPipeline

Notes:

- `NF4` is only for Intel Core Ultra Series 2 NPU and later
- group quantization with `128` is recommended for smaller models up to roughly 4B-5B parameters

Source:

- https://docs.openvino.ai/2025/openvino-workflow-generative/inference-with-genai/inference-with-genai-on-npu.html

## Artifact Layout

Final expected layout:

```text
models/<model_id>/
  cpu/
    manifest.json
    ...
  dml/
    manifest.json
    ...
  openvino_npu/
    manifest.json
    ...
```

Manifest requirements:

- every lane owns its own file inventory and asset paths
- tokenizer, tokenizer config, generation config, and model files are referenced by the lane manifest
- there is no shared root-level tokenizer contract in the final layout
- the exact OpenVINO-side filenames may vary by export tool, but the lane must remain structurally isolated from `cpu/` and `dml/`

## Default Catalog Direction

- the supported shared catalog is now `qwen2.5-1.5b-instruct` first, `qwen3-4b` second
- do not reintroduce `qwen2.5-coder-1.5b`, `qwen3-4b-instruct`, or `qwen3-4b-instruct-2507` into the normal product catalog
- keep OpenVINO CPU on structured chat history; do not send normal OpenVINO chat requests through the legacy prompt-string path
- keep `qwen3-4b` in OpenVINO non-thinking mode until a later pass re-validates thinking support

## Benchmark Order

1. `openvino_npu` with `Qwen2.5-1.5B-Instruct-int4-ov`
2. `genai_dml` with the equivalent DML artifact
3. `ort_cpu` baseline
4. optional small-model sanity check with `Qwen-2.5-coder-0.5B`

Lead Phase 1 with the 1.5B Qwen model for this KPI; Qwen3-4B is the higher-capability tier.

Current repo note:

- `qwen3-4b` is a supported shared model id: INT4 artifact on CPU, locally-quantized INT8_SYM on NPU
- `qwen3-4b` is also validated on the DirectML lane through the `Qwen/Qwen3-4B` self-build path with forced `directml` load and streaming generation
