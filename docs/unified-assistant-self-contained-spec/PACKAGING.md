# Packaging And Distribution For The Self-Contained Line

**Last Updated:** 2026-03-17
**Status:** Packaging target for self-contained external delivery

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

## 4. Bundled Resource Categories

| Resource group              | Needed for                          | Notes                                       |
| --------------------------- | ----------------------------------- | ------------------------------------------- |
| Engine runtime bundle       | all live modes                      | shared engine startup                       |
| Default bundled model       | all live modes                      | `qwen3-4b-instruct-2507`                    |
| App-private Python runtime  | LibreOffice, GIMP runtime ownership | eliminates system Python dependency         |
| GIMP provider assets        | GIMP                                | bundled plugin/server payload and manifests |
| Blender provider assets     | Blender                             | addon payload, bridge helpers, manifests    |
| LibreOffice provider assets | Writer/Slides                       | bundled runtime scripts and manifests       |

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

## 8. Validation Checklist

Before calling the self-contained line ready, verify on Windows:

1. packaged app launches without launcher help
2. engine starts automatically
3. bundled default model resolves and loads
4. no system Python is required
5. first Writer use launches runtime plus LibreOffice
6. first Slides use launches runtime plus LibreOffice
7. first Blender use provisions addon and launches Blender
8. first GIMP use provisions plugin/server and launches GIMP
9. Calc remains disabled
10. upgrade path preserves chats and reprovisions only when asset version changed

## 9. Deferred Packaging Questions

Still deferred after Step 1:

- final installer naming/versioning policy
- whether to ship slim vs full installers later
- future optional model packs
- future Calc packaging requirements
