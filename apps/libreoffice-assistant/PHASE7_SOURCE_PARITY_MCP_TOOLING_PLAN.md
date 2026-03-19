# Phase 7 Plan: Source-Parity MCP Tooling Workspace

Date: 2026-03-19  
Status: Planned (Step 1 docs push)  
Owner: `apps/libreoffice-assistant`

## Goal

Complete the remaining source-parity UX slice by migrating MCP tooling/workspace interactions into the source-parity workspace, while preserving this repo's engine-only runtime and existing MCP hardening behavior.

Source references used for parity mapping:

1. `https://github.com/SmolPC-2-0/smolpc-libreoffice` (`main`)
2. `tauri-app/src/App.svelte`
3. `tauri-app/src/lib/stores/app.svelte.ts`
4. `tauri-app/src/lib/stores/chat.svelte.ts`

Current repo mapping targets:

1. `src/lib/components/SourceParityPanel.svelte`
2. `src/lib/components/McpBridgePanel.svelte`
3. `src/lib/stores/libreofficeController.svelte.ts`
4. `src/lib/stores/libreofficeChat.svelte.ts`

## Scope

In scope:

1. Add a source-parity MCP tooling workspace view inside `SourceParityPanel` (for example a dedicated Tools tab).
2. Migrate MCP runtime controls into source-parity workspace UX using existing commands only:
   - `check_mcp_status`
   - `start_mcp_server`
   - `stop_mcp_server`
   - `list_mcp_tools`
   - `call_mcp_tool`
3. Provide source-parity workspace behavior for:
   - MCP status/readiness summary
   - tool catalog refresh and selection
   - JSON argument templating/editing
   - tool invocation + result inspection
4. Keep chat workflow integration aligned with tooling selection state (especially `tool_first` mode).
5. Preserve existing dependency-gating startup behavior from Phase 6.

Out of scope:

1. Any Ollama provider/runtime reintroduction.
2. Backend command-surface expansion or schema changes.
3. Python MCP helper/tool implementation changes in this phase.
4. Launcher catalog/installer/registration behavior changes.
5. Non-MCP source screens beyond this remaining workspace/tooling slice.

## Acceptance Criteria

1. Source-parity workspace includes MCP tooling UX that covers status, tool catalog, argument editing, and invocation result viewing.
2. MCP tooling interactions in source-parity workspace run through the current engine+MCP command contract with no new provider branches.
3. `tool_first` workflow mode in source-parity chat uses the same selected MCP tool/arguments from workspace tooling state.
4. Engine-only constraints remain intact (no provider toggle, no Ollama command path).
5. Validation gates pass with no regressions:
   - `npm run check:libreoffice`
   - `npm run build:libreoffice`
   - `cargo test -p smolpc-libreoffice-assistant --lib`

## Phase 7 implementation slices

1. Add source-parity MCP tooling component(s) under `src/lib/components/`.
2. Wire source-parity tooling view into `SourceParityPanel` navigation/state.
3. Unify tooling selection/argument template behavior with existing controller state.
4. Keep dependency and readiness signaling consistent between loading gate, chat view, and tooling view.
5. Run validation gates and capture Phase 7 evidence notes for post-implementation docs.

## Risks and mitigations

1. Risk: duplicate MCP UI paths can diverge in behavior.
   - Mitigation: keep command invocations and state authority centralized in `libreofficeController`.
2. Risk: invalid JSON arguments in tooling workspace can create operator confusion.
   - Mitigation: preserve explicit JSON object validation and actionable error messaging.
3. Risk: MCP startup race/regression during workspace actions.
   - Mitigation: retain idempotent start/refresh flow and existing MCP recovery behavior.
4. Risk: parity work accidentally reintroduces provider abstraction drift.
   - Mitigation: enforce engine-only scope and reject Ollama fields/branches in Phase 7 changes.

## Mandatory 3-step phase workflow

Every active phase in this app must follow this delivery sequence:

1. Step 1: Phase planning + docs push
   - Add/update phase plan docs first.
   - Push documentation commit to GitHub before implementation code.
2. Step 2: Implementation + code push
   - Implement the scoped phase changes.
   - Push implementation commit(s) to GitHub.
3. Step 3: Post-implementation docs update + docs push
   - Update migration/status docs to reflect what shipped.
   - Push the post-phase docs commit to GitHub.

No phase is considered complete until all 3 steps are done.