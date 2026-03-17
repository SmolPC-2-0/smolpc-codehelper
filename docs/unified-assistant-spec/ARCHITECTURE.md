# SmolPC Unified Assistant -- Architecture

**Last Updated:** 2026-03-17
**Status:** Canonical architecture for the unified frontend

## 1. Product Shape

SmolPC Unified Assistant is **one product**:

- a single Tauri desktop app
- one shared local inference engine
- six selectable modes inside the app

There is no launcher shell in this architecture. The launcher may exist as a
separate utility in the repository, but it is not part of the runtime model or
merge plan for the unified frontend.

## 2. Modes

| Mode    | Internal Id | Provider Kind   | Source Reference              |
| ------- | ----------- | --------------- | ----------------------------- |
| Code    | `code`      | Local provider  | `apps/codehelper/`            |
| GIMP    | `gimp`      | MCP provider    | `apps/gimp-assistant/`        |
| Blender | `blender`   | Hybrid provider | `apps/blender-assistant/`     |
| Writer  | `writer`    | MCP provider    | `apps/libreoffice-assistant/` |
| Calc    | `calc`      | MCP provider    | `apps/libreoffice-assistant/` |
| Slides  | `impress`   | MCP provider    | `apps/libreoffice-assistant/` |

## 3. High-Level System

```text
+------------------------------------------------------------------+
|                Student PC (Windows 10/11, local only)            |
|                                                                  |
|  +------------------------------------------------------------+  |
|  | Unified Tauri App (`apps/codehelper`)                     |  |
|  |                                                            |  |
|  |  Frontend shell                                           |  |
|  |    - mode dropdown                                        |  |
|  |    - sidebar / per-mode history                           |  |
|  |    - conversation view                                    |  |
|  |    - composer                                             |  |
|  |    - status / diagnostics                                 |  |
|  |                                                            |  |
|  |  Backend orchestrator                                     |  |
|  |    - assistant_send                                       |  |
|  |    - mode provider registry                               |  |
|  |    - plan validation / tool execution                     |  |
|  +------------------------+-----------------------------------+  |
|                           |                                      |
|                           v                                      |
|                +----------------------------+                    |
|                | smolpc-engine-host         |                    |
|                | localhost:19432            |                    |
|                | shared across all modes    |                    |
|                +----------------------------+                    |
|                           |                                      |
|        +------------------+------------------+                   |
|        |                  |                  |                   |
|        v                  v                  v                   |
|   Local Code        External GIMP      Blender hybrid           |
|   provider          MCP provider       provider                 |
|                                         (local bridge server    |
|                                          + external addon)      |
|                                                                  |
|                    LibreOffice MCP provider                      |
|                    shared by Writer/Calc/Slides                  |
+------------------------------------------------------------------+
```

## 4. Runtime Processes

### 4.1 Unified app

- Binary family: current `apps/codehelper`
- Responsibilities:
  - render the shared shell
  - keep one engine connection
  - route requests to the active mode provider
  - stream progress and response events to the UI

### 4.2 Shared engine

- Binary: `smolpc-engine-host`
- Port: `19432`
- Responsibilities:
  - model loading
  - backend selection
  - text generation
  - readiness and status reporting

The engine is never duplicated per mode.

### 4.3 External provider runtimes

- GIMP MCP server
- Blender bridge server hosted by the unified app plus the external addon
- LibreOffice MCP runtime

These are provider-owned integrations. They are not inference runtimes.

## 5. Backend Structure Inside The Unified App

The Tauri backend has four layers:

### 5.1 Engine bridge

Reuses the current Codehelper engine integration for:

- `engine_ensure_started`
- `engine_status`
- generation requests
- status and diagnostics

### 5.2 Assistant orchestrator

Owns:

- `assistant_send`
- `assistant_cancel`
- plan generation
- tool validation
- tool execution
- final summarization back to the user

### 5.3 Mode provider registry

Maps the active `AppMode` to a provider implementation.

### 5.4 Mode providers

- `CodeProvider`
- `GimpProvider`
- `BlenderProvider`
- `LibreOfficeProvider`

## 6. Provider Model

The provider model is the main architectural boundary that keeps this work
merge-safe while engine and standalone-app development continues.

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

The provider interface is mode-aware even when multiple frontend modes share
one provider implementation. This keeps Writer / Calc / Slides status and tool
discovery honest at the provider boundary instead of patching mode identity
only in the command layer.

### Provider ownership

- `CodeProvider` wraps in-app behavior and current Codehelper capabilities.
- `GimpProvider` ports behavior from the GIMP assistant into new unified files.
- `BlenderProvider` ports bridge-based Blender behavior into new unified files
  and owns the bridge runtime, scene cache, and retrieval index for the
  unified app.
- `LibreOfficeProvider` ports behavior from the LibreOffice branch into new
  unified files and serves three frontend submodes.
