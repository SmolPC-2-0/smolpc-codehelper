# Self-Contained Delivery Phases

**Last Updated:** 2026-03-21
**Status:** Branch cut, cleanup, foundation, Phase 5 complete; Phase 6 release packaging and validation is next

## Phase 0: Demo Freeze And Branch Cut

**Status:** complete

### Outcomes

- frozen demo branches retained:
  - `dev/unified-assistant`
  - `docs/unified-assistant-spec`
- freeze tags created:
  - `demo/unified-assistant-freeze-2026-03-17`
  - `demo/unified-assistant-spec-freeze-2026-03-17`
- new mainlines opened:
  - `dev/unified-assistant-self-contained`
  - `docs/unified-assistant-self-contained-spec`

## Phase 1: Self-Contained Master Plan Docs

**Branches**

1. `codex/unified-self-contained-master-plan-docs`
2. merge into `docs/unified-assistant-self-contained-spec`
3. merge docs into `dev/unified-assistant-self-contained`

**Scope**

- document the finish line for a self-contained external-user product
- document the new branch policy
- document runtime ownership by mode
- lock the bundled default model decision
- document provenance requirements for third-party imports

**Exit criteria**

- the full self-contained roadmap is documented
- the new branch policy is documented
- no implementation branch opens before these docs merge through both new mainlines

## Phase 1A: Pre-Phase-2 Baseline Cleanup

**Branches**

1. `codex/unified-self-contained-baseline-cleanup-docs`
2. merge into `docs/unified-assistant-self-contained-spec`
3. `codex/unified-self-contained-baseline-cleanup-sync`
4. merge into `dev/unified-assistant-self-contained`

**Scope**

- clear the remaining non-blocking docs debt from the branch-cut PRs
- add the required docs-sync and status-sync workflow pattern
- keep provenance/license wording honest and explicit
- standardize the self-contained docs terminology around phases

**Exit criteria**

- cleanup docs are merged into `docs/unified-assistant-self-contained-spec`
- cleanup sync is merged into `dev/unified-assistant-self-contained`
- Phase 2 begins only after this cleanup sync branch is merged

## Phase 2: Self-Contained Foundation

**Branches**

1. `codex/unified-self-contained-foundation-docs`
2. merge into `docs/unified-assistant-self-contained-spec`
3. `codex/unified-self-contained-foundation-docs-sync`
4. merge into `dev/unified-assistant-self-contained`
5. `codex/unified-self-contained-foundation`
6. merge into `dev/unified-assistant-self-contained`
7. `codex/unified-self-contained-foundation-status-docs`
8. merge into `docs/unified-assistant-self-contained-spec`
9. `codex/unified-self-contained-foundation-status-sync`
10. merge into `dev/unified-assistant-self-contained`

**Scope**

- setup/provisioning subsystem
- host-app detection for GIMP, Blender, LibreOffice
- resource version manifests
- bundled app-private Python ownership scaffolding
- one bundled default model ownership scaffolding
- setup status/repair surface
- packaged resource contract for Python and models

**Locked Phase 2 non-goals**

- no mode activation changes
- no Blender addon provisioning yet
- no GIMP plugin/server provisioning yet
- no LibreOffice runtime switchover yet
- no host-app launch orchestration yet
- no Calc activation
- no GitHub workflow redesign

**Exit criteria**

- setup subsystem exists in the implementation line
- `setup_status` and `setup_prepare` exist
- setup layer reports host-app and bundled-asset readiness honestly
- packaged resource contracts for bundled Python and the default model are established
- existing Code, GIMP, Blender, Writer, and Slides behavior remains unchanged
- Calc remains explicitly out of scope

**Closeout status**

Complete on the self-contained implementation line:

- `setup_status` and `setup_prepare` landed
- setup banner and setup panel landed
- tracked resource manifests landed for Python, GIMP, Blender, LibreOffice, and models
- model and Python staging scripts landed
- packaged resource config now includes the new foundation roots

The next official branch after Phase 2 closeout docs is:

- `codex/unified-self-contained-libreoffice-docs`

## Workflow Transition After Phase 2

