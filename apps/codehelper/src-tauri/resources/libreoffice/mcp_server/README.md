# MCP Server Resources

This directory contains the Python MCP runtime assets imported into the unified
app for Phase 6B LibreOffice activation.

Imported baseline:

1. source branch: `origin/codex/libreoffice-port-track-a`
2. pinned source commit: `7acad1fa0eb31e32a5485069e85c021d14284455`
3. imported files:
   - `main.py`
   - `libre.py`
   - `helper.py`
   - `helper_utils.py`
   - `helper_test_functions.py`

Runtime contract:

1. Rust launches `main.py` over stdio MCP through `smolpc-mcp-client`
2. `libre.py` communicates with `helper.py` over `localhost:8765`
3. headless office socket remains `localhost:2002`

Notes:

1. `libre.py` currently exposes 27 active MCP tools upstream.
2. Unified Phase 6B activates Writer and Slides only; Calc remains scaffold-only.
3. Keep this integration engine-only in this repo; do not add Ollama runtime
   dependencies while porting.
