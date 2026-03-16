# Current State

**Last Updated:** 2026-03-16
**Phase:** Phase 3 Code mode is merged; Phase 4 GIMP preflight is next

## Branch Roles

| Branch | Role |
|---|---|
| `docs/unified-assistant-spec` | Canonical architecture/spec branch |
| `dev/unified-assistant` | Implementation mainline after docs merge |
| `codex/unified-foundation` | Merged Phase 1 implementation branch |
| `codex/unified-foundation-status-docs` | Phase 1 closeout docs branch |
| `codex/unified-shell-docs` | Merged Phase 2 preflight docs branch |
| `codex/unified-shell` | Merged Phase 2 shell implementation branch |
| `codex/unified-shell-status-docs` | Merged Phase 2 closeout docs branch |
| `codex/unified-shell-followups` | Merged post-Phase-2 shell hardening branch |
| `codex/unified-shell-followups-status-docs` | Merged shell follow-up docs sync branch |
| `codex/unified-code-mode-docs` | Merged Phase 3 preflight docs branch |
| `codex/unified-code-mode` | Merged Phase 3 implementation branch |
| `codex/unified-code-mode-status-docs` | Phase 3 closeout docs branch |
| `codex/unified-gimp-mode-docs` | Next Phase 4 preflight docs branch |

## What Is Done

The documentation baseline for the unified frontend is now defined around:

- one unified Tauri app
- Code as a first-class in-app mode
- six modes total: Code, GIMP, Blender, Writer, Calc, Slides
- one shared `smolpc-engine-host`
- adapters-first migration
- strict merge-safe boundaries

Phase 1 foundation is now merged into `dev/unified-assistant` via PR `#63`.

Merged foundation capabilities now present in `dev/unified-assistant`:

- shared Rust contract crate: `crates/smolpc-assistant-types`
- shared MCP scaffolding crate: `crates/smolpc-mcp-client`
- backend mode/provider skeleton in `apps/codehelper/src-tauri`
- unified Tauri command scaffolding:
  - `list_modes`
  - `mode_status`
  - `mode_refresh_tools`
  - `assistant_send`
  - `assistant_cancel`
  - `mode_undo`
- frontend contract mirrors and typed invoke wrappers in `apps/codehelper/src/lib`
- async MCP client contract in the shared transport crate
- mode-aware shared-provider status/tool interfaces at the provider boundary
- tracked `apps/codehelper/src-tauri/libs/openvino/README.md` placeholder for clean-checkout Tauri builds
- clean frontend audit lockfile with `undici` resolved out of the vulnerable range

Validation completed for the merged foundation:

- `cargo test -p smolpc-assistant-types`
- `cargo test -p smolpc-mcp-client`
- `cargo check -p smolpc-assistant-types`
- `cargo check -p smolpc-mcp-client`
- `cargo check -p smolpc-code-helper`
- `cargo test -p smolpc-code-helper --lib`
- `npm run check --workspace apps/codehelper`
- `npm audit --workspace apps/codehelper --omit=dev --audit-level=high`
- PR checks green, including `Frontend Quality` and `Tauri Build Check`

Phase 2 shell decisions are now locked:

- fresh unified chat storage with no migration from `smolpc_chats`
- per-mode current chat tracking
- `code` as the default active mode
- header-level `AppModeDropdown`
- non-Code modes visible but composer-disabled
- Code-only send/generate/export/benchmark behavior during the shell phase
- no backend contract changes for Phase 2

Phase 2 shell is now merged into `dev/unified-assistant` via PR `#64`.

Merged shell capabilities now present in `dev/unified-assistant`:

- header-level `AppModeDropdown`
- six visible modes in one `apps/codehelper` shell
- fresh unified storage keys:
  - `smolpc_unified_chats_v1`
  - `smolpc_unified_current_chat_by_mode_v1`
  - `smolpc_unified_active_mode_v1`
- per-mode current chat tracking and history filtering
- one auto-created Code chat only on first empty boot
- lazy mode-status loading through the Phase 1 Tauri commands
- non-Code placeholder modes with disabled composer and visible prompt starters
- existing Codehelper send/generate path preserved only in Code mode
- Code-only export, benchmark, and context controls during the shell phase
- root frontend style-gate support for workspace `.svelte` and `.ts` files

