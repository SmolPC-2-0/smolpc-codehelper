# Model Strategy

Checked on: 2026-03-09
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
- size is more realistic for weak laptops than Phi-4-mini
- the OpenVINO-hosted artifact aligns with the chosen native runtime lane

Sources:

- https://github.com/openvinotoolkit/openvino/releases/tag/2026.0.0
- https://huggingface.co/OpenVINO/Qwen2.5-1.5B-Instruct-int4-ov

## Coding-Oriented Backup Model

`OpenVINO/Qwen2.5-Coder-1.5B-Instruct-int4-ov`

Why:

- stronger coding-assistant fit
- still small enough to be realistic on weak laptops
- keeps the same native OpenVINO artifact shape as the primary bring-up lane

Sources:

- https://huggingface.co/OpenVINO/Qwen2.5-Coder-1.5B-Instruct-int4-ov
- https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct

## Fastest Small Sanity-Check Option

`Qwen-2.5-coder-0.5B` exported for OpenVINO NPU

Why:

- OpenVINO `2026.0.0` explicitly calls it out as newly supported on NPU
- it is the quickest way to determine whether native OpenVINO NPU is behaving well on low-end hardware

Source:

- https://github.com/openvinotoolkit/openvino/releases/tag/2026.0.0

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

- the shared-engine default catalog must move off `qwen3-4b-instruct-2507`
- do not ship the native OpenVINO path while the default shared-engine model still assumes a DirectML-required workflow
- the default development target for this migration is the `1.5B` Qwen family
- final user-facing default selection belongs to the model/catalog workstream, but it must be resolved before rollout, not deferred indefinitely

## Benchmark Order

1. `openvino_npu` with `Qwen2.5-1.5B-Instruct-int4-ov`
2. `genai_dml` with the equivalent DML artifact
3. `ort_cpu` baseline
4. optional small-model sanity check with `Qwen-2.5-coder-0.5B`

Do not lead Phase 1 with Phi-4-mini for this KPI.
