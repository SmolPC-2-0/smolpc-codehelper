# Git Workflow For The Unified Assistant

**Last Updated:** 2026-03-16
**Status:** Required workflow for docs and implementation branches

## 1. Branch Roles

| Branch | Role |
|---|---|
| `docs/unified-assistant-spec` | Canonical spec branch |
| `dev/unified-assistant` | Implementation integration mainline |
| `codex/*` | Narrow work branches |

## 2. Docs-First Rule

The unified frontend always follows this sequence:

1. update the design on `docs/unified-assistant-spec`
2. merge the refreshed docs into `docs/unified-assistant-spec`
3. merge `docs/unified-assistant-spec` into `dev/unified-assistant`
4. branch implementation work from `dev/unified-assistant`

Do not start feature branches for the unified app before the relevant docs are
merged into `dev/unified-assistant`.

## 3. Working Branch For The Documentation Refresh

For this documentation phase use:

- `codex/unified-spec-refresh` from `docs/unified-assistant-spec`

## 4. Implementation Branch Layout

All implementation branches are created from `dev/unified-assistant`.

Recommended branch set:

| Branch | Scope |
|---|---|
| `codex/unified-foundation` | shared DTOs, provider interface, registry, stream contracts |
| `codex/unified-shell` | shared shell, mode dropdown, history filtering |
| `codex/unified-code-mode` | preserve current Codehelper behavior as Code mode |
| `codex/unified-gimp-mode` | GIMP provider port |
| `codex/unified-blender-mode` | Blender provider port |
| `codex/unified-libreoffice-mode` | Writer/Calc/Slides provider port |
| `codex/unified-hardening` | packaging, Windows validation, cleanup |

## 5. Branch Boundaries

### Foundation

Allowed changes:

- new shared crates
- new DTOs
- provider registry
- new command contracts

Avoid:

- major UI work
- mode-specific provider implementation

### Shell

Allowed changes:

- `apps/codehelper` shared UI shell
- mode store
- per-mode histories

Avoid:

- porting provider logic from standalone apps

### Mode branches

Each mode branch should touch only:

- the mode's new adapter files inside the unified app
- the smallest necessary shell integration points

Avoid:

- editing the standalone app unless a source bug must be fixed upstream first

## 6. Adapters-First Rule

The unified app ports behavior from standalone apps into **new unified adapter
files**. It does not merge standalone app directories into `apps/codehelper`.

This is the primary merge-safety rule.

## 7. Engine Boundary Rule

Unified branches should consume stable engine contracts where possible.

If a unified feature needs an engine contract change:

1. land the engine change separately
2. merge or rebase that change into `dev/unified-assistant`
3. consume the new contract from unified branches

Do not hide engine API redesign inside a UI branch.

## 8. Sync Rule For Standalone Apps

If new work lands in:

- `apps/gimp-assistant`
- `apps/blender-assistant`
- `apps/libreoffice-assistant`

then the unified work should:

1. identify the accepted source behavior
2. port it into the unified adapter
3. avoid taking ownership of the standalone directory from the unified branch

## 9. PR Targets

| Branch type | PR target |
|---|---|
| doc refresh | `docs/unified-assistant-spec` |
| implementation work | `dev/unified-assistant` |
| engine contract work | engine-focused branch / target as appropriate |

## 10. Merge-Safety Checklist

Before merging a unified implementation branch, check:

- branch was created from `dev/unified-assistant`
- no unnecessary edits landed in standalone app directories
- no hidden engine API redesign landed in the branch
- docs still match the implemented surface
- branch scope matches its intended boundary

## 11. Default Rule When Unsure

If unsure where a change belongs:

- spec/design change -> `docs/unified-assistant-spec`
- shared implementation contract -> `dev/unified-assistant` child branch
- standalone app source fix -> standalone app branch first
- engine contract change -> separate engine work first
