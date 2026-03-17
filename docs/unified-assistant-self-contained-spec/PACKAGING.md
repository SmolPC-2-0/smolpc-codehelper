# Packaging And Distribution For The Self-Contained Line

**Last Updated:** 2026-03-17
**Status:** Packaging target with Phase 2 foundation contract and Phase 3 LibreOffice bundled-Python ownership landed

## 1. Packaging Direction

Ship one Windows installer for `SmolPC Unified Assistant`.

The installer may assume the host apps are installed separately:

- GIMP
- Blender
- LibreOffice / Collabora

The installer must not assume the user will separately install:

- Python
- MCP servers
- Blender addon
- GIMP plugin/server
- model artifacts

## 2. Visible Product Identity

- product name: `SmolPC Unified Assistant`
- bundle identifier: `com.smolpc.codehelper`
- Windows only

## 3. Installed Layout

```text
%LOCALAPPDATA%/Programs/SmolPC Unified Assistant/
  SmolPC Unified Assistant.exe
  resources/
    models/
      qwen3-4b-instruct-2507/
    python/
    gimp/
    blender/
    libreoffice/
    libs/
  runtimes/
    engine-runtime/
      host-data/
```

Phase 2 adds the resource and manifest contract for `resources/models/` and
`resources/python/`, but it does not yet ship the final packaged payloads.

Phase 3 uses that contract for Writer and Slides:

- packaged-mode runtime startup now resolves the prepared bundled Python runtime
- the detected LibreOffice host path is injected into the bundled runtime
- final staged CPython and `uv` payloads still remain packaging-time inputs rather than committed git history

Phase 2 also adds tracked resource roots for:

- `resources/gimp/`
- `resources/blender/`
- `resources/libreoffice/`

## 4. Bundled Resource Categories

| Resource group              | Needed for                          | Notes                                       |
| --------------------------- | ----------------------------------- | ------------------------------------------- |
| Engine runtime bundle       | all live modes                      | shared engine startup                       |
| Default bundled model       | all live modes                      | `qwen3-4b-instruct-2507`                    |
| App-private Python runtime  | LibreOffice, GIMP runtime ownership | eliminates system Python dependency         |
| GIMP provider assets        | GIMP                                | bundled plugin/server payload and manifests |
| Blender provider assets     | Blender                             | addon payload, bridge helpers, manifests    |
| LibreOffice provider assets | Writer/Slides                       | bundled runtime scripts and manifests       |

## 4.1 Phase 3 Bundled Python Delivery

Phase 3 locks the bundled Python delivery source for Writer and Slides to:

- the official Windows x64 CPython embeddable distribution from `python.org`
- a pinned `uv` Windows binary from Astral for packaging/runtime management
- provider-owned wheel/runtime inputs staged into `resources/python/payload/`

Phase 3 packaged-mode rule:

- Writer and Slides must launch from the prepared bundled Python runtime only
- packaged mode must not fall back to system `python` or `python3`

## 5. Host-App Expectations

Still external at shipping time:

- GIMP install
- Blender install
- LibreOffice / Collabora install

Not external:

- runtime provisioning
- plugin/addon installation
- Python runtime
- model provisioning

Phase 2 stop-point:

- host-app detection becomes real
- host-app launch remains deferred
- plugin/addon provisioning remains deferred

Phase 3 live state:

- LibreOffice host detection becomes live for Writer and Slides
- the bundled LibreOffice runtime auto-launches LibreOffice on demand
- GIMP and Blender host-app launch remain deferred

## 6. Packaging Invariants

1. One installer.
2. One app binary.
3. One shared engine runtime.
4. One bundled default model.
5. One bundled Python runtime.
6. Provider assets bundled by provider ownership.
7. No launcher dependency.

## 7. Provisioning In Packaged Builds

Packaged builds must support:

- resolving bundled provider assets from app resources
- provisioning those assets into user-owned host-app profile locations
- version markers so upgrades can reapply only when required
- repair flow through setup commands/UI

Phase 2 establishes the manifest and staging-hook contract behind those goals:

- tracked resource manifests under each provider-owned resource root
- staging scripts for bundled model and bundled Python payloads
- setup commands that validate packaged resource presence without mutating host-app profiles

Those staging hooks now exist in the implementation line under:

- `apps/codehelper/scripts/self-contained/stage-bundled-model.mjs`
- `apps/codehelper/scripts/self-contained/stage-python-runtime.mjs`
- `apps/codehelper/scripts/self-contained/validate-resource-manifests.mjs`

## 8. Validation Checklist

Before calling the self-contained line ready, verify on Windows:

1. packaged app launches without launcher help
2. engine starts automatically
3. bundled default model resolves and loads
4. Writer and Slides use bundled Python only; no system Python is required in packaged mode
5. first Writer use launches runtime plus LibreOffice
6. first Slides use launches runtime plus LibreOffice
7. first Blender use provisions addon and launches Blender
8. first GIMP use provisions plugin/server and launches GIMP
9. Calc remains disabled
10. upgrade path preserves chats and reprovisions only when asset version changed

Phase 2 validation focuses on:

1. packaged resource manifests resolve honestly
2. setup status reports missing vs ready foundation items
3. setup prepare does not launch host apps
4. existing live mode behavior does not regress

## 9. Deferred Packaging Questions

Still deferred after Step 1:

- final installer naming/versioning policy
- whether to ship slim vs full installers later
- future optional model packs
- future Calc packaging requirements
