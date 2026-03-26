# Model Installer Resources

This directory contains resources bundled into the Tauri app installer for model management.

- `model-archives.example.json` — Example manifest format for offline model archives
- `model-archives.json` — Generated at build time by the packaging scripts (not checked in)

The actual model archives are distributed alongside the app installer in the offline bundle,
not inside the Tauri package itself.
