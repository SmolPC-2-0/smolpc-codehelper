# GIMP AI Assistant

A desktop app that lets you control GIMP with natural language. Type things like "draw a red circle", "increase brightness", or "undo" and the app executes them directly in GIMP.

Built with Tauri (Rust backend) + SvelteKit frontend. Talks to GIMP via the [gimp-mcp](https://github.com/anthropics/gimp-mcp) plugin.

---

## Prerequisites

- **macOS** (Linux may work but untested)
- **GIMP 3.0+** — must be the Python-enabled build (the standard download from gimp.org works)
- **Node.js** 18+
- **Rust** + Cargo — install via [rustup.rs](https://rustup.rs)
- **uv** — Python package manager used to run the MCP server:
  ```bash
  curl -LsSf https://astral.sh/uv/install.sh | sh
  ```
- **Ollama** *(optional)* — only needed for commands that aren't recognised by the built-in fast paths (e.g. unusual drawing operations). Install from [ollama.com](https://ollama.com) and pull a model:
  ```bash
  ollama pull llama3.2
  ```

---

## Setup

### 1. Clone both repos

This app depends on a separate Python MCP server that bridges it to GIMP. Clone both:

```bash
git clone https://github.com/YOUR_USERNAME/smolpc-gimp
git clone https://github.com/anthropics/gimp-mcp
```

> **Note the path where you cloned `gimp-mcp`** — you'll need it in step 3.

### 2. Install the GIMP plugin

Copy `gimp-mcp-plugin.py` from the `gimp-mcp` repo into GIMP's plugin folder:

**macOS:**
```bash
cp gimp-mcp/gimp-mcp-plugin.py ~/Library/Application\ Support/GIMP/3.0/plug-ins/gimp-mcp-plugin/gimp-mcp-plugin.py
```

Then open GIMP. The plugin registers itself as "MCP Server" under Filters. You don't need to run it manually — the app starts the server for you.

### 3. Update the hardcoded path

Open `src-tauri/src/mcp.rs` and change line 8 to point to wherever you cloned `gimp-mcp`:

```rust
// Change this:
const GIMP_MCP_PATH: &str = "/Users/aishah/gimp-mcp";

// To wherever you cloned it, e.g.:
const GIMP_MCP_PATH: &str = "/Users/yourname/gimp-mcp";
```

Also update the same path in `src-tauri/src/lib.rs` around line 30:
```rust
let gimp_mcp_dir = "/Users/yourname/gimp-mcp";
```

### 4. Install dependencies and run

```bash
cd smolpc-gimp
npm install
npm run tauri dev
```

---

## Usage

1. Open GIMP and open an image
2. Launch this app (`npm run tauri dev`)
3. The app will auto-connect to GIMP. The status bar shows connection state.
4. Type a command and press Enter:

| Command | What it does |
|---|---|
| `draw a red circle` | Draws a filled red circle in the center |
| `add a blue line` | Draws a diagonal blue line |
| `draw a green triangle` | Draws a filled green triangle |
| `increase brightness` | Brightens the image |
| `increase contrast` | Increases contrast |
| `blur` | Applies a gaussian blur |
| `undo` | Reverts the last change |

Commands not in the list above are sent to Ollama (if running) to generate the GIMP Python code dynamically.

---

## Troubleshooting

**"Failed to call LLM: error sending request for url (http://localhost:11434)"**
Ollama isn't running. Either start it with `ollama serve`, or only use commands that are in the fast-path list above (drawing shapes, brightness, contrast, blur, undo). You don't need Ollama for those.

**"MCP not connected" in the status bar**
Make sure GIMP is open and the MCP plugin is installed. Try clicking "Start MCP Server" in the app if the button is visible, or restart GIMP.

**"No image open in GIMP"**
Open an image in GIMP first (File → Open).

**Undo doesn't work as expected**
The app uses a clipboard-based undo (not GIMP's built-in undo stack) because the MCP plugin runs as a long-lived process. Only the most recent operation can be undone.
