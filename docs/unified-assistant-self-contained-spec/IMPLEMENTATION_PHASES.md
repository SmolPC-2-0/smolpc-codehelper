# Self-Contained Delivery Phases

**Last Updated:** 2026-03-17
**Status:** Branch cut complete; Step 1 docs baseline is the current phase

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

## Phase 2: Self-Contained Foundation

**Branches**

1. `codex/unified-self-contained-foundation-docs`
2. merge into `docs/unified-assistant-self-contained-spec`
3. merge docs into `dev/unified-assistant-self-contained`
4. `codex/unified-self-contained-foundation`
5. merge into `dev/unified-assistant-self-contained`
6. `codex/unified-self-contained-foundation-status-docs`
7. merge into `docs/unified-assistant-self-contained-spec`
8. merge docs into `dev/unified-assistant-self-contained`

**Scope**

- setup/provisioning subsystem
- host-app detection for GIMP, Blender, LibreOffice
- resource version manifests
- bundled app-private Python ownership
- one bundled default model
- startup orchestration for engine and model
- setup status/repair surface

**Exit criteria**

- fresh install requires no system Python
- engine auto-starts
- bundled default model resolves from packaged resources
- setup layer reports host-app, provisioned-asset, and runtime readiness state honestly

## Phase 3: LibreOffice Self-Contained Runtime

**Branches**

1. `codex/unified-self-contained-libreoffice-docs`
2. merge docs into `docs/unified-assistant-self-contained-spec`
3. merge docs into `dev/unified-assistant-self-contained`
4. `codex/unified-self-contained-libreoffice`
5. merge into `dev/unified-assistant-self-contained`
6. `codex/unified-self-contained-libreoffice-status-docs`
7. merge into `docs/unified-assistant-self-contained-spec`
8. merge docs into `dev/unified-assistant-self-contained`

**Scope**

- eliminate external Python for Writer/Slides
- run the bundled LibreOffice runtime scripts from app-private Python
- detect and auto-launch `soffice`
- keep Calc scaffold-only

**Exit criteria**

- Writer and Slides work on a machine with LibreOffice installed and no Python installed
- no manual bootstrap/runtime step remains

## Phase 4: Blender Self-Contained Provisioning

**Branches**

1. `codex/unified-self-contained-blender-docs`
2. merge docs into `docs/unified-assistant-self-contained-spec`
3. merge docs into `dev/unified-assistant-self-contained`
4. `codex/unified-self-contained-blender`
5. merge into `dev/unified-assistant-self-contained`
6. `codex/unified-self-contained-blender-status-docs`
7. merge into `docs/unified-assistant-self-contained-spec`
8. merge docs into `dev/unified-assistant-self-contained`

**Scope**

- bundle Blender addon from existing repo source
- auto-install and auto-enable addon into user profile
- auto-launch Blender when required
- keep current bridge-first design intact

**Exit criteria**

- Blender mode works on a machine with Blender installed but no addon manually installed
- no manual addon installation or enable step remains

## Phase 5: GIMP Self-Contained Provisioning

**Branches**

1. `codex/unified-self-contained-gimp-docs`
2. merge docs into `docs/unified-assistant-self-contained-spec`
3. merge docs into `dev/unified-assistant-self-contained`
4. `codex/unified-self-contained-gimp`
5. merge into `dev/unified-assistant-self-contained`
6. `codex/unified-self-contained-gimp-status-docs`
7. merge into `docs/unified-assistant-self-contained-spec`
8. merge docs into `dev/unified-assistant-self-contained`

**Scope**

- vendor pinned upstream `gimp-mcp` source snapshot
- bundle GIMP plugin/server resources under unified app ownership
- auto-provision GIMP plugin files into the user profile
- auto-launch GIMP and bundled GIMP MCP runtime
- keep unified provider transport on `127.0.0.1:10008`

**Exit criteria**

- GIMP mode works on a machine with GIMP installed but no plugin/server manually configured
- no manual clone, environment variable, plugin copy, or terminal start step remains

## Phase 6: Release Packaging And Validation

**Branches**

1. `codex/unified-self-contained-release-docs`
2. merge docs into `docs/unified-assistant-self-contained-spec`
3. merge docs into `dev/unified-assistant-self-contained`
4. `codex/unified-self-contained-release`
5. merge into `dev/unified-assistant-self-contained`
6. `codex/unified-self-contained-release-status-docs`
7. merge into `docs/unified-assistant-self-contained-spec`
8. merge docs into `dev/unified-assistant-self-contained`

**Scope**

- packaged Windows installer
- bundled model/runtime payload validation
- first-run setup and repair flows
- clean-machine acceptance testing
- final supported install/use documentation

**Exit criteria**

- external users can install one app and use Code, GIMP, Blender, Writer, and Slides without manual secondary setup
- only host-app installs remain external prerequisites
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
