# GIMP AI Assistant

A desktop app that lets you control GIMP with natural language. Type things like "draw a red circle", "blur the top half", or "rotate 90 degrees" and the app executes them directly in GIMP.

Built with Tauri (Rust backend) + SvelteKit frontend. Talks to GIMP via the [gimp-mcp](https://github.com/maorcc/gimp-mcp) plugin.

Part of the [SmolPC](https://github.com/SmolPC-2-0/smolpc-codehelper) monorepo — lives at `apps/gimp-assistant`.

---

## Prerequisites

- **macOS** (Linux may work but untested)
- **GIMP 3.0+** — the standard download from gimp.org works
- **Node.js** 18+
- **Rust** + Cargo — install via [rustup.rs](https://rustup.rs)
- **uv** — Python package manager used to run the MCP server:
  ```bash
  curl -LsSf https://astral.sh/uv/install.sh | sh
  ```
- **SmolPC engine** *(optional)* — only needed for commands not in the built-in fast paths. The engine is bundled with the [SmolPC](https://github.com/SmolPC-2-0/smolpc-codehelper) launcher — start it there before using custom commands.

---

## Setup

### 1. Clone the repo and gimp-mcp

```bash
git clone https://github.com/SmolPC-2-0/smolpc-codehelper
git clone https://github.com/maorcc/gimp-mcp
```

### 2. Install the GIMP plugin

Copy the plugin into GIMP's plugin folder and restart GIMP:

**macOS:**
```bash
mkdir -p ~/Library/Application\ Support/GIMP/3.0/plug-ins/gimp-mcp-plugin
cp gimp-mcp/gimp-mcp-plugin.py ~/Library/Application\ Support/GIMP/3.0/plug-ins/gimp-mcp-plugin/gimp-mcp-plugin.py
```

### 3. Point the app at your gimp-mcp folder

The app looks for `gimp-mcp` at `~/gimp-mcp` by default. If you cloned it elsewhere, set the environment variable before running:

```bash
export GIMP_MCP_PATH=/path/to/your/gimp-mcp
```

Or add it to your shell profile (`~/.zshrc` / `~/.bashrc`) to make it permanent.

### 4. Install dependencies and run

```bash
cd smolpc-codehelper/apps/gimp-assistant
npm install
npm run tauri dev
```

---

## Usage

1. Open GIMP and open an image
2. Launch this app (`npm run tauri dev`)
3. The header shows GIMP connection status — green means connected
4. Type a command and press Enter

### Built-in commands (no SmolPC engine needed)

| Command | What it does |
|---|---|
| `draw a red circle` | Filled circle in the centre |
| `draw a blue heart` | Heart shape |
| `draw a green triangle` | Triangle shape |
| `draw an orange oval` | Ellipse shape |
| `add a line` | Diagonal line across the image |
| `increase brightness` | Brightens the whole image |
| `decrease contrast` | Lowers contrast |
| `blur the image` | Gaussian blur |
| `blur the top half` | Blurs only the top half |
| `brighten the bottom half` | Brightens only the bottom half |
| `darken the left side` | Darkens only the left half |
| `crop to square` | Centre-crops to a square |
| `undo` | Reverts the last change |

### LLM-powered commands (requires SmolPC engine)

Any command not in the list above is sent to the SmolPC engine, which generates GIMP Python code dynamically. Examples:

- `rotate 90 degrees`
- `flip horizontally`
- `convert to black and white`
- `make it more saturated`

---

## Troubleshooting

**"SmolPC engine isn't running" message**
Launch the SmolPC engine from the SmolPC launcher app, or stick to the built-in commands above — they work without it.

**Header shows "GIMP offline"**
Make sure GIMP is open, an image is loaded, and the MCP plugin is installed (check Filters menu — you should see an MCP Server option). Restart GIMP if needed.

**"No image open in GIMP"**
Open an image in GIMP first (File → Open).

**Undo doesn't fully work**
The app uses clipboard-based undo (not GIMP's built-in stack) because the MCP plugin runs as a long-lived process. Only the most recent operation can be undone.
