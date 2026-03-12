# LibreOffice Assistant

Phase 1 is in progress on the shared-engine migration path.

Current Phase 1 baseline in this app:

1. Svelte + Tauri shell under `apps/libreoffice-assistant`.
2. Shared-engine bootstrap/status bridge in `src-tauri/src/lib.rs` using `smolpc-engine-client`.
3. MCP resource placeholder path reserved at `src-tauri/resources/mcp_server`.
4. Onboarding command flow wired:
   - `list_models`
   - `load_model` / `unload_model`
   - `generate_text` / `inference_generate` / `inference_cancel`
   - `check_model_readiness` / `get_inference_backend_status`
5. Diagnostics helpers wired:
   - `create_integration_issue_report`
   - `run_runtime_verification_checklist`

Integration references:

1. `docs/APP_ONBOARDING_PLAYBOOK.md`
2. `docs/ENGINE_API.md`
3. `docs/SMOLPC_SUITE_INTEGRATION.md`

Useful commands from repo root:

1. `npm run dev:libreoffice`
2. `npm run tauri:dev:libreoffice`
3. `npm run check:libreoffice`
