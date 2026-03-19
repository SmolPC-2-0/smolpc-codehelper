# Phase 7: Source-Parity MCP Tooling Workspace

Date: 2026-03-19  
Status: Completed (Step 3 docs push)  
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

1. [x] Source-parity workspace includes MCP tooling UX that covers status, tool catalog, argument editing, and invocation result viewing.
2. [x] MCP tooling interactions in source-parity workspace run through the current engine+MCP command contract with no new provider branches.
3. [x] `tool_first` workflow mode in source-parity chat uses the same selected MCP tool/arguments from workspace tooling state.
4. [x] Engine-only constraints remain intact (no provider toggle, no Ollama command path).
5. [x] Validation gates pass with no regressions:
   - `npm run check:libreoffice`
   - `npm run build:libreoffice`
   - `cargo test -p smolpc-libreoffice-assistant --lib`

## Shipped implementation slices

1. Added source-parity MCP tooling component:
   - `src/lib/components/SourceParityToolsPage.svelte`
2. Wired source-parity tooling view into panel navigation/state:
   - `src/lib/components/SourceParityPanel.svelte` now includes a `Tools` tab and pass-through handlers/props.
3. Unified tooling selection/argument template behavior with existing controller state:
   - `src/App.svelte` now passes MCP tool state/actions into source-parity panel.
4. Preserved chat/tooling state alignment for `tool_first` mode:
   - `src/lib/stores/libreofficeChat.svelte.ts` blocks tool-first send when no tool is selected.
   - `src/lib/stores/libreofficeController.svelte.ts` error guidance now points to Source-Parity Tools tab.
5. Validation gates executed successfully during post-implementation docs finalization.

## Execution result (2026-03-19)

1. `npm run check:libreoffice`
   - Result: pass (`svelte-check found 0 errors and 0 warnings`)
2. `npm run build:libreoffice`
   - Result: pass (`vite build` completed successfully)
3. `cargo test -p smolpc-libreoffice-assistant --lib`
   - Result: pass (`12 passed; 0 failed`)

## Delivery workflow record

1. Step 1 docs push completed:
   - commit: `9f126d7`
   - message: `docs(libreoffice): add phase 7 source parity MCP tooling plan`
2. Step 2 implementation push completed:
   - commit: `d33c865`
   - message: `feat(libreoffice): add source-parity MCP tools workspace`
3. Step 3 post-implementation docs push:
   - completed in current docs finalization commit.

## Risks and mitigations (as planned)

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
