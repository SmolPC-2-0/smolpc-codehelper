# Release & Installer Guide

This document covers building, packaging, and distributing SmolPC Code Helper for Windows x64.

## Installer Overview

The app ships as an **NSIS installer** (`SmolPC Code Helper_x.x.x_x64-setup.exe`) that installs per-user to `%LOCALAPPDATA%\Programs\SmolPC Code Helper\` — no admin rights required.

### What the installer includes

| Component | Size | Description |
|-----------|------|-------------|
| App binary | ~10 MB | `smolpc-desktop.exe` (Tauri shell) |
| Engine sidecar | ~10 MB | `smolpc-engine-host.exe` (inference server) |
| OpenVINO runtime | ~170 MB | 15 DLLs for CPU and NPU inference (2026.0.0) |
| DirectML/ORT runtime | ~40 MB | 4 DLLs for GPU inference via DirectML |
| Python runtime | ~20 MB | Embedded CPython 3.12.9 + uv (for MCP servers) |
| WebView2 installer | ~2 MB | Offline bootstrapper (silent install) |
| App resources | ~1 MB | GIMP, Blender, LibreOffice integrations |

**Total installer size:** ~250-300 MB

### What the installer does NOT include

- **AI models** — Qwen 2.5 1.5B (~1 GB) and Qwen 3 4B (~2-4 GB) are too large to embed. They are distributed via the offline bundle (see below) or expected at `%LOCALAPPDATA%\SmolPC\models\` from a prior install.

## Two Distribution Paths

### 1. NSIS Installer Only (GitHub Releases)

For users with internet access or pre-installed models. The app launches, auto-detects the best backend, and shows a setup banner if no models are found.

### 2. Offline Bundle (USB for Schools)

A folder containing the NSIS installer + model archives + install scripts. This is the intended deployment path for schools without internet.

Contents:
```
smolpc-codehelper-offline/
  SmolPC Code Helper_x.x.x_x64-setup.exe
  Install-CodeHelper.cmd        # Double-click to install everything
  Install-CodeHelper.ps1        # PowerShell orchestrator
  models/
    model-archives.json         # Manifest with checksums
    qwen2.5-1.5b-instruct-dml.zip
    qwen2.5-1.5b-instruct-openvino.zip
    qwen3-4b-dml.zip
    qwen3-4b-openvino.zip
    Install-Models.ps1
    SHA256SUMS.txt
```

The `Install-CodeHelper.cmd` wrapper:
1. Runs the NSIS installer silently
2. Extracts model archives to `%LOCALAPPDATA%\SmolPC\models\`
3. Launches the app

## Building Locally

All commands from `apps/codehelper/`:

### Prerequisites

- Node.js 22+ and npm
- Rust 1.88+ with `x86_64-pc-windows-msvc` target
- Internet access (for first-time DLL downloads)

### Build the NSIS installer

```powershell
# 1. Stage runtime DLLs (one-time, downloads ~300 MB)
npm run runtime:setup:dml           # DirectML + ORT → libs/
npm run runtime:setup:openvino      # OpenVINO 2026 → libs/openvino/
npm run runtime:setup:python        # Python 3.12 + uv → resources/python/payload/

# 2. Build and stage the engine sidecar
npm run engine:build:release        # Release build → binaries/smolpc-engine-host.exe

# 3. Build the NSIS installer
npx tauri build --bundles nsis --target x86_64-pc-windows-msvc
```

Output: `../../target/x86_64-pc-windows-msvc/release/bundle/nsis/SmolPC Code Helper_x.x.x_x64-setup.exe`

### Build the offline bundle (installer + models)

```powershell
# Requires models already downloaded to %LOCALAPPDATA%\SmolPC\models\
npm run model:setup:qwen25-instruct    # Download default model
npm run model:setup:qwen3-4b           # Download secondary model

# Build the full offline bundle
npm run package:offline:bundle
```

Output: `../../dist/smolpc-codehelper-offline/`

## CI Release Workflow

The GitHub Actions workflow (`.github/workflows/release.yml`) triggers on version tags:

```bash
git tag v2.2.0
git push origin v2.2.0
```

The workflow:
1. Checks out the repo
2. Installs Node.js 23 and Rust 1.88
3. Downloads and stages runtime DLLs (DirectML, OpenVINO, Python) — cached across runs
4. Builds the engine sidecar in release mode
5. Validates all required artifacts are present
6. Runs `tauri-action` to build the NSIS installer and create a draft GitHub Release
7. Verifies the installer is >50 MB (sanity check)

The draft release appears in GitHub with the `.exe` attached. Review and publish it manually.

## Install Paths

| Item | Path |
|------|------|
| App install | `%LOCALAPPDATA%\Programs\SmolPC Code Helper\` |
| App binary | `...\smolpc-desktop.exe` |
| Engine sidecar | `...\binaries\smolpc-engine-host.exe` |
| Bundled DLLs | `...\libs\` and `...\libs\openvino\` |
| Models (shared) | `%LOCALAPPDATA%\SmolPC\models\` |
| Engine runtime | `%LOCALAPPDATA%\SmolPC\engine-runtime\` |
| Engine logs | `...\engine-runtime\engine-spawn.log` |

## Code Signing (Future)

Without a code signing certificate, Windows SmartScreen shows "Windows protected your PC" on first run. Users can click "More info" then "Run anyway".

For school deployments, IT admins can:
- Create a GPO exception for the install path
- Add the installer hash to their allow list

To enable code signing in CI, set these GitHub Actions secrets:
- `TAURI_SIGNING_PRIVATE_KEY` — base64-encoded PFX certificate
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — certificate password

The `tauri-action` automatically signs the installer when these are present.

## Troubleshooting

**Installer blocked by SmartScreen:** Click "More info" → "Run anyway". See Code Signing section above.

**App launches but no inference:** Models are missing. Either install the offline bundle or place model files in `%LOCALAPPDATA%\SmolPC\models\`.

**Engine fails to start:** Check `%LOCALAPPDATA%\SmolPC\engine-runtime\engine-spawn.log` for errors.

**`npm run runtime:setup:*` fails with `Get-FileHash` error:** Your Windows PowerShell may have a broken module. Use `pwsh` (PowerShell 7) instead of `powershell` to run the scripts directly.

**`npm ci` fails with EPERM:** Close VS Code (Tailwind IntelliSense extension holds a file lock) and retry.
