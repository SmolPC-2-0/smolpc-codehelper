# Launcher Zone

Launcher is the app-suite entrypoint and app orchestration layer.

## Owned Responsibilities

- app listing/manifest conventions
- launch-or-focus orchestration
- engine readiness gating before app launch

## Current State

- Launcher foundation commands/orchestrator currently live in CodeHelper's Tauri backend at:
  - `apps/codehelper/src-tauri/src/launcher/`
  - `apps/codehelper/src-tauri/src/commands/launcher.rs`
- This directory is the extraction target for a standalone launcher app/repo.

## Next Extraction Goal

Move launcher runtime from embedded module form to this zone as an independently buildable app.
