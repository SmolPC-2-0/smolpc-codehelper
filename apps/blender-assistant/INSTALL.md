# Installation Guide

This guide covers both normal development setup and self-contained release packaging for the current architecture (Rust backend + shared engine sidecar).

## 1) Requirements

### For running from source

- Node.js 18+
- Rust toolchain (`rustup`)
- Tauri platform prerequisites

### For optional model/bootstrap scripts

- Python 3.10+ (only for setup scripts, not runtime serving)

## 2) Run Locally (Developer Flow)

```bash
npm install
npm run runtime:setup
npm run model:setup:qwen2_5
npm run tauri:dev
```

`runtime:setup` runs `scripts/setup-libs.ps1` on Windows (`runtime:setup:sh` is available for Bash users).
`npm run tauri:dev` and `npm run tauri dev` both use wrappers that set `CARGO_TARGET_DIR`.

The app starts a local bridge at `127.0.0.1:5179` and attempts to use the shared engine backend.

## 3) Build Release

```bash
npm run tauri:build
```

Release artifacts are generated under `src-tauri/target/release/`.

## 4) Self-Contained Build (Bundle Model Assets)

If you want packaged model artifacts:

1. Prepare model files (optional but typical):

```powershell
npm run model:setup:qwen2_5
# or
npm run model:setup:qwen3
```

2. Stage model files:

```powershell
npm run bundle:stage:model
```

3. Build:

```powershell
npm run tauri:build:self-contained
```

See `Docs/ENGINE_BUNDLING_SETUP.md` for runtime library and sidecar details.

## 5) Blender Addon Installation

When the desktop app starts, it automatically syncs `blender_addon/blender_helper_http.py` into detected Blender user addon folders.

Then:

1. Open Blender -> Edit -> Preferences -> Add-ons
2. Search for `Blender Learning Assistant`
3. Enable the addon

The addon connects to the desktop app bridge on `127.0.0.1:5179`.

If Blender folders are not detected yet, install manually from `blender_addon/blender_helper_http.py`.

## 6) Troubleshooting

### Shared engine unavailable

- Verify the desktop app is running
- Check logs via app command `open_logs`
- Confirm bundled engine/runtime assets exist if using packaged mode:
  - `src-tauri/binaries/`
  - `src-tauri/libs/`

### Missing model artifacts

Run:

```powershell
npm run bundle:stage:model
```

Then rebuild.

### Forcing Ollama fallback

Set:

```powershell
$env:BLENDER_HELPER_ALLOW_OLLAMA_FALLBACK = "1"
```

Then restart the app. You can also set `BLENDER_HELPER_BACKEND=ollama` to force startup backend.
