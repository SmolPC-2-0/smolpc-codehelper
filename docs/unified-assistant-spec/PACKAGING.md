# Packaging & Distribution

> **Purpose:** Defines the complete packaging, installation, code signing, Python/MCP deployment, update mechanism, and school IT compatibility strategy for the SmolPC Unified Assistant.
>
> **Audience:** Any AI session working on build pipelines, installer configuration, DLL bundling, first-run experience, or deployment to school environments.
>
> **Last Updated:** 2026-03-13

---

## Table of Contents

1. [Installer Format](#installer-format)
2. [Directory Structure](#directory-structure)
3. [DLL Bundling](#dll-bundling)
4. [Model Distribution](#model-distribution)
5. [Python & MCP Server Deployment](#python--mcp-server-deployment)
6. [Code Signing](#code-signing)
7. [First-Run Experience](#first-run-experience)
8. [School IT Compatibility](#school-it-compatibility)
9. [Update Mechanism](#update-mechanism)
10. [Portable Mode](#portable-mode)
11. [Data Protection](#data-protection)
12. [Disk Footprint](#disk-footprint)
13. [Build Pipeline](#build-pipeline)

---

## Installer Format

### Decision: Tauri NSIS `currentUser` Mode

Tauri 2 uses **NSIS** (Nullsoft Scriptable Install System) on Windows by default.

**Configuration in `tauri.conf.json`:**
```json
{
  "bundle": {
    "targets": ["nsis"],
    "windows": {
      "nsis": {
        "installMode": "currentUser",
        "displayLanguageSelector": false,
        "installerIcon": "./icons/installer.ico",
        "headerImage": "./icons/header.bmp"
      }
    }
  }
}
```

### Why `currentUser` Mode

| Mode | Install Location | Registry | Admin Required | School Compatible |
|------|-----------------|----------|----------------|-------------------|
| `currentUser` | `%AppData%\SmolPC CodeHelper` | HKCU | **No** | Yes (usually) |
| `perMachine` | `Program Files\SmolPC CodeHelper` | HKLM | Yes | Depends on IT |
| `both` | User chooses | HKCU or HKLM | Optional | Most flexible |

**`currentUser` is the default** because:
- No admin privileges needed — students can install themselves
- No UAC elevation prompts
- Installs to `%AppData%` — user-writable
- Uninstall via Settings > Apps (registered in HKCU)
- Won't conflict with other users or system software

### Silent Install (for IT Deployment)

```cmd
SmolPC-CodeHelper-Setup.exe /S
```

The `/S` flag enables silent installation. Compatible with:
- **SCCM** (System Center Configuration Manager)
- **Intune** (Microsoft Endpoint Manager)
- **Group Policy** software deployment

For per-machine deployment by IT admins, provide a separate MSI or use `perMachine` mode with admin credentials.

### Custom NSIS Hooks

For advanced installer behavior, Tauri supports custom NSIS hooks via `.nsh` files:
- `src-tauri/nsis/installer.nsh` — Custom install steps
- `src-tauri/nsis/uninstaller.nsh` — Custom uninstall cleanup
- `src-tauri/nsis/header.nsh` — Custom macros

Use cases:
- Clean up model cache on uninstall
- Register file associations
- Add Start Menu shortcuts
- Run first-run setup script

---

## Directory Structure

### Installed App Layout

```
%AppData%\SmolPC CodeHelper\
├── bin\
│   ├── smolpc-codehelper.exe      # Tauri app (unified assistant)
│   ├── smolpc-engine-host.exe     # Inference server (Axum :19432)
│   └── uv.exe                     # Python package manager (sidecar)
│
├── runtimes\
│   ├── onnxruntime\
│   │   ├── onnxruntime.dll        # ONNX Runtime core
│   │   ├── onnxruntime_genai.dll  # GenAI pipeline (C API)
│   │   └── DirectML.dll           # DirectML GPU backend
│   └── openvino\
│       ├── openvino.dll           # OpenVINO core
│       ├── openvino_genai.dll     # GenAI pipeline (C API)
│       ├── openvino_c.dll         # C bindings
│       ├── tbb12.dll              # Threading Building Blocks
│       └── ...                    # Additional OV dependencies
│
├── models\
│   ├── qwen3-1.7b\               # Tier 1 model
│   │   ├── onnx-int4\            # ONNX INT4 artifacts
│   │   │   ├── genai_config.json
│   │   │   ├── model.onnx
│   │   │   ├── model.onnx.data
│   │   │   └── tokenizer.json
│   │   ├── openvino-int4\         # OpenVINO IR INT4 artifacts
│   │   │   ├── openvino_model.xml
│   │   │   ├── openvino_model.bin
│   │   │   └── tokenizer.json
│   │   └── cache\                 # NPU blob cache (runtime-generated)
│   │       └── *.blob
│   └── qwen3-4b\                  # Tier 2 model (if downloaded)
│       ├── onnx-int4\
│       ├── openvino-int4\
│       └── cache\
│
├── mcp-servers\
│   ├── gimp-mcp\                  # GIMP MCP server source
│   │   ├── pyproject.toml
│   │   └── src\
│   ├── blender-mcp\               # Blender MCP server source
│   │   ├── pyproject.toml
│   │   └── src\
│   └── libre-mcp\                 # LibreOffice MCP server source
│       ├── pyproject.toml
│       └── src\
│
├── python\                        # uv-managed Python installation
│   ├── python-3.12\               # Python interpreter (auto-installed)
│   └── venvs\                     # Per-server virtual environments
│       ├── gimp-mcp\
│       ├── blender-mcp\
│       └── libre-mcp\
│
├── data\
│   ├── engine.db                  # Backend decisions, model state (SQLite)
│   ├── conversations\             # Local chat history (per-mode)
│   │   ├── gimp\
│   │   ├── blender\
│   │   └── writer\
│   └── rag\                       # Blender RAG database (TF-IDF index)
│       └── blender-docs\
│
├── cache\
│   └── npu-blobs\                 # Compiled NPU models (generated at runtime)
│
├── logs\
│   ├── engine.log
│   ├── app.log
│   └── mcp\
│       ├── gimp.log
│       ├── blender.log
│       └── libre.log
│
└── config\
    └── settings.json              # User preferences (mode, model, backend)
```

### Key Directory Notes

- **`bin/`** — All executables. Engine host is separate binary (not in-process) so multiple clients can connect.
- **`runtimes/`** — DLLs loaded via `libloading` with absolute paths. Never via system PATH. See [DLL Bundling](#dll-bundling).
- **`models/`** — Each model has subdirectories per format (onnx-int4, openvino-int4). Supports multiple models simultaneously.
- **`mcp-servers/`** — Python source code, not pre-built binaries. `uv` creates venvs on first run.
- **`python/`** — Managed entirely by `uv`. Auto-installed on first run. User never interacts with this directly.
- **`data/`** — All persistent user data. Must be deletable for privacy (see [Data Protection](#data-protection)).
- **`cache/`** — Ephemeral. Can be deleted without data loss. NPU blobs regenerated on next load.

---

## DLL Bundling

### Runtime DLLs

| Runtime | DLLs | Approximate Size | Purpose |
|---------|------|-------------------|---------|
| ONNX Runtime | `onnxruntime.dll` | ~80MB | Core ONNX inference |
| ORT GenAI | `onnxruntime_genai.dll` | ~20MB | GenAI pipeline (tokenizer, KV cache, sampling) |
| DirectML | `DirectML.dll` | ~50MB | GPU acceleration via DirectX 12 |
| OpenVINO | `openvino.dll` + deps | ~200MB | Core OpenVINO inference |
| OV GenAI | `openvino_genai.dll` | ~50MB | GenAI pipeline for OV |
| TBB | `tbb12.dll` | ~5MB | Threading (OpenVINO dependency) |
| **Total** | | **~400-600MB** | |

### DLL Loading Strategy

All DLLs are loaded via the `libloading` crate with **absolute paths**. This is enforced by design:

```rust
// In runtime_loading.rs (centralized — enforced by test)
pub fn load_ort_genai(runtime_dir: &Path) -> Result<Library> {
    let dll_path = runtime_dir.join("onnxruntime").join("onnxruntime_genai.dll");
    unsafe { Library::new(dll_path) }
}
```

**Security: DLL Hijacking Prevention**
- NEVER load DLLs by name only (e.g., `Library::new("onnxruntime.dll")`) — this searches system PATH and is vulnerable to DLL hijacking
- ALWAYS use absolute paths constructed from the known runtime directory
- A test enforces that `libloading` calls only appear in `runtime_loading.rs`

### Tauri Resource Bundling

DLLs are bundled as Tauri resources (not embedded in the binary):

```json
// tauri.conf.json
{
  "bundle": {
    "resources": [
      {
        "path": "runtimes/**/*",
        "target": "runtimes"
      }
    ]
  }
}
```

**Important:** Tauri `bundle.resources` glob must match actual files at build time. If the glob matches zero files, the build fails. Use a placeholder file (e.g., `runtimes/README.md`) with a `.gitignore` exception to ensure the glob always matches:

```
# .gitignore
runtimes/*
!runtimes/README.md
```

### Version Management

- ONNX Runtime: v1.22.0 (macOS/Linux) or v1.22.1 (Windows only)
  - **ONNX Runtime v1.22.1 only ships Windows builds** — use v1.22.0 for cross-platform CI
- OpenVINO: 2025.x (latest stable)
- DLLs are NOT in the git repo (too large). Downloaded during build or CI.

---

## Model Distribution

### Decision: PENDING (See MODEL_STRATEGY.md)

Current recommendation: **Hybrid approach**

1. **Bundle Tier 1 model** (~1-2GB) in installer for immediate offline use
2. **Download Tier 2 model** on-demand if hardware supports it
3. **Support sideloading** from USB drive for fully offline schools
4. **Manual download instructions** for IT admins

### Model Download Implementation

```
First run:
1. Engine starts → detects hardware (RAM, GPU, NPU)
2. Checks models/ directory for available models
3. If bundled Tier 1 model found → load immediately
4. If 16GB+ RAM detected and Tier 2 not present → prompt user to download
5. Download with progress bar, resume support
6. Store in models/ directory
```

### Sideloading Support

IT admins or students can copy model files manually:

```cmd
# Copy from USB or network share
xcopy /E \\server\models\qwen3-4b %AppData%\SmolPC CodeHelper\models\qwen3-4b
```

The engine scans the `models/` directory on startup and registers any valid model directories.

---

## Python & MCP Server Deployment

### Strategy: Bundle `uv` as Tauri Sidecar

[uv](https://docs.astral.sh/uv/) is Astral's fast Python package manager. We bundle it as a Tauri sidecar binary.

### Why `uv` (Not pip, conda, Poetry, or PyInstaller)

| Approach | Size | Admin Needed | Offline | School Compatible | Issues |
|----------|------|-------------|---------|-------------------|--------|
| **uv sidecar** | ~35MB | No | Partial* | Yes | *Needs internet for first Python download |
| pip + system Python | 0MB | No | No | Broken (no Python pre-installed) | Requires Python to be installed |
| conda/miniconda | ~80MB | Sometimes | No | Risky (environment conflicts) | Heavy, slow |
| Poetry | ~20MB | No | No | Requires Python pre-installed | Same as pip |
| PyInstaller | varies | No | Yes | **No** — Windows Defender false positives | Frequent antivirus blocks on school machines |
| Embedded Python zip | ~12MB | No | Yes | Yes | Limited (no pip, manual setup) |

**Decision:** Bundle `uv.exe` (~35MB) with Python embeddable zip fallback for firewalled schools.

### Tauri Sidecar Configuration

```json
// tauri.conf.json
{
  "bundle": {
    "externalBin": [
      "bin/uv"
    ]
  }
}
```

The sidecar binary must be named with platform triple: `bin/uv-x86_64-pc-windows-msvc.exe`

### First-Run Python Setup

```
1. App detects no Python venvs exist
2. Runs: uv.exe python install 3.12 (downloads Python to %LOCALAPPDATA%\uv\python\)
3. For each MCP server:
   a. cd mcp-servers/<server>/
   b. uv.exe venv ../../python/venvs/<server>/ --python 3.12
   c. uv.exe pip install -e . --python ../../python/venvs/<server>/
4. Python + venvs ready (~30-60 seconds on first run, instant after)
```

### uv Python Download Locations

- Python interpreter: `%LOCALAPPDATA%\uv\python\cpython-3.12.*\` (~25-40MB)
- Virtual environments: `%AppData%\SmolPC CodeHelper\python\venvs\<server>\`
- No admin required. No system PATH modification.

### Offline Fallback (Firewalled Schools)

For schools with no internet:

1. **Bundle Python embeddable zip** (~12MB) in `src-tauri/resources/python-embed/`
2. On first run, if `uv python install` fails (no internet):
   - Extract embedded Python to `python/python-3.12/`
   - Use it directly for MCP server execution
3. Dependencies: Bundle MCP server deps as wheels in `resources/wheels/`
   - `uv pip install --offline --find-links ./wheels -e .`

### Can We Reuse Application-Bundled Python?

**No.** GIMP, Blender, and LibreOffice all bundle their own Python, but:
- They install to `Program Files` — needs admin to access
- Different Python versions (GIMP: 3.10, Blender: 3.11, LibreOffice: varies)
- Their Pythons have application-specific site-packages
- Modifying their Python environments risks breaking the host application

### Long-Term: Rewrite MCP Servers in Rust

- **GIMP MCP:** Uses TCP to Script-Fu console. No Python bindings needed. Most feasible to rewrite.
- **Blender MCP:** Uses Python to interact with Blender's Python API. Harder to rewrite.
- **LibreOffice MCP:** Uses UNO API (Python bindings). Hardest to rewrite.

Rust MCP servers would eliminate the Python dependency entirely. Prioritize GIMP first.

### MCPB (MCP Bundle) Format

An emerging standard for distributing MCP servers:
- `.mcpb` file = zip archive containing `manifest.json` + server code + dependencies
- May simplify distribution in the future
- Monitor adoption before committing to this format

---

## Code Signing

### Decision: Azure Trusted Signing

**Cost:** $9.99/month (cheapest legitimate code signing option)

### Why Code Signing Matters

Without signing:
- **Windows SmartScreen** shows "Windows protected your PC" warning → students may be afraid to install
- **Windows Defender** may flag as "potentially unwanted" → IT admins may block
- **AppLocker** policies that whitelist by publisher won't work → locked out of managed environments

With signing:
- SmartScreen shows publisher name → trusted
- Defender treats as verified → no false positives
- AppLocker publisher rules work → IT admins can whitelist

### Tauri Configuration

```json
// tauri.conf.json
{
  "bundle": {
    "windows": {
      "signCommand": "trusted-signing-cli -e %ENDPOINT% -a %ACCOUNT% -c %PROFILE% %1"
    }
  }
}
```

### CI Environment Variables

```env
AZURE_CLIENT_ID=<service-principal-client-id>
AZURE_CLIENT_SECRET=<service-principal-secret>
AZURE_TENANT_ID=<azure-tenant-id>
TRUSTED_SIGNING_ENDPOINT=<signing-endpoint-url>
TRUSTED_SIGNING_ACCOUNT=<account-name>
TRUSTED_SIGNING_PROFILE=<certificate-profile-name>
```

### Setup Steps

1. Create Azure account
2. Subscribe to Azure Trusted Signing ($9.99/month)
3. Create signing account + certificate profile
4. Create service principal for CI
5. Add credentials to GitHub Secrets
6. Add `signCommand` to `tauri.conf.json`

### EV vs Standard

Azure Trusted Signing provides **EV-equivalent** certificates. This means:
- Immediate SmartScreen reputation (no warmup period)
- Strongest trust signal
- Works with all AppLocker policies

---

## First-Run Experience

### Flow

```
1. User installs SmolPC CodeHelper (NSIS installer, ~500MB-2GB depending on model bundling)
2. User launches app for first time
3. App shows "Setting up..." screen with progress:
   a. "Checking hardware..." (RAM, GPU, NPU detection)
   b. "Setting up Python environment..." (uv installs Python, creates venvs)
      - 30-60 seconds with internet
      - Instant if offline fallback used
   c. "Loading AI model..." (loads bundled Tier 1 model)
      - 5-10 seconds for ONNX CPU
      - 30-60 seconds for NPU first load (blob compilation, cached after)
   d. If 16GB+ RAM: "A larger, more capable model is available. Download now?" (optional)
4. App shows main chat UI with mode dropdown
5. First message suggestion: "Try asking me to help with your homework!"
```

### Time Estimates

| Step | With Internet | Without Internet |
|------|--------------|-----------------|
| Python + venvs | 30-60 seconds | Instant (embedded fallback) |
| Model loading (CPU) | 5-10 seconds | 5-10 seconds |
| Model loading (NPU, first time) | 30-60 seconds | 30-60 seconds |
| Model loading (NPU, cached) | 5-10 seconds | 5-10 seconds |
| **Total first run** | **~1-2 minutes** | **~30-70 seconds** |

### Error Handling

- Python install fails → Use embedded fallback, show warning
- Model loading fails → Show error with "Report to IT" instructions
- No compatible hardware → Show minimum requirements
- DLL missing → Show "Reinstall" prompt

---

## School IT Compatibility

### Threat Model

School IT environments have various security policies that may block SmolPC:

| Policy | Impact | Mitigation |
|--------|--------|------------|
| **AppLocker** | May block unsigned executables | Code signing (Azure Trusted Signing) |
| **WDAC** (Windows Defender Application Control) | May block per-user installs | Provide per-machine MSI for IT |
| **Firewall** | May block uv's Python download | Bundle Python embeddable zip |
| **Antivirus** | PyInstaller binaries get false positives | Use uv + source Python (not PyInstaller) |
| **USB restrictions** | May block portable mode from USB | Use installed mode instead |
| **Internet restrictions** | No model downloads | Bundle model in installer |
| **Disk quotas** | Per-user disk limits (~2GB?) | Tier 1 only (~1.5GB total) |

### Deployment Options

#### Option 1: Student Self-Install (Default)

- Download installer from school's internal software portal
- Run installer (no admin needed with `currentUser` mode)
- App installs to `%AppData%`
- First-run setup handles Python + models

#### Option 2: IT Admin Deployment (SCCM/Intune)

```cmd
# Silent per-machine install
SmolPC-CodeHelper-Setup.exe /S /D=C:\Program Files\SmolPC CodeHelper

# Or provide separate MSI for Group Policy deployment
msiexec /i SmolPC-CodeHelper.msi /qn ALLUSERS=1
```

#### Option 3: Portable Mode (USB)

See [Portable Mode](#portable-mode) section.

### Recommended IT Communication

Provide IT admins with:
1. **Signing certificate hash** — for AppLocker publisher rules
2. **Network requirements** — port 19432 (localhost only), optional internet for model downloads
3. **Disk space requirements** — 1.5GB (Tier 1) or 2.5-4.5GB (Tier 2)
4. **Privacy statement** — no telemetry, no cloud, all data local, GDPR/FERPA compliant
5. **Silent install guide** — SCCM/Intune deployment instructions

---

## Update Mechanism

### Tauri Updater Plugin

Tauri 2 has a built-in updater plugin:

```json
// tauri.conf.json
{
  "plugins": {
    "updater": {
      "active": true,
      "dialog": true,
      "endpoints": [
        "https://releases.smolpc.org/{{target}}/{{arch}}/{{current_version}}"
      ],
      "pubkey": "<ed25519-public-key>"
    }
  }
}
```

### Update Components

| Component | Update Method | Frequency |
|-----------|--------------|-----------|
| Tauri app | Tauri updater (delta or full) | On release |
| Engine host | Bundled with app update | On release |
| Runtime DLLs | Bundled with app update (if version changes) | Rare |
| Models | Manual download or in-app check | On model release |
| MCP servers | `uv sync` from bundled source | On app update |
| Python | `uv python install` (if newer needed) | Rare |

### Offline Updates

For schools without internet:
- Download update package on an internet-connected machine
- Transfer via USB or network share
- Run installer over existing installation (NSIS handles in-place upgrade)

---

## Portable Mode

### Concept

Run SmolPC CodeHelper from a USB drive without installation.

### Implementation

```
USB Drive (E:\)
└── SmolPC-Portable\
    ├── SmolPC-CodeHelper.exe     # Launcher (sets APPDATA override)
    ├── bin\                      # Same as installed layout
    ├── runtimes\
    ├── models\
    ├── mcp-servers\
    ├── python\
    ├── data\                    # User data stays on USB
    └── portable.marker          # Presence of this file enables portable mode
```

### Portable Launcher

The launcher detects `portable.marker` and overrides data directories:

```rust
if Path::new("portable.marker").exists() {
    std::env::set_var("SMOLPC_DATA_DIR", "./data");
    std::env::set_var("SMOLPC_CACHE_DIR", "./cache");
}
```

### Caveats

- **USB execution may be blocked** by school IT policies
- **Performance:** USB 2.0 is too slow for model loading. USB 3.0+ recommended.
- **Size:** Full portable install with Tier 1 model is ~1.5-2GB. Fits on most USB drives.

---

## Data Protection

### Compliance Framework

SmolPC is designed for **UK secondary schools** and must comply with:

| Regulation | Requirement | How SmolPC Complies |
|-----------|------------|-------------------|
| **UK GDPR** | Data minimization, purpose limitation | Offline-first — no data leaves device |
| **UK GDPR** | 72-hour breach notification | No remote data = no breach surface |
| **UK GDPR** | DPIA for AI processing | Document: local inference, no profiling |
| **UK GDPR** | DPO required for schools | School's responsibility, not ours |
| **FERPA** | Parental consent for students <18 | No data collection = no consent needed |
| **FERPA** | 45-day data access response | Data is on student's device already |

### Data Stored Locally

| Data Type | Location | Deletable | Sensitive |
|-----------|----------|-----------|-----------|
| Chat conversations | `data/conversations/` | Yes | Low (student prompts/responses) |
| Engine state | `data/engine.db` | Yes | No |
| Model cache | `cache/npu-blobs/` | Yes | No |
| Python environments | `python/` | Yes | No |
| Logs | `logs/` | Yes | Low (may contain prompts) |
| User settings | `config/settings.json` | Yes | No |

### Privacy Controls

1. **Clear conversation history** — Button in settings, deletes `data/conversations/`
2. **Clear all data** — Button in settings, deletes `data/`, `cache/`, `logs/`
3. **No telemetry** — No analytics, no crash reports, no usage tracking
4. **No accounts** — No sign-in, no cloud sync
5. **No network** — Engine runs on localhost only (port 19432, bound to 127.0.0.1)

### Strongest Compliance Position

**Offline-first design is the strongest possible data protection position.** When no data leaves the device:
- No data processor agreements needed
- No cross-border transfer issues
- No data breach notification obligations (no remote data to breach)
- Minimal DPIA scope (local processing only)
- Students own their data completely

---

## Disk Footprint

### Size Estimates

| Component | Size | Notes |
|-----------|------|-------|
| Tauri app binary | ~30MB | Unified assistant |
| Engine host binary | ~15MB | Axum HTTP server |
| ONNX Runtime DLLs | ~150-200MB | onnxruntime.dll + onnxruntime_genai.dll + DirectML.dll |
| OpenVINO DLLs | ~300-400MB | openvino.dll + openvino_genai.dll + dependencies |
| uv.exe | ~35MB | Python package manager |
| MCP server source | ~5MB | Python source code |
| Model (Tier 1, INT4) | ~1-2GB | Qwen3-1.7B or similar |
| Model (Tier 2, INT4) | ~3-6GB | Qwen3-4B or larger |
| Python + venvs | ~50-100MB | Auto-installed by uv |
| NPU blob cache | ~100-500MB | Generated at runtime, varies |
| **Total (Tier 1)** | **~1.5-2.3GB** | Fits on most systems |
| **Total (Tier 2)** | **~3.5-7.0GB** | For 16GB+ systems |

### Installer Download Size

The installer is smaller than installed size (compressed):
- **Slim installer** (no model): ~500-700MB
- **Full installer** (Tier 1 model): ~1.2-1.5GB
- **Everything installer** (Tier 1 + Tier 2): ~3-5GB

### Disk Space Validation

Installer should check available disk space before proceeding:
- Minimum: 2GB free (Tier 1)
- Recommended: 5GB free (Tier 1 + Tier 2 + cache)

---

## Build Pipeline

### Local Development Build

```bash
# Frontend
npm install
npm run dev        # Svelte dev server

# Backend
cd src-tauri
cargo build        # Debug build (no DLLs needed for compilation)

# Full app
npm run tauri dev   # Combined hot-reload
```

### Production Build

```bash
# Prerequisites:
# 1. Runtime DLLs in src-tauri/runtimes/
# 2. Model files in src-tauri/resources/models/ (if bundling)
# 3. uv binary in src-tauri/bin/uv-x86_64-pc-windows-msvc.exe
# 4. Code signing configured (see Code Signing section)

npm run tauri build
```

### CI/CD (GitHub Actions)

```yaml
name: Build & Release
on:
  push:
    tags: ['v*']

jobs:
  build-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 20

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install frontend deps
        run: npm ci

      - name: Download runtime DLLs
        run: |
          # Download pre-built DLLs from release artifacts or Azure storage
          ./scripts/download-runtimes.ps1

      - name: Download uv binary
        run: |
          curl -L https://github.com/astral-sh/uv/releases/latest/download/uv-x86_64-pc-windows-msvc.zip -o uv.zip
          Expand-Archive uv.zip -DestinationPath src-tauri/bin/

      - name: Download model (if bundling)
        run: |
          # Download pre-quantized model from HuggingFace
          ./scripts/download-model.ps1 qwen3-1.7b-int4

      - name: Build Tauri app
        env:
          AZURE_CLIENT_ID: ${{ secrets.AZURE_CLIENT_ID }}
          AZURE_CLIENT_SECRET: ${{ secrets.AZURE_CLIENT_SECRET }}
          AZURE_TENANT_ID: ${{ secrets.AZURE_TENANT_ID }}
        run: npm run tauri build

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: SmolPC-CodeHelper-Windows
          path: src-tauri/target/release/bundle/nsis/*.exe
```

### Build Artifacts

After `npm run tauri build`:
```
src-tauri/target/release/bundle/
├── nsis/
│   ├── SmolPC-CodeHelper-Setup.exe    # NSIS installer
│   └── SmolPC-CodeHelper-Setup.exe.sig # Update signature
└── msi/ (if configured)
    └── SmolPC-CodeHelper.msi
```
