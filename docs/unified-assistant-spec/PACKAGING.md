# Packaging And Distribution

**Last Updated:** 2026-03-17
**Status:** Phase 7 hardening is merged; v1 packaging baseline is closed out with Calc deferred

## 1. Packaging Direction

The shipping target is **one unified Windows desktop app** built from
`apps/codehelper`.

Phase 7 packaging decisions:

- visible packaged identity is `SmolPC Unified Assistant`
- Tauri `productName` and window title now use that name
- the bundle identifier remains `com.smolpc.codehelper` in Phase 7
- launcher-owned resources are removed from the unified app package
- Calc remains scaffold-only and does not block v1 packaging closeout

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

| Resource group          | Needed for         | Notes                                                                              |
| ----------------------- | ------------------ | ---------------------------------------------------------------------------------- |
| Engine runtime bundle   | all modes          | shared engine startup and backend runtime selection                                |
| Models                  | all modes          | shared model discovery                                                             |
| GIMP provider assets    | GIMP               | provider-owned configuration or helper assets only; not the GIMP app itself        |
| Blender bridge assets   | Blender            | bridge helpers and any bundled support files                                       |
| LibreOffice MCP runtime | Writer/Calc/Slides | bundled provider runtime for Writer/Slides in Phase 6B; Calc remains scaffold-only |

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

### 5.2.1 Phase 4 GIMP runtime rule

Phase 4 does not bundle GIMP itself and does not bundle the GIMP MCP plugin as
part of the unified app.

Phase 4 assumes:

- GIMP is installed separately
- the GIMP MCP server / plugin is installed separately
- the unified app connects to that external runtime over TCP

Phase 4 packaging validation covers connection to an external GIMP setup, not
auto-install or first-run provisioning of GIMP.

### 5.2.2 Phase 5 Blender runtime rule

Phase 5 does not bundle Blender itself and does not bundle the Blender addon as
part of the unified app.

Phase 5 assumes:

- Blender is installed separately
- the Blender addon is installed separately
- the unified app hosts the local bridge server used by the addon
- the unified app bundles only Blender-provider-owned assets such as retrieval
  metadata and bridge helper support files
- the current Phase 5 retrieval bundle is
  `apps/codehelper/src-tauri/resources/blender/rag_system/simple_db/metadata.json`
  because the unified Blender provider uses lightweight keyword retrieval and
  does not need vector-database assets in v1

Phase 5 packaging validation covers connection to an external Blender setup and
addon, not Blender installation or addon auto-provisioning.

### 5.2.3 Phase 6B LibreOffice runtime rule

Phase 6B imports and bundles the selected LibreOffice Python MCP runtime
assets under the unified app resource root:

- `apps/codehelper/src-tauri/resources/libreoffice/mcp_server/main.py`
- `apps/codehelper/src-tauri/resources/libreoffice/mcp_server/libre.py`
- `apps/codehelper/src-tauri/resources/libreoffice/mcp_server/helper.py`
- `apps/codehelper/src-tauri/resources/libreoffice/mcp_server/helper_utils.py`
- `apps/codehelper/src-tauri/resources/libreoffice/mcp_server/helper_test_functions.py`

Phase 6B assumes:

- imported runtime assets are pinned to
  `origin/codex/libreoffice-port-track-a` commit
  `7acad1fa0eb31e32a5485069e85c021d14284455`
- the unified app launches the runtime through the shared stdio MCP path
- the runtime contract remains:
  - stdio MCP child process via `main.py`
  - helper socket bridge on `localhost:8765`
  - headless office socket on `localhost:2002`
- the unified app does not bundle LibreOffice or Collabora itself
- the unified app does not add a LibreOffice settings UI in this phase
- runtime activation assumes an external LibreOffice or Collabora install plus
  Python 3 available on the target system

Phase 6B packaging validation covers the first real LibreOffice runtime
connection checks for Writer and Slides while Calc remains scaffold-only.

### 5.2.4 Phase 7 hardening and packaging rule

Phase 7 keeps the Phase 6B runtime addresses unchanged:

- helper socket bridge on `localhost:8765`
- headless office socket on `localhost:2002`

Phase 7 adds:

- authenticated helper traffic inside the imported LibreOffice runtime
- explicit helper message-size bounds and response validation
- explicit bundled LibreOffice runtime resources in Tauri config
- removal of launcher resources from the unified app bundle
- visible packaged branding aligned to `SmolPC Unified Assistant`
- shareable provider-side LibreOffice session ownership during tool execution

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

**Current recorded result:** the Phase 7 branch completed local compile/test
validation and packaged-resource config validation, but no real Windows
packaged-app run was executed in this branch environment. Windows validation is
still a manual shipping follow-up.

## 7. Dev vs Packaged Resolution

### Development

- resources may resolve from the repository checkout
- tracked placeholder resource directories may exist only to satisfy clean
  Tauri build-time path checks
- external apps are launched separately for provider testing
- engine host may resolve from workspace `target/`
- Blender provider testing assumes the unified app hosts the bridge and the
  separately installed addon connects to it

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
- Blender mode fails gracefully if port `5179` is already occupied
- staged LibreOffice resource paths resolve correctly in the unified app

Phase 7 merged with these validation results already recorded:

- `cargo test -p smolpc-mcp-client`
- `cargo check -p smolpc-code-helper`
- `cargo test -p smolpc-code-helper --lib`
- `npm run check --workspace apps/codehelper`
- unified Tauri config tests now verify:
  - visible packaged identity uses `SmolPC Unified Assistant`
  - bundled resources include LibreOffice and Blender assets
  - launcher resources are excluded from the unified app package

## 10. Deferred Packaging Questions

These remain for later implementation phases:

- final Windows installer naming
- final model distribution approach
- whether some provider assets ship always or are staged optionally
- exact bundle layout for Blender supplementary tooling
- future Calc runtime/tool activation scope beyond the current Writer/Slides import
