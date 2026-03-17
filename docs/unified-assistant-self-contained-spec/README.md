# SmolPC Unified Assistant Self-Contained Spec Index

> Read this document first. It defines the branch policy, frozen demo baseline,
> and document map for the self-contained delivery line.

**Last Updated:** 2026-03-17
**Status:** Single-mainline self-contained workflow active; Phase 3 docs preflight next

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

## Freeze Tags

The demo baseline is frozen at:

- `demo/unified-assistant-freeze-2026-03-17` -> `ad31a8e92419557cda9e7e0eb560d18e1c065a54`
- `demo/unified-assistant-spec-freeze-2026-03-17` -> `78412914cacbad183db70cb76eaa541352a55c8c`

## Required Workflow

Phase 0 through Phase 2 used a temporary dual-mainline workflow to get the
self-contained line established cleanly.

Starting with Phase 3, all new self-contained work lands directly on
`dev/unified-assistant-self-contained` in this three-PR sequence:

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
3. [IMPLEMENTATION_PHASES.md](IMPLEMENTATION_PHASES.md)
4. [SETUP_SPEC.md](SETUP_SPEC.md)
5. [SELF_CONTAINED_PLAN.md](SELF_CONTAINED_PLAN.md)

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

| Document                                               | Purpose                                                                           |
| ------------------------------------------------------ | --------------------------------------------------------------------------------- |
| [CURRENT_STATE.md](CURRENT_STATE.md)                   | Frozen demo baseline, new mainlines, current gap to self-contained shipping       |
| [IMPLEMENTATION_PHASES.md](IMPLEMENTATION_PHASES.md)   | Ordered self-contained delivery phases after the branch cut                       |
| [ARCHITECTURE.md](ARCHITECTURE.md)                     | Target architecture for owned runtimes, provisioning, and host-app orchestration  |
| [MCP_INTEGRATION.md](MCP_INTEGRATION.md)               | Mode-by-mode integration ownership, transports, and runtime supervision rules     |
| [PACKAGING.md](PACKAGING.md)                           | Packaged layout, bundled runtime rules, and Windows validation checklist          |
| [MODEL_STRATEGY.md](MODEL_STRATEGY.md)                 | Bundled default model decision and future model packaging policy                  |
| [SETUP_SPEC.md](SETUP_SPEC.md)                         | App-level setup subsystem, public setup DTOs, setup commands, and Phase 2 limits  |
| [SELF_CONTAINED_PLAN.md](SELF_CONTAINED_PLAN.md)       | Master roadmap from demo baseline to externally usable self-contained app         |
| [THIRD_PARTY_PROVENANCE.md](THIRD_PARTY_PROVENANCE.md) | Pinned source, license, and modification tracking for imported third-party assets |
| [GIT_WORKFLOW.md](GIT_WORKFLOW.md)                     | Required branch policy for the self-contained line                                |
| [LEARNINGS.md](LEARNINGS.md)                           | Cross-phase corrections and non-obvious productization gotchas                    |
| [RESOURCES.md](RESOURCES.md)                           | External repos, packaging tools, and upstream integration references              |
| [CODE_CONVENTIONS.md](CODE_CONVENTIONS.md)             | Shared coding standards carried forward from the demo line                        |
| [FRONTEND_SPEC.md](FRONTEND_SPEC.md)                   | Shared shell/UI reference carried forward from the demo line                      |
| [CODE_MODE_SPEC.md](CODE_MODE_SPEC.md)                 | Historical Code-mode reference from the demo line; not a self-contained driver    |
| [VSCODE_EXTENSION_SPEC.md](VSCODE_EXTENSION_SPEC.md)   | Historical extension research reference from the demo line                        |

## Locked Decisions

| Area                  | Decision                                                               |
| --------------------- | ---------------------------------------------------------------------- |
| Host apps             | GIMP, Blender, and LibreOffice remain separately installed             |
| External dependencies | No external Python, MCP server, plugin, addon, or model setup allowed  |
| Default bundled model | `qwen3-4b-instruct-2507`                                               |
| Calc                  | Deferred and disabled                                                  |
| Bundle identifier     | `com.smolpc.codehelper` remains unchanged                              |
| Shipping OS           | Windows only                                                           |
| Python ownership      | Bundled app-private runtime                                            |
| Blender integration   | Reuse existing repo addon source; provision automatically              |
| GIMP integration      | Vendor pinned upstream `gimp-mcp` snapshot and provision automatically |
| Provenance            | Mandatory before bundling imported third-party runtime assets          |

## Current Phase

The current active docs-first phase is Phase 3 LibreOffice self-contained
runtime preflight:

- keep the new single-mainline workflow rigorous while removing docs-sync PRs
- switch Writer and Slides packaged-mode runtime ownership onto bundled Python
- keep Calc scaffold-only
- keep GIMP and Blender provisioning work deferred to later phases

## Rule Of Thumb

If a change improves demo behavior only, it belongs on the frozen demo line only
if a real demo issue exists. If a change improves self-contained ownership,
provisioning, packaging, or first-run experience, it belongs exclusively on the
self-contained line.
