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

| Provider kind | Used by                    | Notes                                                           |
| ------------- | -------------------------- | --------------------------------------------------------------- |
| `local`       | Code                       | No external MCP process required                                |
| `mcp`         | GIMP, Writer, Calc, Slides | JSON-RPC tool execution via MCP transport                       |
| `hybrid`      | Blender                    | Bridge-backed primary path, MCP-compatible extension path later |

## 3. `ToolProvider` Abstraction

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

The assistant orchestrator calls the provider interface. It does not know or
care whether the implementation is local, MCP-backed, or hybrid.

The `mode` argument is required even for shared providers so Writer, Calc, and
Slides can report honest per-submode state and evolve independently without
breaking the provider boundary later.

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

Phase 4 transport default:

- host: `127.0.0.1`
- port: `10008`

Phase 4 implementation rule:

- the unified app connects to an already-available GIMP MCP endpoint
- the shared `smolpc-mcp-client` crate owns the TCP JSON-RPC transport work
- the unified app does not port the standalone app's legacy stdio launcher
  shape into `apps/codehelper`

Rules:

- if GIMP is unavailable, the app shows a friendly actionable error
- tool discovery is refreshed on reconnect
- undo support is surfaced when the provider confirms it
- Phase 4 makes GIMP the first real external-provider mode; `assistant_send`
  becomes live for `gimp` only while other non-Code modes remain placeholder-
  only

### 5.2.1 GIMP Phase 4 action surface

Phase 4 targets parity with the existing proven GIMP assistant surface, not
provider expansion beyond that baseline.

Supported actions:

- GIMP info query
- current image metadata query
- describe current image
- draw line
- draw heart
- draw circle
- draw oval
- draw triangle
- draw filled rectangle / square
- crop to square
- resize width
- increase / decrease brightness
- increase / decrease contrast
- blur entire image
- brighten / darken top, bottom, left, or right half
- increase / decrease contrast in top, bottom, left, or right half
- blur top, bottom, left, or right half
- rotate 90 / 180 / 270
- flip horizontal / vertical
- undo last change

### 5.2.2 GIMP Phase 4 execution model

Phase 4 GIMP requests use a hybrid execution model:

1. deterministic fast paths for existing proven prompt families
2. direct MCP info / metadata queries where appropriate
3. constrained `call_api` fallback for editing requests outside the fast paths

Phase 4 does not attempt to make GIMP fully general.

### 5.2.3 GIMP Phase 4 undo model

Phase 4 preserves the existing clipboard-backed undo behavior from the
standalone GIMP assistant:

- undo restores the last saved clipboard-backed image state
- only the most recent operation is guaranteed undoable
- the frontend only renders Undo when the specific assistant message is marked
  `undoable`

### 5.3 Blender provider

Blender uses a hybrid provider.

Primary path:

- existing Blender bridge behavior hosted by the unified app

Secondary path:

- optional supplementary MCP integration later

Rules:

- bridge-backed workflows remain the primary behavior for parity
- MCP expansion must not block the initial unified port
- Phase 5 keeps Blender bridge-first and does not require `smolpc-mcp-client`
  or `blender-mcp`
- the unified app reuses the existing addon contract without modifying the
  addon itself

### 5.3.1 Blender Phase 5 bridge contract

Phase 5 keeps the existing bridge compatibility surface:

- bridge bind address: `127.0.0.1:5179`
- auth token path:
  `%LOCALAPPDATA%/SmolPC/engine-runtime/bridge-token.txt`
- on Windows packaging this is the canonical token path; non-Windows dev and
  test environments may use the platform-appropriate local app-data equivalent
  while preserving the same addon-facing token-file contract
- the unified app hosts the bridge server
- the external Blender addon connects to that bridge

Phase 5 startup rule:

- bridge startup is lazy and non-fatal
- a bridge bind failure degrades Blender mode only
- a bind conflict must not stop the unified app from launching

