This directory holds platform-specific runtime libraries used by ONNX Runtime.

Run `scripts/setup-libs.sh` to download verified binaries for your platform.

Windows bundle files:
- `onnxruntime.dll`
- `onnxruntime_providers_shared.dll`
- `DirectML.dll`

macOS/Linux bundle files:
- `libonnxruntime.dylib` (macOS)
- `libonnxruntime.so` (Linux)

These files are NOT checked into git (they are large binary dependencies).
