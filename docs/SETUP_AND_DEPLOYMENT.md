# Setup and Deployment

This document covers how SmolPC Code Helper gets from a build artifact to a working installation on a student's machine — the NSIS installer, first-run setup wizard, model provisioning, offline USB deployment, and runtime requirements.

## NSIS Installer

The desktop app ships as a Windows NSIS installer built by Tauri.

**Key properties:**

- **Install mode:** `currentUser` — no admin rights required. Installs to `%LOCALAPPDATA%\Programs\SmolPC Code Helper\`.
- **WebView2:** Bundled as an offline installer. The app uses WebView2 for the Svelte frontend, so it must be present. The offline bundling ensures no internet is needed.
- **VC++ Redistributable:** The NSIS pre-install hook checks for `vcruntime140.dll` and attempts a silent install of `vc_redist.x64.exe` if missing. This is non-fatal — the install continues even if the redist fails.
- **Breadcrumb:** The post-install hook creates `%LOCALAPPDATA%\SmolPC 2.0\` and writes `installer-source.txt` containing the directory the installer was launched from (`$EXEDIR`). This tells the app where to find model archives on a USB drive.

**Bundled resources:** The installer packages these directories:

| Source | Installed as |
|---|---|
| `libs/` | `libs/` (OpenVINO + ORT DLLs) |
| `binaries/` | `binaries/` (engine sidecar) |
| `resources/models/` | `models/` (model manifests) |
| `resources/python/` | `python/` (bundled Python 3.12 runtime) |
| `connectors/gimp/resources/` | `gimp/` (plugin + bridge + upstream) |
| `connectors/blender/resources/` | `blender/` (addon + RAG index) |
| `connectors/libreoffice/resources/` | `libreoffice/` (MCP server) |
| `resources/launcher/` | `launcher/` (app manifest) |

## First-Run Setup Wizard

When the app launches for the first time, it checks whether AI models are provisioned.

### Boot State Check

`App.svelte` calls the `get_boot_state` Tauri command, which returns:

- `models_provisioned` — true if `%LOCALAPPDATA%\SmolPC 2.0\models\` contains at least one model subdirectory
- `portable` — true if a `.portable` sentinel file exists next to the executable

The setup wizard is shown only if `!models_provisioned && !portable`. Portable mode (for pre-configured USB deployments) bypasses setup entirely.

### Setup Wizard Flow

1. **Source detection** — the wizard calls `detect_model_sources`, which scans for available model archives in priority order:
   - **Breadcrumb**: reads `installer-source.txt` for the USB/network path where the installer was launched. If the path exists and contains `models/model-archives.json`, it is offered as the first source.
   - **Drive scan** (Windows only): scans drives C:\ through Z:\ in parallel, with a 3-second timeout per drive. Looks for directories starting with `SmolPC*` that contain a manifest. Drives A: and B: are skipped (legacy floppy).
   - **Breadcrumb recovery**: if the breadcrumb path no longer exists (USB drive letter changed), tries the same relative path on other drive letters.
   - **Internet**: if HuggingFace API is reachable (HEAD request, 5-second timeout), an internet source is offered.

2. **Source selection** — the `SourceSelector` component displays available sources with labels. Local sources show the folder path. Internet sources show the recommended model name, download size, and reason (based on hardware detection). If no sources are found, the user sees an amber alert with retry/skip options.

3. **Hardware-aware recommendation** — `get_recommended_model` inspects the hardware cache:
   - 16 GB+ RAM → Qwen 3 4B
   - <16 GB RAM → Qwen 2.5 1.5B
   - Returns `model_id`, `backend`, `display_name`, `download_size_bytes`, and a human-readable `reason`

4. **Provisioning** — once the user selects a source, `provision_models` runs. Progress events stream to the frontend via a Tauri Channel:
   - `ArchiveStarted { name, total_bytes }` — a new archive is being processed
   - `Progress { bytes_done, total_bytes }` — extraction progress
   - `Verifying { name }` — SHA-256 checksum verification in progress
   - `ArchiveComplete { name }` — archive finished
   - `Error { code, message, retryable }` — failure with retry hint
   - `Complete { models_installed }` — all done

5. **Completion** — the wizard auto-transitions to the main app when provisioning completes. The app then calls `performStartup()` to spawn the engine.

### Provisioning Pipeline

The `provision_models` command follows this sequence:

1. **Singleton guard** — acquire a Windows global mutex (`Global\SmolPC-Provisioning`) to prevent concurrent provisioning across app instances. If the mutex is already held, return `AlreadyRunning`.
2. **Parse manifest** — read `models/model-archives.json` from the source directory. The manifest lists archive entries with `id`, `backend`, `archive_path`, and `sha256`.
3. **Filter** — if specific `model_ids` were requested, filter the manifest. An empty list means "install all."
4. **Disk space check** — archives require ~1.5x headroom for extraction. If insufficient, return `DiskFull` with available vs. required sizes.
5. **For each archive:**
   a. Check cancellation flag (atomic bool, checked at archive boundaries)
   b. Verify SHA-256 checksum (blocking task, 8 KB chunk reads)
   c. Extract to a temp directory (`target.with_extension("extracting")`)
   d. Strip common top-level directory prefix if present (prevents path doubling)
   e. Validate against zip-slip attacks (`enclosed_name()`)
   f. Atomic rename temp → final path (`%LOCALAPPDATA%\SmolPC 2.0\models\{id}\{backend}\`)
   g. If rename fails cross-device, fall back to recursive copy + delete
6. **Return** `ProvisioningResult` with installed model IDs and total bytes processed.

### Model Archives Manifest

The `model-archives.json` format:

```json
{
  "version": 1,
  "models": [
    {
      "id": "qwen2.5-1.5b-instruct",
      "backend": "dml",
      "archive_name": "qwen2.5-1.5b-instruct-dml.zip",
      "archive_path": "qwen2.5-1.5b-instruct-dml.zip",
      "sha256": "a1b2c3..."
    },
    {
      "id": "qwen2.5-1.5b-instruct",
      "backend": "openvino",
      "archive_name": "qwen2.5-1.5b-instruct-openvino.zip",
      "archive_path": "qwen2.5-1.5b-instruct-openvino.zip",
      "sha256": "d4e5f6..."
    }
  ]
}
```

A single model can have multiple entries (one per backend). The engine discovers models at runtime by scanning `%LOCALAPPDATA%\SmolPC 2.0\models\{model_id}\{backend}\` for backend-specific files (`.xml`+`.bin` for OpenVINO IR, `.onnx` for DirectML).

### Cancel and Retry

- **Cancel**: sets an `AtomicBool` flag checked at archive boundaries. Partial extractions are cleaned up. Partial downloads (internet source) are kept for HTTP Range resume.
- **Retry**: the user can retry from the error screen. The provisioning pipeline restarts, skipping archives that were already successfully extracted.
- **Skip**: the user can skip setup entirely. The app launches without AI capabilities — the engine will fail to load a model, and the UI shows appropriate messaging.

## USB Offline Deployment

The primary deployment target is schools without internet. A teacher or IT admin prepares a USB drive, and students install by double-clicking a script.

### Bundle Structure

```
SmolPC-Full/
  SmolPC Code Helper_1.0.0_x64-setup.exe
  Install-CodeHelper.cmd
  Install-CodeHelper.ps1
  models/
    model-archives.json
    SHA256SUMS.txt
    Install-Models.ps1
    qwen2.5-1.5b-instruct-dml.zip
    qwen2.5-1.5b-instruct-openvino.zip
    qwen3-4b-openvino.zip
