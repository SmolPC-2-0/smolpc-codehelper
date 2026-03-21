# SmolPC Unified Assistant Self-Contained Spec Index

> Read this document first. It defines the branch policy, frozen demo baseline,
> and document map for the self-contained delivery line.

**Last Updated:** 2026-03-21
**Status:** Single-mainline self-contained workflow active; Phase 5 GIMP self-contained provisioning complete; Windows source-testing handoff is merged and broader Windows source testing is the active gate before broader Phase 6 packaging work

## Project Summary

This spec line defines how SmolPC Unified Assistant moves from a
demo-capable unified app into a self-contained Windows product.

Finish-line definition:

- one installed Windows app: `SmolPC Unified Assistant`
- user manually installs only the host creative/office apps:
  - GIMP
  - Blender
  - LibreOffice / Collabora
- the unified app owns and auto-manages:
  - engine startup
  - bundled default model
  - app-private Python runtime
  - LibreOffice runtime scripts
  - Blender addon provisioning
  - GIMP plugin/server provisioning
  - on-demand host-app launch orchestration
- live shipped modes:
  - Code
  - GIMP
  - Blender
  - Writer
  - Slides
- `Calc` remains intentionally deferred

## Branch Roles

| Branch                                       | Role                                                                 |
| -------------------------------------------- | -------------------------------------------------------------------- |
| `dev/unified-assistant`                      | Frozen demo implementation baseline                                  |
| `docs/unified-assistant-spec`                | Frozen demo/spec baseline                                            |
| `dev/unified-assistant-self-contained`       | Sole active self-contained implementation and documentation mainline |
| `docs/unified-assistant-self-contained-spec` | Frozen self-contained archive/reference snapshot                     |
| `codex/*`                                    | Narrow work branches from `dev/unified-assistant-self-contained`     |

## Current Mainline Snapshot

As of 2026-03-21:

- source of truth is `dev/unified-assistant-self-contained`
- `docs/unified-assistant-self-contained-spec` remains a frozen archive at
  `06d32a5219b69d8182079843c79661aca98ad220` and is not kept in sync
- Phase 4 Blender closeout docs are merged on the self-contained mainline
- Phase 5 GIMP preflight docs, implementation, and closeout docs are merged on
  the self-contained mainline
- the Windows source-testing guide and results template are merged on the
  self-contained mainline
- the narrow Windows source-testing prep changes are merged on the
  self-contained mainline
- `dev/unified-assistant-self-contained` is now handoff-ready for broader
  source-based Windows functional testing from clean developer clones
- after initial Windows test results and any narrow follow-up fixes, the next
  new docs-first branch returns to:
  - `codex/unified-self-contained-release-docs`

## Freeze Tags

The demo baseline is frozen at:

- `demo/unified-assistant-freeze-2026-03-17` -> `ad31a8e92419557cda9e7e0eb560d18e1c065a54`
- `demo/unified-assistant-spec-freeze-2026-03-17` -> `78412914cacbad183db70cb76eaa541352a55c8c`

## Required Workflow

Phase 0 through Phase 2 used a temporary dual-mainline workflow to get the
self-contained line established cleanly.

Starting with Phase 3, all new self-contained work lands directly on
`dev/unified-assistant-self-contained` through this standard three-PR phase
sequence:

1. open `codex/<phase>-docs` from `origin/dev/unified-assistant-self-contained`
2. merge the docs-only preflight PR into `dev/unified-assistant-self-contained`
3. open `codex/<phase>` from updated `origin/dev/unified-assistant-self-contained`
4. merge the implementation PR into `dev/unified-assistant-self-contained`
5. open `codex/<phase>-status-docs` from updated `origin/dev/unified-assistant-self-contained`
6. merge the closeout docs PR into `dev/unified-assistant-self-contained`

No future self-contained PRs should target
`docs/unified-assistant-self-contained-spec`. That branch remains as an
archived reference snapshot of the dual-mainline period.

Never branch new self-contained work from the frozen demo branches.

