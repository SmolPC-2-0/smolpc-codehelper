# Offline Installers

USB-distributable installer bundles for SmolPC Code Helper. Each bundle contains the NSIS installer and AI model archives — no internet required.

## Available Bundles

| Bundle | Models | Size | USB Size |
|--------|--------|------|----------|
| `usb/` | All models (Qwen 2.5 + Qwen 3) | ~6.5 GB | 8 GB USB |
| `qwen2.5-1.5b-instruct/` | Qwen 2.5 1.5B only (lightweight) | ~2.2 GB | 4 GB USB |
| `qwen3-4b/` | Qwen 3 4B only (advanced) | ~4.7 GB | 8 GB USB |

**Recommended for 8 GB RAM machines:** `qwen2.5-1.5b-instruct/`
**Recommended for 16 GB RAM machines:** `qwen3-4b/`

## How to Deploy

1. Copy the chosen bundle folder to a USB stick
2. On the target Windows machine, double-click `Install-CodeHelper.cmd`
3. The script installs the app silently, extracts the AI models, and launches

No internet connection or admin rights required.

## How to Build

Prerequisites: Node.js 22+, Rust 1.88+, PowerShell 7 (`pwsh`), models downloaded to `%LOCALAPPDATA%\SmolPC\models\`.

```powershell
cd apps/codehelper

# Stage runtime DLLs (one-time)
pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/setup-directml-runtime.ps1 -Force
pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/setup-openvino-runtime.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/setup-bundled-python-runtime.ps1

# Download models (one-time, ~7 GB total)
npm run model:setup:qwen25-instruct
npm run model:setup:qwen3-4b

# Build the full offline bundle (all models)
pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/package-offline-bundle.ps1 -OutputDir ../../installers/usb
```

Per-model bundles are assembled from the full bundle by copying the installer and filtering model archives.

## What Gets Installed

| Item | Path |
|------|------|
| App + engine + DLLs | `%LOCALAPPDATA%\Programs\SmolPC Code Helper\` |
| AI models | `%LOCALAPPDATA%\SmolPC\models\` |
| Engine runtime data | `%LOCALAPPDATA%\SmolPC\engine-runtime\` |
