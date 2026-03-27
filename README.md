# SmolPC 2.0

An offline AI assistant platform for secondary school students, built in partnership with Intel and UCL.

SmolPC 2.0 runs large language models locally on student laptops — no cloud, no telemetry, no internet required. It targets budget Windows hardware (8 GB RAM minimum) and uses Intel OpenVINO for NPU acceleration, DirectML for discrete GPUs, and a CPU fallback so every machine gets inference.

The platform includes Code Helper (a coding assistant), plus connectors for Blender, GIMP, and LibreOffice that extend the AI assistant into creative and productivity apps.

The app ships as a single installer with bundled models, runtimes, and a setup wizard that detects available hardware and provisions the right model automatically.

## Installation (Pre-Built)

The `SmolPC_2.0_Final_Build/` folder at the repository root contains the ready-to-use distribution:

```
SmolPC_2.0_Final_Build/
├── SmolPC 2.0_2.2.0_x64-setup.exe   # NSIS installer
└── models/                           # Pre-built model archives
    ├── qwen2.5-1.5b-instruct/       #   8 GB machines
    └── qwen3-4b/                     #   16 GB+ machines
```

1. Run the installer (no admin rights required).
2. On first launch, the setup wizard detects the models folder and provisions them automatically. If models are not detected, point the wizard to the `models/` directory.

The app auto-detects your hardware and selects the best backend:

| Priority | Backend | Hardware Required |
|----------|---------|-------------------|
| 1 | DirectML | Discrete GPU (NVIDIA, AMD) |
| 2 | OpenVINO NPU | Intel Core Ultra NPU |
| 3 | CPU | Any (fallback) |

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

## Development Setup

### Prerequisites

| Requirement | Version |
|-------------|---------|
| **Rust** | 1.88+ (pinned via `rust-toolchain.toml`) |
| **Node.js** | 18+ (23 used in CI) |
| **OS** | Windows 11 |
| **RAM** | 8 GB minimum, 16 GB recommended |
| **Hardware** | Intel Core Ultra (NPU) recommended; discrete GPU (DirectML) or CPU-only supported |

### 1. Clone and install dependencies

```bash
git clone https://github.com/SmolPC-2-0/CodeHelper.git
cd CodeHelper
npm ci
```

### 2. Stage runtime libraries (one-time)

These scripts download the native DLLs the engine needs at runtime. Run from the repo root:

```powershell
# OpenVINO runtime (required for NPU and CPU inference)
npm run runtime:setup:openvino

# DirectML runtime (required for discrete GPU inference)
npm run runtime:setup:dml

# Bundled Python (required for GIMP/Blender/LibreOffice connectors)
npm run runtime:setup:python
```

### 3. Download models (one-time)

Models are downloaded from HuggingFace and installed to `%LOCALAPPDATA%\SmolPC\models\`:

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

### Rebuilding the installer

After making code changes, rebuild the NSIS installer:

```powershell
npm run tauri build
```

This produces `target/release/bundle/nsis/SmolPC 2.0_<version>_x64-setup.exe`.

## Known Limitations

- **NPU first-time compilation takes 3-5 minutes.** The first launch on an Intel NPU compiles the model to hardware-specific blobs. Subsequent launches load from cache in seconds.
- **NPU context window is fixed at ~2048 input tokens.** Long multi-turn conversations must switch to CPU mode. See [`docs/engine/NPU_GUIDE.md`](docs/engine/NPU_GUIDE.md) for details.
- **Qwen3-4B requires 16 GB RAM.** Machines with less than 16 GB are limited to Qwen 2.5 1.5B.
- **DirectML on Intel integrated GPUs produces garbage output.** Only discrete GPUs (NVIDIA, AMD) are accepted as DirectML candidates.
- **NPU device corruption after heavy usage.** Repeated model loading/unloading can leave the NPU in a bad state. Reboot to recover. See [`docs/engine/NPU_GUIDE.md`](docs/engine/NPU_GUIDE.md#known-limitations-and-workarounds).

## Documentation

| Area | Docs |
|------|------|
| **Architecture** | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) |
| **Engine** | [`docs/engine/`](docs/engine/) — API reference, lifecycle, inference deep dive, NPU guide |
| **Hardware** | [`docs/hardware/`](docs/hardware/) — hardware detection, model selection, OpenVINO FFI |
| **Development** | [`docs/development/`](docs/development/) — workflow, testing, benchmarks, design decisions |
| **Apps** | [`docs/apps/`](docs/apps/) — mode capabilities, connector development guide |
| **Security** | [`docs/SECURITY_AND_PRIVACY.md`](docs/SECURITY_AND_PRIVACY.md) |

## License

[MIT](LICENSE)