Phases 0 through 2 used the temporary dual-mainline workflow that kept
`docs/unified-assistant-self-contained-spec` and
`dev/unified-assistant-self-contained` moving in lockstep.

That transition period is complete.

Starting in Phase 3:

- `dev/unified-assistant-self-contained` is the sole active self-contained mainline
- `docs/unified-assistant-self-contained-spec` remains as a frozen archive/reference snapshot
- every future phase uses a 3-PR docs-only -> implementation -> closeout-docs flow on
  `dev/unified-assistant-self-contained`

## Phase 3: LibreOffice Self-Contained Runtime

**Status:** complete

**Branches**

1. `codex/unified-self-contained-libreoffice-docs`
2. merge docs into `dev/unified-assistant-self-contained`
3. `codex/unified-self-contained-libreoffice`
4. merge into `dev/unified-assistant-self-contained`
5. `codex/unified-self-contained-libreoffice-status-docs`
6. merge docs into `dev/unified-assistant-self-contained`

**Scope**

- eliminate external Python for Writer/Slides
- run the bundled LibreOffice runtime scripts from app-private Python
- detect and auto-launch `soffice`
- keep Calc scaffold-only

**Exit criteria**

- Writer and Slides work on a machine with LibreOffice installed and no Python installed
- no manual bootstrap/runtime step remains

**Closeout status**

Complete on the self-contained implementation line:

- Writer and Slides now consume the setup-prepared bundled Python runtime in packaged mode
- packaged mode no longer falls back to system `python` or `python3`
- LibreOffice host detection is now wired through the setup locator
- the detected `soffice` path is passed into the bundled runtime
- the bundled runtime auto-launches LibreOffice on demand
- Calc remains scaffold-only

The next official branch after Phase 3 closeout docs is:

- `codex/unified-self-contained-blender-docs`

## Phase 4: Blender Self-Contained Provisioning

**Status:** complete

**Branches**

1. `codex/unified-self-contained-blender-docs`
2. merge docs into `dev/unified-assistant-self-contained`
3. `codex/unified-self-contained-blender`
4. merge into `dev/unified-assistant-self-contained`
5. `codex/unified-self-contained-blender-status-docs`
6. merge docs into `dev/unified-assistant-self-contained`

**Scope**

- bundle Blender addon from existing repo source
- auto-install and auto-enable addon into user profile
- auto-launch Blender when required
- keep current bridge-first design intact

**Locked Phase 4 decisions**

- authoritative addon source remains:
  - `apps/blender-assistant/blender_addon/blender_helper_http.py`
- the unified app copies a pinned snapshot into:
  - `apps/codehelper/src-tauri/resources/blender/addon/blender_helper_http.py`
- addon module id is locked to:
  - `blender_helper_http`
- setup gains one new status item:
  - `blender_addon`
- `setup_prepare()` may provision and enable the addon through Blender CLI background execution, but it must not launch the interactive Blender UI
- Blender mode itself may auto-provision on first use if the addon is missing
- Blender profile and addon target resolution must come from Blender itself, not guessed profile paths
- the app may auto-launch Blender only when no matching Blender process is already running
- the app must not kill or restart an already running Blender instance
- if Blender is already running without the addon loaded, the app must report that the addon is provisioned for future sessions and that the current session may need reopening once

**Exit criteria**

- Blender mode works on a machine with Blender installed but no addon manually installed
- no manual addon installation or enable step remains

**Closeout status**

Complete on the self-contained implementation line:

- Blender addon snapshot is now bundled at:
  - `apps/codehelper/src-tauri/resources/blender/addon/blender_helper_http.py`
- setup status now includes `blender_addon`
- `setup_prepare()` now provisions and enables the addon through Blender CLI background execution
- setup still does not launch the interactive Blender UI
- Blender mode now provisions and enables the addon on demand, launches Blender only when needed, and preserves already-running Blender sessions

The next official branch after Phase 4 closeout docs is:

- `codex/unified-self-contained-gimp-docs`

## Phase 5: GIMP Self-Contained Provisioning

**Status:** complete

**Branches**

1. `codex/unified-self-contained-gimp-docs`
2. merge docs into `dev/unified-assistant-self-contained`
3. `codex/unified-self-contained-gimp`
4. merge into `dev/unified-assistant-self-contained`
5. `codex/unified-self-contained-gimp-status-docs`
6. merge docs into `dev/unified-assistant-self-contained`

