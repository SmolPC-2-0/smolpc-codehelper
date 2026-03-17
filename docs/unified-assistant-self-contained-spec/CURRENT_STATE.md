# Current State

**Last Updated:** 2026-03-17
**Status:** Demo line frozen; self-contained delivery line opened

## 1. Branch State

### Frozen demo baseline

| Branch                        | Role                         | Frozen Head                                |
| ----------------------------- | ---------------------------- | ------------------------------------------ |
| `dev/unified-assistant`       | Demo implementation baseline | `ad31a8e92419557cda9e7e0eb560d18e1c065a54` |
| `docs/unified-assistant-spec` | Demo/spec baseline           | `78412914cacbad183db70cb76eaa541352a55c8c` |

### Self-contained line

| Branch                                       | Role                        |
| -------------------------------------------- | --------------------------- |
| `dev/unified-assistant-self-contained`       | New implementation mainline |
| `docs/unified-assistant-self-contained-spec` | New docs/spec mainline      |

### Freeze tags

- `demo/unified-assistant-freeze-2026-03-17`
- `demo/unified-assistant-spec-freeze-2026-03-17`

## 2. What The Frozen Demo Baseline Already Does

The frozen demo line already provides:

- one unified Tauri app
- six visible modes:
  - Code
  - GIMP
  - Blender
  - Writer
  - Calc
  - Slides
- live Code mode on the existing inference path
- live GIMP mode
- live Blender mode
- live Writer mode
- live Slides mode
- intentionally deferred Calc
- unified branding
- no launcher dependency

This baseline is good for demos and architectural reference.

## 3. Why The Demo Line Is Not The Shipping Finish Line

The current unified app is not yet self-contained for external users.

### Remaining external/manual setup in the demo line

| Area                          | Current demo state                                   | Self-contained target                           |
| ----------------------------- | ---------------------------------------------------- | ----------------------------------------------- |
| Engine                        | App-owned and auto-started                           | Keep                                            |
| Models                        | Not guaranteed bundled for external install          | Bundle one default model                        |
| LibreOffice Python            | External Python still assumed in packaged mode       | Bundle app-private Python                       |
| LibreOffice runtime bootstrap | Runtime is app-launched but not fully self-contained | Keep app-owned, remove system Python dependency |
| Blender addon                 | External manual addon install/enable                 | Auto-provision from bundled resources           |
| Blender launch                | External manual app launch                           | Detect and launch automatically                 |
| GIMP plugin/server            | External manual plugin/server setup                  | Bundle and provision automatically              |
| GIMP launch                   | External manual app launch                           | Detect and launch automatically                 |

## 4. Locked Self-Contained Finish Line

The self-contained delivery line must ship:

- one installed Windows app: `SmolPC Unified Assistant`
- no manual installation of:
  - Python
  - MCP servers
  - Blender addon
  - GIMP plugin/server
  - model files
- host apps may remain separate installs:
  - GIMP
  - Blender
  - LibreOffice / Collabora
- the unified app auto-detects and launches host apps on demand
- bundled default model: `qwen3-4b-instruct-2507`
- live modes at finish line:
  - Code
  - GIMP
  - Blender
  - Writer
  - Slides
- Calc remains deferred and disabled

## 5. Source Ownership Summary

| Mode family | Current source status                                             | Self-contained ownership direction                          |
| ----------- | ----------------------------------------------------------------- | ----------------------------------------------------------- |
| Code        | Already owned in `apps/codehelper`                                | Keep                                                        |
| LibreOffice | Runtime scripts already imported into unified resources           | Replace system Python dependency with bundled Python        |
| Blender     | Bridge already owned by unified app; addon still external         | Bundle and provision addon from repo source                 |
| GIMP        | Unified provider exists, but runtime/plugin ownership is external | Vendor pinned upstream `gimp-mcp` snapshot and provision it |

## 6. Step 1 Status

Step 1 of the self-contained plan is this docs line:

- branch cut and freeze policy documented
- new mainlines established
- self-contained master plan documented
- architecture, packaging, model, and provenance rules documented

No implementation branch should open on the self-contained line until this
docs baseline is merged into both new mainlines.

## 7. Next Official Branch

After this docs baseline merges through the new mainlines, the next branch is:

- `codex/unified-self-contained-foundation-docs`

That phase establishes:

- setup/provisioning subsystem
- host-app detection
- resource manifests
- bundled Python ownership
- bundled default model ownership
- startup orchestration

## 8. Known Risks

| Risk                   | Why it matters                                                                                                   |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------- |
| GIMP ownership gap     | GIMP is the furthest from self-contained because the current unified line does not own the plugin/server runtime |
| Third-party provenance | Bundling external runtime assets without exact pin/license tracking would create release risk                    |
| Windows packaging size | Bundled model plus Python/runtime assets will increase installer size materially                                 |
| Host-app variance      | Blender and LibreOffice install locations vary across user machines                                              |
| Calc expectation drift | Users may assume all LibreOffice modes are live; docs and UI must keep Calc explicitly deferred                  |
