# Current State

**Last Updated:** 2026-03-16
**Phase:** Documentation baseline refreshed; implementation not started

## Branch Roles

| Branch | Role |
|---|---|
| `docs/unified-assistant-spec` | Canonical architecture/spec branch |
| `dev/unified-assistant` | Implementation mainline after docs merge |
| `codex/unified-spec-refresh` | Documentation refresh working branch |

## What Is Done

The documentation baseline for the unified frontend is now defined around:

- one unified Tauri app
- Code as a first-class in-app mode
- six modes total: Code, GIMP, Blender, Writer, Calc, Slides
- one shared `smolpc-engine-host`
- adapters-first migration
- strict merge-safe boundaries

The standalone apps remain source references during the future port:

- `apps/gimp-assistant`
- `apps/blender-assistant`
- `apps/libreoffice-assistant`

## What Has Not Started

Implementation has **not** started yet.

Specifically not started:

- unified shell refactor in `apps/codehelper`
- shared provider interfaces
- mode provider ports
- launcher cleanup
- packaging changes
- Windows end-to-end validation for the unified app

## Next Workstreams After Docs Merge

These begin only after the documentation baseline is merged into
`dev/unified-assistant`.

1. Foundation
   - shared DTOs
   - provider interface
   - mode registry
2. Unified shell
   - mode dropdown
   - per-mode histories
   - shared status model
3. Code mode integration
4. GIMP provider port
5. Blender provider port
6. LibreOffice provider port
7. Hardening and Windows packaging validation

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

- the refreshed docs are merged into `docs/unified-assistant-spec`
- those docs are merged into `dev/unified-assistant`
- contributors can begin implementation without re-deciding the product shape
