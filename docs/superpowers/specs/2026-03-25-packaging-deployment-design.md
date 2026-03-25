# SmolPC Packaging & Deployment Design

## Context

SmolPC Code Helper needs a packaging strategy that supports two distribution paths:
1. **Online installer** — single NSIS exe (~300 MB), models downloaded on first run
2. **Offline USB bundle** — NSIS installer + pre-built model archives, 100% offline

Additionally, a **portable variant** (no install, run from folder) serves locked-down machines where the user lacks admin and SmartScreen may block installers.

### Requirements

- USB stick testing on various laptops with mixed admin access
- 3 offline bundle variants: Lite (Qwen 2.5 1.5B), Standard (Qwen 3 4B), Full (both)
- Smart first-run provisioning: detect local model archives or download from internet
- Hardware-aware model recommendation for the download path
- Code signing planned (design for signed builds)
- Updates not a concern during testing phase

### Constraints

- NSIS hard 2 GB installer limit — models cannot be inside the installer
- Total model payload: 900 MB to 7 GB depending on variant
- Target machines: 8-16 GB RAM, Intel NPU or NVIDIA discrete GPU, Windows 10/11
- Must work fully offline after provisioning (no cloud, no telemetry, no accounts)

---

## Distribution Artifacts

The build system produces 5 artifacts from a single pipeline:

| Artifact | Contents | Size | Use Case |
|----------|----------|------|----------|
| `SmolPC_x.x.x_x64-setup.exe` | App + engine + DLLs + Python. No models. | ~300 MB | Online install |
| `SmolPC-Lite.zip` | Installer + Qwen 2.5 1.5B model archives (OpenVINO + DML) | ~2.5 GB | Offline USB, 8 GB RAM machines |
| `SmolPC-Standard.zip` | Installer + Qwen 3 4B model archives (OpenVINO + DML) | ~5.5 GB | Offline USB, 16 GB+ RAM machines |
| `SmolPC-Full.zip` | Installer + all model archives | ~7.5 GB | Offline USB, test everything |
| `SmolPC-Portable.zip` | Pre-extracted app folder + Qwen 2.5 1.5B models (ready to run) | ~1.5 GB | Locked-down machines |

---

## First-Run Provisioning Flow

When the app launches and finds no models in `%LOCALAPPDATA%\SmolPC\models\`, it enters provisioning mode.

### Step 1 — Source Detection (automatic, <1 second)

The app checks for model archives in order:

1. **NSIS breadcrumb** — during install, NSIS writes `$EXEDIR` to `%LOCALAPPDATA%\SmolPC\installer-source.txt`. If that path still has a `models/` folder with a valid `model-archives.json`, use it.
2. **Removable drives** — scan mounted drives for `SmolPC*/models/model-archives.json` (USB reinserted or breadcrumb stale).
3. **Internet** — if no local source found, offer download from HuggingFace.

### Step 2 — Model Selection

| Source | Behavior |
|--------|----------|
| **Local (USB/folder)** | Extracts ALL archives found. Bundle variant already determined what's included. Shows: *"Installing AI models from local media..."* |
| **Internet** | Hardware probe runs, recommends one model+backend combo. User sees recommendation with "Download" button. Advanced dropdown for alternate selection. |

Hardware-based recommendation table:

| Hardware | RAM | Model | Download Size |
|----------|-----|-------|--------------|
| Intel NPU | 16 GB+ | Qwen 3 4B OpenVINO | ~2.2 GB |
| Intel NPU | <16 GB | Qwen 2.5 1.5B OpenVINO | ~900 MB |
| NVIDIA discrete GPU | 16 GB+ | Qwen 3 4B DirectML | ~2.9 GB |
| NVIDIA discrete GPU | <16 GB | Qwen 2.5 1.5B DirectML | ~1.3 GB |
| CPU only | any | Qwen 2.5 1.5B OpenVINO | ~900 MB |

### Step 3 — Extract/Download with Progress

Full-screen Svelte UI with:
- Per-archive progress bar (bytes done / total bytes)
- Current file name, speed, ETA (for downloads)
- SHA256 verification after each archive
- "Cancel" button that cleans up partial files

Progress streamed from Rust via `tauri::ipc::Channel<ProvisioningEvent>`.

### Step 4 — Transition

Extraction completes -> engine starts -> model loads -> app transitions to normal chat UI. No restart needed.

### Error Handling

- USB removed mid-extraction: pause, prompt to reinsert, resume
- Download interrupted: resume from last byte (HTTP range request)
- Disk full: clear error message with required space
- Checksum mismatch: delete corrupt file, retry once, then error with "Re-download" button

---

## Tauri App Architecture

### New Module: `app/src-tauri/src/provisioning/`

```
app/src-tauri/src/provisioning/
  mod.rs              # Public API: Tauri commands
  source.rs           # Source detection (breadcrumb, drive scan, internet)
  extractor.rs        # ZIP extraction with progress callbacks
  downloader.rs       # HTTP download with resume + range requests
  manifest.rs         # Parse model-archives.json, SHA256 verification
