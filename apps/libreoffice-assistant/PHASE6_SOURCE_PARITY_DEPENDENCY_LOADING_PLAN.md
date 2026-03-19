# Phase 6 Plan: Source-Parity Dependency Loading UX

Date: 2026-03-19  
Status: Planned (Step 1 docs push)  
Owner: `apps/libreoffice-assistant`

## Goal

Migrate the remaining high-value source-parity startup UX slice by adding a dependency-loading experience before chat use, adapted for this repo's engine-only runtime.

Source reference used for parity mapping:

1. `https://github.com/SmolPC-2-0/smolpc-libreoffice` (`main`)
2. `tauri-app/src/lib/components/LoadingScreen.svelte`
3. `tauri-app/src/lib/stores/app.svelte.ts`
4. `tauri-app/src/App.svelte`

## Scope

In scope:

1. Add a source-parity loading/dependency component in `apps/libreoffice-assistant/src/lib/components/`.
2. Gate the source-parity chat surface behind dependency readiness checks derived from existing app state:
   - engine health
   - model catalog presence
   - selected model resolution
   - MCP runtime status
3. Add operator actions in the loading surface using existing controller commands:
   - refresh checks
   - ensure engine started
   - start MCP server
4. Preserve settings accessibility while dependency gates are unresolved.

Out of scope:

1. New backend Tauri commands for dependency checks.
2. Launcher catalog/installer/registration behavior changes.
3. Ollama/runtime provider reintroduction.

## Acceptance Criteria

1. Source-parity loading screen component exists and is wired into `SourceParityPanel`.
2. Chat view is not shown until dependencies are ready.
3. Dependency state is driven by existing engine+MCP+model app state without command-surface changes.
4. Existing diagnostics/workflow panels continue to operate.
5. Validation gates pass:
   - `npm run check:libreoffice`
   - `npm run build:libreoffice`
   - `cargo test -p smolpc-libreoffice-assistant --lib`

## Mandatory 3-step Phase Workflow

Every active phase in this app must follow this delivery sequence:

1. Step 1: Phase planning + docs push
   - Add/update phase planning docs first.
   - Push docs commit before implementation code.
2. Step 2: Implementation + code push
   - Implement scoped code changes.
   - Push implementation commit(s).
3. Step 3: Post-implementation docs update + docs push
   - Update migration/status docs and validation evidence.
   - Push final docs commit.
