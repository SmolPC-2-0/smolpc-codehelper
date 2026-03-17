# Current State

**Last Updated:** 2026-03-17
**Phase:** Phase 7 hardening and packaging is merged; v1 unified app is complete with Calc deferred

## Branch Roles

| Branch                                             | Role                                       |
| -------------------------------------------------- | ------------------------------------------ |
| `docs/unified-assistant-spec`                      | Canonical architecture/spec branch         |
| `dev/unified-assistant`                            | Implementation mainline after docs merge   |
| `codex/unified-foundation`                         | Merged Phase 1 implementation branch       |
| `codex/unified-foundation-status-docs`             | Phase 1 closeout docs branch               |
| `codex/unified-shell-docs`                         | Merged Phase 2 preflight docs branch       |
| `codex/unified-shell`                              | Merged Phase 2 shell implementation branch |
| `codex/unified-shell-status-docs`                  | Merged Phase 2 closeout docs branch        |
| `codex/unified-shell-followups`                    | Merged post-Phase-2 shell hardening branch |
| `codex/unified-shell-followups-status-docs`        | Merged shell follow-up docs sync branch    |
| `codex/unified-code-mode-docs`                     | Merged Phase 3 preflight docs branch       |
| `codex/unified-code-mode`                          | Merged Phase 3 implementation branch       |
| `codex/unified-code-mode-status-docs`              | Merged Phase 3 closeout docs branch        |
| `codex/unified-gimp-mode-docs`                     | Merged Phase 4 preflight docs branch       |
| `codex/unified-gimp-mode`                          | Merged Phase 4 implementation branch       |
| `codex/unified-gimp-mode-status-docs`              | Merged Phase 4 closeout docs branch        |
| `codex/unified-blender-mode-docs`                  | Merged Phase 5 preflight docs branch       |
| `codex/unified-blender-mode`                       | Merged Phase 5 implementation branch       |
| `codex/unified-blender-mode-status-docs`           | Merged Phase 5 closeout docs branch        |
| `codex/unified-libreoffice-mode-docs`              | Merged Phase 6A preflight docs branch      |
| `codex/unified-libreoffice-mode`                   | Merged Phase 6A implementation branch      |
| `codex/unified-libreoffice-mode-status-docs`       | Merged Phase 6A closeout docs branch       |
| `codex/unified-libreoffice-activation-docs`        | Merged Phase 6B preflight docs branch      |
| `codex/unified-libreoffice-activation`             | Merged Phase 6B implementation branch      |
| `codex/unified-libreoffice-activation-status-docs` | Merged Phase 6B closeout docs branch       |
| `codex/unified-hardening-docs`                     | Merged Phase 7 preflight docs branch       |
| `codex/unified-hardening`                          | Merged Phase 7 implementation branch       |
| `codex/unified-hardening-status-docs`              | Merged Phase 7 closeout docs branch        |

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

Phase 4 GIMP mode is now merged into `dev/unified-assistant` via PR `#67`.

Merged GIMP behavior now present in `dev/unified-assistant`:

- `assistant_send` is now operational for `mode === gimp`
- `mode_status(gimp)` now reports real provider connection state and tool
  discovery
- `mode_refresh_tools(gimp)` now forces reconnect and tool rediscovery
- `mode_undo(gimp)` now delegates to the real GIMP provider path
- the placeholder `modes/gimp.rs` file has been replaced with a real unified
  GIMP adapter module tree under `apps/codehelper/src-tauri/src/modes/gimp/`
- the shared `smolpc-mcp-client` crate now includes TCP JSON-RPC transport and
  MCP session helpers used by the unified GIMP provider
- the unified shell now enables the GIMP composer, routes GIMP requests through
  `assistantSend()`, renders tool activity and explain text, and exposes Undo
  only on undoable GIMP responses
- Code mode still uses the existing Codehelper inference path
- Blender / Writer / Calc / Slides remain visible placeholders

Merged GIMP v1 action surface:

- GIMP info query
- current image metadata query
- describe current image
- draw line
- draw heart
- draw circle
- draw oval
- draw triangle
- draw filled rectangle / square
- crop to square
- resize width
- increase / decrease brightness
- increase / decrease contrast
- blur entire image
- brighten / darken top, bottom, left, or right half
- increase / decrease contrast in top, bottom, left, or right half
- blur top, bottom, left, or right half
- rotate 90 / 180 / 270
- flip horizontal / vertical
- undo last change

Validation completed for the merged GIMP mode:

- `cargo test -p smolpc-mcp-client`
- `cargo check -p smolpc-code-helper`
- `cargo test -p smolpc-code-helper --lib modes::gimp`
- `npm run check --workspace apps/codehelper`
- PR `#67` merged into `dev/unified-assistant`