```

Note: Hardware detection reuses the existing `app/src-tauri/src/hardware/` module (`HardwareCache`, `detect_hardware` command). No new hardware probe — the provisioning module calls the existing detection infrastructure to get GPU/RAM info for model recommendation. This respects zone ownership: the app's hardware detector provides display-level info, the engine owns backend selection policy.

### Tauri Commands

```rust
#[tauri::command]
async fn detect_model_sources(app: AppHandle) -> Result<Vec<ModelSource>, ProvisioningError>;

#[tauri::command]
async fn get_recommended_model(app: AppHandle) -> Result<ModelRecommendation, ProvisioningError>;

#[tauri::command]
async fn provision_models(
    app: AppHandle,
    source: ModelSource,
    model_ids: Vec<String>,
    channel: Channel<ProvisioningEvent>,
) -> Result<ProvisioningResult, ProvisioningError>;

#[tauri::command]
async fn cancel_provisioning(app: AppHandle) -> Result<(), ProvisioningError>;
```

### ProvisioningEvent Enum

```rust
enum ProvisioningEvent {
    ArchiveStarted { name: String, total_bytes: u64 },
    Progress { bytes_done: u64, total_bytes: u64 },
    Verifying { name: String },
    ArchiveComplete { name: String },
    Error { code: ProvisioningErrorCode, message: String, retryable: bool },
    Complete { models_installed: Vec<String> },
}

enum ProvisioningErrorCode {
    DiskFull,
    SourceUnavailable,  // USB removed, breadcrumb stale
    NetworkError,       // Download failed, timeout
    ChecksumMismatch,
    ExtractionFailed,
    Cancelled,
}
```

### model-archives.json Schema

The provisioning module parses this manifest (produced by `build-*-model-archives.ps1`):

```json
{
  "version": 1,
  "models": [
    {
      "id": "qwen2.5-1.5b-instruct",
      "backend": "openvino",
      "archive_name": "qwen2.5-1.5b-instruct-openvino.zip",
      "archive_path": "models/qwen2.5-1.5b-instruct-openvino.zip",
      "sha256": "abcdef1234..."
    }
  ]
}
```

### Frontend

New `SetupWizard.svelte` component shown when no models are detected. Calls Tauri commands, listens to Channel, renders progress UI. On `Complete`, triggers engine startup via existing `EngineSupervisor` flow and transitions to main view.

### Existing Code Changes

- `app/src-tauri/src/main.rs` — register new commands, add startup model presence check
- `app/src-tauri/src/engine/supervisor.rs` — add portable mode awareness to `resolve_models_dir()` and `SpawnPaths`: when `is_portable()`, set `SMOLPC_MODELS_DIR` to `exe_dir/models/` and do not pass `resource_dir` to the engine (so it falls back to exe-relative DLL resolution)
- New NSIS hook file (`app/src-tauri/nsis/hooks.nsh`) — write breadcrumb during install. Path is relative to `src-tauri/` directory.
- All HTTP requests for model downloads go through the Rust backend (`downloader.rs`), never from the WebView — so no CSP changes needed

### No Engine Changes Required

- Engine code — still discovers models via `SMOLPC_MODELS_DIR` env var (set by supervisor) or its own fallback chain
- `runtime_bundles.rs` — DLL resolution unchanged; the exe-relative fallback (`exe_parent/libs/`) activates automatically when `resource_dir` is `None`, which the supervisor controls in portable mode

Note: The `EngineSupervisor` IS modified for portable mode (see Portable Mode section below), but provisioning does not interact with the supervisor — provisioning completes before engine startup.

---

## NSIS Installer Configuration

### Hook File: `app/src-tauri/nsis/hooks.nsh`

```nsis
!macro NSIS_HOOK_POSTINSTALL
  ; Write breadcrumb so the app knows where the installer was launched from
  ; $EXEDIR = directory containing the .exe (e.g., E:\SmolPC-Lite\)
  FileOpen $0 "$LOCALAPPDATA\SmolPC\installer-source.txt" w
  FileWrite $0 "$EXEDIR"
  FileClose $0
