# Model Strategy

Checked on: 2026-03-12
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

`OpenVINO/Qwen3-4B-int4-ov`

Why:

- an official OpenVINO-hosted Qwen3 4B artifact exists and can back the shared `qwen3-4b` model id across OpenVINO CPU, OpenVINO NPU, and DirectML
- the current supported OpenVINO pass runs it in non-thinking mode only so stopping and answer quality stay aligned with upstream guidance

Source:

- https://huggingface.co/OpenVINO/Qwen3-4B-int4-ov

## Export Rules For Native OpenVINO NPU

Use Optimum Intel / OpenVINO NPU guidance:

- `--sym`
- `--weight-format int4` or `--weight-format nf4`
- `--ratio 1.0`
- `--group-size -1` or `128` depending on model size

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

- `qwen3-4b` is a supported shared model id backed by the official `OpenVINO/Qwen3-4B-int4-ov` artifact on the OpenVINO lanes
