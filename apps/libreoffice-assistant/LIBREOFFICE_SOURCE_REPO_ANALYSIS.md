# LibreOffice Source Repo Analysis (smolpc-libreoffice)

Date: 2026-03-12  
Source analyzed: `/Users/mts/smolpc/smolpc-libreoffice` (branch: `codex/libreoffice-engine-migration-start`)

Planning doc linked to this analysis:

1. `apps/libreoffice-assistant/MIGRATION_PLAN.md`

## Why this exists

This document maps the current `smolpc-libreoffice` implementation to the unified launcher baseline in this repo.

Baseline references for this mapping:

1. `docs/APP_ONBOARDING_PLAYBOOK.md`
2. `docs/ENGINE_API.md`
3. `docs/SMOLPC_SUITE_INTEGRATION.md`

## Executive Summary

The source LibreOffice repo is a usable Tauri/Svelte app with working MCP + LibreOffice tooling, but it is still in a dual-provider transition state (`ollama` + `smolpc_engine`).  
For the unified launcher migration, the MCP/UNO stack and chat/tool orchestration are the key reusable assets.  
Ollama-specific wiring should not be ported.

## Source Inventory

### App and runtime shape

1. Frontend: Svelte 5 app with stores for app init, settings, chat streaming, and MCP tools.
2. Backend: Tauri Rust app with command modules:
   - `commands/ai.rs` (provider-agnostic stream entrypoint)
   - `commands/mcp.rs`
   - `commands/system.rs`
   - `commands/config.rs`
   - `commands/ollama.rs` (legacy provider path)
3. Services:
   - `services/smolpc_engine_service.rs` (HTTP-based shared-engine integration with fallback logic)
   - `services/ollama_service.rs` (legacy)
   - `services/mcp_client.rs` (stdio JSON-RPC bridge to Python MCP server)
4. Python MCP assets:
   - `src-tauri/resources/mcp_server/main.py`
   - `src-tauri/resources/mcp_server/libre.py`
   - `src-tauri/resources/mcp_server/helper.py`
   - `src-tauri/resources/mcp_server/helper_utils.py`
   - `src-tauri/resources/mcp_server/helper_test_functions.py`

### MCP/tooling coverage

1. `libre.py` exposes 27 active `@mcp.tool()` tools (Writer + Impress + document utilities).
2. Tool execution is serialized through a queue worker and helper socket bridge.
3. `helper.py` is large and UNO-heavy; this is core migration value and should be reused, not rewritten in Rust initially.

### Shared-engine implementation quality

`services/smolpc_engine_service.rs` includes mature compatibility behavior:

1. Health check fallback: `/engine/health` -> `/health`.
2. Generate fallback: `/v1/chat/completions` -> legacy `/generate`.
3. Models fallback: `/v1/models` -> legacy `/models`.
4. Bearer token auto-discovery:
   - `SMOLPC_ENGINE_TOKEN`
   - `%LOCALAPPDATA%\\SmolPC\\engine-runtime\\engine-token.txt`
5. Stream parser handles:
   - SSE `data:` framing
   - `[DONE]`
   - OpenAI-style delta text
   - tool call payload variants
   - split streamed tool-call argument fragments
6. File has unit tests for stream parsing and tool-call accumulation.

Note: In this monorepo we prefer `smolpc-engine-client` typed integration over a custom HTTP wrapper, so this service is a behavioral reference more than direct copy target.

## Legacy/Ollama leftovers (must not be ported)

### Rust backend leftovers

1. `src-tauri/src/commands/ollama.rs`
2. `src-tauri/src/services/ollama_service.rs`
3. `src-tauri/src/models/ollama.rs` used as shared message type namespace
4. `src-tauri/src/lib.rs` still registers ollama commands:
   - `commands::ollama::list_ollama_models`
   - `commands::ollama::chat_stream`
   - `commands::ollama::check_ollama_running`
5. `commands/system.rs` still exposes `check_ollama`.
6. `services/process_manager.rs` still carries `ollama_process` and `libreoffice_process` fields not needed in engine-only path.

### Frontend/config leftovers