!macroend
```

### tauri.conf.json Addition

```json
{
  "bundle": {
    "windows": {
      "nsis": {
        "installerHooks": "nsis/hooks.nsh"
      }
    }
  }
}
```

### Code Signing

When certificate is available, add `signCommand` to `tauri.conf.json`:

```json
{
  "bundle": {
    "windows": {
      "signCommand": "signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /f cert.pfx /p %CERT_PASSWORD% %1"
    }
  }
}
```

Exact command depends on certificate type (PFX, hardware token, Azure Artifact Signing).

### SmartScreen (Unsigned Builds)

- Default "Warn" mode: user clicks "More info" then "Run anyway" (two extra clicks)
- "Block" mode on locked-down machines: installer is blocked; use portable variant
- Once signed: warnings disappear after certificate builds reputation (days to weeks)
- Azure Artifact Signing ($10/mo) skips the reputation period

---

## Build Pipeline

### Script: `app/scripts/build-release.ps1`

```
Usage:
  .\build-release.ps1                    # Online-only installer
  .\build-release.ps1 -Variant Lite      # Installer + 1.5B models
  .\build-release.ps1 -Variant Standard  # Installer + 4B models
  .\build-release.ps1 -Variant Full      # Installer + all models
  .\build-release.ps1 -Variant Portable  # Pre-extracted portable folder
```

### Build Steps

1. **Stage runtimes** — run existing `setup-directml-runtime.ps1`, `setup-openvino-runtime.ps1`, `setup-bundled-python-runtime.ps1` (idempotent)
2. **Stage engine sidecar** — run existing `stage-engine-sidecar.ps1` (release build)
3. **Build NSIS installer** — `npm run tauri build` produces `SmolPC_x.x.x_x64-setup.exe`
4. **Sign** — if certificate is configured, sign the installer
5. **Variant-specific packaging** — assemble USB folder or portable folder, ZIP

### Offline USB Layout

```
SmolPC-Lite/
  SmolPC_x.x.x_x64-setup.exe              # Signed NSIS installer (~300 MB)
  models/
    model-archives.json                     # Manifest with SHA256 checksums
    qwen2.5-1.5b-instruct-openvino.zip     # ~900 MB
    qwen2.5-1.5b-instruct-dml.zip          # ~1.3 GB
```

### Portable Layout

```
SmolPC-Portable/
  SmolPC 2.0.exe
  smolpc-engine-host.exe
  libs/                                     # ORT + DirectML DLLs
  libs/openvino/                            # OpenVINO DLLs
  python/                                   # Embedded CPython 3.12 + uv (for connectors)
  data/                                     # Created on first run: logs, config, engine PID
  models/                                   # Pre-extracted, ready to use
    qwen2.5-1.5b-instruct/
      openvino/
      dml/
```

The portable layout mirrors the installed layout but flattened — no Tauri `resources/` indirection. `libs/`, `python/`, and `models/` sit directly alongside the exe so that exe-relative path resolution works without Tauri's path resolver.

### Output Structure

```
dist/
  online/
    SmolPC_x.x.x_x64-setup.exe
  offline/
    SmolPC-Lite.zip
    SmolPC-Standard.zip
    SmolPC-Full.zip
  portable/
    SmolPC-Portable.zip
