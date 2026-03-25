# SmolPC 2.0

SmolPC 2.0 is a Windows-first local AI desktop platform for education. This repository contains the shared inference engine, the current Tauri desktop app, and the supporting launcher/app zones used to build a private, offline-first assistant experience for classroom workflows.

The current integrated desktop app in this repo is **SmolPC Code Helper**, which consumes the shared SmolPC engine and packages the current user-facing shell. The wider platform architecture is designed to support coding, office, image-editing, and 3D workflows through one local runtime.

## Highlights

- Offline-first local AI desktop app
- Windows-focused deployment
- Shared engine/runtime instead of app-local inference
- Hardware-aware backend selection
- Tauri + Rust + Svelte architecture
- Offline bundle workflow for school deployment
- MIT-licensed repository

## Repository Layout

```text
.
├── engine/
│   └── crates/
│       ├── smolpc-engine-core
│       ├── smolpc-engine-host
│       └── smolpc-engine-client
├── launcher/
├── app/                    (CodeHelper)
├── apps/
│   ├── libreoffice-assistant/
│   ├── gimp-assistant/
│   └── blender-assistant/
└── docs/
```

## Architecture

The repository is organised into three zones:

- `engine/`
  Shared runtime, backend/model detection, readiness lifecycle, inference execution, and API contract.
- `launcher/`
  App-suite entrypoint and orchestration layer.
- `apps/`
  Product-facing apps that consume the shared engine through documented contracts.

The current source-of-truth architecture docs are:

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- [docs/ENGINE_API.md](docs/ENGINE_API.md)
- [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md)

## Prerequisites

For local development on Windows:

- Windows 10/11
- Node.js 22+ and npm
- Rust 1.88+ with `x86_64-pc-windows-msvc`
- Git
- Internet access for first-time runtime/model downloads

## Quick Start

Run these commands from the **repo root** unless stated otherwise.

### 1. Install dependencies

```bash
npm ci
cargo check --workspace
```

### 2. Stage shared runtime dependencies

```bash
npm run runtime:setup:openvino
npm run runtime:setup:python
```

Optional DirectML runtime setup is app-local:

```powershell
cd apps/codehelper
npm run runtime:setup:dml
cd ../..
```

### 3. Download models

Recommended baseline:

```bash
npm run model:setup:qwen3-4b
```

Optional smaller model:

```bash
npm run model:setup:qwen25-instruct
```

### 4. Run the app in development

Automatic backend selection:

```bash
npm run tauri:dev
```

DirectML-focused development path:

```bash
npm run tauri:dml
```

### 5. Run checks

```bash
npm run check
npm run boundary:check
cargo test -p smolpc-engine-core -p smolpc-engine-client -p smolpc-engine-host
cargo check -p smolpc-code-helper
```

## Development Notes

- The engine owns startup/readiness/inference lifecycle.
- Apps should integrate through `smolpc-engine-client` or the documented localhost HTTP contract.
- Legacy app-local inference paths and Ollama-owned command paths have been removed from the current architecture.
- Boundary checks are enforced through `scripts/check-boundaries.ps1`.

## Installation Paths

Current default Windows install/runtime locations:

- App install:
  `%LOCALAPPDATA%\Programs\SmolPC Code Helper\`
- App binary:
  `%LOCALAPPDATA%\Programs\SmolPC Code Helper\smolpc-code-helper.exe`
- Shared models:
  `%LOCALAPPDATA%\SmolPC\models\`
- Engine runtime:
  `%LOCALAPPDATA%\SmolPC\engine-runtime\`
- Engine log:
  `%LOCALAPPDATA%\SmolPC\engine-runtime\engine-spawn.log`

## Building a Windows Installer

For release packaging, run these commands from:

```powershell
cd apps/codehelper
```

### 1. Stage runtimes

```powershell
npm run runtime:setup:dml
npm run runtime:setup:openvino
npm run runtime:setup:python
```

### 2. Build the engine sidecar

```powershell
npm run engine:build:release
```

This stages the current engine sidecar into the app bundle inputs before the installer build.

### 3. Build the NSIS installer

```powershell
npx tauri build --bundles nsis --target x86_64-pc-windows-msvc
```

Output:

```text
../../target/x86_64-pc-windows-msvc/release/bundle/nsis/
```

The current packaging flow produces a per-user NSIS installer in the standard Tauri output directory. In many school or lab environments this should avoid admin rights, but it is still worth validating against the target machine image and local IT policy before broad rollout.

## Offline Deployment Bundle

The current Windows deployment workflow is the **offline bundle**, which packages:

- the NSIS installer
- model archives
- install scripts
- runtime dependencies needed for local execution

### Build the offline bundle

From `apps/codehelper/`:

```powershell
npm run model:setup:qwen25-instruct
npm run model:setup:qwen3-4b
npm run package:offline:bundle
```

Output:

```text
../../dist/smolpc-codehelper-offline/
```

Typical bundle contents:

```text
smolpc-codehelper-offline/
  SmolPC Code Helper_x.x.x_x64-setup.exe
  Install-CodeHelper.cmd
  Install-CodeHelper.ps1
  models/
    model-archives.json
    Install-Models.ps1
    ...
```

This is the intended path for offline or school deployment on current `main`, but it should still be validated on a clean target machine before treating it as a final distribution process.

## Deployment Guide for a Clean Windows Machine

This is the typical deployment flow for schools or offline machines.

### Option A: Offline bundle

1. Build the offline bundle on a development machine.
2. Copy the `smolpc-codehelper-offline/` folder to USB or a network share.
3. On the target Windows machine, open the folder.
4. Run `Install-CodeHelper.cmd`.
5. The current installer flow should:
   - install the app
   - install model archives to `%LOCALAPPDATA%\SmolPC\models\`
   - launch the app
6. On first launch, the engine should auto-detect the best supported backend on that machine.

For real deployments, test this flow on the same class of clean target hardware before wider rollout.

### Option B: Installer only

If models are already present on the machine, or you are distributing them separately:

1. Build the NSIS installer.
2. Run the installer on the target machine.
3. Ensure models exist under `%LOCALAPPDATA%\SmolPC\models\`.
4. Launch the app normally.

## Troubleshooting

### App launches but no inference works

The most common cause is missing models. Check:

```text
%LOCALAPPDATA%\SmolPC\models\
```

### Engine fails to start

Check:

```text
%LOCALAPPDATA%\SmolPC\engine-runtime\engine-spawn.log
```

### `npm run runtime:setup:*` fails in Windows PowerShell

If `Get-FileHash` or script execution fails, try running the scripts in PowerShell 7 (`pwsh`).

### `npm ci` fails with `EPERM`

Close any tools that may be holding file locks, then retry.

### SmartScreen warning on install

Unsigned Windows installers may trigger SmartScreen. Use `More info` -> `Run anyway`, or distribute through an approved school IT process.

## Useful Docs

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- [docs/ENGINE_API.md](docs/ENGINE_API.md)
- [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md)
- [docs/RELEASE.md](docs/RELEASE.md)
- [engine/README.md](engine/README.md)
- [launcher/README.md](launcher/README.md)
- [apps/README.md](apps/README.md)
- [apps/codehelper/README.md](apps/codehelper/README.md)

## License

This repository is currently licensed under the MIT License. See [LICENSE](LICENSE).
