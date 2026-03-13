# MCP Integration

> **Purpose:** Defines the Model Context Protocol (MCP) integration strategy for all application modes — GIMP, Blender, LibreOffice (Writer, Calc, Impress). Covers the generalized MCP client, per-application MCP servers, tool selection, plan validation, and execution patterns.
>
> **Audience:** Any AI session working on MCP client implementation, MCP server integration, tool routing, or mode-specific tool execution.
>
> **Last Updated:** 2026-03-13

---

## Table of Contents

1. [MCP Overview](#mcp-overview)
2. [Generalized MCP Client](#generalized-mcp-client)
3. [GIMP Integration](#gimp-integration)
4. [Blender Integration](#blender-integration)
5. [LibreOffice Integration](#libreoffice-integration)
6. [Tool Selection & Prompt Templates](#tool-selection--prompt-templates)
7. [Plan Validation & Execution Pattern](#plan-validation--execution-pattern)
8. [MCP Server Lifecycle Management](#mcp-server-lifecycle-management)
9. [Error Handling & Recovery](#error-handling--recovery)

---

## MCP Overview

### What is MCP?

The **Model Context Protocol** (MCP) is a standard for connecting AI models to external tools and data sources. It defines a JSON-RPC 2.0 protocol for:

- **Tool discovery:** Client asks server "what tools do you have?"
- **Tool execution:** Client tells server "run this tool with these arguments"
- **Resource access:** Client requests data from server (files, database records, etc.)
- **Prompt templates:** Server provides pre-built prompt templates for common tasks

### Why MCP for SmolPC?

Each application (GIMP, Blender, LibreOffice) has its own API for programmatic control. MCP servers wrap these APIs into a standardized protocol, so the unified assistant's MCP client can talk to any application the same way.

Without MCP:
- GIMP: Script-Fu console over TCP
- Blender: Python API via addon
- LibreOffice: UNO API via Python bridge

With MCP:
- All applications: JSON-RPC `tools/call` with tool name + arguments

### MCP Transport Options

| Transport | How It Works | Used By |
|-----------|-------------|---------|
| **stdio** | Client spawns server as child process, communicates via stdin/stdout | LibreOffice MCP |
| **TCP** | Client connects to server's TCP socket | GIMP MCP (port 10008) |
| **HTTP** | Client sends HTTP requests to server endpoint | Blender HTTP bridge (not MCP) |
| **SSE** | Server pushes events over HTTP (Server-Sent Events) | Future use |

### MCP Message Format

```json
// Client → Server: List available tools
{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}

// Server → Client: Tool list response
{
  "jsonrpc": "2.0", "id": 1,
  "result": {
    "tools": [
      {
        "name": "draw_line",
        "description": "Draw a line on the active image",
        "inputSchema": {
          "type": "object",
          "properties": {
            "x1": {"type": "number"},
            "y1": {"type": "number"},
            "x2": {"type": "number"},
            "y2": {"type": "number"}
          },
          "required": ["x1", "y1", "x2", "y2"]
        }
      }
    ]
  }
}

// Client → Server: Call a tool
{
  "jsonrpc": "2.0", "id": 2,
  "method": "tools/call",
  "params": {
    "name": "draw_line",
    "arguments": {"x1": 50, "y1": 50, "x2": 200, "y2": 200}
  }
}

// Server → Client: Tool result
{
  "jsonrpc": "2.0", "id": 2,
  "result": {
    "content": [
      {"type": "text", "text": "Line drawn successfully from (50,50) to (200,200)"}
    ]
  }
}
```

---

## Generalized MCP Client

### Architecture

The unified Tauri app needs a single MCP client that can connect to multiple MCP servers simultaneously (one per application mode). The client must support all three transport types.

```
┌────────────────────────────────────────────┐
│            MCP Client (Rust)                │
│                                            │
│  ┌──────────────────────────────────────┐  │
│  │         Connection Manager            │  │
│  │  ┌─────────┐ ┌─────┐ ┌──────────┐  │  │
│  │  │ stdio   │ │ TCP │ │ HTTP/SSE │  │  │
│  │  │transport│ │trans│ │ transport│  │  │
│  │  └────┬────┘ └──┬──┘ └────┬─────┘  │  │
│  │       │         │         │          │  │
│  │  ┌────┴─────────┴─────────┴───────┐  │  │
│  │  │     JSON-RPC 2.0 Layer         │  │  │
│  │  │  (serialize/deserialize/route) │  │  │
│  │  └────────────────────────────────┘  │  │
│  └──────────────────────────────────────┘  │
│                                            │
│  ┌──────────────────────────────────────┐  │
│  │         Mode Router                   │  │
│  │  GIMP mode → gimp-mcp (TCP :10008)   │  │
│  │  Blender  → blender-mcp (TCP :9876)  │  │
│  │             + HTTP bridge (:5179)     │  │
│  │  Writer   → mcp-libre (stdio)        │  │
│  │  Calc     → mcp-libre (stdio)        │  │
│  │  Impress  → mcp-libre (stdio)        │  │
│  └──────────────────────────────────────┘  │
└────────────────────────────────────────────┘
```

### Rust Interface (Proposed)

```rust
/// MCP client configuration per mode
pub struct McpServerConfig {
    pub name: String,           // "gimp-mcp", "blender-mcp", "mcp-libre"
    pub transport: Transport,   // Stdio, Tcp, Http
    pub auto_start: bool,       // Whether to spawn server on mode switch
}

pub enum Transport {
    Stdio {
        command: String,        // "uv run gimp-mcp"
        args: Vec<String>,
        cwd: PathBuf,
    },
    Tcp {
        host: String,           // "127.0.0.1"
        port: u16,              // 10008
    },
    Http {
        url: String,            // "http://127.0.0.1:5179"
    },
}

/// MCP client trait
pub trait McpClient {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn list_tools(&self) -> Result<Vec<Tool>>;
    async fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<ToolResult>;
    async fn is_connected(&self) -> bool;
}

/// Tool definition from MCP server
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Result from tool execution
pub struct ToolResult {
    pub content: Vec<Content>,
    pub is_error: bool,
}
```

### Connection Lifecycle

```
Mode switch to GIMP:
1. Disconnect from previous MCP server (if any)
2. Check if GIMP MCP server is running (TCP connect test to :10008)
3. If not running and auto_start:
   a. Spawn MCP server via uv: `uv run --python venvs/gimp-mcp/ gimp-mcp`
   b. Wait for TCP port to become available (with timeout)
4. Connect to MCP server
5. Call tools/list to discover available tools
6. Update UI with available tools for this mode
7. Ready for user interaction
```

---

## GIMP Integration

### MCP Server: `maorcc/gimp-mcp`

| Property | Value |
|----------|-------|
| GitHub | `maorcc/gimp-mcp` |
| Stars | 58 |
| Transport | TCP on port 10008 |
| Language | Python |
| Protocol | MCP (JSON-RPC 2.0) |
| Status | **Already integrated** in GIMP assistant |

### How It Works

```
GIMP (running) ←─ Script-Fu Console (TCP) ─→ gimp-mcp (Python) ←─ MCP (TCP :10008) ─→ Tauri App
```

1. GIMP exposes a Script-Fu console on a TCP port
2. `gimp-mcp` connects to Script-Fu and wraps GIMP's API as MCP tools
3. The Tauri app connects to `gimp-mcp` on port 10008

### Key Tools

| Tool | Description | Use Case |
|------|-------------|----------|
| `call_api` | Execute arbitrary GIMP PDB procedure | **Escape hatch** — any GIMP API call |
| `list_images` | List open images | Context gathering |
| `get_image_info` | Get image dimensions, layers, etc. | Context gathering |
| `create_layer` | Create a new layer | Layer operations |
| `apply_filter` | Apply a GIMP filter | Image processing |

### `call_api` Escape Hatch

The most important tool. It allows arbitrary GIMP API calls:

```json
{
  "name": "call_api",
  "arguments": {
    "api_path": "exec",
    "args": ["pyGObject-console", [
      "from gi.repository import Gimp",
      "image = Gimp.get_images()[0]",
      "Gimp.displays_flush()"
    ]],
    "kwargs": {}
  }
}
```

This means the model can execute ANY GIMP operation, not just the predefined tools. The model generates Python code for complex operations.

### Fast Paths (Tauri Commands)

The GIMP assistant has Tauri commands that bypass MCP for common operations:

```rust
// In src-tauri/src/commands/
#[tauri::command]
async fn macro_draw_line(x1: i32, y1: i32, x2: i32, y2: i32) -> Result<(), String> { ... }

#[tauri::command]
async fn macro_crop_square() -> Result<(), String> { ... }

#[tauri::command]
async fn macro_resize(width: i32) -> Result<(), String> { ... }

#[tauri::command]
async fn macro_undo() -> Result<(), String> { ... }
```

These call MCP internally but provide a simpler API for the frontend. The unified assistant should preserve this pattern for common operations.

### Current Integration (Reference)

The GIMP assistant's current integration is in:
- `apps/gimp-assistant/src-tauri/src/commands/mcp.rs` — MCP client Tauri commands
- `apps/gimp-assistant/src-tauri/src/commands/macros.rs` — Fast path macros
- `apps/gimp-assistant/src-tauri/src/mcp/` — MCP client implementation
- `apps/gimp-assistant/src-tauri/src/plan_llm.rs` — Plan validation + execution

---

## Blender Integration

### Decision: Hybrid Approach

The blender-assistant is **production-ready (v7.0.0)**, not scaffolding. It uses a custom HTTP bridge, NOT MCP. The decision is to keep both:

1. **Existing HTTP bridge** — For proven production features (scene analysis, code generation, RAG)
2. **blender-mcp** — For broader MCP ecosystem compatibility and additional tools

### Existing HTTP Bridge

| Property | Value |
|----------|-------|
| Framework | Axum on port 5179 |
| Auth | Token-based |
| Transport | HTTP |
| Language | Rust (server) + Python (Blender addon) |

```
Blender (running)
  └─ blender_helper_http.py (addon, HTTP client)
       └─ sends scene data, asks questions
            └─→ blender-assistant (Axum :5179)
                  └─ has RAG (TF-IDF, keyword-based) for Blender docs
                  └─ talks to smolpc-engine-host (:19432) for inference
```

**Capabilities:**
- Scene data extraction (objects, materials, modifiers)
- Code generation (Blender Python API)
- Keyword-based RAG for Blender documentation (TF-IDF, no embeddings)
- Token-authenticated HTTP endpoints
- Ollama fallback (opt-in, for development without engine)

### blender-mcp (New Addition)

| Property | Value |
|----------|-------|
| GitHub | `ahujasid/blender-mcp` |
| Stars | 17,700 |
| Transport | TCP on port 9876 |
| Language | Python |
| Protocol | MCP (JSON-RPC 2.0) |

**Key Tool:**
```json
{
  "name": "execute_blender_code",
  "description": "Execute Python code in Blender's Python environment",
  "inputSchema": {
    "type": "object",
    "properties": {
      "code": {"type": "string", "description": "Python code to execute in Blender"}
    },
    "required": ["code"]
  }
}
```

### Hybrid Integration Architecture

```
Unified Tauri App (Blender mode)
├── MCP Client → blender-mcp (TCP :9876)
│   └── execute_blender_code: arbitrary Python execution in Blender
│   └── Other MCP tools: scene inspection, material editing, etc.
│
├── HTTP Client → blender-assistant bridge (HTTP :5179)
│   └── RAG queries: "How do I create a particle system?"
│   └── Scene analysis: "What objects are in the scene?"
│   └── Code generation: "Generate code to create a sphere"
│
└── Both talk to the same smolpc-engine-host (:19432)
```

### Why Hybrid?

- **HTTP bridge** is proven, has RAG, understands scene context. Don't discard working code.
- **blender-mcp** has ecosystem momentum (17.7K stars), provides standard MCP interface.
- Both can coexist — different ports, different protocols.
- Over time, can migrate HTTP bridge features into MCP server if desired.

---

## LibreOffice Integration

### MCP Server: `patrup/mcp-libre`

| Property | Value |
|----------|-------|
| GitHub | `patrup/mcp-libre` |
| Tools (standalone) | 14 |
| Tools (extension mode) | 73 |
| Transport | stdio (spawned as child process) |
| Language | Python |
| Protocol | MCP (JSON-RPC 2.0) |
| Status | Official MCP listing |

### Why `mcp-libre`?

It's the most comprehensive LibreOffice MCP server available:
- Official MCP registry listing
- Covers Writer, Calc, AND Impress
- Extension mode unlocks 73 tools (vs 14 in standalone)
- Active development

### Tool Categories

#### Writer Mode

| Tool | Description |
|------|-------------|
| `create_document` | Create new Writer document |
| `insert_text` | Insert text at position |
| `format_text` | Apply formatting (bold, italic, font, size) |
| `insert_table` | Create a table |
| `insert_image` | Insert an image from file |
| `save_document` | Save current document |
| `export_pdf` | Export to PDF |
| `find_replace` | Find and replace text |
| ... (extension mode adds many more) | |

#### Calc Mode

| Tool | Description |
|------|-------------|
| `create_spreadsheet` | Create new Calc spreadsheet |
| `set_cell_value` | Set value in specific cell |
| `get_cell_value` | Read value from cell |
| `format_cells` | Apply cell formatting |
| `insert_formula` | Insert formula in cell |
| `create_chart` | Create chart from data range |
| `sort_range` | Sort data range |
| ... | |

#### Impress Mode

| Tool | Description |
|------|-------------|
| `create_presentation` | Create new Impress presentation |
| `add_slide` | Add new slide |
| `insert_text_box` | Add text box to slide |
| `insert_image` | Add image to slide |
| `set_slide_layout` | Set slide layout template |
| `add_animation` | Add animation to element |
| ... | |

### Extension Mode vs Standalone

- **Standalone (14 tools):** Basic document operations. Works without LibreOffice running.
- **Extension mode (73 tools):** Full API access. Requires LibreOffice to be running with the extension installed.

**Recommendation:** Use extension mode for maximum capability. The user will already have LibreOffice open when using Writer/Calc/Impress modes.

### stdio Transport

Unlike GIMP (TCP) and Blender (TCP), `mcp-libre` uses **stdio** transport:

```
Tauri App spawns:  uv run --python venvs/libre-mcp/ mcp-libre --mode extension
                   ├── stdin:  Tauri writes JSON-RPC requests
                   └── stdout: Tauri reads JSON-RPC responses
```

The MCP client must handle stdio communication (pipe stdin/stdout of the child process). This is the simplest transport — no port conflicts, no TCP connection management.

---

## Tool Selection & Prompt Templates

### System Prompt Structure

Each mode has a system prompt that includes:
1. **Role definition** — What the assistant does in this mode
2. **Available tools** — List of tools from the MCP server
3. **Tool usage instructions** — How to format tool calls
4. **Constraints** — What the assistant should NOT do

### Example: GIMP Mode System Prompt

```
You are a GIMP image editing assistant. You help students edit images using GIMP.

Available tools:
- call_api: Execute any GIMP API procedure
- list_images: List open images
- get_image_info: Get image details
- create_layer: Create a new layer
- apply_filter: Apply a GIMP filter

When the user asks you to edit an image:
1. First check what image is open using list_images
2. Get the image details using get_image_info
3. Plan the edits needed
4. Execute the edits using call_api or specific tools
5. Summarize what you did

Always explain what you're doing in simple terms. Students are ages 11-18.
Do not execute destructive operations without confirmation.
```

### Dynamic Tool Injection

Tools are discovered dynamically via `tools/list`, NOT hardcoded. The system prompt is built at runtime:

```rust
async fn build_system_prompt(mode: &AppMode, mcp: &McpClient) -> String {
    let tools = mcp.list_tools().await?;
    let tool_descriptions = tools.iter()
        .map(|t| format!("- {}: {}", t.name, t.description))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "{role_prompt}\n\nAvailable tools:\n{tool_descriptions}\n\n{usage_instructions}",
        role_prompt = mode.role_prompt(),
        tool_descriptions = tool_descriptions,
        usage_instructions = mode.tool_usage_instructions(),
    )
}
```

### Per-Mode Suggestion Chips

Each mode shows different quick-action suggestions in the empty state:

| Mode | Suggestions |
|------|-------------|
| GIMP | "Draw a red circle", "Increase brightness", "Blur the image", "Resize to 800px" |
| Blender | "Create a cube", "Add a material", "Set up lighting", "Render the scene" |
| Writer | "Create a new document", "Make the title bold", "Insert a table", "Export to PDF" |
| Calc | "Create a formula", "Format as currency", "Create a chart", "Sort by column A" |
| Impress | "Add a new slide", "Insert a title", "Add an image", "Set slide transition" |

---

## Plan Validation & Execution Pattern

### Origin: GIMP Assistant's `plan_llm.rs`

The GIMP assistant has a proven pattern for structured tool execution. This pattern should be generalized for all modes.

### Pattern Description

```
User: "Draw a red circle in the center of the image"

1. PLAN GENERATION
   Model generates a structured plan:
   [
     {"step": 1, "tool": "get_image_info", "args": {}, "reason": "Get image dimensions"},
     {"step": 2, "tool": "call_api", "args": {"api_path": "exec", "args": [...]}, "reason": "Create circular selection"},
     {"step": 3, "tool": "call_api", "args": {"api_path": "exec", "args": [...]}, "reason": "Set foreground to red"},
     {"step": 4, "tool": "call_api", "args": {"api_path": "exec", "args": [...]}, "reason": "Fill selection"},
     {"step": 5, "tool": "call_api", "args": {"api_path": "exec", "args": [...]}, "reason": "Remove selection"}
   ]

2. PLAN VALIDATION
   For each step:
   - Tool name exists in available tools? ✓
   - Arguments match tool's input schema? ✓
   - No dangerous operations (file deletion, network access)? ✓
   - Dependencies satisfied (step 2 needs step 1's result)? ✓

3. PLAN EXECUTION
   Execute steps sequentially:
   - Step 1: get_image_info → {width: 800, height: 600}
   - Step 2: call_api → "Selection created"
   - Step 3: call_api → "Foreground set to #ff0000"
   - Step 4: call_api → "Selection filled"
   - Step 5: call_api → "Selection removed"

4. RESULT SUMMARY
   Model summarizes: "I drew a red circle (radius 100px) centered at (400, 300) on your image."

5. UNDO SUPPORT
   Push undo marker. User can click "Undo" to revert all steps.
```

### Generalized Plan Executor (Proposed)

```rust
pub struct PlanStep {
    pub step_number: u32,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub reason: String,
    pub depends_on: Option<u32>,  // Previous step this depends on
}

pub struct Plan {
    pub steps: Vec<PlanStep>,
    pub description: String,
}

pub struct PlanExecutor {
    mcp: Arc<dyn McpClient>,
    available_tools: Vec<Tool>,
}

impl PlanExecutor {
    /// Validate plan against available tools
    pub fn validate(&self, plan: &Plan) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        for step in &plan.steps {
            // Check tool exists
            if !self.available_tools.iter().any(|t| t.name == step.tool_name) {
                errors.push(ValidationError::UnknownTool(step.tool_name.clone()));
            }
            // Check arguments match schema
            if let Some(tool) = self.available_tools.iter().find(|t| t.name == step.tool_name) {
                if let Err(e) = validate_args(&step.arguments, &tool.input_schema) {
                    errors.push(ValidationError::InvalidArgs(step.step_number, e));
                }
            }
            // Check dependencies
            if let Some(dep) = step.depends_on {
                if dep >= step.step_number {
                    errors.push(ValidationError::CircularDependency(step.step_number));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    /// Execute plan steps sequentially
    pub async fn execute(&self, plan: &Plan, on_progress: Channel<PlanProgress>) -> Result<Vec<ToolResult>> {
        let mut results = Vec::new();
        for step in &plan.steps {
            on_progress.send(PlanProgress::StepStarted(step.step_number, step.reason.clone()))?;
            let result = self.mcp.call_tool(&step.tool_name, step.arguments.clone()).await?;
            if result.is_error {
                on_progress.send(PlanProgress::StepFailed(step.step_number, result.error_text()))?;
                return Err(format!("Step {} failed: {}", step.step_number, result.error_text()).into());
            }
            on_progress.send(PlanProgress::StepCompleted(step.step_number))?;
            results.push(result);
        }
        Ok(results)
    }
}
```

### Tool Call Format in Model Output

The model needs to output tool calls in a parseable format. Two approaches:

#### Approach 1: JSON in Markdown Code Block

```markdown
```tool_call
{"tool": "draw_line", "arguments": {"x1": 50, "y1": 50, "x2": 200, "y2": 200}}
```​
```

#### Approach 2: Native Function Calling (Preferred)

If the model supports native function calling (Qwen3 does), use the model's built-in tool call format:

```json
// In the system prompt, define tools using the model's expected format
// Qwen3 uses a specific format for tool definitions and calls
// The model outputs structured tool calls that can be parsed directly
```

**Recommendation:** Use native function calling if the model supports it. Fall back to JSON code blocks for models without native support. The plan executor should handle both formats.

---

## MCP Server Lifecycle Management

### Server States

```
┌─────────┐     spawn     ┌──────────┐    connect    ┌───────────┐
│ Stopped │──────────────→│ Starting │─────────────→│ Connected │
└─────────┘               └──────────┘               └─────┬─────┘
     ↑                         │                           │
     │         timeout         │      disconnect /         │
     │←────────────────────────┘      mode switch          │
     │                                                     │
     └─────────────────────────────────────────────────────┘
```

### Startup Strategy

| Mode | Server | Auto-Start? | Notes |
|------|--------|-------------|-------|
| GIMP | gimp-mcp | No — requires GIMP to be running | Check if GIMP's Script-Fu port is open |
| Blender | blender-mcp | No — requires Blender to be running | Check if Blender addon's port is open |
| Blender | HTTP bridge | Yes — can start independently | Spawned as sidecar |
| Writer | mcp-libre | Yes — stdio transport | Spawned on mode switch |
| Calc | mcp-libre | Yes — same server, different context | Reuse Writer's instance |
| Impress | mcp-libre | Yes — same server, different context | Reuse Writer's instance |

### Health Checking

```rust
/// Check if an MCP server is alive and responding
async fn health_check(client: &McpClient) -> McpHealth {
    match client.list_tools().await {
        Ok(tools) if !tools.is_empty() => McpHealth::Healthy(tools.len()),
        Ok(_) => McpHealth::Connected, // Connected but no tools
        Err(_) => McpHealth::Disconnected,
    }
}
```

### Graceful Shutdown

When the app closes or mode switches:
1. Send any pending tool calls' cancel signals
2. Close MCP connection cleanly
3. If we spawned the server (stdio), send SIGTERM and wait for exit
4. If server was pre-existing (TCP), just disconnect

---

## Error Handling & Recovery

### Common Errors

| Error | Cause | Recovery |
|-------|-------|----------|
| Connection refused | MCP server not running | Show "Start GIMP first" message |
| Timeout | MCP server hung | Cancel request, reconnect |
| Unknown tool | Model hallucinated a tool name | Retry with correct tool list in prompt |
| Tool execution error | GIMP/Blender/LO API error | Report error to user, suggest alternative |
| Invalid arguments | Model passed wrong arg types | Retry with corrected arguments |

### Retry Strategy

```rust
const MAX_RETRIES: u32 = 2;
const RETRY_DELAY_MS: u64 = 500;

async fn call_tool_with_retry(
    client: &McpClient,
    name: &str,
    args: Value,
) -> Result<ToolResult> {
    let mut last_error = None;
    for attempt in 0..=MAX_RETRIES {
        match client.call_tool(name, args.clone()).await {
            Ok(result) if !result.is_error => return Ok(result),
            Ok(result) => {
                last_error = Some(result.error_text());
                // Tool returned an error — don't retry (it's a logical error, not transient)
                break;
            }
            Err(e) => {
                last_error = Some(e.to_string());
                if attempt < MAX_RETRIES {
                    tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                    // Try reconnecting
                    let _ = client.reconnect().await;
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| "Unknown error".into()).into())
}
```

### User-Facing Error Messages

Errors should be:
- **Simple** — "GIMP isn't connected. Make sure GIMP is running." (not "TCP connection to 127.0.0.1:10008 failed: ECONNREFUSED")
- **Actionable** — Tell the user what to do, not what went wrong technically
- **Age-appropriate** — Students are 11-18, keep it friendly

### Connection Status UI

The status bar shows per-mode connection status:
- 🟢 Connected (MCP server responding)
- 🟡 Connecting... (attempting to connect)
- 🔴 Disconnected (server not available)
- ⚙️ Starting... (spawning server process)
