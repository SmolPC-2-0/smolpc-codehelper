# Mode Capabilities

SmolPC Code Helper supports five application modes. Each mode routes through the unified assistant shell but targets a different host application with mode-specific tools, system prompts, and capabilities.

## Capability Matrix

| Capability | Code | GIMP | Blender | Writer | Impress |
|---|---|---|---|---|---|
| Streaming chat | Yes | Yes | Yes | Yes | Yes |
| Multi-turn conversation | Yes | Yes | Yes | Yes | Yes |
| Tool execution | No | Yes | Yes | Yes | Yes |
| Undo support | No | Yes (clipboard) | No | No | No |
| Voice I/O | Yes | Yes | Yes | Yes | Yes |
| Host app required | No | GIMP 3.x | Blender | LibreOffice | LibreOffice |
| Model info panel | Yes | Yes | Yes | Yes | Yes |
| Hardware panel | Yes | Yes | Yes | Yes | Yes |
| Export chat | Yes | No | No | No | No |
| Context controls | Yes | No | No | No | No |
| Provider kind | Local | MCP | Hybrid | MCP | MCP |

## Code Mode

**Provider:** `CodeProvider` (built-in, stateless)
**Tools:** None
**Host app:** None

Code mode is the default. It provides direct access to the inference engine for coding assistance — bug fixes, code generation, explanations, and reviews. There is no tool execution layer; the LLM response is the final output.

**What it does:**
- Streaming chat with syntax-highlighted code blocks
- Multi-turn conversation with full history
- Code generation from natural-language prompts
- Bug analysis and fix suggestions
- Code review and explanation
- Voice input/output (Windows only)

**What makes it unique:**
- Only mode with chat export
- Only mode with context controls (adjusts how much conversation history is sent)
- No connection to any external application — works immediately after engine startup
- System prompt tuned for coding assistance with secondary school students

**Suggestions:** "Fix this bug and explain the root cause", "Write a function from this prompt", "Review this snippet for mistakes"

## GIMP Mode

**Provider:** `GimpProvider` (MCP over TCP)
**Tools:** Dynamic, discovered via MCP `tools/list` from the GIMP plugin
**Host app:** GIMP 3.x (GIMP 2.x rejected during setup)

GIMP mode provides AI-assisted image editing. It uses a three-tier execution model: pre-computed fast paths for common operations, direct tool calls for simple queries, and LLM-planned multi-step edits for complex requests.

**What it does:**
- 30+ fast-path operations: draw shapes (line, heart, circle, oval, triangle, rectangle, square), brightness/contrast (full image or regions), blur (full or region), rotate (90/180/270), flip (horizontal/vertical), crop to square, resize by width
- AI-planned PDB operations for complex edits
- Image metadata inspection (dimensions, base type, filename)
- GIMP environment info (version, platform)
- Clipboard-backed undo for reversible operations

**What makes it unique:**
- Only mode with undo support (uses GIMP clipboard to preserve state before edits)
- Fast paths bypass the LLM entirely — keyword heuristics detect common patterns and execute pre-built Python macros
- Color extraction for shapes (14 named colors: red, blue, green, yellow, orange, purple, pink, cyan, magenta, brown, grey, black, white; default: blue)
- Region-aware operations ("blur the top half", "brighten the left side")
- Requires GIMP to be running with the provisioned plugin active

**IPC chain:** Rust connector → TCP MCP → Python bridge (port 10008) → TCP socket → GIMP plugin (port 9877) → GIMP PDB

**Suggestions:** "Blur the top half of the image", "Crop this image to a square", "Rotate the image 90 degrees clockwise"

## Blender Mode

**Provider:** `BlenderProvider` (Hybrid — HTTP REST + local RAG)
**Tools:** 2 static tools: `scene_current`, `retrieve_rag_context`
**Host app:** Blender (any version with Python addon support)

Blender mode provides scene-aware tutoring. Unlike GIMP mode, it does not execute operations in Blender — it retrieves the live scene context and generates step-by-step UI instructions grounded in Blender API documentation.

**What it does:**
- Live scene awareness: object list, active object, edit mode, render engine, modifier stacks
- UI-based instruction generation: menus, hotkeys, panels, property editors
- Blender API documentation retrieval via keyword RAG
- Scene-state questions answered directly from the live snapshot
- Workflow guidance for modeling, modifiers, materials, and animation

**What makes it unique:**
- Hybrid provider: combines local RAG (keyword search over bundled Blender API docs) with engine generation
- Never generates Python code — the system prompt explicitly forbids `bpy` commands and requires UI-only instructions
- Smart RAG skipping: scene-state questions ("what's in my scene") skip documentation retrieval since the answer is already in the snapshot
- Scene staleness detection: data older than 30 seconds is reported as disconnected
- Up to 40 objects listed in the prompt with types and modifier details

**IPC chain:** Blender addon → HTTP POST `/scene/update` → Rust bridge server (port 5179) → Scene cache → Connector reads cache

