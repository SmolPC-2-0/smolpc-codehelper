# LibreOffice Assistant Phase 1 Status

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
5. Frontend Phase 1 control panel for:
   - bootstrap status
   - model load/unload
   - non-stream generation
   - stream generation and cancel
   - readiness and backend diagnostics
   - runtime verification execution
   - integration issue payload generation/copy
6. Rust unit tests for desired-model restore + runtime checklist evaluation.

## Validation run (local)

1. `cargo check -p smolpc-libreoffice-assistant`
2. `cargo test -p smolpc-libreoffice-assistant --lib`
3. `npm run check:libreoffice`
4. `npm run build:libreoffice`

All passed.

## Remaining Phase 1 scope

1. Replace temporary UI with LibreOffice-task-specific UX and prompt templates.
2. Integrate real LibreOffice-side tool bridge (UNO/MCP process wiring) once repo/code is provided.
3. End-to-end runtime verification on Windows target hardware (DML-first lane checks).
4. Capture and attach first real issue-report payload generated from a failing run.