```

### Installation Flow

1. Student double-clicks `Install-CodeHelper.cmd`
2. The CMD wrapper launches `Install-CodeHelper.ps1` via PowerShell
3. The PowerShell script:
   a. Runs the NSIS installer silently (`/S` flag) — installs to `%LOCALAPPDATA%\Programs\SmolPC Code Helper\`
   b. The installer's post-install hook writes `installer-source.txt` pointing back to the USB drive
   c. Extracts model archives to `%LOCALAPPDATA%\SmolPC 2.0\models\`
   d. Launches the app
4. The app detects `models_provisioned = true`, skips the setup wizard, and starts the engine

No admin rights required. No internet required.

### Bundle Variants

Three pre-built variants are produced by the build pipeline:

| Variant | Models | USB Size | Target Hardware |
|---|---|---|---|
| Full | Qwen 2.5 1.5B + Qwen 3 4B | ~8 GB | Mixed classrooms |
| Lite | Qwen 2.5 1.5B only | ~3 GB | 8 GB RAM machines |
| Standard | Qwen 3 4B only | ~5 GB | 16 GB+ RAM machines |

### Creating Custom Bundles

The build pipeline (`app/scripts/build-windows-local-bundle.ps1`) runs 5 phases:

1. **Engine sidecar** — compile the engine binary
2. **DirectML model archives** — package ONNX models with genai config
3. **OpenVINO model archives** — package IR models with manifests
4. **Tauri NSIS build** — compile the desktop app installer
5. **Bundle assembly** — merge manifests, checksums, and scripts into the offline bundle directory

To create a bundle with only specific models, modify the model lists in phases 2-3 or build a custom `model-archives.json`.

## Portable Mode

If a `.portable` sentinel file exists next to the executable, the app enters portable mode:

- Skips the setup wizard entirely
- Does not write to `%LOCALAPPDATA%`
- Expects models and runtimes to be co-located with the executable

This is for pre-configured deployments where everything is already in place (e.g., a teacher's USB with the app and models pre-extracted).

## Runtime Provisioning

Beyond AI models, the app provisions several runtimes during setup:

### OpenVINO DLLs

14-15 DLLs bundled in `libs/openvino/`. Loaded at runtime by `runtime_loading.rs` in strict dependency order. Includes TBB, OpenVINO core, CPU plugin, NPU plugin (optional), GenAI, and tokenizers. Version-pinned to OpenVINO 2026.0.0.

### DirectML / ORT DLLs

4 DLLs bundled in `libs/`: `onnxruntime.dll`, `onnxruntime_providers_shared.dll`, `onnxruntime-genai.dll`, `DirectML.dll`. Loaded at runtime for DirectML GPU inference.

### Python Runtime

Bundled Python 3.12 with `uv.exe` and `uvx.exe`. Staged from `resources/python/` to `%LOCALAPPDATA%\SmolPC 2.0\setup\python\` during first-run setup. Used by the GIMP bridge and LibreOffice MCP server. A manifest tracks the bundled version for idempotent re-staging.

### Connector Addons/Plugins

- **Blender**: `blender_helper_http.py` addon copied to Blender's addon directory and enabled via headless Blender execution
- **GIMP**: `gimp-mcp-plugin` directory copied to `%APPDATA%\GIMP\{version}\plug-ins\`
- **LibreOffice**: MCP server script (`mcp_server.py`) runs from bundled resources directory

Each has a provision marker (JSON file in `setup/state/`) that tracks the version and timestamp, enabling idempotent re-provisioning when the app updates.

## Setup Status System

The setup system tracks 8 items:

| Item | ID | Required | Auto-Prepare |
|---|---|---|---|
| Engine runtime | `engine_runtime` | Yes | No |
| Bundled model | `bundled_model` | Yes | No |
| Bundled Python | `bundled_python` | Yes | Yes |
| Blender host app | `host_blender` | No | No |
| GIMP host app | `host_gimp` | No | No |
| LibreOffice host app | `host_libreoffice` | No | No |
| Blender addon | `blender_addon` | No | Yes (if Blender detected) |
| GIMP plugin runtime | `gimp_plugin_runtime` | No | Yes (if GIMP detected) |

Overall state logic:

- **Error** — any required item has error state, or a previous error is cached
- **Needs Attention** — any required item is not ready
- **Ready** — all required items are ready, no errors

The setup store auto-prepares items that have `canPrepare = true` and are not yet ready. Host app detection results are cached to `setup/state/host-detection-cache.json` with atomic write (write to `.tmp`, backup to `.bak`, rename).

Mode availability in the UI is gated by setup items: GIMP mode requires `host_gimp` to be ready, Blender mode requires `host_blender`, and Writer/Impress require `host_libreoffice`. Unavailable modes appear dimmed in the mode dropdown with a reason message.

## Directory Layout

After full installation, the system uses these directories:

```
%LOCALAPPDATA%\Programs\SmolPC Code Helper\    # App install (NSIS)
  SmolPC Code Helper.exe                        # Main executable
  libs/                                         # Runtime DLLs
    openvino/                                   # OpenVINO DLLs (14-15 files)
    onnxruntime.dll                             # ORT DLLs
    ...
  resources/                                    # Bundled resources
    models/                                     # Model manifests
    python/                                     # Python runtime payload
    blender/                                    # Blender addon + RAG
    gimp/                                       # GIMP plugin + bridge
    libreoffice/                                # LibreOffice MCP server

