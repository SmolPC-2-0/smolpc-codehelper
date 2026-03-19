# Phase 4 Plan: Unified Frontend Controller Extraction

Date: 2026-03-19  
Status: Planning document (Step 1 artifact)  
Owner: `apps/libreoffice-assistant`

## Goal

Extract orchestration/state logic from `src/App.svelte` into dedicated frontend controller modules so the app shell becomes composition-first and easier to merge into unified frontend flows.

## Scope

In scope:

1. Move LibreOffice app orchestration/state to `src/lib/stores/*.svelte.ts` modules.
2. Keep runtime behavior unchanged (same command surface, same workflow outcomes, same MCP recovery behavior).
3. Keep `App.svelte` focused on wiring components + callbacks.

Out of scope:

1. Launcher behavior/registration changes.
2. Engine command contract changes.
3. MCP protocol/schema changes.

## Phase 4 implementation slices

1. Introduce `src/lib/stores/libreofficeController.svelte.ts`.
2. Move non-UI helpers and invoke-based actions from `App.svelte` into the controller.
3. Keep component props API stable where possible.
4. Re-run frontend verification:
   - `npm run check:libreoffice`
   - `npm run build:libreoffice`

## Acceptance criteria

1. `App.svelte` no longer owns primary orchestration logic.
2. Existing UI interactions still work via controller methods.
3. Existing workflow outcomes remain unchanged:
   - `model_assisted_success`
   - `cpu_local_fallback`
   - `failed_with_error`
4. Validation gates pass with no new warnings/errors.

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
