# Offline Installers

This directory contains tooling and output for fully offline, USB-distributable SmolPC Code Helper installers.

## Available Bundles

| Bundle | Models | Recommended target |
|------|------|------|
| `usb/` | All models (Qwen 2.5 + Qwen 3) | 8 GB+ USB, mixed hardware |
| `qwen2.5-1.5b-instruct/` | Qwen 2.5 1.5B only (lightweight) | 4 GB+ USB, 8 GB RAM machines |
| `qwen3-4b/` | Qwen 3 4B only (advanced) | 8 GB+ USB, 16 GB RAM machines |

## What a bundle contains

Each bundle follows this layout:

```text
<bundle>/
  SmolPC Code Helper_x.x.x_x64-setup.exe
  Install-CodeHelper.cmd
  Install-CodeHelper.ps1
  models/
    model-archives.json
    SHA256SUMS.txt
    Install-Models.ps1
    <model-archive>.zip
```

## How to build

Prerequisites: Node.js 22+, Rust 1.88+, PowerShell 7 (`pwsh`), models downloaded to `%LOCALAPPDATA%\SmolPC\models\`.

From the repo root:

```powershell
cd app

# 1. Stage runtime DLLs (one-time)
pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/setup-directml-runtime.ps1 -Force
pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/setup-openvino-runtime.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/setup-bundled-python-runtime.ps1

# 2. Download models (one-time, ~7 GB total)
npm run model:setup:qwen25-instruct
npm run model:setup:qwen3-4b

# 3. Build the full offline bundle (all models)
pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/package-offline-bundle.ps1 -OutputDir ../../installers/usb
```

Output lands in `installers/usb/`. Per-model bundles are assembled from the full bundle by copying the installer and filtering model archives.

## How to deploy

1. Copy the chosen bundle folder to a USB stick
2. On the target Windows machine, double-click `Install-CodeHelper.cmd`
3. The wrapper script will:
   - Install the app silently (no admin required) to `%LOCALAPPDATA%\Programs\SmolPC Code Helper\`
   - Extract AI models to `%LOCALAPPDATA%\SmolPC\models\`
   - Launch the app and auto-detect the best backend (DirectML GPU, OpenVINO NPU, or CPU)

No internet connection is required at any point, and admin rights are not required.

## What gets installed

| Item | Path |
|------|------|
| App + engine + DLLs | `%LOCALAPPDATA%\Programs\SmolPC Code Helper\` |
| AI models | `%LOCALAPPDATA%\SmolPC\models\` |
| Engine runtime data | `%LOCALAPPDATA%\SmolPC\engine-runtime\` |

## Troubleshooting

**Install-CodeHelper.cmd fails silently:** Run `Install-CodeHelper.ps1` directly in PowerShell to see error output.

**Models fail to extract:** Run `models\Install-Models.ps1 -Force` manually to re-extract with verbose output.

**App launches but no inference:** Check that models exist in `%LOCALAPPDATA%\SmolPC\models\`. The app's setup panel will show which models are detected.

**SmartScreen blocks the installer:** Click "More info" then "Run anyway". For school-wide deployment, IT can whitelist the installer hash or install path via GPO.
