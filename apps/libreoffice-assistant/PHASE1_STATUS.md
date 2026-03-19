# LibreOffice Assistant Phase 1 Status

Historical note (2026-03-16): this file records Phase 1-4 implementation evidence from the earlier launcher-era baseline.  
Use current planning docs for active scope:

1. `apps/libreoffice-assistant/MIGRATION_PLAN.md`
2. `apps/libreoffice-assistant/UNIFIED_FRONTEND_HANDOFF.md`

Primary planning doc for next phases:

1. `apps/libreoffice-assistant/MIGRATION_PLAN.md`

## Implemented in this branch

1. App scaffold and workspace registration (`npm` + Cargo workspaces).
2. Shared engine bridge startup/status commands:
   - `get_bootstrap_status`
   - `ensure_engine_started`
3. Shared engine inference command surface:
   - `list_models`
   - `load_model`
   - `unload_model`
   - `generate_text`
   - `inference_generate`
   - `inference_cancel`
   - `is_generating`
   - `get_current_model`
   - `get_inference_backend_status`
   - `check_model_readiness`
   - `check_model_exists`
4. Diagnostics + verification command surface:
   - `create_integration_issue_report`
   - `run_runtime_verification_checklist`
   - `export_phase1_evidence_bundle`
5. Frontend Phase 1 control panel for:
   - bootstrap status
   - model load/unload
   - non-stream generation
   - stream generation and cancel
   - readiness and backend diagnostics
   - runtime verification execution
   - integration issue payload generation/copy
6. Rust unit tests for desired-model restore + runtime checklist evaluation.
7. Windows verification runbook:
   - `apps/libreoffice-assistant/WINDOWS_PHASE1_VERIFICATION.md`
8. Windows Phase 2 MCP verification runbook:
   - `apps/libreoffice-assistant/WINDOWS_PHASE2_MCP_VERIFICATION.md`
9. Source repo audit completed for migration planning:
   - `apps/libreoffice-assistant/LIBREOFFICE_SOURCE_REPO_ANALYSIS.md`
10. Phase 3 workflow panel and orchestration hardened in `src/App.svelte`:
   - strict preflight checks before workflow execution
   - JSON fallback tool-call parsing
   - tool-first fast path for CPU lane
   - explicit run outcomes (`model_assisted_success`, `cpu_local_fallback`, `failed_with_error`)
   - helper connection refusal recovery (MCP restart + one retry)
11. MCP runtime hardening in `src-tauri/src/services/mcp_client.rs`:
   - idempotent start semantics
   - long-lived response pump (no per-request thread spawn)
   - request/response ID correlation guardrails
   - deterministic stop with cleanup-safe restart
12. Unified frontend controller extraction (2026-03-19):
   - app orchestration/state moved from `src/App.svelte` into `src/lib/stores/libreofficeController.svelte.ts`
   - UI panel sections modularized under `src/lib/components/*`
   - `src/App.svelte` reduced to composition/wiring layer

## Validation run (local)

1. `cargo check -p smolpc-libreoffice-assistant`
2. `cargo test -p smolpc-libreoffice-assistant --lib`
3. `npm run check:libreoffice`
4. `npm run build:libreoffice`

All passed.

## Source audit findings applied (2026-03-12)

From `/Users/mts/smolpc/smolpc-libreoffice`:

1. MCP/UNO stack is ready for reuse and includes 27 active tools in `libre.py`.
2. Source app still contains legacy Ollama dual-provider wiring (Rust commands/services + settings UI).
3. Source shared-engine layer is HTTP-based with compatibility parsing; this repo should keep the typed `smolpc-engine-client` path.
4. Migration should import MCP assets + chat/tool UX while keeping engine-only runtime in unified launcher.

## Next implementation scope (post-Phase 1)

Tracked in:

1. `apps/libreoffice-assistant/MIGRATION_PLAN.md`

## Phase 2 progress snapshot (2026-03-12)

1. MCP runtime assets imported into `src-tauri/resources/mcp_server`.
2. MCP Rust model/service modules added:
   - `src-tauri/src/models/mcp.rs`
   - `src-tauri/src/services/mcp_client.rs`
3. Tauri MCP commands added and registered:
   - `start_mcp_server`
   - `check_mcp_status`
   - `stop_mcp_server`
   - `list_mcp_tools`
   - `call_mcp_tool`
4. Frontend MCP diagnostics panel added to `src/App.svelte`.
5. Post-change validation passed:
   - `cargo check -p smolpc-libreoffice-assistant`
   - `cargo test -p smolpc-libreoffice-assistant --lib`
   - `npm run check:libreoffice`
   - `npm run build:libreoffice`

## Phase 2 Windows verification completion (2026-03-12)

1. MCP server startup verified in app UI (`running: true`).
2. MCP tool discovery verified in app UI (`tools_loaded: 27`).
3. Read-only MCP tool invocation verified from app UI:
   - tool: `list_documents`
   - directory: `C:\\Users\\<YOUR_USER>\\Documents`
   - result included `test.docx` and `test.odt`.
4. Windows runbook used:
   - `apps/libreoffice-assistant/WINDOWS_PHASE2_MCP_VERIFICATION.md`

## Phase 3 acceptance validation (2026-03-15)

1. DirectML lane status captured with active model `qwen3-4b-instruct-2507`.
2. DirectML lane model-assisted response captured without fallback summary.
3. CPU lane status captured with forced override, active model `qwen2.5-coder-1.5b`.
4. CPU lane generation validated after explicit `load_model` recovery from `No model loaded`.
5. Live MCP runtime verification captured (initialize + tools/list + `list_documents` call).
6. Validation artifacts documented in:
   - `apps/libreoffice-assistant/WINDOWS_PHASE3_WORKFLOW_VERIFICATION.md`
   - `apps/libreoffice-assistant/WINDOWS_PHASE3_TEAMMATE_RESULTS_2026-03-15.md`
