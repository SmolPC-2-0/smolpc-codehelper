# LibreOffice Assistant Windows Phase 2 MCP Verification

Use this runbook on a Windows machine to validate the newly integrated MCP bridge flow.

## Goal

Verify that the app can:

1. Start the bundled LibreOffice MCP runtime.
2. Discover MCP tools.
3. Execute at least one safe/read-only MCP tool call from the app UI.

## Preconditions

1. Branch includes the Phase 2 MCP integration changes.
2. Windows machine has:
   - Node.js and npm
   - Rust toolchain
   - Python 3.12+
   - LibreOffice (or Collabora Office) installed
3. Shared engine baseline is available (model setup already run at least once).

## One-time setup

From repo root:

1. `npm install`
2. `npm run check:libreoffice`
3. `npm run build:libreoffice`

Install MCP Python dependencies:

1. `cd apps/libreoffice-assistant/src-tauri/resources/mcp_server`
2. `python -m venv .venv`
3. `.venv\\Scripts\\activate`
4. `pip install mcp httpx pillow`
5. `cd ..\\..\\..\\..\\..`

## App startup

1. Run `npm run tauri:dev:libreoffice`.
2. In UI, click `Ensure Engine Started`.
3. Confirm model controls and runtime sections load without backend errors.

## MCP verification flow

1. In `MCP Bridge` panel, click `Refresh MCP Status`.
2. Click `Start MCP Server`.
3. Confirm:
   - `running: true`
   - `tools_loaded` is greater than `0` after `Refresh MCP Tools`
4. Select a safe/read-only tool from dropdown.
5. Set JSON args in `Tool Arguments` textbox.
6. Click `Invoke MCP Tool`.
7. Confirm `MCP tool result` JSON appears and contains output content.

Recommended first read-only call:

1. Tool: `list_documents`
2. Arguments:

```json
{
  "directory": "C:\\Users\\<YOUR_USER>\\Documents"
}
```

## Required artifacts for review

1. Screenshot: MCP panel showing `running: true` and non-zero `tools_loaded`.
2. JSON output of one successful read-only MCP tool call.
3. If any failure occurs:
   - MCP status error text
   - issue report JSON from `Integration Issue Report` section
   - evidence bundle path/file from `Export Evidence Bundle`

## Troubleshooting

1. `Start MCP Server` fails with Python/module error:
   - Re-activate `.venv`
   - Re-run `pip install mcp httpx pillow`
2. Tool list remains empty:
   - Click `Refresh MCP Tools`
   - Confirm MCP server is still running
   - Check Tauri logs for MCP initialization/tool-list errors
3. Tool call fails with LibreOffice connection errors:
   - Verify LibreOffice/Collabora is installed in expected Windows path
   - Ensure no local policy blocks launching `soffice.exe`
4. If needed, retry with explicit Python:
   - Launch app with `SMOLPC_PYTHON_PATH=python` set in environment

