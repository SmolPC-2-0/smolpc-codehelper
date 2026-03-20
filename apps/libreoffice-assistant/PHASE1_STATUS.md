# LibreOffice Assistant Phase 1 Status

Historical note (2026-03-16): this file records implementation evidence across Phases 1-9 from the earlier launcher-era baseline to current source-parity slices.  
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
13. Source-parity UX/store migration slice (chat/settings) (2026-03-19):
   - added source-parity settings/chat stores:
     - `src/lib/stores/libreofficeSettings.svelte.ts`
     - `src/lib/stores/libreofficeChat.svelte.ts`
   - added source-parity chat/settings UI:
     - `src/lib/components/SourceParityPanel.svelte`
     - `src/lib/components/SourceParitySettingsPage.svelte`
   - wired settings into controller runtime behavior:
     - selected model synchronization
     - `python_path` passed to MCP startup calls
     - workflow prompt/config overrides (`system_prompt`, `temperature`, `max_tokens`)
14. Source-parity dependency-loading startup slice (2026-03-19):
   - added source-parity loading component:
     - `src/lib/components/SourceParityLoadingScreen.svelte`
   - added source-parity dependency state types:
     - `src/lib/types/sourceParity.ts`
   - gated source-parity chat behind dependency readiness:
     - engine health
     - model catalog availability
     - selected model resolution
     - MCP runtime status
15. Source-parity MCP tooling workspace slice (2026-03-19):
    - added source-parity tools workspace component:
      - `src/lib/components/SourceParityToolsPage.svelte`
    - wired source-parity tools tab in `src/lib/components/SourceParityPanel.svelte`:
     - MCP status/start/stop/refresh controls
     - tool catalog selection + template application
     - JSON argument validation + invocation result rendering
    - tightened tool-first workflow behavior in chat/controller:
      - tool-first send now requires a selected MCP tool
      - error messaging points operators to Source-Parity Tools tab
16. Source-parity chat session persistence + resume UX slice (2026-03-19):
    - added schema-versioned chat-session persistence types in `src/lib/types/sourceParity.ts`
    - added robust localStorage helpers (including key removal) in `src/lib/utils/storage.ts`
    - added startup restore + malformed-payload fallback handling in `src/lib/stores/libreofficeChat.svelte.ts`
    - added explicit confirmation gate for session clear in `src/lib/components/SourceParityPanel.svelte`
17. Source-parity session resume hardening + safety UX slice (2026-03-20):
    - added source-parity session restore metadata in `src/lib/stores/libreofficeChat.svelte.ts`
    - added resumed-session banner and corrupt-reset warning banner in `src/lib/components/SourceParityPanel.svelte`
    - replaced clear action with explicit `Start New Session` confirmation UX
    - bounded persisted chat payload to latest message history in localStorage
    - added parse-corrupt payload status handling in `src/lib/utils/storage.ts` and operator reset signaling

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

## Phase 5 source-parity chat/settings slice validation (2026-03-19)

1. Source-parity stores and UI components added under `src/lib/stores` and `src/lib/components`.
2. Local frontend and backend validations passed:
   - `npm run check:libreoffice`
   - `npm run build:libreoffice`
   - `cargo test -p smolpc-libreoffice-assistant --lib`
3. Active phase planning doc:
   - `apps/libreoffice-assistant/PHASE5_SOURCE_PARITY_CHAT_SETTINGS_PLAN.md`

## Phase 6 source-parity dependency-loading slice validation (2026-03-19)

1. Source-parity dependency-loading screen added and wired into `SourceParityPanel` + `App.svelte`.
2. Local frontend and backend validations passed:
   - `npm run check:libreoffice`
   - `npm run build:libreoffice`
   - `cargo test -p smolpc-libreoffice-assistant --lib`
3. Active phase planning doc:
   - `apps/libreoffice-assistant/PHASE6_SOURCE_PARITY_DEPENDENCY_LOADING_PLAN.md`

## Phase 7 source-parity MCP tooling workspace validation (2026-03-19)

1. Source-parity tools workspace shipped in UI with MCP controls, tool selection, JSON argument editing, and tool result inspection.
2. Tool-first workflow guard now enforces MCP tool selection in source-parity chat mode.
3. Local frontend and backend validations passed:
   - `npm run check:libreoffice` (`svelte-check found 0 errors and 0 warnings`)
   - `npm run build:libreoffice` (`vite build` completed successfully)
   - `cargo test -p smolpc-libreoffice-assistant --lib` (`12 passed; 0 failed`)
4. Active phase planning doc:
   - `apps/libreoffice-assistant/PHASE7_SOURCE_PARITY_MCP_TOOLING_PLAN.md`

## Phase 8 source-parity chat session persistence validation (2026-03-19)

1. Source-parity chat messages now persist as a schema-versioned localStorage session payload.
2. Startup restore now hydrates persisted chat messages and safely falls back when payload shape/version is invalid.
3. Source-parity clear-session action now requires explicit operator confirmation before deleting session history.
4. Local frontend and backend validations passed:
   - `npm run check:libreoffice` (`svelte-check found 0 errors and 0 warnings`)
   - `npm run build:libreoffice` (`vite build` completed successfully)
   - `cargo test -p smolpc-libreoffice-assistant --lib` (`12 passed; 0 failed`)
5. Active phase planning doc:
   - `apps/libreoffice-assistant/PHASE8_SOURCE_PARITY_CHAT_PERSISTENCE_PLAN.md`

## Phase 9 source-parity session resume hardening validation (2026-03-20)

1. Source-parity chat store now captures restore metadata (`restoreHappened`, restored count, persisted saved-at timestamp).
2. Source-parity chat panel now shows explicit resumed-session and corrupt-reset operator-facing banners.
3. Persisted session payload now stores only bounded recent history, preventing unbounded localStorage growth.
4. Local frontend and backend validations passed:
   - `npm run check:libreoffice` (`svelte-check found 0 errors and 0 warnings`)
   - `npm run build:libreoffice` (`vite build` completed successfully)
   - `cargo test -p smolpc-libreoffice-assistant --lib` (`12 passed; 0 failed`)
5. Active phase planning doc:
   - `apps/libreoffice-assistant/PHASE9_SOURCE_PARITY_SESSION_HARDENING_PLAN.md`
