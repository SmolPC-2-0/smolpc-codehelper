# Phase 5: SmolPC Launcher & Ecosystem

**Goal:** Create unified launcher for all SmolPC apps with shared engine and profiles.

**Prerequisites:**
- Phase 1-4 complete
- Understanding of MCP (Model Context Protocol)

---

## Objectives

1. Create SmolPC Launcher application
2. Implement unified authentication/profiles
3. Enable engine sharing across apps
4. Implement MCP for app integration
5. Integrate GIMP Assistant
6. Integrate LibreOffice Assistant

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    SmolPC Launcher                          │
│                  (Profile Selection)                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Code Helper  │  │GIMP Assistant│  │ LibreOffice  │      │
│  │              │  │              │  │  Assistant   │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                 │                 │               │
│         │      MCP        │      MCP        │               │
│         │                 │                 │               │
│  ┌──────┴─────────────────┴─────────────────┴───────┐      │
│  │              smolpc-engine (daemon)               │      │
│  │           Shared across all apps                  │      │
│  └───────────────────────────────────────────────────┘      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Deliverables

### Launcher
- [ ] Launcher application (Tauri)
- [ ] Profile selection on launch
- [ ] App launcher grid
- [ ] Settings access
- [ ] Engine status display

### Engine Daemon
- [ ] Run engine as background service
- [ ] HTTP API for IPC
- [ ] SSE for streaming
- [ ] Health check endpoint
- [ ] Graceful shutdown

### MCP Integration
- [ ] MCP server in engine
- [ ] Tool definitions for each app type
- [ ] Context passing
- [ ] Result handling

### App Integration
- [ ] GIMP plugin skeleton
- [ ] LibreOffice extension skeleton
- [ ] MCP client implementation

---

## Engine Daemon API

```
POST /generate
  Body: { messages, params }
  Response: SSE stream of tokens

POST /cancel
  Response: { ok: true }

GET /status
  Response: { modelLoaded, provider, memory }

GET /health
  Response: { ok: true }
```

---

## MCP Tool Definitions

### Code Helper Tools
- `explain_code` - Explain selected code
- `fix_code` - Fix bugs in code
- `generate_code` - Generate code from description

### GIMP Assistant Tools
- `describe_image` - Describe image content
- `suggest_edit` - Suggest image edits
- `remove_background` - Background removal guidance

### LibreOffice Tools
- `improve_writing` - Improve document text
- `summarize` - Summarize document
- `format_suggestion` - Suggest formatting

---

## Success Criteria

| Criteria | Target |
|----------|--------|
| Launcher opens all apps | Yes |
| Single profile across apps | Yes |
| Engine loads once, shared | Yes |
| MCP communication works | Yes |

---

*When Phase 5 is complete, proceed to PHASE-6.md for VS Code Extension.*