- Phase 6A lands the LibreOffice provider scaffold and shared stdio MCP prep.
- Phase 6B activates Writer and Slides through that same shared provider while
  Calc remains scaffold-only.

## 7. Frontend Shell

The frontend shell is shared by every mode and lives in `apps/codehelper/src/`.

Shared surfaces:

- app identity area
- mode dropdown
- per-mode history list
- conversation view
- composer
- status bar
- diagnostics surfaces enabled by mode capabilities

Mode switching changes:

- system prompt
- active provider
- suggestion chips
- visible chat history
- tool availability
- mode-specific actions such as undo

Mode switching does **not**:

- restart the engine
- change the product shell
- move the user into a different executable

## 8. Request Flow

```text
1. User submits prompt in selected mode
2. Frontend calls assistant_send(request, on_event)
3. Backend ensures engine is ready
4. Backend resolves provider for request.mode
5. Backend decides:
   - direct answer only, or
   - answer plus tool plan
6. If tools are required:
   - provider connects if needed
   - tool list is validated
   - actions are executed
7. Backend streams:
   - status events
   - tool call/result events
   - token events when the mode uses streaming generation
   - completion or error
8. Frontend stores final response metadata
```

### Phase 5 Blender request path

After Phase 5:

1. `assistant_send` is operational for `gimp` and `blender`.
2. Blender requests use the hybrid provider path:
   - ensure the local bridge runtime is available
   - fetch current scene snapshot through the provider
   - optionally retrieve Blender-doc contexts
   - generate the tutoring answer through the shared engine
3. Blender remains bridge-first. Supplemental MCP work stays deferred.

### Phase 6B LibreOffice activation path

After Phase 6B:

1. `LibreOfficeProvider` is still shared by Writer / Calc / Slides.
2. Writer and Slides are live runtime-backed modes in the shell.
3. Calc remains a visible scaffold-only mode with disabled composer.
4. `assistant_send` is operational for `writer` and `impress` only.
5. `assistant_send(calc)` remains scaffold-only.
6. Shared stdio MCP transport in `smolpc-mcp-client` now drives a real
   LibreOffice runtime through the staged Python `main.py` entrypoint.
7. The shared runtime contract remains:
   - stdio MCP child process
   - helper socket bridge on `localhost:8765`
   - headless office socket on `localhost:2002`
8. The imported runtime assets live under:
   `apps/codehelper/src-tauri/resources/libreoffice/mcp_server/`
9. Live tool execution is mode-filtered at the provider boundary:
   - Writer uses the Writer allowlist
   - Slides uses the Slides allowlist
   - Calc exposes no live tools in this phase
10. The unified branch imports the runtime assets from
    `origin/codex/libreoffice-port-track-a` commit
    `7acad1fa0eb31e32a5485069e85c021d14284455` and continues treating that
    branch as a read-only reference source.
11. LibreOffice execution stays intentionally narrow in this phase:
    - one tool call maximum per assistant turn
    - one summary follow-up maximum after tool execution
    - deterministic local summary fallback if the summary step fails, times out,
      or is cancelled after the document tool already ran

## 9. Repository Boundaries

### 9.1 Active implementation zone

All unified implementation lands under:

- `apps/codehelper/`
- new shared crates for assistant DTOs / MCP transport if needed
- provider-owned bundled assets for modes such as Blender retrieval metadata

### 9.2 Reference-only app zones during the port

These remain source references while their behavior is ported:

- `apps/gimp-assistant/`
- `apps/blender-assistant/`
- `apps/libreoffice-assistant/`

Unified branches should not take ownership of those directories except for
small upstream bug fixes that belong to the standalone app itself.

### 9.3 Engine boundary

Engine work continues under:

- `engine/crates/smolpc-engine-client/`
- `engine/crates/smolpc-engine-core/`
- `engine/crates/smolpc-engine-host/`

Unified frontend branches should consume stable engine contracts. If the
unified app needs an engine contract change, that change should land separately
and then be pulled into the unified branches.

## 10. Merge-Safe Boundaries

| Area            | Rule                                                         |
| --------------- | ------------------------------------------------------------ |
| Unified shell   | Implement in `apps/codehelper` only                          |
| Standalone apps | Port behavior into adapters; do not merge directories        |
| Engine          | Prefer contract reuse over direct internal edits             |
| Packaging       | Document and validate after provider integration, not before |
| Launcher        | Out of scope for runtime ownership                           |

## 11. Critical Invariants

1. One unified app, not a launcher plus child product shells.
2. Code is a first-class in-app mode.
3. One shared engine process across all modes.
4. Mode providers are the only backend integration surface for mode-specific
   tools.
5. Writer, Calc, and Slides share one LibreOffice backend runtime.
6. Standalone apps remain merge-safe sources of truth while the port is in
   progress.
7. Windows is the delivery target for packaging and validation.
