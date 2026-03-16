# Unified Assistant Implementation Phases

**Last Updated:** 2026-03-16
**Status:** Phase 2 shell and shell follow-up are merged; Phase 3 Code-mode docs preflight is next

## Phase 0: Documentation Baseline

**Branch flow**

1. `codex/unified-spec-refresh`
2. merge into `docs/unified-assistant-spec`
3. merge `docs/unified-assistant-spec` into `dev/unified-assistant`

**Exit criteria**

- all unified docs are internally consistent
- `dev/unified-assistant` contains the refreshed docs
- implementation can begin without re-deciding architecture

## Phase 1: Foundation

**Suggested branches:** `codex/unified-foundation-docs` as needed, then `codex/unified-foundation`

**Scope**

- shared DTOs
- shared crates:
  - `smolpc-assistant-types`
  - `smolpc-mcp-client`
- provider interfaces
- mode registry
- stream event contracts
- minimal command scaffolding
- frontend contract mirrors and typed invoke wrappers

**Exit criteria**

- backend contracts exist for all six modes
- frontend mirrors exist for the new unified commands
- no standalone app behavior has been ported yet

**Current branch status**

- implemented and merged via PR `#63`
- remote implementation baseline now lives on `dev/unified-assistant`
- contract follow-ups landed:
  - async MCP client interface
  - mode-aware shared-provider contract
  - tracked OpenVINO placeholder directory
  - clean frontend audit lockfile

## Branch Order After Phase 2

1. `codex/unified-shell-followups-status-docs`
2. merge into `docs/unified-assistant-spec`
3. merge `docs/unified-assistant-spec` into `dev/unified-assistant`
4. `codex/unified-code-mode-docs`
5. merge into `docs/unified-assistant-spec`
6. merge `docs/unified-assistant-spec` into `dev/unified-assistant`
7. `codex/unified-code-mode`
8. closeout docs
9. `codex/unified-gimp-mode-docs`
10. `codex/unified-gimp-mode`
11. `codex/unified-blender-mode-docs`
12. `codex/unified-blender-mode`
13. `codex/unified-libreoffice-mode-docs`
14. `codex/unified-libreoffice-mode`
15. `codex/unified-hardening-docs`
16. `codex/unified-hardening`

## Phase 2: Unified Shell

**Suggested branches:** `codex/unified-shell-docs`, `codex/unified-shell`, `codex/unified-shell-status-docs`, then `codex/unified-shell-followups-status-docs` as needed

**Scope**

- docs-first preflight for shell store shape, storage versioning, and placeholder mode behavior
- mode dropdown
- per-mode histories
- shared shell capability flags
- mode-aware status model

**Preflight decisions**

- fresh unified storage keys:
  - `smolpc_unified_chats_v1`
  - `smolpc_unified_current_chat_by_mode_v1`
  - `smolpc_unified_active_mode_v1`
- no migration from old standalone chat keys
- per-mode current chat tracking
- `code` default active mode
- non-Code modes visible with disabled composer
- Code-only send/generate/export/benchmark behavior
- no backend contract changes in Phase 2

**Exit criteria**

- `apps/codehelper` can present all six modes in one shell
- Code mode remains the default and closest to current behavior
- non-Code modes are honest placeholders rather than fake chat integrations

**Current branch status**

- preflight docs merged into `docs/unified-assistant-spec`, then into `dev/unified-assistant`
- implementation merged via PR `#64`
- shell hardening follow-up merged via PR `#65`
- merged shell capabilities:
  - `AppModeDropdown` in the header
  - per-mode unified chat/history state
  - fresh unified storage keys with no standalone-chat migration
  - lazy mode-status loading and caching
  - visible non-Code placeholder modes with disabled composer
  - existing Codehelper send/generate path preserved only for Code
  - Code-only export, benchmark, and context controls in Phase 2
- merged shell follow-up behaviors:
  - explicit top-level init error handling
  - fallback mode configs if `list_modes` fails
  - surfaced shell warning/error state in the header
  - centralized benchmark-overlay capability cleanup
- CI unblock follow-ups landed with the shell:
  - root `.prettierrc` Svelte override
  - root `eslint.config.mjs`
  - tracked `apps/codehelper/.gitignore`

## Phase 3: Code Mode

**Suggested branches:** `codex/unified-code-mode-docs`, then `codex/unified-code-mode`

**Scope**

- preserve current Codehelper experience inside the unified shell
- polish Code mode to feel fully intentional inside the unified shell
- keep the current inference path active rather than activating `assistant_send`

**Phase 3 preflight decisions**

- parity polish, not orchestration activation
- no backend contract changes
- Code-only send/regenerate/continue/branch/export behavior remains unchanged
- Code-mode visible status should prefer real `inferenceStore.status` over scaffold provider copy
- mode switching during Code generation remains allowed
- Code-mode shell copy should feel like current Codehelper, not a generic placeholder

**Exit criteria**

- Code mode is feature-equivalent enough to serve as the baseline mode in the unified app

## Phase 4: GIMP Mode

**Suggested branches:** `codex/unified-gimp-mode-docs`, then `codex/unified-gimp-mode`

**Scope**

- port GIMP behavior into a GIMP provider
- support tool execution and undo

**Exit criteria**

- GIMP mode can connect, execute a validated action, and undo it

## Phase 5: Blender Mode

**Suggested branches:** `codex/unified-blender-mode-docs`, then `codex/unified-blender-mode`

**Scope**

- port Blender bridge behavior into a Blender provider
- preserve bridge-first workflows

**Exit criteria**

- Blender mode can complete a bridge-backed scene workflow in the unified app

## Phase 6: LibreOffice Modes

**Suggested branches:** `codex/unified-libreoffice-mode-docs`, then `codex/unified-libreoffice-mode`

**Scope**

- port from the LibreOffice source branch into one shared provider
- expose Writer, Calc, and Slides as separate frontend modes

**Exit criteria**

- Writer, Calc, and Slides share one provider runtime and feel like distinct frontend modes

## Phase 7: Hardening And Packaging

**Suggested branches:** `codex/unified-hardening-docs`, then `codex/unified-hardening`

**Scope**

- remove remaining launcher assumptions
- validate packaged resource paths
- Windows-only end-to-end verification

**Exit criteria**

- unified app is Windows-valid without launcher runtime ownership

## Merge-Safety Constraints

These apply to every implementation phase:

1. no hidden engine redesign in UI branches
2. no takeover of standalone app directories
3. port behavior into new unified adapters
4. keep branch scope narrow

## Windows Validation Milestones

| Milestone | Required proof |
|---|---|
| Foundation ready | commands and DTOs compile cleanly |
| Shell ready | six modes visible in one shell |
| Code ready | existing Codehelper experience preserved |
| GIMP ready | tool call and undo work |
| Blender ready | bridge-backed workflow works |
| LibreOffice ready | Writer/Calc/Slides all work via one provider |
| Packaging ready | packaged app runs without launcher ownership |