1. Provider selector remains in settings UI (`ollama` vs `smolpc_engine`).
2. Settings schema defaults to `ai_provider: "ollama"`.
3. `ollama_url` still part of TS + Rust settings models.
4. App initialization chooses dependency check based on provider.

For unified launcher integration, provider selection should be removed and engine should be the only runtime.

## Mapping to Unified Launcher Plan

### Keep as-is or near as-is

1. Python MCP server assets (`main.py`, `libre.py`, `helper.py`, utilities).
2. Rust MCP client model (stdio JSON-RPC + cached tool list + call execution).
3. Frontend chat/tool loop behavior:
   - stream chunks
   - tool-call detection
   - bounded recursion / max tool-call safeguards
4. Settings fields unrelated to provider:
   - `documents_path`
   - `libreoffice_path`
   - `python_path`
   - prompt + generation controls

### Adapt (not direct copy)

1. Replace source HTTP engine service usage with this repo’s `smolpc-engine-client` command layer.
2. Replace provider-aware command calls (`chat_stream_ai`, `list_ai_models`) with engine-only commands.
3. Keep tool-call JSON fallback parser logic from frontend store, but remove provider-specific branches.
4. Align model defaults with onboarding baseline:
   - primary `qwen3-4b-instruct-2507`
   - fallback `qwen2.5-coder-1.5b`

### Drop

1. All `commands/ollama.rs` usage.
2. All `services/ollama_service.rs` usage.
3. All config/UI provider toggles and `ollama_url`.
4. Ollama-specific dependency checks and labels.

## Integration Phases (updated from source audit)

### Phase 1 (already in this branch)

1. Engine bootstrap + diagnostics shell exists in `apps/libreoffice-assistant`.
2. Shared engine client surface already wired and validated.

### Phase 2 (next implementation target)

1. Port MCP Python assets into this repo:
   - copy files from source `tauri-app/src-tauri/resources/mcp_server/*`
   - preserve existing file names to minimize behavior drift
2. Implement Rust MCP bridge in this app:
   - `commands/mcp.rs`
   - `services/mcp_client.rs`
   - `models/mcp.rs`
   - invoke registration in `src-tauri/src/lib.rs`
3. Add startup/status integration:
   - dependency checks for Python + engine + LibreOffice
   - start/stop MCP process lifecycle
4. Add minimal MCP validation view in existing Phase 1 UI:
   - start/check MCP
   - list tools
   - call selected tool with JSON payload

Acceptance for Phase 2:

1. `start_mcp_server` returns running=true.
2. `list_mcp_tools` returns non-empty tool catalog.
3. At least one read-only tool call succeeds from app UI.

### Phase 3 (chat workflow migration)

1. Port chat UX/stores/components from source repo to this app.
2. Adapt chat store invoke path to engine-only commands.
3. Keep tool safety caps:
   - max tool chain depth
   - max tool calls per response
4. Preserve fallback tool-call JSON parsing for engine responses that do not emit structured tool calls.

Acceptance for Phase 3:

1. Streamed assistant responses render in UI.
2. Tool calls are executed through MCP and fed back to the model loop.
3. End-to-end document workflow succeeds in Windows lane.

### Phase 4 (packaging and hardening)

1. Bundle MCP assets for production Tauri packaging.
2. Verify platform behavior:
   - Windows production target
   - macOS dev path (manual helper flow) as optional support
3. Capture evidence bundle and issue-report payloads for review.

## Risks and Mitigations

1. Risk: Source config defaults still bias to Ollama.
   - Mitigation: enforce engine-only defaults in migrated settings model.
2. Risk: MCP helper runtime differs across OS.
   - Mitigation: keep source startup strategy and explicitly document macOS manual path.
3. Risk: Tool-call shape variance from engine streaming.
   - Mitigation: preserve existing tool delta merge and JSON fallback parsing behavior.
4. Risk: Drift between copied Python assets and upstream source changes.
   - Mitigation: pin a source commit reference when importing assets and note sync procedure.

## Recommended Immediate Next Actions

1. Begin Phase 2 by importing MCP Python assets and wiring `mcp_client` + commands in this app.
2. Keep all provider-facing surfaces engine-only from day one in this repo.
3. After MCP command wiring lands, execute the existing Windows verification runbook and attach first evidence bundle.