### 5.3.2 Blender Phase 5 tutoring surface

Phase 5 Blender scope is scene-aware tutoring chat, not full standalone-app
parity.

Included in Phase 5:

- live scene-aware question answering
- local Blender-doc retrieval grounding
- shared-engine generation
- token streaming with cancellation
- bridge-backed scene status in the unified shell
- existing tutoring-style chat actions that fit the unified shell

Deferred from Phase 5:

- backend toggle UI
- Ollama fallback UI
- separate scene-analysis UI
- standalone scene panel recreation
- Blender undo
- Blender export emphasis
- Blender benchmark surfaces

### 5.3.3 Blender Phase 5 pseudo-tools

Phase 5 Blender does not add a general MCP tool surface.

The provider exposes exactly two internal pseudo-tools:

- `scene_current`
- `retrieve_rag_context`

The final tutoring answer is produced by the shared engine, not by a provider
tool.

### 5.3.4 Blender Phase 5 retrieval model

Phase 5 ports the standalone Blender assistant's local retrieval approach:

- bundled Blender documentation metadata
- local keyword retrieval only
- no vector database expansion
- no remote retrieval service

Retrieval rules:

- skip retrieval for obvious scene-state questions
- use retrieval for broader Blender workflow questions
- retrieval load failure must degrade gracefully to scene-aware chat only
- retrieval load failure must be surfaced in provider detail so the shell can
  report that Blender-doc grounding is temporarily unavailable

### 5.3.5 Blender Phase 5 status semantics

`mode_status(blender)` uses these states:

- `disconnected`: bridge runtime is unavailable or could not be started
- `connecting`: transient during explicit start or refresh work
- `connected`: bridge runtime is healthy, even if no live scene snapshot is
  currently available
- `error`: startup or bridge interaction failed after the provider was tried

Detail rules:

- no scene snapshot yet must be surfaced clearly
- stale scene snapshot must be surfaced clearly
- missing live scene data does not disable generic Blender tutoring chat
- `supports_tools = true` because Blender exposes the internal pseudo-tools
  `scene_current` and `retrieve_rag_context`, not because Phase 5 adds a
  general external tool surface
- `supports_undo = false`

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
4. Shared providers still surface the requested submode back through
   `ProviderStateDto.mode` and any mode-sensitive tool list decisions.
5. Phase 6A only lands the shared provider scaffold; live LibreOffice runtime
   activation is deferred to a later follow-up branch.

## 5.5 `smolpc-mcp-client` scaffolding contract

The Phase 1 transport crate is intentionally small, but one contract is locked
early:

```rust
#[async_trait]
pub trait JsonRpcClient {
    async fn call(
        &self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, McpClientError>;
}
```

The transport call is async from the start because stdio and TCP MCP flows are
inherently asynchronous. The foundation branch must not ship a synchronous call
signature that later mode branches would need to break.

Phase 6A extends this crate with shared stdio transport support so the unified
LibreOffice provider can later use the same client layer as the other provider
families instead of importing a standalone-app-specific MCP client.

## 6. DTO Contracts

