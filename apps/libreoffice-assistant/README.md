# LibreOffice Assistant

Engine-only migration target for porting the external LibreOffice app into the unified SmolPC launcher stack.

## Documentation map

Use these docs in order:

1. `apps/libreoffice-assistant/MIGRATION_PLAN.md` (source-of-truth plan)
2. `apps/libreoffice-assistant/LIBREOFFICE_SOURCE_REPO_ANALYSIS.md` (detailed source audit)
3. `apps/libreoffice-assistant/PHASE1_STATUS.md` (implemented baseline + validations)
4. `apps/libreoffice-assistant/WINDOWS_PHASE1_VERIFICATION.md` (verification runbook)
5. `apps/libreoffice-assistant/WINDOWS_PHASE2_MCP_VERIFICATION.md` (MCP bridge Windows test runbook)

## Current migration status

Phase 1 shared-engine baseline is implemented in this app:

1. Svelte + Tauri shell under `apps/libreoffice-assistant`.
2. Shared-engine bootstrap/status bridge in `src-tauri/src/lib.rs` using `smolpc-engine-client`.
3. Engine command flow wired:
   - `list_models`
   - `load_model` / `unload_model`
   - `generate_text` / `inference_generate` / `inference_cancel`
   - `check_model_readiness` / `get_inference_backend_status`
4. Diagnostics helpers wired:
   - `create_integration_issue_report`
   - `run_runtime_verification_checklist`
   - `export_phase1_evidence_bundle`
5. Phase 2 MCP bridge baseline now wired:
   - Python MCP assets imported under `src-tauri/resources/mcp_server`
   - Rust MCP commands (`start/check/stop/list/call`) registered
   - MCP diagnostics/tool-call panel added to `src/App.svelte`

## Source migration baseline

Source repo analyzed: `/Users/mts/smolpc/smolpc-libreoffice` on 2026-03-12.

Detailed analysis and file-by-file migration deltas:

1. `apps/libreoffice-assistant/MIGRATION_PLAN.md`
2. `apps/libreoffice-assistant/LIBREOFFICE_SOURCE_REPO_ANALYSIS.md`

Key outcome from analysis:

1. Reuse MCP/UNO assets and chat tool loop from source repo.
2. Do not port Ollama paths; this repo remains engine-only.
3. Keep integration contract-first against:
   - `docs/APP_ONBOARDING_PLAYBOOK.md`
   - `docs/ENGINE_API.md`
   - `docs/SMOLPC_SUITE_INTEGRATION.md`

## Useful commands

From repo root:

1. `npm run dev:libreoffice`
2. `npm run tauri:dev:libreoffice`
3. `npm run check:libreoffice`
4. `cargo test -p smolpc-libreoffice-assistant --lib`
