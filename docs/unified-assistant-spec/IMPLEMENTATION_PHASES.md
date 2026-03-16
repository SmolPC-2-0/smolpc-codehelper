# Unified Assistant Implementation Phases

**Last Updated:** 2026-03-16
**Status:** Phase 5 Blender mode is merged; Phase 6 LibreOffice preflight is next

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
- provider interfaces
- mode registry
- stream event contracts
- minimal command scaffolding

**Exit criteria**

- backend contracts exist for all six modes
- no standalone app behavior has been ported yet

**Current branch status**

- implemented and merged via PR `#63`
- remote implementation baseline now lives on `dev/unified-assistant`
- contract follow-ups landed:
  - async MCP client interface
  - mode-aware shared-provider contract
  - tracked OpenVINO placeholder directory
  - clean frontend audit lockfile

## Branch Order After Phase 5

1. `codex/unified-libreoffice-mode-docs`
2. merge into `docs/unified-assistant-spec`
3. merge `docs/unified-assistant-spec` into `dev/unified-assistant`
4. `codex/unified-libreoffice-mode`
5. closeout docs
6. `codex/unified-hardening-docs`
7. `codex/unified-hardening`

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
- `assistant_send` remains scaffold-only through Phase 3

**Exit criteria**

- Code mode is feature-equivalent enough to serve as the baseline mode in the
  unified app

**Current branch status**

- preflight docs merged into `docs/unified-assistant-spec`, then into
  `dev/unified-assistant`
- implementation merged via PR `#66`
- merged Code-mode polish now present in `dev/unified-assistant`:
  - active Code-mode shell status prefers live `inferenceStore.status`
  - Code-mode header, welcome state, and fallback metadata now use
    Codehelper-specific wording
  - backend mode config defaults and frontend fallback mode configs are aligned
  - mode switching during Code generation remains allowed
  - `assistant_send` remains scaffold-only and unused by active Code mode

## Phase 4: GIMP Mode

**Suggested branches:** `codex/unified-gimp-mode-docs`, then `codex/unified-gimp-mode`

**Scope**

- port GIMP behavior into a GIMP provider
- make GIMP the first real external-provider mode
- activate `assistant_send` for `gimp` only
- support tool execution and undo

**Phase 4 preflight decisions**

- GIMP is the first real external-provider mode
- Code mode stays on the existing Codehelper inference path
- Blender / Writer / Calc / Slides remain placeholder-only
- GIMP transport uses TCP MCP with default `127.0.0.1:10008`
- the shared `smolpc-mcp-client` crate owns the TCP transport additions
- GIMP Phase 4 targets parity with the current proven assistant surface rather
  than expanding beyond it
- GIMP uses deterministic fast paths first and constrained `call_api` fallback
  second
- GIMP undo stays clipboard-backed rather than depending on the native GIMP
  undo stack
- GIMP uses structured status / tool events and does not require token
  streaming in this phase
- GIMP and Code are the only live modes after Phase 4

**Exit criteria**

- GIMP mode can connect, execute a validated action, and undo it

**Current branch status**

- preflight docs merged into `docs/unified-assistant-spec`, then into
  `dev/unified-assistant`
- implementation merged via PR `#67`
- merged GIMP behavior now present in `dev/unified-assistant`:
  - `assistant_send` is operational for `gimp` only
  - `mode_status(gimp)` reports live provider connection state and tool
    discovery
  - `mode_refresh_tools(gimp)` forces reconnect and tool rediscovery
  - `mode_undo(gimp)` delegates to the real provider path
  - the unified shell enables the GIMP composer and uses `assistantSend()` for
    GIMP chats
  - GIMP responses can render explain text, tool activity, and Undo
  - Code mode still uses the existing Codehelper inference path
  - non-Code, non-GIMP modes remain placeholders

## Phase 5: Blender Mode

**Suggested branches:** `codex/unified-blender-mode-docs`, then `codex/unified-blender-mode`

**Scope**

- port Blender bridge behavior into a Blender provider
- keep Blender bridge-first and shared-engine-only
- include scene-aware tutoring chat plus local Blender-doc retrieval grounding
- preserve token streaming with cancellation
- keep standalone Blender app code as a reference source rather than merging it

**Phase 5 preflight decisions**

- Blender becomes the second live non-Code mode after GIMP
- `assistant_send` becomes operational for `blender`
- Blender stays bridge-first and does not require `blender-mcp`
- Blender keeps the existing addon bridge contract:
  - `127.0.0.1:5179`
  - `%LOCALAPPDATA%/SmolPC/engine-runtime/bridge-token.txt`
- bridge startup is lazy and non-fatal to unified app startup
- Blender uses shared engine only; no Ollama fallback UI or backend toggle in
  the unified shell
- Blender includes the standalone app's lightweight local retrieval path
- Blender uses token streaming plus structured provider events
- Blender keeps tutoring-style chat actions that fit the unified shell:
  - `Regenerate`
  - `Continue`
  - `Branch Chat`
- Blender undo remains unsupported in Phase 5
- Writer / Calc / Slides remain placeholders

**Exit criteria**

- Blender mode can complete a bridge-backed scene workflow in the unified app

**Current branch status**

- preflight docs merged into `docs/unified-assistant-spec`, then into
  `dev/unified-assistant`
- implementation merged via PR `#69`
- merged Blender behavior now present in `dev/unified-assistant`:
  - lazy bridge runtime startup hosted by the unified app
  - live `mode_status(blender)` and `mode_refresh_tools(blender)`
  - `assistant_send(mode=blender)` with token streaming
  - scene snapshot plus retrieval-grounded tutoring prompts
  - Blender shell copy and prompt starters now reflect a live mode
  - Blender assistant messages keep `Regenerate`, `Continue`, and
    `Branch Chat`
  - no edits to `apps/blender-assistant/`

## Phase 6: LibreOffice Modes

**Suggested branches:** `codex/unified-libreoffice-mode-docs`, then `codex/unified-libreoffice-mode`

**Scope**

- port from the LibreOffice source branch into one shared provider
- expose Writer, Calc, and Slides as separate frontend modes

**Exit criteria**

- Writer, Calc, and Slides share one provider runtime and feel like distinct
  frontend modes

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
