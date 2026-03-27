# SmolPC 2.0

An offline AI assistant platform for secondary school students, built in partnership with Intel and UCL.

SmolPC 2.0 runs large language models locally on student laptops — no cloud, no telemetry, no internet required. It targets budget Windows hardware (8 GB RAM minimum) and uses Intel OpenVINO for NPU acceleration, DirectML for discrete GPUs, and a CPU fallback so every machine gets inference.

The platform includes Code Helper (a coding assistant), plus connectors for Blender, GIMP, and LibreOffice that extend the AI assistant into creative and productivity apps.

The app ships as a single installer with bundled models, runtimes, and a setup wizard that detects available hardware and provisions the right model automatically.

## Architecture at a Glance

```
Tauri 2 Desktop App (Svelte 5 + Tailwind 4)
        │
        │ HTTP (localhost:19432)
        ▼
   Inference Engine (Rust / Axum)
   ├── OpenVINO GenAI (CPU + NPU)
   ├── DirectML via ONNX Runtime (discrete GPU)
   └── TTS Sidecar (localhost:19433)
```

The engine runs as a standalone local HTTP server. The desktop app connects via `smolpc-engine-client`. Connectors for Blender, GIMP, and LibreOffice extend the assistant into creative and productivity apps.

## Prerequisites

| Requirement | Version |
|-------------|---------|
| **Rust** | 1.88+ (pinned via `rust-toolchain.toml`) |
| **Node.js** | 18+ (23 used in CI) |
| **OS** | Windows 11 |
| **RAM** | 8 GB minimum, 16 GB recommended |
| **Hardware** | Intel Core Ultra (NPU) recommended; discrete GPU (DirectML) or CPU-only supported |

## Quick Start

### 1. Clone and install dependencies

```bash
git clone https://github.com/SmolPC-2-0/CodeHelper.git
cd CodeHelper
npm ci
```

### 2. Stage runtime libraries (one-time)

From the `app/` directory:

```powershell
# OpenVINO runtime (required for NPU and CPU inference)
npm run runtime:setup:openvino

# DirectML runtime (required for discrete GPU inference)
npm run runtime:setup:dml

# Bundled Python (required for GIMP/Blender connectors)
npm run runtime:setup:python
```

### 3. Download models (one-time)

```powershell
# Lightweight model for 8 GB machines (~900 MB - 1.3 GB)
npm run model:setup:qwen25-instruct

# Advanced model for 16 GB+ machines (~2.2 - 2.9 GB)
npm run model:setup:qwen3-4b
```

### 4. Run the app

```powershell
# Full Tauri app with hot reload (from repo root)
npm run tauri:dev

# Or run just the engine server
cargo run -p smolpc-engine-host

# Or run just the frontend
cd app && npm run dev
```

The app auto-detects your hardware and selects the best backend:

| Priority | Backend | Hardware Required |
|----------|---------|-------------------|
| 1 | DirectML | Discrete GPU (NVIDIA, AMD) |
| 2 | OpenVINO NPU | Intel Core Ultra NPU |
| 3 | CPU | Any (fallback) |

## Build for Production

### NSIS Installer

```powershell
cd app
npm run tauri build
```

The installer requires no admin rights and bundles an offline WebView2 installer.

### Offline USB Bundle

For school deployment with no internet:

```powershell
cd app
npm run package:offline
```

Creates a self-contained bundle (installer + models + runtimes) for USB deployment. See [`installers/README.md`](installers/README.md) for details.

## Documentation

See [`docs/`](docs/) for full documentation, including architecture, engine API reference, deployment guides, and deep dives into inference, hardware detection, and connector development.

## License

[MIT](LICENSE)
