# CodeHelper App

CodeHelper is the current frontend and Tauri shell for the unified app on
`dev/unified-assistant-self-contained`.

## Start Here

For the current Windows source-based functional validation pass, use:

- `docs/unified-assistant-self-contained-spec/WINDOWS_SOURCE_TESTING.md`
- `docs/unified-assistant-self-contained-spec/WINDOWS_SOURCE_TEST_RESULTS_TEMPLATE.md`

Those docs are the source of truth for testing the unified app before any
engine reconciliation, packaging, or installer work.

## Structure

- `apps/codehelper/src`: Svelte frontend
- `apps/codehelper/src-tauri`: Rust/Tauri backend
- `apps/codehelper/scripts`: app-local development scripts

## Commands (from repo root)

- `npm run check`
- `npm run tauri:dev`
- `npm run tauri:dml`
- `npm run model:setup:qwen3`
- `node apps/codehelper/scripts/self-contained/validate-resource-manifests.mjs`

## Windows Source-Test Quickstart

1. Use a separate clean clone on `origin/dev/unified-assistant-self-contained`.
2. Run `npm ci`.
3. Run `npm run check`.
4. If you want Code mode to execute prompts, either:
   - make sure `SMOLPC_MODELS_DIR` already points at a working shared model
     root, or
   - run `npm run model:setup:qwen3`.
5. Launch the app with `npm run tauri:dev`.

## Current Source-Test Expectations

- This branch is being tested for source-based functionality, not packaged
  self-contained delivery yet.
- `bundled_model` and `bundled_python` can still show `not_prepared` in the
  setup panel during source-mode testing if packaged payloads have not been
  staged.
- That does not automatically mean the branch is broken for this test pass.
- Setup detail text and per-mode status are the source of truth for whether a
  mode is usable on the tester's machine.

## Engine Integration

- Startup/readiness flow is engine-driven (`ensure_started` + readiness status).
- Inference generation/cancel/list/load calls route through
  `smolpc-engine-client`.
- App-local inference engine modules are removed.
