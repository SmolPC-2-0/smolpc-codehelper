# MCP Server Resources

This directory is reserved for LibreOffice MCP Python runtime assets used by the app.

Planning reference:

1. `apps/libreoffice-assistant/MIGRATION_PLAN.md` (Phase 2 MCP runtime port)

Source import baseline:

1. `/Users/mts/smolpc/smolpc-libreoffice/tauri-app/src-tauri/resources/mcp_server/main.py`
2. `/Users/mts/smolpc/smolpc-libreoffice/tauri-app/src-tauri/resources/mcp_server/libre.py`
3. `/Users/mts/smolpc/smolpc-libreoffice/tauri-app/src-tauri/resources/mcp_server/helper.py`
4. `/Users/mts/smolpc/smolpc-libreoffice/tauri-app/src-tauri/resources/mcp_server/helper_utils.py`
5. `/Users/mts/smolpc/smolpc-libreoffice/tauri-app/src-tauri/resources/mcp_server/helper_test_functions.py`

Import status:

1. Imported into this repo on 2026-03-12 for Phase 2 migration work.

Notes:

1. Source `libre.py` currently exposes 27 active MCP tools.
2. Source stack communicates via stdio (Rust <-> Python MCP) and socket bridge (`libre.py` <-> `helper.py` on `localhost:8765`).
3. Keep this integration engine-only in this repo; do not add Ollama runtime dependencies while porting.