Phase 5 Blender mode is now merged into `dev/unified-assistant` via PR `#69`.

Merged Blender behavior now present in `dev/unified-assistant`:

- `assistant_send` is now operational for `mode === blender`
- `mode_status(blender)` now reports live provider state from the lazy-start
  bridge-backed Blender provider
- `mode_refresh_tools(blender)` now refreshes bridge/runtime health and
  pseudo-tool availability
- Blender stays bridge-first and shared-engine-only
- the unified app now hosts the local Blender bridge server on
  `127.0.0.1:5179`
- bridge startup is lazy and non-fatal to unified app startup; port conflicts
  degrade Blender mode only
- the unified app now bundles provider-owned Blender retrieval metadata under
  `apps/codehelper/src-tauri/resources/blender/rag_system/simple_db/metadata.json`
- Blender questions now use scene snapshot plus local Blender-doc retrieval
  grounding when appropriate
- Blender uses token streaming with cancellation through the shared
  `assistant_send` surface
- Blender assistant messages keep tutoring-style conversation actions that fit
  the unified shell:
  - `Regenerate`
  - `Continue`
  - `Branch Chat`
- Blender undo remains unsupported
- Code mode still uses the existing Codehelper inference path
- GIMP keeps the Phase 4 MCP-backed path unchanged
- Writer / Calc / Slides remain visible placeholders

Validation completed for the merged Blender mode:

- `cargo check -p smolpc-code-helper`
- `cargo test -p smolpc-code-helper --lib`
- `npm run check --workspace apps/codehelper`
- PR `#69` merged into `dev/unified-assistant`

The standalone apps remain source references during the future port:

- `apps/gimp-assistant`
- `apps/blender-assistant`
- `apps/libreoffice-assistant`

LibreOffice source-branch progress currently lives on
`origin/codex/libreoffice-port-track-a`:

- standalone shared-engine baseline complete
- standalone MCP runtime port complete
- standalone Phase 3 workflow preview complete with CPU-lane validation
- Writer / Slides tool coverage is ahead of Calc-specific coverage

Phase 6A LibreOffice scaffolding is now merged into `dev/unified-assistant`
via PR `#75`.

Merged LibreOffice scaffolding now present in `dev/unified-assistant`:

- shared stdio MCP transport support now exists in `smolpc-mcp-client`
  through `StdioJsonRpcClient` and `McpSession::connect_stdio(...)`
- the old single-file LibreOffice placeholder has been replaced with a
  stateful shared-provider scaffold under
  `apps/codehelper/src-tauri/src/modes/libreoffice/`
- a tracked staged resource placeholder now exists at
  `apps/codehelper/src-tauri/resources/libreoffice/mcp_server/README.md`
- `mode_status(writer|calc|impress)` now returns scaffold-aware provider detail
  rather than the original generic foundation placeholder wording
- `mode_refresh_tools(writer|calc|impress)` now revalidates the staged
  scaffold only and does not launch a LibreOffice runtime
- Writer, Calc, and Slides keep honest scaffold copy, mode-specific disabled
  placeholders, and no live send path in the unified shell
- `assistant_send` remains scaffold-only for `writer`, `calc`, and `impress`
- no files under `apps/libreoffice-assistant/` were modified

Validation completed for the merged LibreOffice scaffolding:

- `cargo test -p smolpc-mcp-client`
- `cargo check -p smolpc-code-helper`
- `cargo test -p smolpc-code-helper --lib`
- `npm run check --workspace apps/codehelper`
- PR `#75` checks green, including `Incremental Style Gates` and
  `Tauri Build Check`

Phase 6B LibreOffice activation is now merged into `dev/unified-assistant`
via PR `#78`.

Merged LibreOffice activation now present in `dev/unified-assistant`:

- imported Python MCP runtime assets now live under
  `apps/codehelper/src-tauri/resources/libreoffice/mcp_server/`
- `assistant_send` is now operational for `writer` and `impress`
- `assistant_send(calc)` remains scaffold-only
- `mode_status(writer|impress)` now reports real runtime-backed provider state
- `mode_refresh_tools(writer|impress)` now starts or refreshes the shared stdio
  MCP runtime and returns mode-filtered allowlisted tools
- `mode_status(calc)` and `mode_refresh_tools(calc)` remain scaffold-aware and
  do not launch the runtime
- Writer and Slides now use the unified shell's live request path with status,
  tool, token, complete, and error events
- Writer and Slides enforce one tool call maximum per assistant turn plus one
  summary follow-up maximum
