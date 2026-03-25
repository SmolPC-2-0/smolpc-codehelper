# LibreOffice Assistant Porting Guide (Unified Launcher + Shared Engine)

Status: Active migration plan (launcher-aware, merge-safe)  
Updated: 2026-03-12  
Scope: Port `/Users/mts/smolpc/smolpc-libreoffice` into this monorepo under `apps/libreoffice-assistant`, with launcher integration staged to avoid rework while launcher PRs are in flight.

## 1. Decision Summary

Proceed with LibreOffice migration now. Do not wait for launcher PR merge.

Execution model:

1. Track A (start now): launcher-agnostic app migration work.
2. Track B (after launcher branch settles): final launcher binding and manifest wiring.

Reason:

1. Most LibreOffice migration work is independent of launcher code location.
2. Blocking would delay high-value engine/MCP hardening unnecessarily.
3. Launcher-specific changes can be isolated to one late integration phase.

## 2. Contract and Boundary Rules

Hard constraints for this port:

1. Inference must use shared engine contract only:
   1. Preferred for Rust/Tauri: `smolpc-engine-client`.
   2. Fallback only when needed: documented `/engine/*` and `/v1/*` HTTP endpoints.
2. No app-local inference runtime ownership.
3. No Ollama provider/runtime path in final LibreOffice app.
4. LibreOffice tool/MCP bridge remains app-owned logic.

Reference docs:

1. `docs/APP_ONBOARDING_PLAYBOOK.md`
2. `docs/ENGINE_API.md`
3. `docs/SMOLPC_SUITE_INTEGRATION.md`
4. `docs/ARCHITECTURE.md`

## 3. Repository Snapshot (As of 2026-03-12)

## 3.1 Stable on `main`

1. Launcher orchestration commands exist under CodeHelper backend:
   1. `app/src-tauri/src/launcher/`
   2. `app/src-tauri/src/commands/launcher.rs`
2. Launcher manifest currently lives at:
   1. `app/src-tauri/resources/launcher/apps.manifest.json`
3. `apps/libreoffice-assistant` remains a staging root.

## 3.2 In-flight launcher work

PR `#53` (`review/launcher-exe-installer-work`) introduces a standalone `launcher/` app workspace and Blender onboarding, but is expected to be reworked before merge.

Planning implication:

1. Do not bind LibreOffice migration to the exact current PR #53 file set.
2. Do design for eventual standalone launcher ownership.
3. Keep launcher-specific work isolated so rebasing is cheap.

## 3.3 Root `src-tauri` residue

Root `/src-tauri` is generated artifact residue (`target`, `gen/schemas`) and is not an app zone.

Rules:

1. Do not place LibreOffice source in root `/src-tauri`.
2. Keep LibreOffice source under `apps/libreoffice-assistant/...` only.

## 4. Two-Track Migration Plan

## Track A: Start Now (Launcher-Agnostic)

These tasks can be implemented immediately with very low launcher rework risk:

1. App scaffolding under `apps/libreoffice-assistant`.
2. Workspace wiring:
   1. add Rust crate member in root `Cargo.toml`
   2. add npm workspace entry in root `package.json` when frontend is present
3. Backend engine-only migration:
   1. `smolpc-engine-client` integration
   2. startup/status/load/generate/cancel flows
4. MCP lifecycle hardening:
   1. idempotent start
   2. cleanup on init failure
   3. JSON-RPC id correlation
   4. timeout/thread safety
5. Frontend migration to engine-only UX and recoverable startup states.
6. Tests and evidence for onboarding checklist items not dependent on launcher manifest location.

## Track B: Defer Until Launcher Settles

Do after launcher branch merges or launcher shape is finalized:

1. Final manifest entry for LibreOffice app.
2. Launch/focus command wiring for dev and packaged modes.
3. End-to-end launcher UI verification for LibreOffice card and launch behavior.
4. Any launcher-path-specific docs that depend on final merge structure.

## 5. Launcher Integration Matrix (Use This To Avoid Rework)

Until launcher merge settles, treat manifest path as an environment-dependent target:

1. Current `main` runtime path:
   1. `app/src-tauri/resources/launcher/apps.manifest.json`
2. Candidate post-merge path (if standalone launcher lands):
   1. `launcher/src-tauri/resources/launcher/apps.manifest.json`

Implementation rule:

1. Keep LibreOffice launcher payload shape stable.
2. Switch only file location and runtime owner when launcher finalizes.

Recommended manifest payload for LibreOffice:

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

If final launcher schema supports `launch_command`, include it for dev convenience, but keep packaged `exe_path` canonical.

## 6. Detailed Phases

## Phase 0: Preflight

1. Confirm local branch and local-only workflow.
2. Confirm baseline commands:
   1. `cargo check --workspace`
   2. `npm run check`
3. Snapshot legacy source:
   1. `/Users/mts/smolpc/smolpc-libreoffice/tauri-app`

