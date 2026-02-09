# Windows Offline Installer (Fat Bundle)

This project can now produce a single-file Windows NSIS installer that bundles:

- `qwen2.5-coder-1.5b/model.onnx`
- `qwen2.5-coder-1.5b/tokenizer.json`
- `onnxruntime.dll`

The installer runs in `currentUser` mode (no admin required).

## Build On Windows

From repo root:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-windows-offline-installer.ps1
```

Preflight only (no build):

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-windows-offline-installer.ps1 -PreflightOnly
```

The script will:

1. Ensure `src-tauri/libs/onnxruntime.dll` exists (downloads it if missing).
   - Download path verifies the ONNX Runtime archive SHA256 before extraction.
2. Verify required model/tokenizer/runtime files.
3. Run `npm ci` and `cargo check`.
4. Build NSIS installer via `npm run tauri build -- --bundles nsis`.
5. Print final installer path and size.

## Output

Expected installer location:

`src-tauri/target/release/bundle/nsis/*.exe`

## Fresh-Laptop Validation Checklist

1. Use a clean Windows x64 laptop with no dev tools installed.
2. Run the installer EXE.
3. Launch app and confirm it opens without any setup prompts for model/runtime files.
4. Verify model auto-load works and chat can generate a short response.
5. Verify no admin rights were required for install.

## Notes

- WebView2 install mode is `downloadBootstrapper` to avoid pushing installer size over NSIS limits.
- Internet may be required during install if the machine does not already have WebView2 runtime.