- Writer and Slides fall back to a deterministic local summary if the summary
  generation step fails, times out, or is cancelled after a successful tool
  call
- Writer and Slides do not expose Undo, Regenerate, Continue, or Branch Chat
  because document tool calls are side-effectful
- Calc remains an honest scaffold-only mode with disabled composer and no fake
  tool surface
- no files under `apps/libreoffice-assistant/` were modified

Validation completed for the merged LibreOffice activation:

- `cargo test -p smolpc-mcp-client`
- `cargo check -p smolpc-code-helper`
- `cargo test -p smolpc-code-helper --lib`
- `npm run check --workspace apps/codehelper`
- PR `#78` merged into `dev/unified-assistant`

Phase 7 hardening and packaging is now merged into `dev/unified-assistant`
via PR `#82`.

Merged Phase 7 hardening now present in `dev/unified-assistant`:

- LibreOffice helper traffic now uses a per-runtime auth token inside the
  imported Python runtime
- LibreOffice helper framing now enforces a hard `10 MiB` message-size ceiling
  on both request and response payloads
- LibreOffice helper responses are now schema-validated before use and malformed
  responses are treated as hard runtime errors
- the LibreOffice runtime now receives a provider-owned validated log directory
  path from Rust rather than trusting arbitrary external log-path input
- LibreOffice runtime startup and shutdown now use bounded polling and
  terminate-then-kill cleanup instead of blind sleeps
- `LibreOfficeProvider` now keeps session ownership behind a clonable shared
  handle so tool execution no longer depends on holding the provider state lock
  across MCP calls
- launcher commands, launcher state wiring, launcher resources, and launcher
  ownership assumptions have been removed from the unified app
- packaged product identity now uses `SmolPC Unified Assistant` for visible app
  naming while keeping the bundle identifier `com.smolpc.codehelper`
- Calc remains scaffold-only and post-v1

Validation completed for the merged Phase 7 hardening branch:

- `cargo test -p smolpc-mcp-client`
- `cargo check -p smolpc-code-helper`
- `cargo test -p smolpc-code-helper --lib`
- `npm run check --workspace apps/codehelper`
- local packaging/resource validation in the unified Tauri config test now
  verifies bundled LibreOffice and Blender resources while excluding launcher
  resources
- real Windows packaged-app validation was not executed in this branch and
  remains a manual shipping follow-up

## What Has Not Started

- live Calc activation inside the unified app
- migrating Code mode onto `assistant_send`
- a real Windows packaged-app smoke test on target hardware
- any bundle-identifier migration away from `com.smolpc.codehelper`

## Next Workstreams

Phase 7 closes the required v1 unified-app branch flow. There is no mandatory
Phase 8 branch locked yet.

If post-v1 work starts, it should still follow the same docs-first workflow:

1. create a docs branch from `origin/docs/unified-assistant-spec`
2. merge docs into `docs/unified-assistant-spec`
3. merge docs into `dev/unified-assistant`
4. create the implementation branch from updated `origin/dev/unified-assistant`

The most likely post-v1 follow-ups are:

1. live Calc activation
2. a real Windows packaged-app validation pass
3. optional packaging/distribution follow-ups such as bundle-identifier
   migration

## Known Risks

| Risk                              | Impact                                                                                   |
| --------------------------------- | ---------------------------------------------------------------------------------------- |
| Engine branch churn               | unified app may need contract updates while the engine is still evolving                 |
| Standalone app branch churn       | Blender and LibreOffice behavior may continue changing during the remaining ports        |
| External GIMP runtime assumptions | Phase 4 depends on a separate GIMP install and MCP plugin/server already being available |
| Packaging/runtime validation      | third-party runtime paths may behave differently in packaged Windows builds              |
| LibreOffice port alignment        | the LibreOffice source branch must stay aligned with the unified provider scaffold plan  |

## Merge-Safe Rules

1. Do docs work first.
2. Merge docs into `dev/unified-assistant`.
3. Create implementation branches from `dev/unified-assistant` only.
4. Port behavior into new unified adapters rather than merging standalone app directories.
5. Treat engine contract changes as separate work when possible.

## Current Success Condition

The current v1 baseline is correct only when:

1. Phase 7 hardening is merged into `dev/unified-assistant`
2. the Phase 7 closeout docs are merged into `docs/unified-assistant-spec`
3. those docs are merged back into `dev/unified-assistant`
4. Code, GIMP, Blender, Writer, and Slides are documented as live
5. Calc remains documented as scaffold-only and post-v1
6. launcher assumptions are documented as removed from the unified app runtime
7. Windows packaged-app validation is recorded as still pending manual follow-up