## Phase 1: Scaffold `apps/libreoffice-assistant`

1. Add frontend shell under `apps/libreoffice-assistant/src`.
2. Add backend under `apps/libreoffice-assistant/src-tauri`.
3. Add app package config and Tauri config.
4. Wire workspace memberships.

Suggested layout:

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

## Phase 2: Engine-Only Backend Port

1. Use `smolpc-engine-client` for engine lifecycle and generation.
2. Remove provider branching (`ollama` vs engine).
3. Implement required operations:
   1. ensure started
   2. status
   3. list models
   4. stream generation
   5. cancel
4. Implement required operational handling:
   1. 429 queue full
   2. 504 queue timeout
   3. stream error codes (`INFERENCE_GENERATION_CANCELLED`, `ENGINE_STREAM_ERROR`)
   4. metrics event handling

## Phase 3: MCP Hardening (Must Land During Port)

1. Prevent duplicate MCP process spawn.
2. Stop process on initialization failure.
3. Validate response/request id matching.
4. Eliminate timeout-driven thread leak patterns.
5. Honor configured `python_path`.
6. Ensure deterministic shutdown behavior.

## Phase 4: Frontend Engine-Only UX

1. Remove Ollama settings and provider selector.
2. Default to shared engine settings/model baseline.
3. Keep settings/retry accessible even when startup checks fail.
4. Align stream parsing with contract SSE semantics.
5. Surface engine diagnostics:
   1. `active_backend`
   2. `selection_reason`
   3. `selected_device_name`
   4. readiness/error fields

## Phase 5: Launcher Binding (Track B)

1. Add LibreOffice manifest entry in finalized launcher owner path.
2. Validate launch/focus flow using `launcher_launch_or_focus`.
3. Validate API major compatibility gating.
4. Add launcher-specific test evidence.

## Phase 6: Packaging

1. Bundle app-owned MCP resources in LibreOffice `tauri.conf.json`.
2. Keep inference ownership in shared engine, not app.
3. Ensure resource resolution works in dev and packaged modes.

## 7. Verification Matrix

Port is complete only when all checks pass.

## 7.1 Engine and app checks (Track A)

1. `GET /engine/health` returns success.
2. `GET /engine/meta` returns protocol major compatible with app.
3. `POST /engine/load` succeeds for baseline model.
4. Non-stream generation succeeds and returns metrics.
5. Stream generation emits tokens, metrics event, and `[DONE]`.
6. Cancel flow succeeds.
7. App correctly handles 429 and 504.
8. MCP tool flow works end-to-end.

## 7.2 Launcher checks (Track B)

1. Launcher lists LibreOffice app entry.
2. Launch-or-focus launches when app is down.
3. Launch-or-focus focuses when app is already running.
4. Engine API major gate blocks incompatible launches.

## 7.3 Commands

1. `cargo check --workspace`
2. `cargo test --workspace`
3. `npm run check`
4. `npm run boundary:check`

## 8. Legacy-to-Target Mapping

Source legacy repo:

1. `/Users/mts/smolpc/smolpc-libreoffice/tauri-app`

Mapping:

| Legacy area | Target area | Migration action |
| --- | --- | --- |
| `src-tauri/src/commands/ai.rs` | `apps/libreoffice-assistant/src-tauri/src/commands/ai.rs` | Keep chat orchestration, remove provider branching, make engine-only |
| `src-tauri/src/services/smolpc_engine_service.rs` | `apps/libreoffice-assistant/src-tauri/src/services/` | Prefer `smolpc-engine-client`; retain custom parsing only if required |
| `src-tauri/src/services/mcp_client.rs` | `apps/libreoffice-assistant/src-tauri/src/services/mcp_client.rs` | Port with lifecycle and RPC-correlation fixes |
| `src-tauri/src/commands/system.rs` | `apps/libreoffice-assistant/src-tauri/src/commands/system.rs` | Remove Ollama checks/messages |
| `src/lib/components/SettingsPage.svelte` | `apps/libreoffice-assistant/src/lib/components/` | Convert to engine-only settings and diagnostics |
| `resources/mcp_server/*` | `apps/libreoffice-assistant/src-tauri/resources/mcp_server/*` | Keep app-owned; unify startup path |

## 9. Recommended PR Slices

1. PR-1 scaffolding + workspace wiring.
2. PR-2 engine-only backend port.
3. PR-3 MCP hardening.
4. PR-4 frontend engine-only migration.
5. PR-5 launcher binding in finalized launcher path.
6. PR-6 cleanup/docs/final validation evidence.

## 10. Explicit No-Ollama Rule

For LibreOffice final state:

1. No Ollama provider enum branch.
2. No Ollama health gate.
3. No Ollama URL/model settings in UI.
4. No Ollama command modules in LibreOffice backend.

Temporary migration shims are acceptable only in short-lived intermediate commits and must be removed before merge to `main`.
