# Getting Started

This is the fastest way to get the current app running and confirm end-to-end behavior.

## 1) Launch the Desktop App

```bash
npm install
npm run runtime:setup
npm run model:setup:qwen2_5
npm run tauri:dev
```

`runtime:setup` runs `scripts/setup-libs.ps1` on Windows (`runtime:setup:sh` is available if you prefer Bash).
`npm run tauri:dev` and `npm run tauri dev` both use wrappers that configure `CARGO_TARGET_DIR`.

Expected startup behavior:

- Rust backend initializes local RAG index
- Bridge starts on `http://127.0.0.1:5179`
- Shared engine is started/reused and model autoload is attempted

## 2) Install and Enable the Blender Addon

1. Blender -> Edit -> Preferences -> Add-ons
2. Install `blender_addon/blender_helper_http.py`
3. Enable the addon
4. Open 3D Viewport sidebar (`N`) -> `Learn` tab

The addon sends scene updates and asks via the bridge endpoint.

## 3) Verify Status in App

- `Blender` indicator should show scene connectivity when Blender is open
- `RAG` indicator should show connection/backend status
- Backend toggle in the header switches between `ENGINE` and `OLLAMA` (when allowed)

## 4) Ask a Question

In the desktop app:

1. Create/select a chat
2. Ask a Blender question
3. Confirm token streaming appears live
4. Use `Stop generation` to cancel mid-response

## 5) Optional: Prepare Local Model Artifacts

If you are packaging self-contained builds:

```powershell
npm run model:setup:qwen2_5
# or
npm run model:setup:qwen3
npm run bundle:stage:model
```

## 6) Common Issues

### Bridge not reachable from addon

- Ensure the desktop app is running
- Check `http://127.0.0.1:5179/health`

### Engine backend unavailable

- Verify sidecar/runtime assets (`src-tauri/binaries`, `src-tauri/libs`) for packaged mode
- Or enable fallback:
  - `BLENDER_HELPER_ALLOW_OLLAMA_FALLBACK=1`

### No model loaded

- Ensure model files exist under your model root (default `%LOCALAPPDATA%\SmolPC\models`)
- Restage bundle assets with `npm run bundle:stage:model` before build
