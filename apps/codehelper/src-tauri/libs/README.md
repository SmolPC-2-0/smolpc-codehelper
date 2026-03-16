This directory holds app-local runtime libraries for the shared engine.

Current local staging flows:
- ORT / DirectML: `scripts/setup-libs.sh`
- OpenVINO NPU: `npm run runtime:setup:openvino`

Windows ORT bundle files:
- `onnxruntime.dll`
- `onnxruntime_providers_shared.dll`
- `DirectML.dll`
- `onnxruntime-genai.dll`

Windows OpenVINO bundle files live under `libs/openvino/`:
- `openvino.dll`
- `openvino_c.dll`
- `openvino_genai.dll`
- `openvino_genai_c.dll`
- `openvino_tokenizers.dll`
- `openvino_intel_npu_plugin.dll`
- `openvino_intel_npu_compiler.dll`
- `openvino_intel_cpu_plugin.dll`
- `openvino_ir_frontend.dll`
- `tbb12.dll`
- `tbbbind_2_5.dll`
- `tbbmalloc.dll`
- `tbbmalloc_proxy.dll`
- `icudt70.dll`
- `icuuc70.dll`

OpenVINO staging on Windows uses the official 2026 GenAI archive:
- `https://storage.openvinotoolkit.org/repositories/openvino_genai/packages/2026.0/windows/openvino_genai_windows_2026.0.0.0_x86_64.zip`

The setup script validates the GenAI C ABI on `openvino_genai_c.dll` before copying the bundle into this directory.

The actual Windows OpenVINO runtime files are not checked into git. This
repository does keep `libs/openvino/README.md` checked in so clean checkouts
and CI have a real directory for Tauri resource resolution. Development and
packaged Windows builds still require the setup script to stage the runtime DLLs
into that directory before launch.
