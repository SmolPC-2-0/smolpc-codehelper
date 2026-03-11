# LibreOffice Assistant Porting Guide (Unified Launcher + Shared Engine)

Status: Draft migration blueprint  
Updated: 2026-03-10  
Scope: Port `/Users/mts/smolpc/smolpc-libreoffice` into this monorepo under `apps/libreoffice-assistant` and launch it through the unified launcher flow.

## 1. Goal and Boundaries

This guide defines the concrete path to move the legacy LibreOffice app from an Ollama-era codebase to the SmolPC shared engine + launcher architecture.

Required boundaries:

1. Inference must use shared engine contract only (`smolpc-engine-client` for Rust/Tauri or documented `/engine/*` and `/v1/*` HTTP contract).
2. No app-local inference runtime ownership.
3. No Ollama provider/runtime path in the LibreOffice app after cutover.
4. LibreOffice app should be launchable via launcher manifest with engine readiness gate.

Reference contract docs:

1. `docs/APP_ONBOARDING_PLAYBOOK.md`
2. `docs/ENGINE_API.md`
3. `docs/SMOLPC_SUITE_INTEGRATION.md`

## 2. Current Repository Reality (Important)

## 2.1 Launcher location today

Launcher orchestration is currently implemented inside CodeHelper backend:

1. `apps/codehelper/src-tauri/src/launcher/`
2. `apps/codehelper/src-tauri/src/commands/launcher.rs`
3. Manifest file: `apps/codehelper/src-tauri/resources/launcher/apps.manifest.json`

`launcher/` zone is still the extraction target, not yet the active runtime implementation.

## 2.2 LibreOffice app zone today

`apps/libreoffice-assistant` currently exists as a skeleton root only.  
No migrated frontend/backend code is present yet.

## 2.3 Root `src-tauri/` review (explicitly requested)

Observed at monorepo root:

1. `/src-tauri/gen/schemas/*`
2. `/src-tauri/target/*`
3. Approx size: `~2.8G`

This directory is generated artifact output, not a workspace member and not a valid app home in this monorepo layout.  
Do not port LibreOffice code into root `/src-tauri`.

Decision:

1. Treat root `/src-tauri` as local generated residue.
2. Port into `apps/libreoffice-assistant/...` only.
3. Keep build/test commands scoped to workspace paths.

## 3. Target End State

After migration, architecture should be:

1. `apps/libreoffice-assistant/` contains LibreOffice app frontend + `src-tauri`.
2. App backend uses `smolpc-engine-client` for engine lifecycle and inference calls.
3. MCP/LibreOffice tool bridge remains app-owned logic (Python sidecar + tool orchestration), not engine-owned.
4. Launcher manifest contains a `libreoffice-assistant` entry with `min_engine_api_major: 1`.
5. LibreOffice app can be launched/focused by `launcher_launch_or_focus`.

## 4. Migration Strategy (Phased)

## Phase 0: Preflight and Hygiene

1. Confirm branch and local-only workflow.
2. Keep root `/src-tauri` out of migration path.
3. Confirm baseline compile state before introducing new app:
   1. `cargo check --workspace`
   2. `npm run check`
4. Capture legacy app baseline from `/Users/mts/smolpc/smolpc-libreoffice/tauri-app`.

## Phase 1: Scaffold `apps/libreoffice-assistant`

Create app as first-class monorepo app (not loose root artifacts):

1. Add frontend app files under `apps/libreoffice-assistant/src`.
2. Add backend under `apps/libreoffice-assistant/src-tauri`.
3. Add app package metadata (`package.json`, Vite/Svelte config if used).
4. Add Rust crate to workspace in root `Cargo.toml`.
5. Add npm workspace entry in root `package.json` when frontend build is ready.

Recommended structure:

```text
apps/libreoffice-assistant/
  README.md
  package.json
  src/
  src-tauri/
    Cargo.toml
    tauri.conf.json
    src/
      commands/
      services/
      models/
    resources/
      mcp_server/
```

## Phase 2: Backend Port (Engine-Only)

Port logic from legacy `tauri-app/src-tauri` with engine-only contract:

1. Add `smolpc-engine-client` dependency to LibreOffice app crate.
2. Implement `InferenceState`-style client resolver (reuse CodeHelper pattern):
   1. `connect_or_spawn(...)`
   2. shared runtime dir/token handling
   3. startup mode/runtime preference handling if needed
3. Remove provider abstraction (`ollama` vs engine) from LibreOffice app API.
4. Replace provider-specific commands with engine-only commands.

Minimum command surface for LibreOffice app:

1. `engine_ensure_started` (or equivalent startup gate)
2. `engine_status` (readiness + diagnostics)
3. `list_models`
4. streaming chat/generation path (tool-calling compatible)
5. `cancel` passthrough for active generation

Engine contract behaviors to support:

1. 429 queue full handling
2. 504 queue timeout handling
3. stream error payload handling (`INFERENCE_GENERATION_CANCELLED`, `ENGINE_STREAM_ERROR`)
4. metrics event support (`chat.completion.metrics` / `smolpc_metrics`)

## Phase 3: MCP Bridge Hardening During Port

Legacy LibreOffice MCP path needs hardening while porting:

1. Make MCP `start` idempotent (do not respawn if already running).
2. On MCP init failure, stop/cleanup spawned process.
3. Correlate JSON-RPC response `id` to request `id`.
4. Avoid unbounded per-request reader-thread leak risk.
5. Use configured `python_path` consistently instead of ignoring it.
6. Keep a reliable shutdown path in `Drop` and explicit stop command.

