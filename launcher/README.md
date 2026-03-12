# Launcher Zone

Launcher is the app-suite entrypoint and app orchestration layer.

## Owned Responsibilities

- app catalog + registry conventions
- launch-or-focus orchestration
- install / repair orchestration
- engine readiness gating before app launch

## Current State

- Launcher data sources:
  - Bundled catalog: `launcher/src-tauri/resources/launcher/apps.catalog.json`
  - Per-user registry: `%LOCALAPPDATA%/SmolPC/launcher/apps.registry.json`
- Installer-facing CLI helper (Rust binary):
  - `launcher-register register --app-id <id> --exe-path <abs> ...`
  - `launcher-register unregister --app-id <id>`
  - `launcher-register list`

## Next Extraction Goal

Move launcher runtime from embedded module form to this zone as an independently buildable app.
