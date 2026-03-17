# LibreOffice MCP Runtime Placeholder

This directory is intentionally staged as a tracked placeholder during unified
Phase 6A scaffolding.

Reference source branch:

- `codex/libreoffice-port-track-a`

Reference source commit observed during planning:

- `7acad1fa0eb31e32a5485069e85c021d14284455`

Expected future imported files from the standalone branch:

- `main.py`
- `libre.py`
- `helper.py`
- `helper_utils.py`
- `helper_test_functions.py`

Expected future runtime shape:

- Rust child-process stdio MCP transport
- Python `main.py` entrypoint
- helper socket bridge on `localhost:8765`

This branch does **not** import the full Python MCP runtime yet. The standalone
LibreOffice work is still evolving on `codex/libreoffice-port-track-a`, so the
unified app only stages the resource path and provider scaffolding here.
