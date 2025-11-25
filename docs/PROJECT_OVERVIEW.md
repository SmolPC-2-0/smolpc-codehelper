# SmolPC CodeHelper - Complete Project Overview

**Last Updated:** November 18, 2024

## Table of Contents

1. [Project Mission](#project-mission)
2. [What is SmolPC CodeHelper?](#what-is-smolpc-codehelper)
3. [Technology Stack](#technology-stack)
4. [Project Architecture](#project-architecture)
5. [Current Features](#current-features)
6. [LibreOffice Integration (In Progress)](#libreoffice-integration-in-progress)
7. [Codebase Structure](#codebase-structure)
8. [How It All Works](#how-it-all-works)
9. [Development Roadmap](#development-roadmap)
10. [Getting Started for Contributors](#getting-started-for-contributors)

---

## Project Mission

**SmolPC 2.0** aims to create an integrated offline AI assistant ecosystem for students with low-resource computers. The project consists of three main tools unified into a single application:

1. **CodeHelper** - AI-powered coding assistant
2. **LibreOffice AI** - AI integration for document editing
3. **Blender AI** - AI integration for 3D modeling

**Why?** Students in resource-constrained environments need powerful tools that:
- Run completely offline (no internet required)
- Work on low-end hardware (2-4GB RAM laptops)
- Provide professional-grade AI assistance
- Create small, portable executables (~10MB)

---

## What is SmolPC CodeHelper?

SmolPC CodeHelper is a **desktop application** that serves as an AI-powered coding assistant running entirely offline using local LLMs (Large Language Models) via Ollama.

### Key Capabilities:

**Current (CodeHelper MVP):**
- ✅ Chat with local AI models (Qwen 2.5 Coder, DeepSeek Coder, etc.)
- ✅ Streaming responses with real-time updates
- ✅ Read and edit code files directly from the UI
- ✅ Save generated code to disk
- ✅ Performance benchmarking for model comparison
- ✅ Tiny executable size (~10MB for the app itself)

**In Progress (LibreOffice Integration):**
- 🚧 Create, edit, and format LibreOffice documents via AI
- 🚧 Natural language document operations
- 🚧 27 LibreOffice tools (text, tables, images, presentations)

**Planned (Blender Integration):**
- 📅 3D modeling assistance
- 📅 Scene creation and manipulation
- 📅 Material and shader editing

---

## Technology Stack

### Frontend
- **Svelte 5** (Runes mode) - Reactive UI framework
- **TypeScript** - Type-safe JavaScript
- **Tailwind CSS** - Utility-first CSS
- **Vite** - Fast build tool and dev server

### Backend (Tauri)
- **Rust** - Systems programming language for native performance
- **Tauri 2.6** - Cross-platform desktop framework
- **Tokio** - Async runtime for Rust
- **serde_json** - JSON serialization/deserialization

### AI/ML Infrastructure
- **Ollama** - Local LLM runtime
  - Qwen 2.5 Coder (1.5B, 7B)
  - DeepSeek Coder V2 (16B, 236B)
  - Other code-specialized models

### LibreOffice Integration
- **Python 3.x** - Scripting language for MCP server
- **MCP (Model Context Protocol)** - AI-to-application communication
  - JSON-RPC 2.0 over stdio
  - Python MCP server (FastMCP)
- **LibreOffice Python (UNO API)** - Document manipulation
- **Socket communication** - IPC between components

### Build & Deployment
- **npm/pnpm** - Frontend package management
- **Cargo** - Rust package management
- **Tauri CLI** - Application bundling

---

## Project Architecture

### High-Level System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    SmolPC Unified App                        │
│                      (Tauri 2.6)                             │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │              │  │              │  │              │      │
│  │  CodeHelper  │  │ LibreOffice  │  │  Blender AI  │      │
│  │     UI       │  │      UI      │  │      UI      │      │
│  │  (Svelte 5) │  │  (Svelte 5) │  │  (Svelte 5) │      │
│  │              │  │              │  │              │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                 │                 │              │
│         ▼                 ▼                 ▼              │
│  ┌──────────────────────────────────────────────────┐      │
│  │           Tauri Commands (Rust)                  │      │
│  │  - read/write files                              │      │
│  │  - Ollama communication                          │      │
│  │  - MCP client (LibreOffice)                      │      │
│  │  - Process management                            │      │
│  └──────┬─────────────────┬──────────────┬──────────┘      │
│         │                 │              │                 │
└─────────┼─────────────────┼──────────────┼─────────────────┘
          │                 │              │
          ▼                 ▼              ▼
    ┌──────────┐    ┌──────────────┐   ┌────────┐
    │  Ollama  │    │   Python     │   │Blender │
    │  (HTTP)  │    │ MCP Server   │   │  API   │
    │          │    │              │   │        │
    └──────────┘    └──────┬───────┘   └────────┘
                           │
                           ▼
                    ┌──────────────┐
                    │    Helper    │
                    │   (Macro)    │
                    │              │
                    └──────┬───────┘
                           │
                           ▼
                    ┌──────────────┐
                    │ LibreOffice  │
                    │  (Headless)  │
                    └──────────────┘
```

### LibreOffice Integration Architecture (Detailed)

This is the most complex part of the system. Here's how it works:

```
┌─────────────────────────────────────────────────────────────────┐
│                        Svelte Frontend                           │
│                   (LibreOffice.svelte)                          │
└────────────────────────────┬────────────────────────────────────┘
                             │ invoke("call_libreoffice_tool")
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Tauri Rust Backend                          │
│                  (src-tauri/src/libreoffice/)                   │
│                                                                  │
│  ┌────────────────────────────────────────────────────────┐    │
│  │              MCPClient (mcp_client.rs)                 │    │
│  │  - Manages Python MCP server process                   │    │
│  │  - Sends JSON-RPC 2.0 requests over stdin             │    │
│  │  - Receives responses from stdout                      │    │
│  │  - Request/response matching with channels            │    │
│  └────────────────┬───────────────────────────────────────┘    │
│                   │                                             │
│  ┌────────────────▼───────────────────────────────────────┐    │
│  │         ProcessManager (process_manager.rs)            │    │
│  │  - Finds Python executable (prefers venv)              │    │
│  │  - Spawns Python process with stdio pipes             │    │
│  │  - Manages process lifecycle (kill, wait)             │    │
│  └────────────────┬───────────────────────────────────────┘    │
│                   │                                             │
└───────────────────┼─────────────────────────────────────────────┘
                    │ spawns
                    ▼
┌─────────────────────────────────────────────────────────────────┐
│            Python MCP Server (main.py + libre.py)                │
│         (src-tauri/mcp-servers/libreoffice/venv)                │
│                                                                  │
│  1. Starts LibreOffice in headless mode (port 2002)            │
│  2. Starts Helper macro in LibreOffice (port 8765)             │
│  3. Exposes 27 tools via MCP protocol:                         │
│     - create_blank_document                                     │
│     - add_text, add_heading, add_paragraph                      │
│     - format_text, search_replace_text                          │
│     - add_table, format_table, insert_image                     │
│     - create_blank_presentation, add_slide                      │
│     - ... and 17 more                                           │
│                                                                  │
│  Listens on: STDIN (JSON-RPC requests)                         │
│  Responds on: STDOUT (JSON-RPC responses)                       │
└────────────────────────┬────────────────────────────────────────┘
                         │ Socket connection (localhost:8765)
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│              Helper Macro (mcp_helper.py)                        │
│   Running inside LibreOffice as a Python macro                  │
│   (~/Library/.../LibreOffice/4/user/Scripts/python/)           │
│                                                                  │
│  - Receives commands via socket (port 8765)                     │
│  - Uses UNO API to manipulate LibreOffice                       │
│  - Returns results to MCP server                                │
│  - Runs in background thread (daemon)                           │
└────────────────────────┬────────────────────────────────────────┘
                         │ UNO API calls
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│              LibreOffice (Headless Mode)                         │
│                    (port 2002, URP protocol)                     │
│                                                                  │
│  - Runs without GUI                                             │
│  - Accepts UNO API commands                                     │
│  - Creates/edits documents                                       │
│  - Saves to disk                                                │
└─────────────────────────────────────────────────────────────────┘
```

### Data Flow Example: Creating a Document

```
1. User clicks "Create Document" in Svelte UI
   └─> invoke('call_libreoffice_tool', {
         name: 'create_blank_document',
         args: { filename: 'test.odt', title: 'My Doc' }
       })

2. Tauri command handler receives request
   └─> MCPClient.call_tool("create_blank_document", args)

3. MCP Client sends JSON-RPC request to Python via stdin:
   └─> {"jsonrpc":"2.0","id":42,"method":"tools/call",
        "params":{"name":"create_blank_document","arguments":{...}}}

4. Python MCP Server receives request
   └─> Parses JSON-RPC
   └─> Looks up tool handler for 'create_blank_document'
   └─> Sends command to Helper via socket (port 8765):
       {"action": "create_document", "doc_type": "text", ...}

5. Helper Macro receives command
   └─> Uses UNO API: desktop.loadComponentFromURL(...)
   └─> LibreOffice creates document
   └─> Helper sends success response via socket

6. Python MCP Server receives Helper response
   └─> Formats as JSON-RPC response
   └─> Writes to stdout:
       {"jsonrpc":"2.0","id":42,"result":{...}}

7. MCP Client receives response from stdout
   └─> Background task reads line
   └─> Parses JSON-RPC response
   └─> Matches ID to pending request
   └─> Sends result via oneshot channel

8. Tauri command completes
   └─> Returns result to Svelte frontend
   └─> UI updates: "Document created at ~/Documents/test.odt"
```

---

## Current Features

### 1. CodeHelper (Completed ✅)

**Chat Interface:**
- Real-time streaming responses from local LLMs
- Syntax highlighting for code blocks
- Message history persistence
- Cancel generation mid-stream

**File Operations:**
- Read files from disk
- Write/save generated code
- File picker integration

**Ollama Integration:**
- HTTP API communication with local Ollama instance
- Model selection and switching
- Support for all Ollama-compatible models
- Streaming responses with Server-Sent Events (SSE)

**Performance Benchmarking:**
- Compare different models
- Track tokens/second, latency, memory usage
- Export benchmark results to CSV
- View historical benchmarks

### 2. Routing System (Completed ✅)

**Home Page:**
- Central navigation hub
- Cards for CodeHelper, LibreOffice AI, Blender AI
- Clean, responsive design

**Route Management:**
- Client-side routing with Svelte state
- Routes: `/`, `/codehelper`, `/libreoffice`, `/blender`
- Back navigation support

### 3. LibreOffice Integration (In Progress 🚧)

**Phase 1: MCP Server Setup (✅ COMPLETE)**
- Python MCP server copied from SmolPC-2.0 repo
- macOS compatibility fixes
- Helper macro installation and setup
- Full MCP protocol tested end-to-end
- 27 LibreOffice tools exposed via MCP

**Phase 2: Rust MCP Client (✅ STEPS 1-3 COMPLETE)**

**Step 1: Types and Module Structure (✅)**
- JSON-RPC 2.0 types (request, response, error, notification)
- MCP protocol types (initialize, tool call, content)
- Error types (14 variants covering all failure modes)
- 11 unit tests validating serialization

**Step 2: Process Manager (✅)**
- Cross-platform Python executable detection
- Virtual environment Python preference
- MCP server script location detection
- Async process spawning with tokio
- Process lifecycle management (spawn, kill, wait)
- Auto-cleanup on drop

**Step 3: MCP Client Core (✅)**
- Complete JSON-RPC 2.0 implementation
- MCP handshake (initialize → initialized)
- Tool listing and calling
- Request/response matching with oneshot channels
- Background task for continuous stdout reading
- Non-JSON line skipping (log messages)
- 30-second timeout per request
- Full integration tested successfully

**What Works Right Now:**
```rust
// This code successfully creates a LibreOffice document!
let client = MCPClient::new().await?;
let result = client.call_tool(
    "create_blank_document",
    json!({
        "filename": "test.odt",
        "title": "Test Document"
    })
).await?;
// Document created at ~/Documents/test.odt ✅
```

**Phase 2: Remaining Steps**
- Step 4: State management (wrap MCP client in Tauri state)
- Step 5: Tauri commands (expose to frontend)
- Step 6: Frontend UI (Svelte interface)
- Step 7: Error handling and recovery
- Step 8: Cross-platform testing
- Step 9: Documentation

### 4. Blender Integration (Planned 📅)

- Similar architecture to LibreOffice
- Python MCP server for Blender API
- 3D modeling operations
- Material and shader editing
- Scene management

---

## Codebase Structure

```
smolpc-codehelper/
├── src/                          # Frontend (Svelte)
│   ├── lib/
│   │   ├── components/
│   │   │   ├── Chat.svelte              # Main chat interface
│   │   │   ├── ChatMessage.svelte       # Individual message display
│   │   │   ├── Home.svelte              # Home page with navigation
│   │   │   ├── LibreOffice.svelte       # LibreOffice UI (placeholder)
│   │   │   └── Blender.svelte           # Blender UI (placeholder)
│   │   ├── services/
│   │   │   └── libreoffice.ts           # LibreOffice service (TODO)
│   │   └── types.ts                     # TypeScript type definitions
│   ├── App.svelte                       # Main app component with routing
│   └── main.ts                          # App entry point
│
├── src-tauri/                    # Backend (Rust)
│   ├── src/
│   │   ├── commands/
│   │   │   ├── default.rs               # File I/O commands
│   │   │   ├── ollama.rs                # Ollama API integration
│   │   │   └── benchmark.rs             # Performance benchmarking
│   │   ├── libreoffice/
│   │   │   ├── mod.rs                   # Module exports
│   │   │   ├── types.rs                 # JSON-RPC & MCP types (426 lines)
│   │   │   ├── process_manager.rs       # Python process lifecycle (359 lines)
│   │   │   └── mcp_client.rs            # MCP client core (458 lines)
│   │   ├── benchmark/
│   │   │   ├── mod.rs                   # Benchmarking system
│   │   │   ├── metrics.rs               # Performance metrics
│   │   │   └── export.rs                # CSV export
│   │   ├── commands.rs                  # Command registration
│   │   ├── lib.rs                       # Tauri app setup
│   │   └── main.rs                      # Entry point
│   │
│   ├── mcp-servers/              # MCP server implementations
│   │   └── libreoffice/
│   │       ├── venv/                    # Python virtual environment
│   │       ├── main.py                  # MCP server launcher (5.8KB)
│   │       ├── libre.py                 # MCP server core (49KB)
│   │       ├── helper.py                # UNO API bridge (153KB)
│   │       ├── helper_utils.py          # Helper utilities (6KB)
│   │       ├── helper_test_functions.py # Test functions (43KB)
│   │       ├── pyproject.toml           # Python dependencies
│   │       └── SETUP_MAC.md             # macOS setup instructions
│   │
│   ├── Cargo.toml                # Rust dependencies
│   ├── build.rs                  # Build script
│   └── tauri.conf.json           # Tauri configuration
│
├── package.json                  # Frontend dependencies
├── vite.config.ts                # Vite configuration
├── tailwind.config.js            # Tailwind CSS config
└── tsconfig.json                 # TypeScript config
```

---

## How It All Works

### CodeHelper Flow

1. **User Opens App**
   - Tauri launches with Svelte frontend
   - Home page displays three tool cards
   - User clicks "CodeHelper"

2. **Chat Interface Loads**
   - Checks if Ollama is running (`check_ollama` command)
   - Fetches available models (`get_ollama_models`)
   - Displays chat interface

3. **User Sends Message**
   - Frontend calls `invoke('generate_stream', { prompt, model })`
   - Rust backend makes HTTP request to Ollama
   - Streams response chunks back to frontend via events
   - Frontend displays response in real-time with markdown rendering

4. **File Operations**
   - User clicks "Read File" → `invoke('read', { path })` → Rust reads file → Returns content
   - User clicks "Save Code" → `invoke('write', { path, content })` → Rust writes to disk

### LibreOffice Integration Flow (Current Implementation)

1. **Startup (Manual - Phase 1)**
   ```bash
   # Terminal: Start LibreOffice headless
   soffice --accept="socket,host=localhost,port=2002;urp;" &

   # LibreOffice GUI: Tools → Macros → Run mcp_helper.start_helper
   # This starts the helper on port 8765
   ```

2. **Test via Rust (Working Now!)**
   ```rust
   // In Rust tests (cargo test libreoffice::mcp_client)
   let client = MCPClient::new().await?;  // Spawns Python MCP server
   let tools = client.list_tools().await?;  // Gets 27 tools
   let result = client.call_tool("create_blank_document", args).await?;
   // ✅ Document created at ~/Documents/test.odt
   ```

3. **Next: Tauri Commands (Phase 2 Step 5)**
   ```rust
   // Will expose to frontend:
   #[tauri::command]
   async fn call_libreoffice_tool(
       state: State<'_, LibreOfficeState>,
       tool_name: String,
       params: Value
   ) -> Result<Value, String>
   ```

4. **Future: Frontend Integration (Phase 2 Step 6)**
   ```typescript
   // Frontend will call:
   const result = await invoke('call_libreoffice_tool', {
     toolName: 'create_blank_document',
     params: { filename: 'report.odt', title: 'My Report' }
   });
   ```

### Key Technologies Explained

**Tauri:**
- Wraps web frontend (HTML/CSS/JS) in native app
- Rust backend exposes commands to frontend via IPC
- Commands are async and type-safe
- Produces small binaries (~10MB vs Electron's 100MB+)

**Svelte 5 Runes:**
- `$state()` - Reactive state
- `$derived()` - Computed values
- `$effect()` - Side effects
- Signals-based reactivity (similar to Solid.js)

**Ollama:**
- Local LLM server running on HTTP (default: localhost:11434)
- Provides `/api/generate` endpoint for streaming
- Supports various models (Qwen, DeepSeek, LLaMA, etc.)
- No GPU required (but faster with GPU)

**MCP (Model Context Protocol):**
- Standard protocol for AI-to-application communication
- Uses JSON-RPC 2.0 over stdin/stdout
- Handshake: `initialize` → response → `initialized` notification
- Tool calling: `tools/list`, `tools/call`
- Designed by Anthropic for Claude integrations

**JSON-RPC 2.0:**
- Request: `{"jsonrpc":"2.0","id":1,"method":"...","params":{...}}`
- Response: `{"jsonrpc":"2.0","id":1,"result":{...}}`
- Error: `{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"..."}}`
- Notification (no response): `{"jsonrpc":"2.0","method":"...","params":{...}}`

**UNO API (Universal Network Objects):**
- LibreOffice's API for programmatic access
- Available in Python, Java, C++
- Access via socket connection (URP protocol)
- Comprehensive document manipulation capabilities

---

## Development Roadmap

### ✅ Completed Milestones

**M0: Project Setup (Oct 2024)**
- Tauri + Svelte 5 + Rust project structure
- Basic UI with Tailwind CSS
- Development workflow established

**M1: CodeHelper MVP (Dec 1, 2024 - COMPLETED EARLY)**
- Ollama integration with streaming
- Chat interface with syntax highlighting
- File operations (read/write)
- Model selection
- Performance benchmarking
- Unified app routing system

**M1.5: LibreOffice MCP Setup (Nov 18, 2024)**
- Python MCP server integrated
- macOS compatibility achieved
- Helper macro working
- Full stack tested end-to-end
- Rust MCP client core complete

### 🚧 In Progress

**M2: LibreOffice Integration (Target: Dec 20, 2024)**

**Phase 2 - Rust Integration (STEPS 1-3 DONE)**
- ✅ Step 1: Types and module structure
- ✅ Step 2: Process manager
- ✅ Step 3: MCP client core
- 🚧 Step 4: State management (NEXT)
  - Wrap MCP client in Tauri state
  - Lazy initialization on first use
  - Client persistence across calls
- 📅 Step 5: Tauri commands
  - Expose `call_libreoffice_tool` to frontend
  - Add health check commands
  - Add service start/stop commands
- 📅 Step 6: Frontend integration
  - Build LibreOffice UI in Svelte
  - Document creation forms
  - Tool selection interface
  - Error display and handling
- 📅 Step 7: Error handling & recovery
  - Auto-restart on crash
  - User-friendly error messages
  - Fallback mechanisms
- 📅 Step 8: Cross-platform testing
  - Test on Windows
  - Test on Linux
  - Fix platform-specific issues
- 📅 Step 9: Documentation
  - User guide
  - API documentation
  - Troubleshooting guide

**Phase 3 - UI Polish**
- Document list view
- Recent documents
- Template selection
- Progress indicators
- Success/error notifications

### 📅 Planned Milestones

**M3: Blender Integration (Target: Jan 5, 2025)**
- Python MCP server for Blender
- Similar architecture to LibreOffice
- Basic 3D operations
- Material editing
- Scene management

**M4: Integration & Testing (Target: Jan 20, 2025)**
- End-to-end testing
- Performance optimization
- Cross-platform validation
- Bug fixes

**M5: Documentation & Release (Target: Feb 1, 2025)**
- User documentation
- Developer guides
- Installation packages
- CI/CD setup

**M6: Demo Day (March 3, 2026)**
- Live demonstration
- Feedback collection
- Iteration planning

---

## Getting Started for Contributors

### Prerequisites

**Required:**
- Node.js 18+ (for frontend)
- Rust 1.77+ (for Tauri backend)
- Ollama (for CodeHelper AI features)

**For LibreOffice Integration:**
- Python 3.9+
- LibreOffice 7.0+ (or Collabora Office)
- macOS/Linux/Windows

**For Blender Integration (future):**
- Blender 3.0+

### Setup Instructions

1. **Clone the Repository**
   ```bash
   git clone https://github.com/SmolPC-2-0/smolpc-codehelper.git
   cd smolpc-codehelper
   ```

2. **Install Frontend Dependencies**
   ```bash
   npm install
   # or
   pnpm install
   ```

3. **Install Rust (if needed)**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

4. **Install Tauri CLI**
   ```bash
   cargo install tauri-cli --version "^2.0.0"
   ```

5. **Install Ollama (for CodeHelper)**
   ```bash
   # macOS
   brew install ollama

   # Linux
   curl -fsSL https://ollama.com/install.sh | sh

   # Windows
   # Download from https://ollama.com/download
   ```

6. **Start Ollama and Download a Model**
   ```bash
   ollama serve  # Start Ollama server
   ollama pull qwen2.5-coder:1.5b  # Download small model
   ```

7. **Run the App in Development Mode**
   ```bash
   npm run tauri dev
   ```

### Setting Up LibreOffice Integration (macOS)

1. **Set up Python virtual environment:**
   ```bash
   cd src-tauri/mcp-servers/libreoffice
   python3 -m venv venv
   source venv/bin/activate
   pip install -r pyproject.toml  # Install dependencies
   ```

2. **Install Helper Macro:**
   ```bash
   # Copy helper files to LibreOffice
   cp src-tauri/mcp-servers/libreoffice/helper.py \
      ~/Library/Application\ Support/LibreOffice/4/user/Scripts/python/mcp_helper.py

   cp src-tauri/mcp-servers/libreoffice/helper_utils.py \
      ~/Library/Application\ Support/LibreOffice/4/user/Scripts/python/

   cp src-tauri/mcp-servers/libreoffice/helper_test_functions.py \
      ~/Library/Application\ Support/LibreOffice/4/user/Scripts/python/
   ```

3. **Start LibreOffice in Headless Mode:**
   ```bash
   /Applications/LibreOffice.app/Contents/MacOS/soffice \
     --accept="socket,host=localhost,port=2002;urp;" &
   ```

4. **Run Helper Macro:**
   - Open LibreOffice
   - Go to Tools → Macros → Organize Python Scripts
   - Expand "My Macros" → "mcp_helper"
   - Select `start_helper` and click "Run"
   - You should see: "MCP Helper started in background thread"

5. **Test the Integration:**
   ```bash
   cd src-tauri
   cargo test --lib libreoffice::mcp_client -- --nocapture
   ```

   Expected output:
   ```
   test libreoffice::mcp_client::tests::test_mcp_client_new ... ok
   test libreoffice::mcp_client::tests::test_mcp_client_list_tools ... ok
   test libreoffice::mcp_client::tests::test_mcp_client_call_tool ... ok

   test result: ok. 3 passed; 0 failed
   ```

### Project Commands

```bash
# Development
npm run dev              # Run Vite dev server only
npm run tauri dev        # Run full Tauri app with hot reload

# Building
npm run build            # Build frontend
npm run tauri build      # Build full application

# Testing
cargo test               # Run all Rust tests
cargo test libreoffice   # Run LibreOffice tests only
npm run test             # Run frontend tests (if configured)

# Linting
cargo clippy             # Rust linter
npm run lint             # Frontend linter
```

### Contributing Guidelines

1. **Branch Naming:**
   - `feature/description` - New features
   - `fix/description` - Bug fixes
   - `docs/description` - Documentation updates
   - `refactor/description` - Code refactoring

2. **Commit Messages:**
   - Use conventional commits format
   - `feat:` for features
   - `fix:` for bug fixes
   - `docs:` for documentation
   - `refactor:` for refactoring
   - Example: `feat: add document template selection`

3. **Pull Requests:**
   - Create PR against `main` branch
   - Fill out PR template
   - Ensure all tests pass
   - Request review from maintainers

4. **Code Style:**
   - Rust: Follow `rustfmt` defaults
   - TypeScript: Follow project ESLint config
   - Use meaningful variable names
   - Add comments for complex logic
   - Write unit tests for new features

---

## Architecture Decisions & Rationale

### Why Tauri over Electron?

**Size:**
- Tauri: ~10MB executable
- Electron: ~100MB+ executable
- Critical for target users with limited storage

**Performance:**
- Tauri uses native webview (WebKit on macOS, WebView2 on Windows)
- Lower memory footprint
- Faster startup time

**Security:**
- Rust's memory safety
- No Node.js runtime in production
- Smaller attack surface

### Why Svelte 5 over React/Vue?

**Bundle Size:**
- Svelte compiles to vanilla JS
- No runtime overhead
- Smaller bundle size

**Performance:**
- True reactivity without virtual DOM
- Faster updates
- Better for low-end hardware

**Developer Experience:**
- Less boilerplate than React
- Built-in state management (runes)
- Excellent TypeScript support

### Why Python MCP Server for LibreOffice?

**Proven Architecture:**
- Existing SmolPC-2.0 implementation works
- Well-tested in production
- Comprehensive LibreOffice UNO integration

**Cross-Platform:**
- Python is available everywhere
- LibreOffice Python API is consistent
- Easy to debug and extend

**Separation of Concerns:**
- Rust handles app logic and IPC
- Python handles LibreOffice specifics
- Clear boundaries between components

### Why JSON-RPC over gRPC/REST?

**Simplicity:**
- Standard protocol
- Easy to debug (human-readable)
- No code generation needed

**MCP Compatibility:**
- MCP protocol uses JSON-RPC 2.0
- Standardized by Anthropic
- Growing ecosystem support

**Stdio Transport:**
- Works with process spawning
- No network configuration needed
- Secure (local only)

---

## Technical Challenges & Solutions

### Challenge 1: LibreOffice Python Standalone Execution

**Problem:**
On macOS, LibreOffice's bundled Python cannot run standalone due to code signing restrictions.

**Solution:**
Run the helper as a LibreOffice macro instead of a standalone process:
- Helper installed in `~/Library/.../LibreOffice/4/user/Scripts/python/`
- Executed via LibreOffice's macro system
- Runs in background thread with `g_exportedScripts`
- Communicates via socket (port 8765)

### Challenge 2: Python Dependencies in Virtual Environment

**Problem:**
System Python doesn't have MCP dependencies (`mcp[cli]`, `httpx`, etc.)

**Solution:**
ProcessManager now prefers virtual environment Python:
- Checks for `venv/bin/python3` in MCP server directory
- Falls back to system Python if venv not found
- Cross-platform path detection (macOS, Windows, Linux)

### Challenge 3: Non-JSON Output from MCP Server

**Problem:**
Python MCP server outputs log messages before JSON-RPC responses:
```
Office socket already running on port 2002
Helper script already running on port 8765
Starting Office MCP server...
{"jsonrpc":"2.0","id":1,"result":{...}}
```

**Solution:**
MCP client skips non-JSON lines:
- Continuously reads stdout in loop
- Tries to parse each line as JSON-RPC
- Skips lines that fail to parse
- Logs skipped lines for debugging

### Challenge 4: Async Request/Response Matching

**Problem:**
Multiple tools can be called concurrently, but JSON-RPC responses come in order.

**Solution:**
Request/response matching with channels:
- Each request gets unique ID (atomic counter)
- Create oneshot channel for each request
- Store channel in HashMap by ID
- Background task reads stdout and matches IDs
- Send response through channel when found
- 30-second timeout per request

### Challenge 5: Process Lifecycle Management

**Problem:**
Need to ensure Python process is cleaned up on app close or errors.

**Solution:**
ProcessManager with automatic cleanup:
- Tokio process with `kill_on_drop(true)`
- Drop implementation logs cleanup
- Process automatically killed when struct dropped
- Health checking with `is_running()`

---

## Testing Strategy

### Unit Tests

**Rust (src-tauri/src/):**
- Types: Serialization/deserialization (11 tests)
- ProcessManager: Python detection, process spawning (4 tests)
- MCPClient: Handshake, tool calling, lifecycle (3 tests)

**Run tests:**
```bash
cd src-tauri
cargo test
cargo test --lib libreoffice  # LibreOffice tests only
cargo test -- --nocapture      # Show println! output
```

### Integration Tests

**LibreOffice End-to-End:**
1. Start LibreOffice headless
2. Run helper macro
3. Run Rust tests: `cargo test libreoffice::mcp_client`
4. Verify document creation: `ls ~/Documents/test_from_rust.odt`

### Manual Testing

**CodeHelper:**
1. Start Ollama: `ollama serve`
2. Run app: `npm run tauri dev`
3. Navigate to CodeHelper
4. Send message, verify streaming response
5. Read file, verify content displayed
6. Save code, verify file created

**LibreOffice (when UI complete):**
1. Start LibreOffice + helper
2. Navigate to LibreOffice AI
3. Create document via UI
4. Verify document exists
5. Test formatting operations
6. Test error handling

---

## Performance Metrics

### Current Performance (CodeHelper)

**App Size:**
- Development: ~50MB (with debug symbols)
- Production: ~10MB (optimized)
- First load: <2 seconds on low-end hardware

**Memory Usage:**
- Idle: ~60MB
- Active chat: ~80MB
- Peak (large model): ~120MB

**Response Time:**
- Ollama API: 10-50ms latency
- File operations: <5ms
- Streaming: Real-time (chunked)

### Target Performance (LibreOffice)

**Document Operations:**
- Create document: <500ms
- Add text: <100ms
- Format text: <200ms
- Insert image: <300ms

**MCP Communication:**
- Handshake: <1s
- Tool list: <200ms
- Tool call: <500ms (excluding LibreOffice operation time)

---

## Deployment

### Building for Production

```bash
# Build frontend
npm run build

# Build Tauri app
npm run tauri build
```

**Output:**
- macOS: `src-tauri/target/release/bundle/macos/SmolPC CodeHelper.app`
- Windows: `src-tauri/target/release/bundle/msi/SmolPC CodeHelper.msi`
- Linux: `src-tauri/target/release/bundle/deb/smolpc-code-helper.deb`

### Distribution

**Current:**
- Manual download from GitHub releases
- Install Ollama separately
- Install LibreOffice separately (for LibreOffice AI)

**Future (Planned):**
- Bundled installers with dependencies
- Auto-update support via Tauri
- Version checking and notifications

---

## Resources & Links

**Project:**
- Repository: https://github.com/SmolPC-2-0/smolpc-codehelper
- Issues: https://github.com/SmolPC-2-0/smolpc-codehelper/issues
- Discussions: https://github.com/SmolPC-2-0/smolpc-codehelper/discussions

**Related Projects:**
- SmolPC-2.0: https://github.com/SmolPC-2-0/SmolPC-2.0 (Original LibreOffice MCP server)

**Technologies:**
- Tauri: https://tauri.app/
- Svelte 5: https://svelte-5-preview.vercel.app/
- Ollama: https://ollama.com/
- MCP Protocol: https://modelcontextprotocol.io/
- LibreOffice UNO: https://api.libreoffice.org/

**Learning Resources:**
- Tauri Guides: https://tauri.app/v1/guides/
- Svelte Tutorial: https://learn.svelte.dev/
- Rust Book: https://doc.rust-lang.org/book/
- Ollama API: https://github.com/ollama/ollama/blob/main/docs/api.md

---

## FAQ

**Q: Why not just use ChatGPT/Claude?**
A: Our target users have unreliable internet or no internet at all. Offline AI is essential for their workflow.

**Q: What hardware do I need?**
A: Minimum: 4GB RAM, 2-core CPU, 1GB disk space. Recommended: 8GB RAM, 4-core CPU, 5GB disk space (with models).

**Q: Which LLM models work best?**
A: For coding: Qwen 2.5 Coder (1.5B for low-end, 7B for better quality), DeepSeek Coder V2 (16B if you have 16GB+ RAM).

**Q: Does this work on Windows/Linux?**
A: CodeHelper: Yes, fully tested. LibreOffice integration: macOS complete, Windows/Linux in testing (Phase 2 Step 8).

**Q: Can I use this without Ollama?**
A: CodeHelper requires Ollama. LibreOffice AI and Blender AI will work independently once complete.

**Q: How do I add a new tool to LibreOffice?**
A: Add the tool definition in `libre.py`, implement the function, update the helper if needed. See existing tools for examples.

**Q: Can I contribute?**
A: Yes! Check the [Contributing Guidelines](#contributing-guidelines) above. We welcome all contributions.

**Q: What's the license?**
A: MIT License (check LICENSE file in repository).

---

## Acknowledgments

**Original Authors:**
- SmolPC Team

**Technologies:**
- Tauri Team
- Svelte Team
- Ollama Team
- Anthropic (MCP Protocol)
- LibreOffice/The Document Foundation

**Community:**
- All contributors and testers
- Students who inspired this project

---

## Contact & Support

**For Issues:** https://github.com/SmolPC-2-0/smolpc-codehelper/issues

**For Questions:** https://github.com/SmolPC-2-0/smolpc-codehelper/discussions

**For Direct Contact:** [Add contact info here]

---

*Last updated: November 18, 2024*
*Version: 2.0.0-alpha*
*Status: LibreOffice Integration Phase 2 in Progress*
