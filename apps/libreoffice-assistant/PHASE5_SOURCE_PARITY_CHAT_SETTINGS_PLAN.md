# Phase 5 Plan: Source-Parity UX/Store Migration (Chat + Settings Slice)

Date: 2026-03-19  
Status: Completed (Step 3 docs update)  
Owner: `apps/libreoffice-assistant`

## Goal

Migrate a source-parity frontend slice from the external LibreOffice app into this repo:

1. chat UX/store parity
2. settings UX/store parity

while preserving this repo's engine-only runtime and existing MCP/workflow hardening behavior.

Source reference used for parity mapping:

1. `https://github.com/SmolPC-2-0/smolpc-libreoffice` (`main`, commit `d0d737079b08f84580a20060db0c4f7f39a7ebc2`)
2. `tauri-app/src/lib/stores/chat.svelte.ts`
3. `tauri-app/src/lib/stores/settings.svelte.ts`
4. `tauri-app/src/lib/components/SettingsPage.svelte`
5. `tauri-app/src/App.svelte`

## Scope

In scope:

1. Add dedicated source-parity frontend stores for settings and chat orchestration.
2. Add source-parity chat/settings UI surfaces in `apps/libreoffice-assistant`.
3. Keep parity with source store shape where valid for engine-only:
   - keep `selected_model`, `python_path`, `documents_path`, `libreoffice_path`, theme, prompt, temperature, max tokens
   - remove Ollama runtime fields from active behavior
4. Wire settings into existing controller/workflow path:
   - selected model
   - python command for MCP startup
   - system prompt and generation controls

Out of scope:

1. Backend command-surface expansion for persistent settings APIs.
2. Launcher registration/catalog behavior changes.
3. Full parity for every legacy source view in one phase.

## Acceptance Criteria

1. Source-parity chat/settings stores exist under `src/lib/stores/`.
2. Source-parity chat/settings UI is available in app runtime.
3. Engine-only behavior remains intact (no Ollama command path).
4. Existing diagnostics/workflow panels remain operational.
5. Validation gates pass:
   - `npm run check:libreoffice`
   - `npm run build:libreoffice`
   - `cargo test -p smolpc-libreoffice-assistant --lib`

## Execution Result (2026-03-19)

Delivered in this phase:

1. Added source-parity type models:
   - `src/lib/types/sourceParity.ts`
2. Added source-parity settings/chat stores:
   - `src/lib/stores/libreofficeSettings.svelte.ts`
   - `src/lib/stores/libreofficeChat.svelte.ts`
3. Added source-parity UI components:
   - `src/lib/components/SourceParityPanel.svelte`
   - `src/lib/components/SourceParitySettingsPage.svelte`
   - `src/lib/components/SourceParityChatInput.svelte`
   - `src/lib/components/SourceParityChatMessage.svelte`
4. Added local storage helper:
   - `src/lib/utils/storage.ts`
5. Integrated settings into existing controller runtime behavior:
   - MCP startup now honors configured `python_path`
   - workflow uses configured system prompt, temperature, and max tokens
6. Kept existing diagnostics and workflow verification panels in place.

Validation run:

1. `npm run check:libreoffice` passed.
2. `npm run build:libreoffice` passed.
3. `cargo test -p smolpc-libreoffice-assistant --lib` passed.

GitHub delivery sequence completed:

1. Step 1 docs push completed.
2. Step 2 implementation push completed.
3. Step 3 docs update push completed.

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
