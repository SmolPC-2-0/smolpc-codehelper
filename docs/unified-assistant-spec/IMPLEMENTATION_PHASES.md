# Unified Assistant Implementation Phases

**Last Updated:** 2026-03-17
**Status:** Phase 7 hardening and packaging is merged; v1 is complete with Calc deferred

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

## Branch Order Through Phase 7

1. `codex/unified-hardening-docs`
2. merge into `docs/unified-assistant-spec`
3. merge `docs/unified-assistant-spec` into `dev/unified-assistant`
4. `codex/unified-hardening`
5. merge into `dev/unified-assistant`
6. `codex/unified-hardening-status-docs`
7. merge into `docs/unified-assistant-spec`
8. merge `docs/unified-assistant-spec` into `dev/unified-assistant`

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

- Code mode is feature-equivalent enough to serve as the baseline mode in the unified app

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
- the Windows packaging path above is canonical; non-Windows dev/test
  environments may use the platform-appropriate local app-data equivalent while
  preserving the same addon-facing token-file contract
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

## Phase 6A: LibreOffice Scaffolding

**Suggested branches:** `codex/unified-libreoffice-mode-docs`, then `codex/unified-libreoffice-mode`

**Scope**

- add merge-safe LibreOffice scaffolding into the unified app
- keep Writer, Calc, and Slides visible but disabled
- add shared stdio MCP transport support for future provider activation
- replace the single-file LibreOffice placeholder with a real shared-provider module tree
- stage a tracked LibreOffice resource placeholder for the future MCP runtime sync

**Locked decisions**

- this phase does not activate `assistant_send` for `writer`, `calc`, or `impress`
- this phase does not import the full LibreOffice Python MCP runtime yet
- `origin/codex/libreoffice-port-track-a` remains a read-only reference source
- Calc is not required to be live in this phase
- current source-branch baseline is:
  - Phase 1 shared-engine baseline complete
  - Phase 2 MCP runtime port complete
  - Phase 3 workflow preview complete with CPU-lane validation
  - Writer / Slides coverage ahead of Calc-specific coverage

**Exit criteria**

- `smolpc-mcp-client` supports stdio MCP transport
- unified LibreOffice provider scaffolding exists under `apps/codehelper/src-tauri/src/modes/libreoffice/`
- Writer, Calc, and Slides still share one provider family, feel like distinct frontend modes, and remain honest placeholders
- roadmap and docs explicitly defer live LibreOffice activation to a later branch

**Current branch status**

- preflight docs merged into `docs/unified-assistant-spec`, then into
  `dev/unified-assistant`
- implementation merged via PR `#75`
- merged Phase 6A scaffolding now present in `dev/unified-assistant`:
  - shared stdio MCP transport support in `smolpc-mcp-client`
  - stateful shared `LibreOfficeProvider` scaffold under
    `apps/codehelper/src-tauri/src/modes/libreoffice/`
  - staged placeholder resource root at
    `apps/codehelper/src-tauri/resources/libreoffice/mcp_server/README.md`
  - scaffold-aware `mode_status(writer|calc|impress)` and
    `mode_refresh_tools(writer|calc|impress)`
  - Writer / Calc / Slides remain disabled placeholder modes with honest shell
    copy
  - `assistant_send` remains scaffold-only for `writer`, `calc`, and `impress`
  - no edits to `apps/libreoffice-assistant/`

## Phase 6B: LibreOffice Activation

**Suggested branches:** `codex/unified-libreoffice-activation-docs`, then `codex/unified-libreoffice-activation`

**Scope**

- import the selected LibreOffice MCP runtime assets from the separate source branch
- activate the shared LibreOffice provider for live Writer and Slides execution
- keep Calc scaffold-only while the source branch continues maturing spreadsheet coverage
- reuse the unified app's shared stdio MCP transport rather than porting the standalone Rust MCP client
- keep the work integration-focused and avoid porting the standalone LibreOffice UI

**Locked decisions**

- `origin/codex/libreoffice-port-track-a` remains a read-only reference source pinned to commit
  `7acad1fa0eb31e32a5485069e85c021d14284455` for this phase
- `apps/libreoffice-assistant/` remains untouched in unified activation work
- Writer and Slides are the only live LibreOffice submodes in Phase 6B
- Calc remains visible but scaffold-only after this branch
- `assistant_send` becomes operational for `writer` and `impress` only
- `assistant_send(calc)` remains scaffold-only
- one shared `LibreOfficeProvider` still owns `writer`, `calc`, and `impress`
- the unified app imports these Python runtime assets into
  `apps/codehelper/src-tauri/resources/libreoffice/mcp_server/`:
  - `main.py`
  - `libre.py`
  - `helper.py`
  - `helper_utils.py`
  - `helper_test_functions.py`
- runtime activation uses the existing shared stdio MCP support in
  `smolpc-mcp-client`, not the standalone `mcp_client.rs`
- runtime contract remains:
  - stdio MCP child process via `main.py`
  - helper socket on `localhost:8765`
  - headless office socket on `localhost:2002`
- runtime remains engine-only
- no Ollama paths, provider toggles, settings UI, or standalone MCP diagnostics
  panels are ported
- Writer and Slides use one tool call maximum per assistant turn
- Writer and Slides use one summary follow-up maximum after tool execution
- if summary generation fails, times out, or is cancelled after a successful tool
  call, the unified app returns a deterministic local summary from the tool result
- cancellation stops generation and follow-up summary work but does not roll back
  an already executed LibreOffice document tool

**Writer tool allowlist**

