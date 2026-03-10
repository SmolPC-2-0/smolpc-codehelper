# SmolPC Monorepo Architecture

This document is the current source of truth for repository organization and ownership.

## System Boundaries

- `engine/` owns:
  - startup lifecycle (`ensure_started`)
  - readiness state machine and status contract
  - backend/model detection and selection
  - inference execution and cancellation
- `apps/` own:
  - frontend UX, workflow, product logic
  - tool calling and app-specific orchestration
  - diagnostics display of engine contract fields
- `launcher/` owns:
  - app discovery/manifest
  - launch-or-focus orchestration
  - cross-app entry UX

## Zone Map

- `engine/crates/smolpc-engine-core`: runtime and model domain logic
- `engine/crates/smolpc-engine-host`: localhost API host
- `engine/crates/smolpc-engine-client`: typed client for app integrations
- `apps/codehelper`: current integrated app
- `apps/libreoffice-assistant`, `apps/gimp-assistant`, `apps/blender-assistant`: staged app roots
- `launcher`: launcher zone docs and extraction target

## Contract-First Rule

Apps must integrate through:

1. `smolpc-engine-client` (preferred for Rust/Tauri apps), or
2. documented HTTP contract (`docs/ENGINE_API.md`)

Apps must not depend on engine host internals or app-local inference implementations.

## Active Documentation Standard

For any behavior change PR:

1. Update relevant zone README.
2. Update API/contract docs if interface changed.
3. Update onboarding/migration docs if developer workflow changed.
4. Record architectural decisions in `docs/adr/` when boundaries or precedence rules change.

Older historical docs may still reference legacy paths; treat this file and zone READMEs as primary.