**Suggestions:** "What is in my scene right now?", "How do I add a bevel to the selected object?", "Explain what this modifier stack is doing"

## Writer Mode

**Provider:** `LibreOfficeProvider` (MCP over stdio)
**Tools:** 22 tools (filtered from full catalog by Writer allowlist)
**Host app:** LibreOffice (for opening created documents)

Writer mode creates and edits documents through a Python MCP server that uses `python-docx` and `odfdo` for document manipulation. The LLM acts as a planner — it selects which tool to call and generates the arguments.

**What it does:**
- Create blank documents
- Read document content
- Add text, headings, paragraphs, tables
- Insert images and page breaks
- Format text (bold, italic, font size, color)
- Search and replace
- Delete text or paragraphs
- Apply document styles
- Open completed documents in LibreOffice

**Available tools:** `create_blank_document`, `read_text_document`, `get_document_properties`, `list_documents`, `copy_document`, `add_text`, `add_heading`, `add_paragraph`, `add_table`, `insert_image`, `insert_page_break`, `format_text`, `search_replace_text`, `delete_text`, `format_table`, `delete_paragraph`, `apply_document_style`, `open_in_libreoffice`

**What makes it unique:**
- Intent-based tool pre-filtering: keywords in the user's request narrow the tool catalog from 22 to 1-4 relevant tools before the LLM sees it
- File context enrichment: scans chat history for file paths and injects them as context
- Robust JSON extraction: handles malformed LLM output with balanced brace extraction, markdown fence removal, comma-for-colon repair, and fuzzy tool name matching
- 30-second summary timeout with local fallback (extracts document count and names from tool payload)

**Suggestions:** "Create a blank document called homework", "Add a level 1 heading 'Introduction'", "Insert a two-column table with headers"

## Impress Mode

**Provider:** `LibreOfficeProvider` (MCP over stdio, shared with Writer)
**Tools:** 14 tools (filtered from full catalog by Impress allowlist)
**Host app:** LibreOffice (for opening created presentations)

Impress mode (labeled "Slides" in the UI) creates and edits presentations. It shares the same provider and MCP server as Writer mode but exposes a different set of tools.

**What it does:**
- Create blank presentations
- Read presentation content
- Add, edit, and delete slides
- Edit slide titles and content
- Apply presentation templates
- Format slide text
- Insert images on slides
- Open completed presentations in LibreOffice

**Available tools:** `create_blank_presentation`, `read_presentation`, `get_document_properties`, `list_documents`, `copy_document`, `add_slide`, `edit_slide_content`, `edit_slide_title`, `delete_slide`, `apply_presentation_template`, `format_slide_content`, `format_slide_title`, `insert_slide_image`, `open_in_libreoffice`

**Suggestions:** "Create a blank presentation called 'Science Fair'", "Add a title slide with heading 'Results'", "Insert an image on slide 2"

## Shared Across All Modes

These features work identically regardless of which mode is active:

- **Inference engine** — all modes use the same engine process on `localhost:19432` for LLM generation
- **Chat history** — each mode maintains its own conversation history, persisted per-mode in the chat store
- **Model selection** — the active model (Qwen 2.5 or Qwen 3) and backend (CPU, DirectML, NPU) apply to all modes
- **Hardware detection** — GPU, NPU, CPU, and RAM information is shared across modes
- **Setup wizard** — first-run setup covers all modes: model provisioning, host app detection, addon/plugin installation, Python runtime staging
- **Voice I/O** — Whisper STT and TTS sidecar are available in every mode (Windows only)
- **Streaming events** — all modes emit the same `AssistantStreamEventDto` variants: `Status`, `ToolCall`, `ToolResult`, `Token`, `Complete`, `Error`

## What Varies Per Mode

| Aspect | Where Configured |
|---|---|
| System prompt | `system_prompt_key` in `ModeConfigDto` |
| Available tools | Provider's `list_tools()` (static or MCP-discovered, filtered by mode) |
| Tool execution | Provider's `execute_tool()` (IPC mechanism varies) |
| Host app IPC | Provider implementation (HTTP, TCP MCP, stdio MCP, or none) |
| Bundled resources | `connectors/{name}/resources/` (addon, plugin, RAG index, MCP server) |
| UI suggestions | `suggestions` in `ModeConfigDto` |
| Capabilities | `ModeCapabilitiesDto` flags (undo, export, context controls) |

## Frontend Integration

The mode system is reactive. The frontend mode store (`mode.svelte.ts`) tracks:

- `activeMode` — persisted to localStorage (`smolpc_unified_active_mode_v1`)
- `modeConfigs` — loaded from the `list_modes` Tauri command at startup
- `statusByMode` — per-mode status refreshed on mode switch

The `AppModeDropdown` component renders a horizontal tab bar (desktop) or dropdown (mobile) from the mode config list. Unavailable modes (host app not detected) show reduced opacity with a reason message. Keyboard navigation supports Arrow Up/Down, Enter, Space, and Escape.

New modes added on the backend appear automatically in the dropdown — the frontend iterates over whatever `list_modes` returns.
