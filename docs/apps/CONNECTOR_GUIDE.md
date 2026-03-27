# Connector Guide

Connectors are self-contained Rust crates that integrate SmolPC with third-party applications. Each connector implements the `ToolProvider` trait from `smolpc-connector-common`, bridging the gap between the unified assistant and a host application's native APIs.

The system currently ships four connectors — Code (built-in), Blender, GIMP, and LibreOffice — but the architecture is designed so that adding a new connector requires no changes to the engine, command layer, or streaming infrastructure.

## The ToolProvider Trait

**Source:** `crates/smolpc-connector-common/src/provider.rs`

Every connector implements this trait:

```rust
#[async_trait]
pub trait ToolProvider: Send + Sync {
    async fn connect_if_needed(&self, mode: AppMode) -> Result<ProviderStateDto, String>;
    async fn status(&self, mode: AppMode) -> Result<ProviderStateDto, String>;
    async fn list_tools(&self, mode: AppMode) -> Result<Vec<ToolDefinitionDto>, String>;
    async fn execute_tool(
        &self, mode: AppMode, name: &str, arguments: Value,
    ) -> Result<ToolExecutionResultDto, String>;
    async fn undo_last_action(&self, mode: AppMode) -> Result<(), String>;
    async fn disconnect_if_needed(&self, mode: AppMode) -> Result<(), String>;
}
```

All methods accept `AppMode` as context, which allows a single provider to serve multiple modes (e.g., `LibreOfficeProvider` handles both `Writer` and `Impress`).

| Method | Purpose |
|---|---|
| `connect_if_needed` | Establish connection to the host app if not already connected. Idempotent. |
| `status` | Return current state: connected, disconnected, idle, or error — with an optional detail message. |
| `list_tools` | Return the tools available in the current mode. May be filtered by mode (e.g., Writer vs Impress). |
| `execute_tool` | Run a named tool with JSON arguments. Returns success/failure with a summary and raw payload. |
| `undo_last_action` | Revert the last tool execution. Only supported by GIMP (via clipboard). |
| `disconnect_if_needed` | Clean up: close IPC connections, kill child processes, clear cached state. |

Providers are stored as `Arc<dyn ToolProvider>` in the `ModeProviderRegistry` and dispatched by `AppMode`.

## Shared Infrastructure

**Source:** `crates/smolpc-connector-common/src/`

### CancellationToken

A trait abstraction that lets connectors check for cancellation without depending on the app's `AssistantState`:

```rust
pub trait CancellationToken: Send + Sync {
    fn is_cancelled(&self) -> bool;
}
```

Uses `AtomicBool` with proper memory ordering (`Release` for store, `Acquire` for load). A `MockCancellationToken` is provided for tests.

### TextStreamer / EngineTextStreamer

The `TextStreamer` trait provides streaming text generation to connectors:

```rust
#[async_trait]
pub trait TextStreamer: Send + Sync {
    async fn generate_stream(
        &self,
        messages: &[EngineChatMessage],
        cancel: &dyn CancellationToken,
        on_token: &mut (dyn FnMut(String) + Send),
    ) -> Result<String, String>;
}
```

`EngineTextStreamer` implements this by calling the engine's chat completion endpoint with connector-tuned generation config (max_length=1024, temperature=0.55, top_k=40, top_p=0.9). The `on_token` callback enables streaming to the frontend. Cancellation is checked every 40ms via `tokio::select!`.

### Host App Detection

**Source:** `crates/smolpc-connector-common/src/host_apps.rs`

Detects installed host applications using a priority chain:

1. **Cached path** — if a previous detection stored a path that still exists, reuse it
2. **Windows Registry** — query `HKLM\Software\Microsoft\Windows\CurrentVersion\App Paths\{exe}`
3. **Standard paths** — platform-specific known installation directories
4. **PATH search** — scan the `PATH` environment variable

Returns a `HostAppDetection` with the app ID, label, detected path, and detail message.