- `create_blank_document`
- `read_text_document`
- `get_document_properties`
- `list_documents`
- `copy_document`
- `add_text`
- `add_heading`
- `add_paragraph`
- `add_table`
- `insert_image`
- `insert_page_break`
- `format_text`
- `search_replace_text`
- `delete_text`
- `format_table`
- `delete_paragraph`
- `apply_document_style`

**Slides tool allowlist**

- `create_blank_presentation`
- `read_presentation`
- `get_document_properties`
- `list_documents`
- `copy_document`
- `add_slide`
- `edit_slide_content`
- `edit_slide_title`
- `delete_slide`
- `apply_presentation_template`
- `format_slide_content`
- `format_slide_title`
- `insert_slide_image`

**Calc tool allowlist**

- none in Phase 6B

**Exit criteria**

- Writer and Slides are live through the shared LibreOffice provider
- `assistant_send(writer)` and `assistant_send(impress)` are operational
- Calc remains an honest scaffold-only mode with disabled composer and no fake
  tool surface
- imported Python runtime assets resolve from unified app resources
- `mode_status(writer|impress)` and `mode_refresh_tools(writer|impress)` are
  runtime-backed
- hardening can begin from a real LibreOffice integration baseline

**Current branch status**

- preflight docs merged into `docs/unified-assistant-spec`, then into
  `dev/unified-assistant`
- implementation merged via PR `#78`
- merged Phase 6B activation now present in `dev/unified-assistant`:
  - imported Python runtime assets now live under
    `apps/codehelper/src-tauri/resources/libreoffice/mcp_server/`
  - Writer and Slides are live runtime-backed modes through the shared
    `LibreOfficeProvider`
  - Calc remains scaffold-only with disabled composer and no live send path
  - `assistant_send(writer)` and `assistant_send(impress)` are operational
  - `mode_status(writer|impress)` and `mode_refresh_tools(writer|impress)` are
    live and mode-filtered
  - Writer and Slides enforce one tool call maximum per turn plus one summary
    follow-up maximum
  - deterministic local summary fallback is used if summary generation fails,
    times out, or is cancelled after a successful tool call
  - Writer and Slides do not expose Undo, Regenerate, Continue, or Branch Chat
    in the unified shell
  - no edits to `apps/libreoffice-assistant/`

## Phase 7: Hardening And Packaging

**Suggested branches:** `codex/unified-hardening-docs`, then `codex/unified-hardening`

**Scope**

- finish the unified app for v1 with Calc still deferred
- harden the imported LibreOffice runtime and shared provider session handling
- remove remaining launcher assumptions
- validate packaged resource paths and packaged identity cleanup
- Windows-only end-to-end verification and launcher-independent startup

**Phase 7 preflight decisions**

- Phase 7 is the v1 finish-line phase for the unified app.
- Calc remains scaffold-only after Phase 7 and is explicitly post-v1 work.
- No new mode activation happens in Phase 7.
- Code mode does not switch onto `assistant_send` in Phase 7.
- LibreOffice helper transport stays on `localhost:8765`; office socket stays
  `localhost:2002`.
- Hardening focuses on authentication, framing bounds, subprocess lifecycle,
  response validation, packaging cleanup, and launcher removal rather than a new
  transport family.
- Visible packaged app branding becomes `SmolPC Unified Assistant`.
- The bundle identifier remains `com.smolpc.codehelper` in Phase 7 to avoid an
  installer-migration decision inside the hardening branch.
- Phase 7 closeout is explicit:
  `codex/unified-hardening-status-docs`.

**Exit criteria**

- unified app is Windows-valid without launcher runtime ownership
- v1 is considered complete with Code, GIMP, Blender, Writer, and Slides live
  while Calc remains deferred

**Current branch status**

- preflight docs merged into `docs/unified-assistant-spec`, then into
  `dev/unified-assistant`
- implementation merged via PR `#82`
- merged Phase 7 hardening now present in `dev/unified-assistant`:
  - LibreOffice helper traffic now uses a per-runtime auth token
  - LibreOffice helper framing now enforces hard request/response size bounds
  - LibreOffice helper responses are schema-validated before use
  - LibreOffice runtime startup/shutdown now use bounded polling and explicit
    terminate-then-kill cleanup
  - provider-owned LibreOffice log paths are resolved and validated by the
    unified app
  - `LibreOfficeProvider` now uses shareable session ownership for live tool
    execution
  - launcher commands/resources/state have been removed from the unified app
  - visible packaged identity now uses `SmolPC Unified Assistant`
  - bundle identifier remains `com.smolpc.codehelper`
  - Calc remains scaffold-only and post-v1
- validation completed in the implementation branch:
  - `cargo test -p smolpc-mcp-client`
  - `cargo check -p smolpc-code-helper`
  - `cargo test -p smolpc-code-helper --lib`
  - `npm run check --workspace apps/codehelper`
  - Windows packaged-app validation is still a manual follow-up outside this
    branch environment

## Merge-Safety Constraints

These apply to every implementation phase:

1. no hidden engine redesign in UI branches
2. no takeover of standalone app directories
3. port behavior into new unified adapters
4. keep branch scope narrow

## Windows Validation Milestones

| Milestone         | Required proof                                     |
| ----------------- | -------------------------------------------------- |
| Foundation ready  | commands and DTOs compile cleanly                  |
| Shell ready       | six modes visible in one shell                     |
| Code ready        | existing Codehelper experience preserved           |
| GIMP ready        | tool call and undo work                            |
| Blender ready     | bridge-backed workflow works                       |
| LibreOffice ready | Writer/Slides live via one provider; Calc deferred |
| Packaging ready   | packaged app runs without launcher ownership       |
