# Windows Source Test Results Template

Copy this into the issue, PR comment, or shared testing log for the current
`dev/unified-assistant-self-contained` functional validation pass.

```md
# Windows Source Test Report

- Tester:
- Date:
- Commit:
- Windows version:
- Clone path:

## Environment

- Node version:
- Rust version:
- PATH Python available:
- Repo `.venv` available:
- `SMOLPC_MODELS_DIR` configured:

## Host Apps

- GIMP version:
- Blender version:
- LibreOffice or Collabora version:

## Preflight

- `npm ci`:
- `npm run check`:
- `cargo test -p smolpc-code-helper`:
- `node apps/codehelper/scripts/self-contained/validate-resource-manifests.mjs`:
- `npm run model:setup:qwen3`:

## Launch

- `npm run tauri:dev`:
- Setup banner loaded:
- Setup panel loaded:

## Functional Results

- Code mode:
- GIMP mode:
- Blender mode:
- Writer mode:
- Slides mode:
- Calc remains disabled:

## Setup Notes

- `bundled_model` state:
- `bundled_python` state:
- `host_gimp` state:
- `gimp_plugin_runtime` state:
- `host_blender` state:
- `blender_addon` state:
- `host_libreoffice` state:

## Failures

- Exact failing step:
- Exact app detail text:
- Relevant terminal output:

## Overall Read

- Ready for broader Windows source testing:
- Blockers to fix before broader testing:
```
