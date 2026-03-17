# MCP And Provider Integration For The Self-Contained Line

**Last Updated:** 2026-03-17
**Status:** Integration ownership spec for self-contained delivery

## 1. Scope

This document defines how each non-Code mode becomes self-contained without
changing the public assistant command surface.

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
- packaged mode must use bundled Python only
- no packaged-mode fallback to system `python` or `python3`

Ownership rules:

- LibreOffice / Collabora remains separately installed
- unified app owns Python runtime and MCP scripts
- provider auto-launches `soffice` when required

Mode rules:

- Writer live
- Slides live
- Calc scaffold-only

### 5.3 Blender

Transport/runtime rules:

- bridge remains hosted by unified app on `127.0.0.1:5179`
- bridge token path contract remains unchanged
- addon-facing protocol remains unchanged

Ownership rules:

- Blender remains separately installed
- addon payload becomes bundled provider-owned resource
- unified app provisions and enables addon automatically
- unified app launches Blender when needed

### 5.4 GIMP

Transport/runtime rules:

- unified provider keeps TCP MCP transport on `127.0.0.1:10008`
- bundled GIMP MCP runtime is launched by the unified app

Ownership rules:

- GIMP remains separately installed
- plugin/server payload is bundled by the unified app
- unified app provisions plugin files into the user GIMP profile
- unified app launches GIMP when needed
- unified app launches the bundled GIMP MCP server when needed

## 6. Setup API Expectations

`setup_status` should expose:

- host app detected/missing
- provider assets provisioned/outdated/missing
- provider runtime ready/not ready
- repair action availability

`setup_prepare` should:

- provision missing bundled assets
- validate packaged runtime resources
- repair version drift when safe
- never silently mutate user profile state without explicit provider/setup intent

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

## 9. Non-Goals In This Line

- Calc activation
- Blender MCP expansion
- GIMP feature expansion beyond current action surface
- Code migration onto `assistant_send`