```rust
pub struct ProviderStateDto {
    pub mode: AppMode,
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

pub struct ModeStatusDto {
    pub mode: AppMode,
    pub engine_ready: bool,
    pub provider_state: ProviderStateDto,
    pub available_tools: Vec<ToolDefinitionDto>,
    pub last_error: Option<String>,
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

| Provider    | Auto-start                                          | Disconnect behavior                                                                |
| ----------- | --------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Code        | Not applicable                                      | No-op                                                                              |
| GIMP        | No, depends on external app availability            | Disconnect cleanly; do not claim ownership of the external app                     |
| Blender     | Lazy-start local bridge server on first Blender use | Cleanly stop bridge runtime on app exit; do not claim ownership of Blender itself  |
| LibreOffice | Phase 6A: no; activation deferred                   | Keep shared runtime alive across Writer/Calc/Slides switches once activation lands |

## 8. Undo Support

| Mode    | Undo support                                |
| ------- | ------------------------------------------- |
| Code    | Optional; not guaranteed in v1              |
| GIMP    | Yes, first-class where provider supports it |
| Blender | No in Phase 5                               |
| Writer  | Optional; provider-backed if available      |
| Calc    | Optional; provider-backed if available      |
| Slides  | Optional; provider-backed if available      |

The frontend should only render undo affordances when:

- the mode capability says undo is supported, and
- the specific message/action is marked `undoable`

## 9. Failure Behavior

| Failure               | Meaning                                              | User-facing message style                                 |
| --------------------- | ---------------------------------------------------- | --------------------------------------------------------- |
| provider unavailable  | external tool stack is not ready                     | tell the user what to start or reconnect                  |
| tools missing         | provider connected but did not expose required tools | explain that the mode is connected but incomplete         |
| validation failure    | model proposed a bad tool or arguments               | tell the user the action could not be safely executed     |
| execution failure     | provider tool failed                                 | explain what failed and suggest retry or a simpler action |
| disconnect during run | provider died mid-request                            | surface a reconnect / retry action                        |

Messages must stay student-friendly and actionable.

## 9.1 Phase 1 placeholder behavior

The foundation branch intentionally keeps provider behavior narrow:

- Code provider reports scaffold status and no tools
- GIMP / Blender / LibreOffice providers report placeholder disconnected status
- `mode_refresh_tools` is an intentional no-op in Phase 1 and becomes real in
  later phases
- `assistant_send` accepts the final request shape but still returns
  `UNIFIED_ASSISTANT_NOT_IMPLEMENTED`

## 9.2 Phase 4 GIMP activation rule

Phase 4 activates the provider-backed command surface only for GIMP:

- `assistant_send` is operational for `mode == gimp`
- `mode_status(gimp)` reports real connection state and discovered tools
- `mode_refresh_tools(gimp)` performs a real reconnect / rediscovery attempt
- `mode_undo(gimp)` performs a real provider-backed undo
- `assistant_send` remains scaffold-only for `blender`, `writer`, `calc`, and
  `impress`
- Code mode keeps the existing Codehelper inference path rather than routing
  through `assistant_send`

## 9.3 Phase 5 Blender activation rule

Phase 5 activates the provider-backed command surface for Blender while keeping
Code and GIMP behavior stable:

- `assistant_send` is operational for `mode == blender`
- `assistant_send` remains shared-engine-backed for Blender rather than using
  MCP transport
- `mode_status(blender)` reports live bridge runtime state and internal
  pseudo-tool availability
- `mode_refresh_tools(blender)` performs a real bridge/runtime refresh and
  one explicit retrieval reload attempt without a background retry loop or
  backoff contract in Phase 5
- `mode_undo(blender)` remains unsupported
- Code mode keeps the existing Codehelper inference path
- GIMP mode keeps the current Phase 4 MCP-backed execution path
- `assistant_send` remains scaffold-only for `writer`, `calc`, and `impress`

## 9.4 Phase 6A LibreOffice scaffolding rule

Phase 6A keeps LibreOffice execution non-live while landing the merge-safe
provider scaffold:

- `assistant_send` remains scaffold-only for `writer`, `calc`, and `impress`
- `mode_status(writer|calc|impress)` returns scaffold-aware provider detail
  rather than the original generic foundation placeholder wording
- `mode_refresh_tools(writer|calc|impress)` validates the staged scaffold only
  and does not launch a LibreOffice runtime
- `available_tools` remains empty for LibreOffice modes in this phase
- Calc is explicitly not required to be live in this phase because the current
  source branch does not yet provide parity-level spreadsheet tooling

## 10. Planner Boundary

The frontend never executes tools directly.

The assistant backend owns:

- plan generation
- tool validation
- provider calls
- final natural-language summarization

This keeps tool behavior consistent across Code, GIMP, Blender, Writer, Calc,
and Slides.