```

### Relationship to Existing Scripts

`build-release.ps1` replaces `build-windows-local-bundle.ps1` (415 lines) and `package-offline-bundle.ps1` (104 lines). It calls the existing `setup-*.ps1` and `build-*-model-archives.ps1` scripts as substeps. The two replaced scripts should be deleted after `build-release.ps1` is verified working — they should not coexist as it creates confusion about which script to use.

The existing `Install-CodeHelper.cmd` and `Install-Models.ps1` in the offline bundle output are no longer needed — the app's first-run provisioning replaces their functionality. However, `Install-Models.ps1` may still be useful as a standalone CLI tool for IT admins who want to pre-stage models without running the app.

---

## Portable Mode

### Detection

```rust
fn is_portable() -> bool {
    // Only check in release builds — dev builds may have models/ next to target/debug/
    if cfg!(debug_assertions) {
        return false;
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("models").exists()))
        .unwrap_or(false)
}
```

If `models/` exists next to the exe in a release build, it's portable mode. Debug builds always use installed mode to avoid false-positives from dev `models/` directories.

### Behavior Differences

| Concern | Installed | Portable |
|---------|-----------|----------|
| Model path | `%LOCALAPPDATA%\SmolPC\models\` | `exe_dir/models/` |
| Engine DLL resolution | `resource_dir/libs/` (Tauri resolver) | `exe_dir/libs/` (exe-relative fallback) |
| Engine spawn | Tauri sidecar path resolution | `exe_dir/smolpc-engine-host.exe` |
| App data (logs, config) | `%LOCALAPPDATA%\SmolPC\` | `exe_dir/data/` |
| Engine PID/token/logs | `%LOCALAPPDATA%\SmolPC\engine-runtime\` | `exe_dir/data/engine-runtime\` |
| First-run provisioning | Yes | Skipped (models present) |

### Engine Resolution Chain (Portable Mode)

The engine's path resolution depends on what the Tauri supervisor passes. The full chain for portable mode:

1. **Supervisor detects portable mode** via `is_portable()` at startup
2. **Supervisor does NOT pass `resource_dir`** to `EngineConnectOptions` — this is the key change. When `resource_dir` is `None`, the engine's `production_lib_root()` falls back to `exe_parent/libs/`
3. **Supervisor sets `SMOLPC_MODELS_DIR`** to `exe_dir/models/` in the spawn environment
4. **Supervisor sets `shared_runtime_dir`** to `exe_dir/data/engine-runtime/` for PID file, token, and logs

This requires changes to `supervisor.rs`:
- `resolve_models_dir()` — return `exe_dir/models/` in portable mode
- `spawn_engine()` — omit `resource_dir` from engine options in portable mode
- `shared_runtime_dir` — return `exe_dir/data/engine-runtime/` in portable mode

Engine code itself requires **zero changes** — it already supports both `resource_dir`-based and exe-relative resolution.

### Limitations

- No Start Menu shortcut, no uninstaller, no file associations
- No auto-update path
- WebView2 must already be installed on the machine. WebView2 was distributed to Windows 10 via Windows Update starting late 2021 and ships with all Windows 11 installations. Older Windows 10 builds (pre-late-2021 updates) may not have it — the portable README should note this requirement.

---

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| NSIS 2 GB limit hit if DLL payload grows | Low | Blocks release | Monitor installer size in build script; fail if >1.5 GB |
| USB removed mid-extraction | Medium | Partial files | Extract to temp dir, atomic rename on completion |
| HuggingFace download interrupted | Medium | Frustrating UX | HTTP range-request resume, progress persisted to disk |
| SmartScreen blocks installer on locked-down machine | High | Can't install | Portable variant as fallback |
| Disk space insufficient for extraction | Medium | Extraction fails | Pre-check space; clear error with required vs available |
| Model checksum mismatch | Low | Model won't load | SHA256 verify, re-download/re-copy option |
| Portable on network drive | Low | Slow/broken | Detect network path, warn user |
| WebView2 missing (portable only) | Low | App won't start | Ships with Win10 1803+; document in README |
