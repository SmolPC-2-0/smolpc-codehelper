# Current State

**Last Updated:** 2026-03-16
**Phase:** Phase 2 shell preflight is locked on `codex/unified-shell-docs`; shell implementation is next

## Branch Roles

| Branch | Role |
|---|---|
| `docs/unified-assistant-spec` | Canonical architecture/spec branch |
| `dev/unified-assistant` | Implementation mainline after docs merge |
| `codex/unified-foundation` | Merged Phase 1 implementation branch |
| `codex/unified-foundation-status-docs` | Phase 1 closeout docs branch |
| `codex/unified-shell-docs` | Active Phase 2 preflight docs branch |
| `codex/unified-shell` | Next implementation branch after docs merge |

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

The standalone apps remain source references during the future port:

- `apps/gimp-assistant`
- `apps/blender-assistant`
- `apps/libreoffice-assistant`

## What Has Not Started

- unified shell refactor in `apps/codehelper`
- real provider integrations for Code, GIMP, Blender, or LibreOffice
- mode provider ports
- launcher cleanup beyond the foundation test fix
- unified-app packaging hardening beyond the tracked OpenVINO placeholder
- Windows end-to-end validation for the unified app

## Next Workstreams

The next official step after these docs merge is `codex/unified-shell`:

1. merge `codex/unified-shell-docs` into `docs/unified-assistant-spec`
2. merge `docs/unified-assistant-spec` into `dev/unified-assistant`
3. create `codex/unified-shell`
4. implement Unified Shell:
   - mode dropdown
   - per-mode histories
   - shared status model
5. close out Phase 2 with `codex/unified-shell-status-docs`
6. continue serial merge order:
   - Code mode integration
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
4. Port behavior into new unified adapters rather than merging standalone app
   directories.
5. Treat engine contract changes as separate work when possible.

## Current Success Condition

The current preflight step is complete only when:

- Phase 2 shell preflight docs are merged into `docs/unified-assistant-spec`
- those docs are merged into `dev/unified-assistant`
- the team can branch `codex/unified-shell` without reinterpreting shell behavior
