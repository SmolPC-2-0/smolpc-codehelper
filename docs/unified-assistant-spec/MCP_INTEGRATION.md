# MCP And Provider Integration

**Last Updated:** 2026-03-16
**Status:** Canonical tool-integration spec for the unified app

## 1. Scope

The unified app needs one backend abstraction for all mode-specific actions. Not
every mode uses MCP, so this document defines a **provider model** that covers:

- local provider behavior for Code
- MCP-backed behavior for GIMP and LibreOffice modes
- hybrid behavior for Blender

## 2. Provider Taxonomy

| Provider kind | Used by | Notes |
|---|---|---|
| `local` | Code | No external MCP process required |
| `mcp` | GIMP, Writer, Calc, Slides | JSON-RPC tool execution via MCP transport |
| `hybrid` | Blender | Bridge-backed primary path, MCP-compatible extension path later |

## 3. `ToolProvider` Abstraction

```rust
pub trait ToolProvider {
    async fn connect_if_needed(&self) -> Result<ProviderStateDto, String>;
    async fn status(&self) -> Result<ProviderStateDto, String>;
    async fn list_tools(&self) -> Result<Vec<ToolDefinitionDto>, String>;
    async fn execute_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolExecutionResultDto, String>;
    async fn undo_last_action(&self) -> Result<(), String>;
    async fn disconnect_if_needed(&self) -> Result<(), String>;
}
```

The assistant orchestrator calls the provider interface. It does not know or
care whether the implementation is local, MCP-backed, or hybrid.

## 4. Registry

The backend owns one `ModeProviderRegistry`:

```rust
pub struct ModeProviderRegistry {
    pub code: Arc<dyn ToolProvider>,
    pub gimp: Arc<dyn ToolProvider>,
    pub blender: Arc<dyn ToolProvider>,
    pub libreoffice: Arc<dyn ToolProvider>,
}
```

Mode mapping:

- `code` -> `code`
- `gimp` -> `gimp`
- `blender` -> `blender`
- `writer` -> `libreoffice`
- `calc` -> `libreoffice`
- `impress` -> `libreoffice`

## 5. Provider Definitions

### 5.1 Code provider

Code mode uses a local provider.

Responsibilities:

- expose Codehelper-local actions where they exist
- keep code-mode behavior inside the unified app
- avoid any launcher or external editor dependency in this plan

Connection behavior:

- no external process startup
- status is derived from in-app readiness and local command availability

Undo behavior:

- optional and limited to any future local actions that support undo
- not required for initial docs baseline

### 5.2 GIMP provider

GIMP uses an MCP-backed provider.

Runtime shape:

- external GIMP app
- GIMP MCP server
- TCP / JSON-RPC communication

Rules:

- if GIMP is unavailable, the app shows a friendly actionable error
- tool discovery is refreshed on reconnect
- undo support is surfaced when the provider confirms it

### 5.3 Blender provider

Blender uses a hybrid provider.

Primary path:

- existing Blender bridge behavior

Secondary path:

- optional supplementary MCP integration later

Rules:

- bridge-backed workflows remain the primary behavior for parity
- MCP expansion must not block the initial unified port

### 5.4 LibreOffice provider

LibreOffice uses one shared provider for three frontend modes:

- Writer
- Calc
- Slides

Rules:

1. One backend runtime connection serves all three modes.
2. Frontend mode config changes prompt, suggestions, and UI labeling.
3. Mode switching between Writer/Calc/Slides should not spawn separate
   provider processes.

## 6. DTO Contracts

```rust
pub struct ProviderStateDto {
    pub mode: String,
    pub state: String,
    pub detail: Option<String>,
    pub supports_tools: bool,
    pub supports_undo: bool,
}

pub struct ToolDefinitionDto {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

pub struct ToolExecutionResultDto {
    pub name: String,
    pub ok: bool,
    pub summary: String,
    pub payload: serde_json::Value,
}
```

## 7. Connection Lifecycle

### Shared lifecycle

1. Resolve the provider for the active mode.
2. Ask for provider status.
3. Connect if needed.
4. Discover tools if the provider supports tools.
5. Execute validated actions.
6. Keep the provider connection warm while the mode remains active.
7. Disconnect or park the provider only when needed by mode switch or app exit.

### Per-provider rules

| Provider | Auto-start | Disconnect behavior |
|---|---|---|
| Code | Not applicable | No-op |
| GIMP | No, depends on external app availability | Disconnect cleanly; do not claim ownership of the external app |
| Blender | Connect to bridge first; start helper process only if part of bridge contract | Clean disconnect from bridge |
| LibreOffice | Yes, provider may own its MCP runtime | Keep shared runtime alive across Writer/Calc/Slides switches |

## 8. Undo Support

| Mode | Undo support |
|---|---|
| Code | Optional; not guaranteed in v1 |
| GIMP | Yes, first-class where provider supports it |
| Blender | Optional; depends on bridge/provider parity |
| Writer | Optional; provider-backed if available |
| Calc | Optional; provider-backed if available |
| Slides | Optional; provider-backed if available |

The frontend should only render undo affordances when:

- the mode capability says undo is supported, and
- the specific message/action is marked `undoable`

## 9. Failure Behavior

| Failure | Meaning | User-facing message style |
|---|---|---|
| provider unavailable | external tool stack is not ready | tell the user what to start or reconnect |
| tools missing | provider connected but did not expose required tools | explain that the mode is connected but incomplete |
| validation failure | model proposed a bad tool or arguments | tell the user the action could not be safely executed |
| execution failure | provider tool failed | explain what failed and suggest retry or a simpler action |
| disconnect during run | provider died mid-request | surface a reconnect / retry action |

Messages must stay student-friendly and actionable.

## 10. Planner Boundary

The frontend never executes tools directly.

The assistant backend owns:

- plan generation
- tool validation
- provider calls
- final natural-language summarization

This keeps tool behavior consistent across Code, GIMP, Blender, Writer, Calc,
and Slides.