## Reading Order

### New session

1. This README
2. [CURRENT_STATE.md](CURRENT_STATE.md)
3. [GIT_WORKFLOW.md](GIT_WORKFLOW.md)
4. [SELF_CONTAINED_PLAN.md](SELF_CONTAINED_PLAN.md)

### Architecture / backend work

1. [ARCHITECTURE.md](ARCHITECTURE.md)
2. [MCP_INTEGRATION.md](MCP_INTEGRATION.md)
3. [PACKAGING.md](PACKAGING.md)
4. [SETUP_SPEC.md](SETUP_SPEC.md)
5. [THIRD_PARTY_PROVENANCE.md](THIRD_PARTY_PROVENANCE.md)

### Runtime / model work

1. [MODEL_STRATEGY.md](MODEL_STRATEGY.md)
2. [PACKAGING.md](PACKAGING.md)
3. [RESOURCES.md](RESOURCES.md)
4. [LEARNINGS.md](LEARNINGS.md)

### Workflow / delivery work

1. [CURRENT_STATE.md](CURRENT_STATE.md)
2. [GIT_WORKFLOW.md](GIT_WORKFLOW.md)
3. [WINDOWS_SOURCE_TESTING.md](WINDOWS_SOURCE_TESTING.md)
4. [IMPLEMENTATION_PHASES.md](IMPLEMENTATION_PHASES.md)
5. [SETUP_SPEC.md](SETUP_SPEC.md)
6. [SELF_CONTAINED_PLAN.md](SELF_CONTAINED_PLAN.md)

`SETUP_SPEC.md` is intentionally cross-listed in both architecture and workflow
reading orders because it defines both the technical setup contract and the
phase-boundary rules for how that setup surface evolves.

### Historical carried-over references

These documents were carried forward from the frozen demo-spec line. They remain
useful as reference material, but they are not the primary drivers for the
self-contained roadmap phases.

1. [CODE_CONVENTIONS.md](CODE_CONVENTIONS.md)
2. [CODE_MODE_SPEC.md](CODE_MODE_SPEC.md)
3. [FRONTEND_SPEC.md](FRONTEND_SPEC.md)
4. [VSCODE_EXTENSION_SPEC.md](VSCODE_EXTENSION_SPEC.md)

## Document Index

| Document                                                                           | Purpose                                                                           |
| ---------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| [CURRENT_STATE.md](CURRENT_STATE.md)                                               | Frozen demo baseline, new mainlines, current gap to self-contained shipping       |
| [IMPLEMENTATION_PHASES.md](IMPLEMENTATION_PHASES.md)                               | Ordered self-contained delivery phases after the branch cut                       |
| [ARCHITECTURE.md](ARCHITECTURE.md)                                                 | Target architecture for owned runtimes, provisioning, and host-app orchestration  |
| [MCP_INTEGRATION.md](MCP_INTEGRATION.md)                                           | Mode-by-mode integration ownership, transports, and runtime supervision rules     |
| [PACKAGING.md](PACKAGING.md)                                                       | Packaged layout, bundled runtime rules, and Windows validation checklist          |
| [MODEL_STRATEGY.md](MODEL_STRATEGY.md)                                             | Bundled default model decision and future model packaging policy                  |
| [SETUP_SPEC.md](SETUP_SPEC.md)                                                     | App-level setup subsystem, public setup DTOs, setup commands, and Phase 2 limits  |
| [SELF_CONTAINED_PLAN.md](SELF_CONTAINED_PLAN.md)                                   | Master roadmap from demo baseline to externally usable self-contained app         |
| [THIRD_PARTY_PROVENANCE.md](THIRD_PARTY_PROVENANCE.md)                             | Pinned source, license, and modification tracking for imported third-party assets |
| [GIT_WORKFLOW.md](GIT_WORKFLOW.md)                                                 | Required branch policy for the self-contained line                                |
| [WINDOWS_SOURCE_TESTING.md](WINDOWS_SOURCE_TESTING.md)                             | Decision-complete Windows source-testing guide for the current functional gate    |
| [WINDOWS_SOURCE_TEST_RESULTS_TEMPLATE.md](WINDOWS_SOURCE_TEST_RESULTS_TEMPLATE.md) | Short reporting template for repeated Windows source-test runs                    |
| [LEARNINGS.md](LEARNINGS.md)                                                       | Cross-phase corrections and non-obvious productization gotchas                    |
| [RESOURCES.md](RESOURCES.md)                                                       | External repos, packaging tools, and upstream integration references              |
| [CODE_CONVENTIONS.md](CODE_CONVENTIONS.md)                                         | Shared coding standards carried forward from the demo line                        |
| [FRONTEND_SPEC.md](FRONTEND_SPEC.md)                                               | Shared shell/UI reference carried forward from the demo line                      |
| [CODE_MODE_SPEC.md](CODE_MODE_SPEC.md)                                             | Historical Code-mode reference from the demo line; not a self-contained driver    |
| [VSCODE_EXTENSION_SPEC.md](VSCODE_EXTENSION_SPEC.md)                               | Historical extension research reference from the demo line                        |

