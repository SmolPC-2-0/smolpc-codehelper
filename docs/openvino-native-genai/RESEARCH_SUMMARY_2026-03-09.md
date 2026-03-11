# Research Summary

Date: 2026-03-09
Method: volatile claims rechecked against official primary sources.

## 1. OpenVINO-native facts that drive the plan

1. OpenVINO `2026.0.0` is the current Intel release baseline this plan is anchored to.
   - release date: `2026-02-23`
   - source:
     - https://github.com/openvinotoolkit/openvino/releases/tag/2026.0.0

2. OpenVINO `2026.0.0` explicitly expands NPU LLM support and GenAI capabilities.
   - release notes call out NPU support for `Qwen2.5-1.5B-Instruct`, `Qwen3-Embedding-0.6B`, and `Qwen-2.5-coder-0.5B`
   - release notes also call out OpenVINO GenAI speculative decoding on NPU and ahead-of-time/on-device compilation improvements
   - sources:
     - https://github.com/openvinotoolkit/openvino/releases/tag/2026.0.0
     - https://docs.openvino.ai/nightly/about-openvino/release-notes-openvino.html

3. OpenVINO GenAI is version-coupled to OpenVINO and OpenVINO Tokenizers.
   - official install docs say changing `MAJOR`, `MINOR`, or `PATCH` can break ABI
   - implication: SmolPC must ship a pinned tuple, not mix versions
   - source:
     - https://openvinotoolkit.github.io/openvino.genai/docs/getting-started/installation/

4. OpenVINO NPU support on Windows depends on machine state the app does not control.
   - official NPU device docs require Windows 11 64-bit and an installed NPU driver
   - official system requirements say NPU drivers are not included in the OpenVINO toolkit package
   - sources:
     - https://docs.openvino.ai/2025/openvino-workflow/running-inference/inference-devices-and-modes/npu-device.html
     - https://docs.openvino.ai/nightly/about-openvino/release-notes-openvino/system-requirements.html

5. OpenVINO GenAI on NPU has model/export/perf constraints that are directly relevant to weak laptops.
   - official guide says Optimum Intel is the primary export path
   - recommended export settings include `--sym`, `--weight-format int4|nf4`, and `--ratio 1.0`
   - `NF4` is only supported on Intel Core Ultra Series 2 NPUs and later
   - `MAX_PROMPT_LEN`, `MIN_RESPONSE_LEN`, `PREFILL_HINT`, and `GENERATE_HINT` materially affect prompt handling, compile/runtime behavior, and warm-state performance
   - sources:
     - https://docs.openvino.ai/2025/openvino-workflow-generative/inference-with-genai/inference-with-genai-on-npu.html

6. OpenVINO provides a real local redistribution story for native deployment.
   - official local-distribution docs say `openvino` is always required
   - `openvino_c` is required for C-language applications
   - device plugins such as `openvino_intel_npu_plugin` and `openvino_intel_cpu_plugin` must be shipped if used
   - OpenVINO depends on TBB libraries
   - converting models to OpenVINO IR reduces the frontend libraries that must be shipped
   - source:
     - https://docs.openvino.ai/2025/openvino-workflow/deployment-locally/local-distribution-libraries.html

7. OpenVINO GenAI has a practical native integration surface beyond Python-only usage.
   - the current sample index exposes `C`, `C++`, `JavaScript`, and `Python` samples
   - the `C` sample set includes `text_generation`
   - source:
     - https://openvinotoolkit.github.io/openvino.genai/docs/samples/

## 2. Driver and troubleshooting facts

1. OpenVINO NPU failures are expected and diagnosable; they should not be treated as fatal app failures.
   - official NPU docs say the plugin needs an NPU driver to compile and execute models
   - source:
     - https://docs.openvino.ai/2025/openvino-workflow/running-inference/inference-devices-and-modes/npu-device.html

2. OpenVINO GenAI docs provide a concrete driver floor for troubleshooting.
   - official NPU GenAI guide says to update to Intel NPU driver `32.0.100.3104` or newer if execution failures occur
   - this is troubleshooting guidance, not stated as a universal hard gate for all NPU usage
   - source:
     - https://docs.openvino.ai/2025/openvino-workflow-generative/inference-with-genai/inference-with-genai-on-npu.html

3. OpenVINO recommends target-machine caching rather than shipping opaque precompiled artifacts as the default strategy.
   - official NPU GenAI guide says NPU compilation happens on-the-fly and may take substantial time
   - official guide documents `CACHE_DIR` as the preferred cache mechanism and notes `NPUW_CACHE_DIR` is legacy since OpenVINO `2025.1`
   - source:
      - https://docs.openvino.ai/2025/openvino-workflow-generative/inference-with-genai/inference-with-genai-on-npu.html

4. OpenVINO exposes enough NPU properties to support better classification than just "driver missing" versus "driver outdated".
   - official NPU device docs list `ov::available_devices` and `ov::intel_npu::driver_version` as readable properties
   - implication: SmolPC can separate `missing`, `unknown`, `recommended_update`, and `unusable` states if the native lane captures those properties
   - sources:
     - https://docs.openvino.ai/2025/openvino-workflow/running-inference/inference-devices-and-modes/npu-device.html
     - https://docs.openvino.ai/2024/api/c_cpp_api/group__ov__runtime__npu__prop__cpp__api.html

## 3. ORT facts that still matter

1. ONNX Runtime `v1.24.3` is still the latest checked runtime release for the existing CPU and DirectML lanes.
   - release date: `2026-03-05`
   - source:
     - https://github.com/microsoft/onnxruntime/releases/tag/v1.24.3

2. `ort` `v2.0.0-rc.12` remains relevant for the existing ORT-backed lanes.
   - publish date: `2026-03-05`
   - source:
     - https://github.com/pykeio/ort/releases/tag/v2.0.0-rc.12

3. ONNX Runtime GenAI still documents first-class Windows build flows for CPU, DirectML, and NvTensorRtRtx, but not OpenVINO.
   - current build docs show `--use_dml`, `--use_cuda`, and `--use_trt_rtx`
   - no documented `--use_openvino` path was found
   - implication: `OpenVINO via ORT GenAI` remains a research path, not the chosen implementation
   - source:
     - https://onnxruntime.ai/docs/genai/howto/build-from-source.html

## 4. Stale guidance removed from the plan

- `ORT + OpenVINO EP` is no longer a product path in SmolPC
- Intel OpenVINO EP tuple guidance remains historically interesting, but it is not the implementation driver anymore
- ORT OpenVINO EP docs should not be used to steer architecture unless this plan is reopened
