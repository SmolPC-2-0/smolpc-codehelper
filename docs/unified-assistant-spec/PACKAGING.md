# Packaging And Distribution

**Last Updated:** 2026-03-16
**Status:** Packaging baseline for the unified app

## 1. Packaging Direction

The shipping target is **one unified Windows desktop app** built from
`apps/codehelper`.

The launcher is not required for:

- installation
- runtime ownership
- app switching
- engine management

If a launcher exists elsewhere in the repository, it is outside the packaging
scope of the unified frontend.

## 2. Target Platform

- Windows 10/11 primary
- current-user installation
- no admin rights assumed
- offline-first deployment

## 3. Installed Layout

```text
%LOCALAPPDATA%/Programs/SmolPC Unified Assistant/
  SmolPC Unified Assistant.exe
  resources/
    models/
    libs/
    mcp/
      gimp/
      libreoffice/
    blender/
  runtimes/
    engine-runtime/
      host-data/
```

## 4. Bundled Resource Categories

| Resource group | Needed for | Notes |
|---|---|---|
| Engine runtime bundle | all modes | shared engine startup and backend runtime selection |
| Models | all modes | shared model discovery |
| GIMP provider assets | GIMP | provider-owned configuration or helper assets only; not the GIMP app itself |
| Blender bridge assets | Blender | bridge helpers and any bundled support files |
| LibreOffice MCP runtime | Writer/Calc/Slides | bundled provider runtime and support assets |

## 5. Resource Rules

### 5.1 Engine runtime

The unified app must bundle or resolve the resources needed by
`smolpc-engine-host` in both dev and packaged builds.

For clean-checkout development and CI, resource directories referenced by
Tauri must exist in git even when the real Windows runtime payload is not
checked in. In particular, `apps/codehelper/src-tauri/libs/openvino/` should
contain a tracked placeholder file such as `README.md`, while the actual
OpenVINO runtime DLLs continue to be staged by setup scripts.

### 5.2 Provider runtime separation

Provider assets must stay provider-owned:

- GIMP assets remain under a GIMP-specific bundled area
- Blender bridge assets remain under a Blender-specific bundled area
- LibreOffice assets remain under a LibreOffice-specific bundled area

This keeps packaging boundaries aligned with the provider architecture.

### 5.3 No launcher-owned runtime paths

Do not require packaged resources to live under a launcher-specific directory.
The unified app must be self-sufficient.

## 6. Windows-Only Validation

The packaging plan is valid only after Windows verification covers:

1. unified app startup
2. engine startup and health
3. model discovery
4. GIMP provider connection behavior
5. Blender provider connection behavior
6. LibreOffice provider connection behavior
7. packaged resource path resolution

## 7. Dev vs Packaged Resolution

### Development

- resources may resolve from the repository checkout
- tracked placeholder resource directories may exist only to satisfy clean
  Tauri build-time path checks
- external apps are launched separately for provider testing
- engine host may resolve from workspace `target/`

### Packaged

- unified app resolves provider and runtime assets from its own resources
- engine runtime is resolved from packaged runtime paths
- no launcher-owned indirection is assumed

## 8. Packaging Invariants

1. One packaged unified app binary.
2. One shared engine runtime.
3. Provider assets bundled by provider ownership, not launcher ownership.
4. Windows-only delivery target for this workstream.
5. No dependency on standalone app packaging to make the unified app function.

## 9. Validation Checklist

Before calling the packaging plan complete, verify:

- packaged app starts without launcher help
- shared engine starts and reports status
- runtime assets resolve correctly
- GIMP mode fails gracefully if GIMP is not installed or not running
- Blender mode fails gracefully if Blender bridge is unavailable
- Writer/Calc/Slides can each connect through the shared LibreOffice provider

## 10. Deferred Packaging Questions

These remain for later implementation phases:

- final Windows installer naming
- final model distribution approach
- whether some provider assets ship always or are staged optionally
- exact bundle layout for Blender supplementary tooling