**Blender** standard paths (Windows): scans `%ProgramFiles%\Blender Foundation\Blender {version}\` directories, prefers the newest version by numeric sort.

**GIMP** standard paths (Windows): checks per-user install first (`%LOCALAPPDATA%\Programs\GIMP 3\`), then system install. Supports GIMP 3.x and falls back to 2.10.

**LibreOffice** standard paths: checks both LibreOffice and Collabora Office directories.

### Host App Launch

**Source:** `crates/smolpc-connector-common/src/launch.rs`

Launch utilities check if the host app is already running (via `sysinfo` process enumeration) before spawning. Returns `AlreadyRunning` or `Launched`. LibreOffice uses mode flags (`--writer`, `--impress`) to open in the correct application.

### Python Runtime Resolution

**Source:** `crates/smolpc-connector-common/src/python.rs`

Connectors that need Python (GIMP bridge, LibreOffice MCP server) resolve a Python interpreter through:

1. **Bundled Python** — extracted from the app's resource directory to `{app_local_data}/setup/python/` during first-run setup. Includes `python.exe`, `uv.exe`, `uvx.exe`.
2. **Development venv** — in debug mode, checks `.venv/Scripts/python.exe` (Windows) or `.venv/bin/python3` (Unix)
3. **System Python** — fallback to `python` or `python3` on PATH

A `manifest.json` file tracks the bundled Python version. The `prepare_bundled_python` function copies the payload and writes a `.prepared-version` marker for idempotency.

### Manifest Parsing

**Source:** `crates/smolpc-connector-common/src/manifests.rs`

Each connector has a `resources/manifest.json` that tracks bundled resource integrity:

```json
{
  "version": "phase4-blender-addon-payload",
  "source": "in-repo blender resources and addon snapshot",
  "expectedPaths": ["README.md", "rag_system", "addon/blender_helper_http.py"],
  "status": "tracked"
}
```

The `load_manifest` function reads and validates the manifest: all fields must be non-empty, and `expectedPaths` must list at least one file. `missing_expected_paths` checks which files are actually present on disk.

## Connector Architectures

Each connector uses a different IPC mechanism to communicate with its host application:

| Connector | IPC Transport | Protocol | Bridge |
|---|---|---|---|
| Code | None | Direct engine chat | None (built-in) |
| Blender | HTTP REST | Custom JSON | Axum server (Rust, port 5179) |
| GIMP | TCP socket | MCP (Model Context Protocol) | Python bridge process (port 10008) |
| LibreOffice | stdio | MCP | Python MCP server process |

### Blender Connector

**Source:** `connectors/blender/src/`

The Blender connector provides scene-aware tutoring. It does not generate executable code — instead, it retrieves the live Blender scene context and generates UI-based instructions grounded in Blender API documentation.

**Architecture:**

1. An Axum HTTP server (the "bridge") runs on `localhost:5179` with Bearer token authentication
2. A bundled Python addon (`blender_helper_http.py`) runs inside Blender, posting scene snapshots to `/scene/update`
3. The connector reads scene data from the bridge cache and queries a keyword-based RAG index over Blender API documentation

**Data flow for a user question:**

1. User asks "How do I add a bevel to this object?"
2. Connector calls `scene_current` tool — bridge returns the live scene snapshot (objects, active object, mode, modifiers)
3. Heuristic determines if RAG is needed (yes — "bevel" is a workflow hint)
4. `retrieve_rag_context` queries the RAG index with the user's text, returns top 3 documentation chunks
5. System prompt is built with scene context + RAG context
6. Engine generates UI-focused instructions (menus, hotkeys, panels — never Python code)

**Scene snapshot format:**

```json
{
  "object_count": 5,
  "active_object": "Cube",
  "mode": "OBJECT",
  "render_engine": "BLENDER_EEVEE",
  "objects": [
    { "name": "Cube", "object_type": "MESH", "modifiers": [{"name": "Bevel", "modifier_type": "BEVEL"}] },
    { "name": "Camera", "object_type": "CAMERA", "modifiers": [] }
  ]
}
```

Up to 40 objects are listed in the prompt. The scene cache has a 30-second staleness threshold — if no update arrives within that window, the snapshot is reported as disconnected.

**RAG index:** Keyword matching over bundled Blender API documentation chunks. Each chunk has a `signature` (e.g., `bpy.ops.mesh.bevel`), `text`, and `url`. Scoring: `(matching_terms / query_terms) + 0.1 signature bonus`, minimum relevance 0.2.

**Smart RAG skipping:** Scene-query questions ("what's in my scene", "list objects") skip RAG entirely since the scene snapshot already contains the answer. Workflow questions ("how do I add", "modifier", "bevel") trigger RAG retrieval.

**Addon provisioning:**

1. Detect Blender installation
2. Run Blender headless to probe the addon directory: `bpy.utils.user_resource("SCRIPTS", path="addons")`
3. Copy `blender_helper_http.py` to the addon directory
4. Run Blender headless to enable the addon: `addon_utils.enable("blender_helper_http")`
5. Write a version marker to `{app_local_data}/setup/state/blender-addon.json`

**Bridge authentication:** A 48-character random alphanumeric token is generated on startup and written to `%LOCALAPPDATA%\SmolPC 2.0\engine-runtime\bridge-token.txt`. The addon reads this file and sends it as a Bearer token. Token comparison uses constant-time comparison.

### GIMP Connector

**Source:** `connectors/gimp/src/`

The GIMP connector uses a planning/execution model with pre-computed "fast paths" for common operations. It communicates with GIMP through the Model Context Protocol (MCP) over TCP.

**Architecture:**

1. A Python bridge process (`smolpc_gimp_mcp_tcp_bridge.py`) runs as a TCP MCP server on `localhost:10008`
2. A provisioned GIMP plugin (`gimp-mcp-plugin.py`) runs inside GIMP as a persistent extension, listening on `localhost:9877`
3. The bridge relays MCP messages between the Rust connector and the GIMP plugin
4. The plugin exposes GIMP's PDB (Procedure Database) operations as MCP tools

**Execution paths (priority order):**

1. **Fast paths** — 30+ pre-computed macros for common operations: draw shapes, brightness/contrast, blur, rotate, flip, crop, resize. These skip the LLM entirely and execute directly via Python code.
2. **Direct tool calls** — single-tool queries like "get GIMP version" or "describe this image"
3. **AI planning** — for complex edits, the LLM generates a multi-step plan with Python console commands, which are validated and executed sequentially

**Fast path detection:** Keyword-based heuristics in `heuristics.rs` match patterns like "blur the top half" → `blur_region(radius, "top")` or "draw a red heart" → `draw_heart("red")`. Color extraction (14 named colors), region extraction (top/bottom/left/right), and width extraction (digit sequences) are handled by helper functions.

**AI planning flow:**

1. `select_tool()` uses the LLM to classify the request into: GetGimpInfo, GetImageMetadata, CallApi, or None
2. For `CallApi`: `plan_call_api()` generates a JSON plan with `thought`, `explain`, and `steps` containing Python code
3. Python code targets GIMP's pyGObject-console API via `call_api(api_path="exec", args=["pyGObject-console", [...]])`
4. Plans are validated: must use `call_api`, must target `pyGObject-console`
5. Steps executed sequentially with error checking at each step

**Python macro generation** (`macros.rs`): All macros generate Python code using GIMP's GObject Introspection API (`gi.repository.Gimp`, `gi.repository.Gegl`). Operations include clipboard preservation for undo support.

**Plugin provisioning:**

1. Validate GIMP >= 3.x (rejects 2.x)
2. Detect target directory: `%APPDATA%\GIMP\{version}\plug-ins\gimp-mcp-plugin\` (Windows)
3. Copy plugin recursively from bundled resources
4. Set executable permissions (Unix)
5. Write provision marker with manifest version and timestamp

**GIMP version detection:** Infers from executable path (e.g., `gimp-3.exe`), falls back to `gimp --version` command output.

### LibreOffice Connector

**Source:** `connectors/libreoffice/src/`

The LibreOffice connector uses an MCP server over stdio to create and edit documents. A single `LibreOfficeProvider` serves both Writer and Impress modes, filtering available tools by a mode-specific allowlist.

**Architecture:**

1. A Python MCP server (`mcp_server.py`) runs as a child process with stdio transport
2. The server uses `python-docx`, `python-pptx`, and `odfdo` libraries to manipulate documents
3. The connector spawns the server on first connection and communicates via stdin/stdout MCP messages

**Mode profiles:**

- **Writer** — 22 tools: create/read documents, add text/headings/paragraphs/tables, insert images/page breaks, format text, search/replace, delete, apply styles, open in LibreOffice
- **Impress (Slides)** — 14 tools: create/read presentations, add/edit/delete slides, apply templates, format content, insert images, open in LibreOffice

**Execution flow:**

1. Filter tools by user intent (keyword matching narrows the catalog from 22+ to 1-4 relevant tools)
2. Build planner messages with system prompt + filtered tool schema
3. LLM generates a JSON tool call (temperature=0.0 for determinism, max_length=384)
4. Extract the tool call with robust JSON recovery:
   - Direct JSON parse
   - Balanced brace extraction from garbage-wrapped output
   - Markdown code fence extraction
   - JSON repair (comma-for-colon errors)
   - Fuzzy tool name matching (case-insensitive prefix/substring)
5. Execute tool via MCP session
6. Stream summary generation (30-second timeout, with local fallback summary)

**File context enrichment:** The executor scans conversation history for recent absolute file paths and appends them to the user message as context, so the planner knows which file to operate on.

**Retry logic:** Transient MCP failures (broken pipe, channel closed, connection reset, process exited) trigger auto-reconnection before retrying the operation.

## Mode Provider Registry

**Source:** `app/src-tauri/src/modes/registry.rs`

The `ModeProviderRegistry` maps `AppMode` variants to provider instances:

```rust
pub struct ModeProviderRegistry {
    pub code: Arc<dyn ToolProvider>,
    pub gimp: Arc<dyn ToolProvider>,
    pub blender: Arc<dyn ToolProvider>,
    pub libreoffice: Arc<dyn ToolProvider>,
}
```

The `provider_for_mode` method dispatches by `AppMode`:

| AppMode | Provider | Family |
|---|---|---|
| `Code` | `CodeProvider` | Code |
| `Gimp` | `GimpProvider` | Gimp |
| `Blender` | `BlenderProvider` | Blender |
| `Writer` | `LibreOfficeProvider` | LibreOffice |
| `Impress` | `LibreOfficeProvider` | LibreOffice |

Writer and Impress share the same `LibreOfficeProvider` instance — the `AppMode` parameter tells the provider which tool allowlist to apply.

The `CodeProvider` is a stateless placeholder that returns "idle" status and an empty tool list. Code mode uses the engine directly for chat completion without tool execution.

## Adding a New Connector

Step-by-step guide for adding a hypothetical "Inkscape" connector:

### 1. Create the crate

Create `connectors/inkscape/` with the standard structure:

```
connectors/inkscape/
  Cargo.toml
  src/
    lib.rs
    provider.rs       # ToolProvider implementation
    executor.rs       # Request execution logic
    setup.rs          # Host app detection, addon installation
    state.rs          # Connection state
  resources/
    manifest.json     # Resource metadata
