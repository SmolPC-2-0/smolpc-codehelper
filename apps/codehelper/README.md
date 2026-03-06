# CodeHelper App

CodeHelper frontend and Tauri app shell.

## Structure

- `apps/codehelper/src`: Svelte frontend
- `apps/codehelper/src-tauri`: Rust/Tauri backend
- `apps/codehelper/scripts`: app-local development scripts

## Commands (from repo root)

- `npm run tauri:dev`
- `npm run tauri:dml`
- `npm run check`

## Engine Integration

- Startup/readiness flow is engine-driven (`ensure_started` + readiness status).
- Inference generation/cancel/list/load calls route through `smolpc-engine-client`.
- App-local inference engine modules are removed.
