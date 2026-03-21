# MCP And Provider Integration For The Self-Contained Line

**Last Updated:** 2026-03-17
**Status:** Integration ownership spec with Phase 2 setup foundation, Phase 3 LibreOffice runtime ownership landed, Phase 4 Blender provisioning landed, and Phase 5 GIMP provisioning preflight locked

## 1. Scope

This document defines how each non-Code mode becomes self-contained without
changing the public assistant command surface.

Phase 2 foundation adds app-level setup commands and setup state, but it does
not change live mode activation behavior.

Phase 3 uses that foundation to remove packaged-mode external Python from
Writer and Slides without changing the public assistant command surface.

Public command surface remains:

- `list_modes`
- `mode_status`
- `mode_refresh_tools`
- `assistant_send`
- `assistant_cancel`
- `mode_undo`

New app-level setup commands are added separately:

- `setup_status`
- `setup_prepare`

## 2. Provider Taxonomy

| Provider kind | Modes                      | Shipping runtime pattern                              |
| ------------- | -------------------------- | ----------------------------------------------------- |
| `local`       | Code                       | app-owned engine only                                 |
| `mcp`         | GIMP, Writer, Calc, Slides | app-owned runtime or transport plus external host app |
| `hybrid`      | Blender                    | app-owned bridge plus provisioned addon               |

Phase 2 kept the existing live provider behavior intact while adding the shared
setup/provisioning substrate those providers consume later.

That substrate is now merged into the implementation line. Current live-mode
surfaces remain unchanged:

- Code stays live as-is
- GIMP stays live as-is
- Blender stays live as-is
- Writer and Slides stay live while their packaged-mode runtime ownership now uses bundled Python
- Calc stays disabled

## 3. Stable Provider Interface

```rust
pub trait ToolProvider {
    async fn connect_if_needed(&self, mode: AppMode) -> Result<ProviderStateDto, String>;
    async fn status(&self, mode: AppMode) -> Result<ProviderStateDto, String>;
    async fn list_tools(&self, mode: AppMode) -> Result<Vec<ToolDefinitionDto>, String>;
    async fn execute_tool(
        &self,
        mode: AppMode,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolExecutionResultDto, String>;
    async fn undo_last_action(&self, mode: AppMode) -> Result<(), String>;
    async fn disconnect_if_needed(&self, mode: AppMode) -> Result<(), String>;
}
```

The self-contained line does not change this contract. It changes runtime
ownership behind it.

## 4. Integration Matrix

| Mode    | Current demo ownership                   | Self-contained ownership target                    |
| ------- | ---------------------------------------- | -------------------------------------------------- |
| Code    | already app-owned                        | unchanged                                          |
| Writer  | runtime scripts bundled, Python external | bundled scripts + bundled Python                   |
| Slides  | runtime scripts bundled, Python external | bundled scripts + bundled Python                   |
| Blender | bridge app-owned, addon external         | bridge app-owned, addon bundled/provisioned        |
| GIMP    | provider only; plugin/server external    | plugin/server bundled/provisioned and app-launched |
| Calc    | scaffold-only                            | unchanged                                          |

## 5. Mode-Specific Rules

### 5.1 Code

- no provider runtime changes
- bundled default model is the self-contained addition here

### 5.2 LibreOffice

Transport/runtime rules:

- stdio MCP child process via bundled `main.py`
- helper socket stays `localhost:8765`
- office socket stays `localhost:2002`
- provider consumes the prepared bundled Python runtime from setup state
- packaged mode must use bundled Python only
- no packaged-mode fallback to system `python` or `python3`

Ownership rules:

- LibreOffice / Collabora remains separately installed
- unified app owns Python runtime and MCP scripts
- `setup_prepare()` prepares the app-owned Python substrate but does not launch LibreOffice
- provider auto-detects the LibreOffice host path and the bundled runtime auto-launches `soffice` when required
- no Phase 3 settings UI or manual path override UI is added

Mode rules:

