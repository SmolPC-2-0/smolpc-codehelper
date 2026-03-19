# LibreOffice Assistant Migration Plan

Date: 2026-03-16  
Status owner: `apps/libreoffice-assistant`

## Purpose

This is the source-of-truth implementation plan for LibreOffice work in this repo.

Active target: keep `apps/libreoffice-assistant` as a standalone engine+MCP app that is merge-ready for the upcoming unified frontend.

## Decision Update (2026-03-16)

1. Active track is app-first merge prep, not launcher feature expansion.
2. Launcher compatibility remains in place, but launcher-specific binding/registration work is deferred.
3. No runtime behavior change is required in this cycle; this cycle is docs/contract/handoff alignment.

## Baseline references

1. `docs/APP_ONBOARDING_PLAYBOOK.md`
2. `docs/ENGINE_API.md`
3. `docs/SMOLPC_SUITE_INTEGRATION.md`
4. `apps/libreoffice-assistant/LIBREOFFICE_SOURCE_REPO_ANALYSIS.md`
5. `apps/libreoffice-assistant/PHASE1_STATUS.md`
6. `apps/libreoffice-assistant/UNIFIED_FRONTEND_HANDOFF.md`
7. `docs/LIBREOFFICE_UNIFIED_LAUNCHER_PORTING_GUIDE.md` (legacy/deferred reference)

## Non-negotiable constraints

1. Do not reintroduce Ollama runtime paths in this app.
2. Keep engine integration on `smolpc-engine-client` typed commands.
3. Keep contract-first behavior against `/engine/*` and `/v1/*` API semantics.
4. Preserve Windows-first reliability for release validation.
5. Do not remove existing launcher compatibility in this cycle.

## Active Now

Current sprint planning should prioritize:

1. Engine-only + MCP workflow stability in `apps/libreoffice-assistant`.
2. Workflow reliability evidence quality (DirectML lane + CPU fallback lane).
3. Unified frontend handoff contract completeness.

### Active milestones (merge-readiness)

1. Command-surface stability
   - Keep the documented Tauri command contract stable while unified frontend integration is underway.
2. Workflow reliability evidence
   - Keep verified artifacts for model-assisted success and CPU fallback-safe execution.
3. Frontend handoff contract completeness
   - Keep `UNIFIED_FRONTEND_HANDOFF.md` current with command groups, high-level payload expectations, and out-of-scope launcher semantics.

## Mandatory phase workflow (GitHub delivery contract)

Every active LibreOffice phase must use this 3-step delivery workflow:

1. Step 1: plan + docs push
   - Create/update the phase planning docs first.
   - Push docs-only planning commit to GitHub before implementation code.
2. Step 2: implementation + code push
   - Implement the scoped phase work.
   - Push implementation commit(s) to GitHub.
3. Step 3: post-phase docs update + docs push
   - Update migration/status/handoff docs with shipped changes and validation results.
   - Push the post-phase docs commit to GitHub.

Current active phase plan:

1. `apps/libreoffice-assistant/PHASE4_FRONTEND_CONTROLLER_PLAN.md`
   - Status (2026-03-19): completed

## Deferred

The following work is deferred for this sprint unless explicitly reactivated:

1. Launcher catalog and installer UX expansion work.
2. New launcher binding tasks beyond already-shipped compatibility paths.
3. New launcher registration behavior changes.

Deferred means:

1. Existing launcher integration artifacts stay in repo unchanged.
2. Installer hooks/registration docs remain valid as legacy compatibility references.
3. No launcher code removal is part of this plan.

## Current state snapshot

Implemented:

1. Engine bootstrap, model lifecycle, generation, cancel, diagnostics, and verification surfaces.
2. Runtime checklist and evidence export flow in app backend/UI.
3. Source repo audit completed and mapped to migration tasks.
4. MCP bridge acceptance completed with Windows evidence (`running: true`, `tools_loaded: 27`, successful `list_documents` call).
5. Workflow orchestration with JSON fallback parsing and CPU-safe fallback logic.
6. Reliability updates for model-state alignment and lane-aware model policy.
7. MCP runtime hardening:
   - idempotent `start_mcp_server`
   - cleanup-safe initialization failure handling
   - long-lived response pump with request/response ID correlation
8. Phase 3 acceptance artifacts captured for DirectML and CPU fallback lanes.
9. Unified frontend controller extraction completed:
   - `src/App.svelte` now acts as composition shell
   - orchestration/state moved to `src/lib/stores/libreofficeController.svelte.ts`
   - panel UI split into `src/lib/components/*`

Not implemented yet:

1. Full source-parity UX/store migration from the external LibreOffice app.
2. Additional teammate machine matrix entries beyond the required Windows hard gate.

## Verification and quality gates

Contract/doc alignment gates:

1. README points to `MIGRATION_PLAN.md` first and `UNIFIED_FRONTEND_HANDOFF.md` second.
2. Launcher-specific docs are clearly marked legacy/deferred for active sprint planning.
3. Handoff command list matches backend `invoke_handler` command registrations.

Runtime regression gates (no behavior changes expected):

1. `npm run check:libreoffice`
2. `cargo test -p smolpc-libreoffice-assistant --lib`
3. Optional confidence gate: `npm run build:libreoffice`

## Documentation organization contract

Use docs in this order:

1. `MIGRATION_PLAN.md` for active implementation sequence and current scope.
2. `UNIFIED_FRONTEND_HANDOFF.md` for teammate merge contract.
3. `LIBREOFFICE_SOURCE_REPO_ANALYSIS.md` for source audit details and mapping rationale.
4. `PHASE1_STATUS.md` for completed baseline and validation evidence.
5. `WINDOWS_PHASE1_VERIFICATION.md` for runtime evidence capture steps.
6. `WINDOWS_PHASE2_MCP_VERIFICATION.md` for MCP bridge validation on Windows.
7. `WINDOWS_PHASE3_WORKFLOW_VERIFICATION.md` for chat/tool workflow validation matrix.
8. `WINDOWS_PHASE3_TEAMMATE_RESULTS_TEMPLATE.md` for teammate result capture format.
9. `WINDOWS_PHASE3_TEAMMATE_RESULTS_2026-03-15.md` for completed Windows lane matrix evidence.
10. `WINDOWS_INSTALLER_REGISTRATION.md` for legacy/deferred launcher registration contract reference.