%LOCALAPPDATA%\SmolPC 2.0\                     # App data
  installer-source.txt                          # USB breadcrumb (optional)
  models/                                       # Extracted AI models
    qwen2.5-1.5b-instruct/
      openvino/                                 # OpenVINO IR files
      dml/                                      # DirectML ONNX files
    qwen3-4b/
      openvino/
  engine-runtime/                               # Engine process state
    bridge-token.txt                            # Blender bridge auth token
  setup/
    python/                                     # Prepared Python runtime
    state/                                      # Setup markers and cache
      host-detection-cache.json
      blender-addon.json
      gimp-plugin-runtime.json
    logs/
```

## Troubleshooting

### Models not found

If the engine starts but cannot find a model:

1. Check `%LOCALAPPDATA%\SmolPC 2.0\models\` for model subdirectories
2. Each model needs a backend subdirectory (e.g., `openvino/`) containing the model files
3. OpenVINO models need `manifest.json` with `required_files` listing `.xml` and `.bin` files
4. Re-run the setup wizard or manually extract model archives to the correct path

### Backend not selected

If the engine falls back to CPU unexpectedly:

1. Check the engine spawn log for probe results (DXGI probe, NPU probe)
2. `SMOLPC_FORCE_EP=directml` or `SMOLPC_FORCE_EP=openvino_npu` forces a specific backend
3. DirectML requires a discrete GPU — Intel integrated GPUs produce garbage output and are rejected
4. NPU requires the Intel NPU driver (minimum version 32.0.100.3104 recommended)

### Engine won't start

1. Check for stale PID files in `%LOCALAPPDATA%\SmolPC 2.0\engine-runtime\`
2. Check if port 19432 is already in use (another engine instance or conflicting process)
3. Check `engine-spawn.log` for startup errors
4. The engine supervisor auto-restarts with backoff (1s → 2s → 4s), up to 3 restarts per 5-minute window

### GPU not detected

1. Verify a discrete GPU is present (Intel iGPU alone is not sufficient for DirectML)
2. The engine uses DXGI adapter enumeration (~14ms) — check the probe log for adapter names
3. WMI queries are not used (they hang on some Windows machines)
4. The app-side hardware detector does not detect GPUs (sysinfo limitation) — the engine's DXGI probe is authoritative

### Connector not connecting

1. Verify the host app is installed and detected (check setup panel)
2. For GIMP: must be GIMP 3.x (2.x is rejected). Check that the plugin was provisioned to the correct profile directory.
3. For Blender: the addon must be enabled in Blender's preferences. The bridge server runs on port 5179 — check for port conflicts.
4. For LibreOffice: the bundled Python runtime must be prepared. Check `setup/state/` for the provision marker.
