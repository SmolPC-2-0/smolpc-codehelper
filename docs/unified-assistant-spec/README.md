# SmolPC Unified Assistant -- Specification Index

> Read this document first. It defines the product shape, branch flow, and
> reading order for every session working on the unified assistant.

**Last Updated:** 2026-03-16
**Status:** Documentation baseline for the unified frontend

## Project Summary

SmolPC Unified Assistant is a **single Tauri 2 desktop app** for students on
budget Windows laptops. It provides six selectable modes inside one window:

- Code
- GIMP
- Blender
- Writer
- Calc
- Slides

`Slides` is the user-facing label for the internal mode id `impress`.

All modes share one local inference server: `smolpc-engine-host` on port
`19432`. The unified app owns the chat shell and mode switching. The launcher
is **not** part of the product architecture for this workstream.

## Locked Decisions

| Area | Decision |
|---|---|
| Product shell | One unified Tauri app |
| Code mode | Included in the unified app |
| Mode list | Code, GIMP, Blender, Writer, Calc, Slides |
| Engine ownership | Shared `smolpc-engine-host` only |
| Launcher | Not part of architecture or implementation path |
| Target OS | Windows primary |
| Implementation mainline | `dev/unified-assistant` |
| Spec mainline | `docs/unified-assistant-spec` |
| Migration strategy | Adapters first |
| Merge strategy | Strict isolation |

## Authoritative Branches

| Branch | Role |
|---|---|
| `docs/unified-assistant-spec` | Canonical architecture and product spec |
| `dev/unified-assistant` | Implementation integration mainline |
| `codex/*` | Narrow work branches created from one of the two branches above |

## Required Workflow

1. Update or correct the design on `docs/unified-assistant-spec`.
2. Merge the refreshed docs into `docs/unified-assistant-spec`.
3. Merge `docs/unified-assistant-spec` into `dev/unified-assistant`.
4. Create implementation branches from `dev/unified-assistant`.

No implementation branch should be created until the documentation baseline has
been merged into `dev/unified-assistant`.

## Reading Order

### New session

1. This README
2. [ARCHITECTURE.md](ARCHITECTURE.md)
3. [CURRENT_STATE.md](CURRENT_STATE.md)
4. [GIT_WORKFLOW.md](GIT_WORKFLOW.md)

### Frontend work

1. [ARCHITECTURE.md](ARCHITECTURE.md)
2. [FRONTEND_SPEC.md](FRONTEND_SPEC.md)
3. [CODE_MODE_SPEC.md](CODE_MODE_SPEC.md)
4. [CODE_CONVENTIONS.md](CODE_CONVENTIONS.md)

### Backend orchestration work

1. [ARCHITECTURE.md](ARCHITECTURE.md)
2. [MCP_INTEGRATION.md](MCP_INTEGRATION.md)
3. [PACKAGING.md](PACKAGING.md)
4. [CODE_CONVENTIONS.md](CODE_CONVENTIONS.md)

### Branching and rollout work

1. [CURRENT_STATE.md](CURRENT_STATE.md)
2. [GIT_WORKFLOW.md](GIT_WORKFLOW.md)
3. [IMPLEMENTATION_PHASES.md](IMPLEMENTATION_PHASES.md)

## Document Index

| Document | Purpose |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Final system architecture for the unified app, engine, and mode providers |
| [FRONTEND_SPEC.md](FRONTEND_SPEC.md) | UI shell, state model, and frontend contracts for all six modes |
| [CODE_MODE_SPEC.md](CODE_MODE_SPEC.md) | Exact definition of what current Codehelper behavior is preserved in Code mode |
| [MCP_INTEGRATION.md](MCP_INTEGRATION.md) | Tool-provider model and external integration behavior |
| [PACKAGING.md](PACKAGING.md) | Windows packaging and bundled resource requirements |
| [CURRENT_STATE.md](CURRENT_STATE.md) | Current baseline, next workstreams, and known risks |
| [GIT_WORKFLOW.md](GIT_WORKFLOW.md) | Docs-first branching and merge-safe implementation rules |
| [IMPLEMENTATION_PHASES.md](IMPLEMENTATION_PHASES.md) | Ordered implementation phases after the docs baseline is merged |
| [MODEL_STRATEGY.md](MODEL_STRATEGY.md) | Model and runtime strategy; still shared across all modes |
| [CODE_CONVENTIONS.md](CODE_CONVENTIONS.md) | Coding standards for Rust and Svelte |
| [VSCODE_EXTENSION_SPEC.md](VSCODE_EXTENSION_SPEC.md) | Historical/future-work note; not active for this plan |
| [LEARNINGS.md](LEARNINGS.md) | Session corrections and discoveries |
| [RESOURCES.md](RESOURCES.md) | External references and source repositories |

## Repository Direction

The implementation location for the unified product remains
`apps/codehelper/`. The current standalone apps are reference sources during
the port:

- `apps/gimp-assistant/`
- `apps/blender-assistant/`
- `apps/libreoffice-assistant/`

They continue to evolve independently while their capabilities are ported into
new adapter layers inside the unified app.

## Pending Decisions

These remain open after the documentation baseline:

- final model selection for target hardware
- final Blender secondary MCP layering scope
- exact Windows packaging details for third-party runtimes
- final acceptance criteria for the LibreOffice provider on school hardware

## Rule Of Thumb

If a contributor needs to decide between:

- modifying a standalone app directly, or
- porting the behavior into a new adapter inside the unified app

choose the adapter approach unless the standalone app itself has a bug that must
be fixed at the source first.