**Scope**

- vendor pinned upstream `maorcc/gimp-mcp` source snapshot
- bundle GIMP plugin/server resources under unified app ownership
- auto-provision GIMP plugin files into the user profile
- auto-launch GIMP and bundled GIMP MCP runtime
- keep unified provider transport on `127.0.0.1:10008`

**Locked Phase 5 decisions**

- authoritative GIMP runtime source is:
  - upstream `maorcc/gimp-mcp` pinned to an exact commit/tag before implementation PR opens
- unified bundled import target remains:
  - `apps/codehelper/src-tauri/resources/gimp/`
- setup remains app-level:
  - one `Prepare` action only
  - no GIMP-specific setup wizard or path-settings surface
- `setup_prepare()` may provision/repair GIMP plugin/server assets, but it must not launch the interactive GIMP UI
- mode-driven first-use may auto-launch GIMP and the bundled GIMP MCP runtime when required
- existing Blender, LibreOffice, Code, and Calc behaviors must remain unchanged in this phase

**Closeout status**

Complete on the self-contained implementation line:

- the pinned upstream `maorcc/gimp-mcp` snapshot is now vendored under `apps/codehelper/src-tauri/resources/gimp/`
- setup status now includes `gimp_plugin_runtime` separate from `host_gimp`
- `setup_prepare()` now provisions and repairs the bundled GIMP assets without launching the interactive GIMP UI
- GIMP mode now validates detected installs as GIMP 3.x, provisions missing assets on demand, launches GIMP only when needed, and supervises the bundled loopback bridge on `127.0.0.1:10008`
- already running GIMP sessions are reused rather than force-restarted
- Blender, LibreOffice, Code, and Calc behavior remained unchanged during Phase 5

**Exit criteria**

- GIMP mode works on a machine with GIMP installed but no plugin/server manually configured
- no manual clone, environment variable, plugin copy, or terminal start step remains

Immediately after Phase 5 closeout, the unified mainline runs a narrow Windows
source-testing gate:

- docs:
  - `codex/unified-self-contained-functional-test-docs`
- implementation:
  - `codex/unified-self-contained-functional-test-prep`
- closeout docs:
  - `codex/unified-self-contained-functional-test-status-docs`

That gate is now merged and leaves the branch ready for broader Windows
source-based testing without renumbering phases.

After initial Windows testing results and any narrow follow-up fixes, the next
official Phase 6 docs branch is:

- `codex/unified-self-contained-release-docs`

## Phase 6: Release Packaging And Validation

**Branches**

1. `codex/unified-self-contained-release-docs`
2. merge docs into `dev/unified-assistant-self-contained`
3. `codex/unified-self-contained-release`
4. merge into `dev/unified-assistant-self-contained`
5. `codex/unified-self-contained-release-status-docs`
6. merge docs into `dev/unified-assistant-self-contained`

**Scope**

- packaged Windows installer
- bundled model/runtime payload validation
- first-run setup and repair flows
- clean-machine acceptance testing
- final supported install/use documentation

**Exit criteria**

- external users can install one app and use Code, GIMP, Blender, Writer, and Slides without manual secondary setup
- only host-app installs remain external prerequisites
- the GPL-3.0 distribution review for the vendored `maorcc/gimp-mcp` payload is resolved before any public packaging milestone
- Calc remains visible but disabled

## Calc Status

Calc is explicitly out of scope for this self-contained finish line.

This roadmap does not reserve a follow-on Calc activation phase before release.
If Calc activation is pursued later, it should start as a separate post-finish-line
docs-first workstream rather than quietly expanding Phase 6.

## Phase Order Rule

Do not reorder the self-contained phases.

Why:

- Phase 2 provides the setup/provisioning substrate all later phases need
- LibreOffice is already closest to self-contained because its runtime scripts are already in-tree
- Blender can reuse existing repo addon source without third-party vendoring
- GIMP requires the highest external-asset ownership change and should follow the provenance/foundation work
- release packaging is only meaningful after runtime ownership is implemented
