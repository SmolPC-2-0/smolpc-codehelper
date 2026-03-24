# Offline USB Installer

This directory contains the tooling and output for building a fully offline, USB-distributable package of SmolPC Code Helper.

## What the offline bundle contains

```
usb/
  SmolPC Code Helper_x.x.x_x64-setup.exe    # NSIS installer (~278 MB)
  Install-CodeHelper.cmd                      # Double-click to install everything
  Install-CodeHelper.ps1                      # PowerShell orchestrator
  models/
    model-archives.json                       # Manifest with checksums
    SHA256SUMS.txt                            # Checksum file
    Install-Models.ps1                        # Model extraction script
    qwen2.5-1.5b-instruct-dml.zip           # Qwen 2.5 1.5B — DirectML (~1.3 GB)
    qwen2.5-1.5b-instruct-openvino.zip      # Qwen 2.5 1.5B — OpenVINO (~900 MB)
    qwen3-4b-dml.zip                         # Qwen 3 4B — DirectML (~2.9 GB)
    qwen3-4b-openvino.zip                    # Qwen 3 4B — OpenVINO (~2.2 GB)
```

**Total size: ~4-5 GB** — fits on an 8 GB USB stick.

## How to build it

Prerequisites: Node.js 22+, Rust 1.88+, PowerShell 7 (`pwsh`), models downloaded to `%LOCALAPPDATA%\SmolPC\models\`.

From the repo root:

```powershell
cd apps/codehelper

# 1. Stage runtime DLLs (one-time)
pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/setup-directml-runtime.ps1 -Force
pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/setup-openvino-runtime.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/setup-bundled-python-runtime.ps1

# 2. Download models (one-time, ~7 GB total)
npm run model:setup:qwen25-instruct
npm run model:setup:qwen3-4b

# 3. Build the offline bundle
pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/package-offline-bundle.ps1 -OutputDir ../../installers/usb
```

Output lands in `installers/usb/`.

## How to deploy to a school

1. Copy the entire `usb/` folder to a USB stick
2. On the target Windows machine, double-click `Install-CodeHelper.cmd`
3. The wrapper script will:
   - Install the app silently (no admin required) to `%LOCALAPPDATA%\Programs\SmolPC Code Helper\`
   - Extract AI models to `%LOCALAPPDATA%\SmolPC\models\`
   - Launch the app — it auto-detects the best backend (DirectML GPU, OpenVINO NPU, or CPU)

No internet connection is required at any point.

## What gets installed where

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
