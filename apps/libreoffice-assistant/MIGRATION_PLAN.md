# LibreOffice Assistant Migration Plan

Date: 2026-03-12  
Status owner: `apps/libreoffice-assistant`

## Purpose

This is the source-of-truth implementation plan for porting the external LibreOffice app into the unified SmolPC launcher architecture.

Scope is engine-only integration for inference, with MCP/UNO tooling ported from the source LibreOffice repository.

## Baseline references

1. `docs/APP_ONBOARDING_PLAYBOOK.md`
2. `docs/ENGINE_API.md`
3. `docs/SMOLPC_SUITE_INTEGRATION.md`
4. `apps/libreoffice-assistant/LIBREOFFICE_SOURCE_REPO_ANALYSIS.md`
5. `apps/libreoffice-assistant/PHASE1_STATUS.md`

## Non-negotiable constraints

1. Do not reintroduce Ollama runtime paths in this app.
2. Keep engine integration on `smolpc-engine-client` typed commands.
3. Keep contract-first behavior against `/engine/*` and `/v1/*` API semantics.
4. Preserve Windows-first reliability for release validation.

## Current state snapshot

Implemented:

1. Phase 1 engine bootstrap, model lifecycle, generation, cancel, diagnostics, and verification surfaces.
2. Runtime checklist and evidence export flow in app backend/UI.
3. Source repo audit completed and mapped to migration tasks.

Not implemented yet:

1. End-to-end MCP startup/tool-call validation evidence from a Windows run.
2. End-to-end chat tool loop from source app, adapted to engine-only backend.

## Migration phases

### Phase 1: Engine baseline (completed)

Deliverables:

1. Shared engine startup/status bridge.
2. Model lifecycle + generation + cancel command surface.
3. Runtime diagnostics and evidence export surface.

Acceptance:

1. Local checks and tests pass (see `PHASE1_STATUS.md`).

### Phase 2: MCP runtime port and bridge (in progress)

Goal:

Port the source LibreOffice MCP runtime assets and expose them through Tauri commands in this app.

Work items:

1. Import MCP Python files from source repo into:
   - `apps/libreoffice-assistant/src-tauri/resources/mcp_server/`
2. Add Rust MCP structures:
   - `src-tauri/src/models/mcp.rs`
3. Add Rust MCP client service:
   - `src-tauri/src/services/mcp_client.rs`
4. Add Tauri MCP commands:
   - `src-tauri/src/commands/mcp.rs`
5. Register MCP commands and state in:
   - `src-tauri/src/lib.rs`
6. Add minimal frontend MCP diagnostics panel in:
   - `src/App.svelte`
   - start/check/stop MCP
   - list tools
   - call tool with JSON args

Acceptance:

1. `start_mcp_server` returns running state.
2. `list_mcp_tools` returns non-empty tool list.
3. One safe/read-only MCP call succeeds from UI.

Progress update (2026-03-12):

1. Imported MCP runtime assets into `src-tauri/resources/mcp_server`:
   - `main.py`
   - `libre.py`
   - `helper.py`
   - `helper_utils.py`
   - `helper_test_functions.py`
2. Added Rust MCP model/service modules:
   - `src-tauri/src/models/mcp.rs`
   - `src-tauri/src/services/mcp_client.rs`
3. Wired Tauri MCP commands in backend:
   - `start_mcp_server`
   - `check_mcp_status`
   - `stop_mcp_server`
   - `list_mcp_tools`
   - `call_mcp_tool`
4. Added MCP diagnostics panel in frontend `src/App.svelte`:
   - status refresh/start/stop actions
   - tool list refresh/select
   - tool-call JSON invocation and result panel
5. Validation run completed after integration changes:
   - `cargo check -p smolpc-libreoffice-assistant`
   - `cargo test -p smolpc-libreoffice-assistant --lib`
   - `npm run check:libreoffice`
   - `npm run build:libreoffice`

Remaining Phase 2 acceptance work:

1. Run app in `tauri dev` and verify real MCP startup against local Python/LibreOffice environment.
2. Execute one read-only MCP tool call end-to-end from UI and capture result evidence.
3. Follow Windows runbook:
   - `apps/libreoffice-assistant/WINDOWS_PHASE2_MCP_VERIFICATION.md`

### Phase 3: Chat/tool workflow port

Goal:

Port source chat UX/tool orchestration while keeping engine-only invoke paths.

Work items:

1. Port chat components/stores from source app where useful.
2. Keep unified stream event handling in this app.
3. Port tool-call safety controls:
   - max tool chain depth
   - max tool calls per response
4. Preserve fallback JSON tool-call extraction path.
5. Adapt all backend invoke usage to existing engine-only command surface.

Acceptance:

1. Streamed responses render correctly.
2. MCP tool calls execute and feed back into follow-up model turns.
3. At least one end-to-end document flow works on Windows.

### Phase 4: Packaging and hardening

Goal:

Stabilize production behavior and collect review-grade evidence.

Work items:

1. Ensure MCP assets are bundled in Tauri packaging.
2. Verify startup/shutdown lifecycle for MCP child process.
3. Run Windows verification pass and store artifacts.
4. Validate failure-path behavior:
   - MCP start failure
   - tool-call error reporting
   - queue/timeout handling from engine

Acceptance:

1. Windows validation runbook completed with artifacts.
2. Evidence bundle and issue report attached in review thread.

## Task ordering and dependencies

1. Phase 2 must complete before meaningful Phase 3 integration tests.
2. Phase 3 should complete before final Phase 4 packaging validation.
3. No provider abstraction work should be introduced; engine-only is already the target architecture.

## Risks and mitigations

1. Risk: Source MCP scripts drift after import.
   - Mitigation: record source commit and import date in PR notes.
2. Risk: OS-specific process behavior differences.
   - Mitigation: keep Windows as release lane; keep macOS path documented as dev workflow.
3. Risk: Tool-call shape variance in stream payloads.
   - Mitigation: preserve structured + JSON fallback parsing path with safety caps.

## Documentation organization contract

Use docs in this order:

1. `MIGRATION_PLAN.md` for implementation sequence and scope.
2. `LIBREOFFICE_SOURCE_REPO_ANALYSIS.md` for source audit details and mapping rationale.
3. `PHASE1_STATUS.md` for completed baseline and validation evidence.
4. `WINDOWS_PHASE1_VERIFICATION.md` for runtime evidence capture steps.
5. `WINDOWS_PHASE2_MCP_VERIFICATION.md` for MCP bridge validation on Windows.
