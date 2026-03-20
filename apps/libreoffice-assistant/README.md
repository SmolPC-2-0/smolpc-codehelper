# LibreOffice Assistant

Standalone LibreOffice app (`apps/libreoffice-assistant`) with shared-engine inference and app-owned MCP/UNO tooling.

Active direction (2026-03-16): keep this app production-ready as an individual app and merge-ready for the upcoming unified frontend.

## Documentation map

Use these docs in order:

1. `apps/libreoffice-assistant/MIGRATION_PLAN.md` (active source-of-truth plan)
2. `apps/libreoffice-assistant/PHASE5_SOURCE_PARITY_CHAT_SETTINGS_PLAN.md` (active phase plan; follows mandatory 3-step delivery workflow)
3. `apps/libreoffice-assistant/PHASE6_SOURCE_PARITY_DEPENDENCY_LOADING_PLAN.md` (completed source-parity dependency-loading phase)
4. `apps/libreoffice-assistant/PHASE7_SOURCE_PARITY_MCP_TOOLING_PLAN.md` (completed source-parity MCP tooling workspace phase)
5. `apps/libreoffice-assistant/PHASE8_SOURCE_PARITY_CHAT_PERSISTENCE_PLAN.md` (completed source-parity chat persistence/resume phase)
6. `apps/libreoffice-assistant/PHASE4_FRONTEND_CONTROLLER_PLAN.md` (completed previous phase)
7. `apps/libreoffice-assistant/UNIFIED_FRONTEND_HANDOFF.md` (merge contract for unified frontend integration)
8. `apps/libreoffice-assistant/LIBREOFFICE_SOURCE_REPO_ANALYSIS.md` (source audit and mapping rationale)
9. `apps/libreoffice-assistant/PHASE1_STATUS.md` (implemented baseline + validations)
10. `apps/libreoffice-assistant/WINDOWS_PHASE1_VERIFICATION.md` (verification runbook)
11. `apps/libreoffice-assistant/WINDOWS_PHASE2_MCP_VERIFICATION.md` (MCP bridge Windows test runbook)
12. `apps/libreoffice-assistant/WINDOWS_PHASE3_WORKFLOW_VERIFICATION.md` (chat/tool workflow Windows runbook)
13. `apps/libreoffice-assistant/WINDOWS_PHASE3_TEAMMATE_RESULTS_TEMPLATE.md` (teammate matrix result template)
14. `apps/libreoffice-assistant/WINDOWS_PHASE3_TEAMMATE_RESULTS_2026-03-15.md` (completed Windows lane matrix entry)
15. `apps/libreoffice-assistant/WINDOWS_INSTALLER_REGISTRATION.md` (legacy/deferred launcher registration reference)

## Mandatory phase workflow

Every active LibreOffice phase follows this required sequence:

1. Step 1: planning docs update and push to GitHub.
2. Step 2: implementation changes and push to GitHub.
3. Step 3: post-implementation docs update and push to GitHub.

## Current status

Implemented baseline:

1. Svelte + Tauri shell under `apps/libreoffice-assistant`.
2. Shared-engine bootstrap/status bridge in `src-tauri/src/lib.rs` via `smolpc-engine-client`.
3. Engine command surface for model lifecycle, generation, cancel, readiness/status.
4. MCP bridge command surface (`start/check/stop/list/call`) with bundled Python assets.
5. Workflow reliability hardening for DirectML and CPU fallback lanes.
6. Unified frontend controller modularization:
   - `src/App.svelte` is composition-first
   - orchestration/state centralized in `src/lib/stores/libreofficeController.svelte.ts`
7. Source-parity chat/settings migration slice:
   - `src/lib/stores/libreofficeSettings.svelte.ts`
   - `src/lib/stores/libreofficeChat.svelte.ts`
   - `src/lib/components/SourceParityPanel.svelte`
8. Source-parity dependency-loading startup slice:
   - `src/lib/components/SourceParityLoadingScreen.svelte`
   - readiness gate wiring in `src/App.svelte` + `src/lib/components/SourceParityPanel.svelte`
9. Source-parity MCP tooling workspace slice:
   - `src/lib/components/SourceParityToolsPage.svelte`
   - Tools tab + MCP controls wired into `src/lib/components/SourceParityPanel.svelte`
   - tool-first send guard uses Source-Parity tool selection state
10. Source-parity chat session persistence + resume UX slice:
    - schema-versioned localStorage payload for source-parity chat history
    - startup restore of persisted chat session with malformed-payload fallback
    - explicit confirmation gate before clearing local chat session
11. Diagnostics/evidence helpers:
    - `create_integration_issue_report`
    - `run_runtime_verification_checklist`
    - `export_phase1_evidence_bundle`

Active focus now:

1. Keep engine+MCP behavior stable for unified-frontend merge.
2. Keep workflow verification evidence current.
3. Keep handoff contract (`UNIFIED_FRONTEND_HANDOFF.md`) in sync with backend command surface.

Deferred for this cycle:

1. New launcher-specific feature work (catalog/install/registration expansion).
2. Launcher compatibility removals (not in scope).

## Architecture constraints

1. Engine-only runtime path (no Ollama provider/runtime).
2. Contract-first integration against:
   - `docs/APP_ONBOARDING_PLAYBOOK.md`
   - `docs/ENGINE_API.md`
   - `docs/SMOLPC_SUITE_INTEGRATION.md`

## Useful commands

From repo root:

1. `npm run dev:libreoffice`
2. `npm run tauri:dev:libreoffice`
3. `npm run check:libreoffice`
4. `cargo test -p smolpc-libreoffice-assistant --lib`
