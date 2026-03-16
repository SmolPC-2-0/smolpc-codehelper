# Current State

**Last Updated:** 2026-03-16
**Phase:** Foundation stabilization in progress before shell work begins

## Branch Roles

| Branch | Role |
|---|---|
| `docs/unified-assistant-spec` | Canonical architecture/spec branch |
| `dev/unified-assistant` | Implementation mainline after docs merge |
| `codex/unified-spec-refresh` | Documentation refresh working branch |
| `codex/unified-foundation` | Phase 1 foundation implementation branch |
| `codex/unified-foundation-docs` | Docs-first stabilization branch for foundation follow-up contracts |

## What Is Done

The documentation baseline for the unified frontend is now defined around:

- one unified Tauri app
- Code as a first-class in-app mode
- six modes total: Code, GIMP, Blender, Writer, Calc, Slides
- one shared `smolpc-engine-host`
- adapters-first migration
- strict merge-safe boundaries

Current stabilization items before foundation can merge:

- make the MCP transport client contract async before any real transport lands
- make shared-provider status and tool discovery mode-aware at the provider boundary
- commit a tracked OpenVINO placeholder directory so clean Tauri builds pass
- refresh the root npm lockfile so the workspace audit is green

The standalone apps remain source references during the future port:

- `apps/gimp-assistant`
- `apps/blender-assistant`
- `apps/libreoffice-assistant`

## What Has Not Started

- unified shell refactor in `apps/codehelper`
- real provider integrations for Code, GIMP, Blender, or LibreOffice
- mode provider ports
- launcher cleanup
- packaging changes
- Windows end-to-end validation for the unified app

## Next Workstreams After Foundation Stabilization

These begin only after `codex/unified-foundation` is green and merged into
`dev/unified-assistant`.

1. Unified shell
   - mode dropdown
   - per-mode histories
   - shared status model
2. Code mode integration
3. GIMP provider port
4. Blender provider port
5. LibreOffice provider port
6. Hardening and Windows packaging validation

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

## Success Condition For This Phase

This phase is complete only when:

- the foundation follow-up docs are merged into `docs/unified-assistant-spec`
- those docs are merged into `dev/unified-assistant`
- the foundation branch is green and ready to merge
