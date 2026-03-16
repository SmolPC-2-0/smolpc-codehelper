# Unified Assistant Implementation Phases

**Last Updated:** 2026-03-16
**Status:** Post-docs execution roadmap

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

**Suggested branch:** `codex/unified-foundation`

**Scope**

- shared DTOs
- provider interfaces
- mode registry
- stream event contracts
- minimal command scaffolding

**Exit criteria**

- backend contracts exist for all six modes
- no standalone app behavior has been ported yet

## Phase 2: Unified Shell

**Suggested branch:** `codex/unified-shell`

**Scope**

- mode dropdown
- per-mode histories
- shared shell capability flags
- mode-aware status model

**Exit criteria**

- `apps/codehelper` can present all six modes in one shell
- Code mode remains the default and closest to current behavior

## Phase 3: Code Mode

**Suggested branch:** `codex/unified-code-mode`

**Scope**

- preserve current Codehelper experience inside the unified shell
- wire Code mode through the new provider/orchestrator contracts

**Exit criteria**

- Code mode is feature-equivalent enough to serve as the baseline mode in the
  unified app

## Phase 4: GIMP Mode

**Suggested branch:** `codex/unified-gimp-mode`

**Scope**

- port GIMP behavior into a GIMP provider
- support tool execution and undo

**Exit criteria**

- GIMP mode can connect, execute a validated action, and undo it

## Phase 5: Blender Mode

**Suggested branch:** `codex/unified-blender-mode`

**Scope**

- port Blender bridge behavior into a Blender provider
- preserve bridge-first workflows

**Exit criteria**

- Blender mode can complete a bridge-backed scene workflow in the unified app

## Phase 6: LibreOffice Modes

**Suggested branch:** `codex/unified-libreoffice-mode`

**Scope**

- port from the LibreOffice source branch into one shared provider
- expose Writer, Calc, and Slides as separate frontend modes

**Exit criteria**

- Writer, Calc, and Slides share one provider runtime and feel like distinct
  frontend modes

## Phase 7: Hardening And Packaging

**Suggested branch:** `codex/unified-hardening`

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