Validation completed for the merged shell:

- `npm run check --workspace apps/codehelper`
- `cargo check -p smolpc-code-helper`
- `cargo test -p smolpc-code-helper --lib`
- root incremental Prettier checks against the net PR diff
- root incremental ESLint checks against the net PR diff
- PR checks green, including `Incremental Style Gates` and `Tauri Build Check`

Post-Phase-2 shell hardening is now merged into `dev/unified-assistant` via PR `#65`.

Merged shell follow-up behavior now present in `dev/unified-assistant`:

- top-level shell init is explicitly caught rather than left fire-and-forget
- local fallback mode configs keep the mode selector usable if `list_modes` fails or returns no modes
- shell warning/error state is surfaced in the header instead of being silently hidden
- benchmark-overlay cleanup is centralized in one reactive capability gate
- the prompt-starters close button uses a stable icon again

Validation completed for the merged shell follow-up:

- `npm run check --workspace apps/codehelper`
- `cargo check -p smolpc-code-helper`
- `cargo test -p smolpc-code-helper --lib`
- root incremental Prettier checks against the changed frontend files
- root incremental ESLint checks against the changed frontend files
- PR checks green

Phase 3 Code-mode polish is now merged into `dev/unified-assistant` via PR `#66`.

Merged Code-mode behavior now present in `dev/unified-assistant`:

- active Code-mode shell status now prefers live `inferenceStore.status` state
  over scaffold provider copy
- Code-mode header, welcome state, and fallback metadata now use Codehelper-
  specific wording rather than generic shell copy
- backend mode config defaults and frontend fallback mode configs are aligned on
  the same Code subtitle and prompt starters
- mode switching during Code generation remains allowed without forcing
  cancellation or losing the originating Code chat
- `assistant_send` remains scaffold-only after Phase 3; Code mode still uses the
  existing Codehelper inference path

Validation completed for the merged Code-mode polish:

- `npm run check --workspace apps/codehelper`
- `cargo check -p smolpc-code-helper`
- `cargo test -p smolpc-code-helper --lib`
- root incremental Prettier checks against the changed frontend files
- root incremental ESLint checks against the changed frontend files
- PR checks green, including `Frontend Quality` and `Tauri Build Check`

The standalone apps remain source references during the future port:

- `apps/gimp-assistant`
- `apps/blender-assistant`
- `apps/libreoffice-assistant`

## What Has Not Started

- real provider integrations for Code, GIMP, Blender, or LibreOffice
- mode provider ports
- launcher cleanup beyond the foundation test fix
- unified-app packaging hardening beyond the tracked OpenVINO placeholder
- Windows end-to-end validation for the unified app

## Next Workstreams

The next official step after these docs merge is `codex/unified-gimp-mode-docs`:

1. create `codex/unified-gimp-mode-docs`
2. merge `docs/unified-assistant-spec` into `dev/unified-assistant`
3. lock GIMP as the first real external-provider mode
4. lock `assistant_send` activation, undo behavior, and streamed execution
   status in docs first
5. create `codex/unified-gimp-mode`
6. continue serial merge order:
   - GIMP provider port
   - Blender provider port
   - LibreOffice provider port
   - Hardening and Windows packaging validation

## Known Risks

| Risk | Impact |
|---|---|
| Engine branch churn | unified app may need contract updates while the engine is still evolving |
| Standalone app branch churn | GIMP, Blender, and LibreOffice behaviors may continue changing during the port |
| Packaging/runtime validation | third-party runtime paths may behave differently in packaged Windows builds |
| LibreOffice port alignment | the LibreOffice branch must stay aligned with the unified provider design |

## Merge-Safe Rules

1. Do docs work first.
2. Merge docs into `dev/unified-assistant`.
3. Create implementation branches from `dev/unified-assistant` only.
4. Port behavior into new unified adapters rather than merging standalone app directories.
5. Treat engine contract changes as separate work when possible.

## Current Success Condition

The current closeout step is complete only when:

- Phase 3 closeout docs are merged into `docs/unified-assistant-spec`
- those docs are merged into `dev/unified-assistant`
- the team can branch `codex/unified-gimp-mode-docs` without reopening Phase 3
  scope