These fixes should land as part of port, not deferred.

## Phase 4: Frontend Port (Engine-Only UX)

Frontend rules:

1. Remove provider selector and Ollama URLs from settings UI.
2. Default config should be engine-first (shared engine URL/model baseline).
3. Do not block user behind non-recoverable loading screen.
   1. Settings/retry path must remain reachable when startup checks fail.
4. Keep tool-call UX, but align with engine stream semantics.
5. Display engine diagnostics fields from status:
   1. `active_backend`
   2. `selection_reason`
   3. `selected_device_name`
   4. `state` / `error_code` / `retryable`

## Phase 5: Launcher Integration

Add LibreOffice app entry to launcher manifest currently used by runtime:

1. File: `apps/codehelper/src-tauri/resources/launcher/apps.manifest.json`
2. Add new app object:
   1. `app_id`: `libreoffice-assistant`
   2. `display_name`: user-facing app name
   3. `exe_path`: absolute install path
   4. `args`: optional launch args
   5. `focus_command`: optional command for focusing existing app window
   6. `min_engine_api_major`: `1`

Example manifest entry:

```json
{
  "app_id": "libreoffice-assistant",
  "display_name": "SmolPC LibreOffice Assistant",
  "exe_path": "C:\\Program Files\\SmolPC\\LibreOffice Assistant\\SmolPC LibreOffice Assistant.exe",
  "args": [],
  "focus_command": null,
  "min_engine_api_major": 1
}
```

Development note:

1. Launcher supports `SMOLPC_LAUNCHER_MANIFEST` override.
2. Use a dev manifest with local build output paths to test launch/focus before installer packaging.

## Phase 6: Packaging and Resources

For LibreOffice app packaging:

1. Bundle app-owned MCP resources in its `src-tauri/tauri.conf.json`.
2. Keep engine host ownership in shared engine flow (do not embed app-local inference runtime).
3. Ensure resource resolution works in both dev and bundled modes.
4. Keep Python/MCP startup scripts in one canonical location per app package to avoid duplicate drift.

## Phase 7: Verification Matrix (Definition of Done)

Port is complete only when all checks pass.

Engine onboarding checks:

1. `GET /engine/health` success.
2. `GET /engine/meta` protocol major compatible.
3. `POST /engine/load` success for default model.
4. non-stream generation success with metrics.
5. stream generation success with token chunks + metrics + `[DONE]`.
6. cancel path works (`/engine/cancel`).
7. app handles 429 and 504 correctly.

LibreOffice app checks:

1. MCP server starts once and is reusable.
2. MCP tool list loads and tool calls execute correctly.
3. Document create/read/edit tool chain works end-to-end.
4. App remains recoverable when dependencies are missing.

Launcher checks:

1. `launcher_list_apps` returns LibreOffice entry.
2. `launcher_launch_or_focus("libreoffice-assistant")` launches app when not running.
3. Same call focuses existing app when already running.
4. API major gate blocks launch on incompatible engine API version.

CI/local commands (minimum):

1. `cargo check --workspace`
2. `cargo test --workspace`
3. `npm run check`
4. `npm run boundary:check`

## 5. Legacy-to-Target Mapping

Map from legacy repo (`/Users/mts/smolpc/smolpc-libreoffice/tauri-app`) to monorepo target:

| Legacy area | Target area | Migration action |
| --- | --- | --- |
| `src-tauri/src/commands/ai.rs` | `apps/libreoffice-assistant/src-tauri/src/commands/ai.rs` | Keep chat orchestration, remove provider branching, make engine-only |
| `src-tauri/src/services/smolpc_engine_service.rs` | `apps/libreoffice-assistant/src-tauri/src/services/` | Prefer `smolpc-engine-client`; only keep custom parser logic if truly required |
| `src-tauri/src/services/mcp_client.rs` | `apps/libreoffice-assistant/src-tauri/src/services/mcp_client.rs` | Port with lifecycle/id-correlation fixes |
| `src-tauri/src/commands/system.rs` | `apps/libreoffice-assistant/src-tauri/src/commands/system.rs` | Remove Ollama checks and legacy messages |
| `src/lib/components/SettingsPage.svelte` | `apps/libreoffice-assistant/src/lib/components/` | Remove provider selector, expose engine-only settings + diagnostics |
| `resources/mcp_server/*` | `apps/libreoffice-assistant/src-tauri/resources/mcp_server/*` | Keep as app-owned resources, unify start scripts |

## 6. Recommended PR Slices

Use small, reviewable slices:

1. PR-1 scaffolding (`apps/libreoffice-assistant` structure + workspace wiring).
2. PR-2 backend engine-only port (no frontend changes yet).
3. PR-3 MCP lifecycle hardening.
4. PR-4 frontend migration to engine-only UX.
5. PR-5 launcher manifest integration + launch/focus tests.
6. PR-6 docs and final cleanup of legacy/Ollama leftovers.

## 7. Explicit No-Ollama Rule for This Port

For LibreOffice app migration, treat all Ollama code as legacy and remove/avoid it in target implementation:

1. No Ollama provider enum branch.
2. No Ollama health check in dependency gate.
3. No Ollama URL/model settings fields in final UI/config.
4. No Ollama-specific command modules in final LibreOffice app crate.

If temporary compatibility code is needed during transition, keep it behind short-lived migration commits and remove before merge to main.