## Locked Decisions

| Area                  | Decision                                                                      |
| --------------------- | ----------------------------------------------------------------------------- |
| Host apps             | GIMP, Blender, and LibreOffice remain separately installed                    |
| External dependencies | No external Python, MCP server, plugin, addon, or model setup allowed         |
| Default bundled model | `qwen3-4b-instruct-2507`                                                      |
| Calc                  | Deferred and disabled                                                         |
| Bundle identifier     | `com.smolpc.codehelper` remains unchanged                                     |
| Shipping OS           | Windows only                                                                  |
| Python ownership      | Bundled app-private runtime                                                   |
| Blender integration   | Reuse existing repo addon source; provision automatically                     |
| GIMP integration      | Vendor pinned upstream `maorcc/gimp-mcp` snapshot and provision automatically |
| Provenance            | Mandatory before bundling imported third-party runtime assets                 |

## Current Phase

The current mainline-ready phase is still Phase 6 Release Packaging And
Validation, but the immediate gate before broader Phase 6 work is a dedicated
Windows source-based functional validation pass on the unified mainline.

Phase 5 is now complete on `dev/unified-assistant-self-contained`:

- the vendored `maorcc/gimp-mcp` snapshot is bundled under `apps/codehelper/src-tauri/resources/gimp/`
- setup now reports `gimp_plugin_runtime` separately from `host_gimp`
- `setup_prepare()` now provisions and repairs bundled GIMP assets without launching the interactive GIMP UI
- GIMP mode now validates the detected host version, provisions missing assets on demand, launches GIMP only when needed, and supervises the bundled bridge on `127.0.0.1:10008`
- Blender, LibreOffice, Code, and Calc behavior remained unchanged during Phase 5

The source-testing handoff is now merged on the mainline:

- the Windows testing runbook and results template are merged
- the narrow prep fixes are merged:
  - GIMP source mode now prefers prepared bundled Python, then repo `.venv`,
    then PATH Python in debug/source mode
  - Windows host detection now accepts `gimp-3.exe` during system lookup

Use these to start broader Windows testing:

- [WINDOWS_SOURCE_TESTING.md](WINDOWS_SOURCE_TESTING.md)
- [WINDOWS_SOURCE_TEST_RESULTS_TEMPLATE.md](WINDOWS_SOURCE_TEST_RESULTS_TEMPLATE.md)

This pass makes the branch ready for broader Windows testing; it does not
replace the actual tester results from separate Windows laptops.

After initial Windows testing and any narrow follow-up fixes, the branch queue
returns to:

- `codex/unified-self-contained-release-docs`

## Rule Of Thumb

If a change improves demo behavior only, it belongs on the frozen demo line only
if a real demo issue exists. If a change improves self-contained ownership,
provisioning, packaging, or first-run experience, it belongs exclusively on the
self-contained line.