- Writer live
- Slides live
- Calc scaffold-only
- `mode_status(writer|impress)` and `mode_refresh_tools(writer|impress)` report bundled-Python and `soffice` readiness honestly
- `mode_status(calc)` and `mode_refresh_tools(calc)` remain scaffold-only

### 5.3 Blender

Transport/runtime rules:

- bridge remains hosted by unified app on `127.0.0.1:5179`
- bridge token path contract remains unchanged
- addon-facing protocol remains unchanged

Ownership rules:

- Blender remains separately installed
- addon payload becomes bundled provider-owned resource
- authoritative addon source remains `apps/blender-assistant/blender_addon/blender_helper_http.py`
- unified app repackages a pinned snapshot under `resources/blender/addon/blender_helper_http.py`
- addon module id is locked to `blender_helper_http`
- unified app provisions and enables the addon automatically
- unified app resolves the addon target directory by asking Blender itself through background CLI execution
- unified app launches Blender when needed only if no matching Blender process is already running
- unified app never kills or force-restarts an already running Blender instance

### 5.4 GIMP

Transport/runtime rules:

- unified provider keeps TCP MCP transport on `127.0.0.1:10008`
- bundled GIMP MCP runtime is launched by the unified app
- transport contract remains unchanged for Phase 5

Ownership rules:

- GIMP remains separately installed
- authoritative upstream source is `maorcc/gimp-mcp` pinned to an exact commit/tag before import
- plugin/server payload is bundled by the unified app
- unified app provisions plugin files into the user GIMP profile
- unified app launches GIMP when needed
- unified app launches the bundled GIMP MCP server when needed
- unified app should reuse an already-running GIMP session instead of force-restarting it

## 6. Setup API Expectations

`setup_status` should expose:

- host app detected/missing
- bundled foundation assets ready/missing/not prepared
- repair action availability

Phase 2 setup item ids are locked to:

- `engine_runtime`
- `bundled_model`
- `bundled_python`
- `host_gimp`
- `host_blender`
- `host_libreoffice`

Phase 4 added one new setup item id:

- `blender_addon`

Those item ids now define the live `setup_status` wire contract.

The Phase 3 workflow change does not alter those item ids or the
`setup_status` / `setup_prepare` wire contract.

`setup_prepare` should:

- validate manifests
- prepare app-local setup directories
- extract or install the bundled Python runtime when packaged payloads are present
- validate the packaged model manifest and resource path
- refresh host-app detection state

`setup_prepare` must not in Phase 2:

- provision Blender addons
- provision GIMP plugins
- edit Blender or GIMP user profiles
- launch GIMP, Blender, or LibreOffice
- replace `mode_refresh_tools`

Phase 4 live extension:

- `setup_prepare()` may provision and enable the Blender addon through Blender CLI background execution when Blender is installed
- `setup_prepare()` still must not launch the interactive Blender UI

## 7. Provisioning Rules

Every provider-owned external integration must have:

- a version manifest
- a provision target path strategy
- a repair/reapply path
- a status check path

Expected provisioners:

- `LibreOfficeProvisioner`
- `BlenderAddonProvisioner`
- `GimpPluginProvisioner`

Phase 2 establishes the shared provisioning foundation. Phase 4 ships the
Blender addon provisioner path. Phase 5 is the next locked implementation slice
for GIMP provisioning and runtime ownership.

## 8. Runtime Supervision Rules

Every app-owned runtime must support:

- start
- readiness check
- bounded shutdown
- reconnect/reuse behavior
- honest status detail

Expected supervisors:

- shared engine supervisor
- bundled Python runtime supervisor
- LibreOffice runtime supervisor
- Blender bridge supervisor
- GIMP MCP runtime supervisor

Phase 2 introduces the shared setup state and packaged-resource validation
needed before those provider-specific supervisors take ownership in later
phases. Phase 3 is the first phase where one of those supervisors becomes a
real packaged-mode runtime dependency, and Phase 4 extends that ownership with
live Blender addon provisioning and mode-driven Blender launch behavior.

## 9. Non-Goals In This Line

- Calc activation
- Blender MCP expansion
- GIMP feature expansion beyond current action surface
- Code migration onto `assistant_send`
