# GIMP Bundled Resources

This directory contains the Phase 5 bundled GIMP integration assets for the
self-contained unified app.

Imported baseline:

1. source repository: `maorcc/gimp-mcp`
2. pinned source commit: `fb3c6c6a6aa9b4e260a52d277ea3e7bd8330133c`
3. imported upstream files:
   - `upstream/README.md`
   - `upstream/GIMP_MCP_PROTOCOL.md`
   - `upstream/gimp_mcp_server.py`
   - `upstream/pyproject.toml`
   - `upstream/uv.lock`
   - `plugin/gimp-mcp-plugin/gimp-mcp-plugin.py`
   - `LICENSE`

Local Phase 5 ownership changes:

1. the bundled plugin is patched to run as a persistent GIMP 3 extension and
   auto-start its internal socket bridge on GIMP startup
2. the unified app provisions that plugin into the user GIMP profile
3. the unified app launches a SmolPC-owned TCP MCP bridge from:
   - `bridge/smolpc_gimp_mcp_tcp_bridge.py`

Runtime contract:

1. the provisioned GIMP plugin listens on `127.0.0.1:9877` inside GIMP
2. the SmolPC bridge listens on `127.0.0.1:10008`
3. Rust connects to the SmolPC bridge over TCP MCP
4. the bridge forwards tool calls to the provisioned plugin socket