```

Add it to the workspace `Cargo.toml` members list.

### 2. Implement ToolProvider

In `provider.rs`, implement all six trait methods. Key decisions:

- **IPC mechanism**: HTTP (like Blender), TCP MCP (like GIMP), stdio MCP (like LibreOffice), or something else
- **Connection lifecycle**: When to spawn bridge processes, how to detect the host app
- **Tool discovery**: Static tool list or dynamic (via MCP `tools/list`)

### 3. Add the AppMode variant

In `crates/smolpc-assistant-types/src/mode.rs`, add `Inkscape` to the `AppMode` enum.

### 4. Register in ModeProviderRegistry

In `app/src-tauri/src/modes/registry.rs`:

- Add `ProviderFamily::Inkscape`
- Add `inkscape: Arc<dyn ToolProvider>` field
- Wire up `AppMode::Inkscape` in `provider_family()` and `provider_for_mode()`

### 5. Add mode config

In `app/src-tauri/src/modes/config.rs`, add a `ModeConfigDto` for `AppMode::Inkscape` with label, subtitle, icon, suggestions, and capabilities.

### 6. Add setup support

In `setup.rs`, implement:

- Host app detection (register in `host_apps.rs` if needed)
- Addon/plugin provisioning with version markers
- Setup item generation for the setup wizard

### 7. Bundle resources

In `app/src-tauri/tauri.conf.json`, add the connector's resources to the bundle configuration so they ship with the installer.

### 8. Add frontend mode entry

The frontend picks up new modes automatically from the `list_modes` API call. The mode store (`mode.svelte.ts`) and dropdown (`AppModeDropdown.svelte`) iterate over `modeConfigs`, so no frontend code changes are needed beyond adding an icon mapping if using a custom icon.

## Pattern Comparison

| Aspect | Blender | GIMP | LibreOffice |
|---|---|---|---|
| **IPC** | HTTP REST (Axum) | TCP MCP | stdio MCP |
| **Bridge** | Rust HTTP server in-process | Python bridge process | Python MCP server process |
| **Host Plugin** | Python addon (`bpy`) | Python extension (`gi.repository.Gimp`) | None (standalone doc manipulation) |
| **Tool Discovery** | Static (2 tools) | Dynamic (MCP `tools/list`) | Dynamic (MCP `tools/list`) |
| **Execution Model** | RAG + LLM tutoring | Fast paths + LLM planning | LLM planning + tool call |
| **LLM Output** | Natural language instructions | Python console commands | JSON tool call |
| **Undo** | Not supported | Clipboard-based | Not supported |
| **Modes** | 1 (Blender) | 1 (Gimp) | 2 (Writer, Impress) |
| **Auth** | Bearer token (48-char random) | Loopback-only (127.0.0.1) | stdio (same process tree) |
| **Fast Paths** | Smart RAG skip | 30+ keyword macros | Intent-based tool filtering |
