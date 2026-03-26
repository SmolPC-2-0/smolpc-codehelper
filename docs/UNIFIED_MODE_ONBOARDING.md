# Unified Mode Onboarding Guide

Use this guide when you are adding a mode to the current unified SmolPC desktop app shell on `main`.

This is for contributors changing the integrated app under `app/`. If you are onboarding a separate app or helper to the shared engine contract, start with [APP_ONBOARDING_PLAYBOOK.md](./APP_ONBOARDING_PLAYBOOK.md) instead.

For general repo conventions and contribution workflow, also read [CONTRIBUTING.md](./CONTRIBUTING.md).

## Current Main Layout

- the active unified shell lives under `app/`
- connector implementations live under `connectors/`
- the shared provider trait lives in [crates/smolpc-connector-common/src/provider.rs](../crates/smolpc-connector-common/src/provider.rs)

## Start With The Layering

On current `main`, a unified mode is split across a few layers:

1. Shared mode id contract:
   - [crates/smolpc-assistant-types/src/mode.rs](../crates/smolpc-assistant-types/src/mode.rs)
2. Backend mode list and config returned by `list_modes`:
   - [app/src-tauri/src/modes/config.rs](../app/src-tauri/src/modes/config.rs)
3. Backend provider-family routing:
   - [app/src-tauri/src/modes/registry.rs](../app/src-tauri/src/modes/registry.rs)
4. Backend mode commands:
   - [app/src-tauri/src/commands/modes.rs](../app/src-tauri/src/commands/modes.rs)
   - [app/src-tauri/src/commands/assistant.rs](../app/src-tauri/src/commands/assistant.rs)
5. Frontend mode fallback and active-mode state:
   - [app/src/lib/types/mode.ts](../app/src/lib/types/mode.ts)
   - [app/src/lib/stores/mode.svelte.ts](../app/src/lib/stores/mode.svelte.ts)

Important current behavior:

- backend `list_modes` is the primary source of truth for mode configs
- frontend fallback config exists only so the UI still renders when `list_modes` is unavailable
- tool-backed modes route through `assistant_send`
- direct chat modes use the engine chat path in [app/src/App.svelte](../app/src/App.svelte)

## Choose The Onboarding Shape

Most mode work on current `main` falls into one of these three cases.

### Case 1: Add A New Mode Id / Direct-Chat Mode

Use this when the mode is mostly a new identity around direct engine chat, not a tool-calling provider flow.

Required updates:

1. Add the new `AppMode` enum variant in [crates/smolpc-assistant-types/src/mode.rs](../crates/smolpc-assistant-types/src/mode.rs).
2. Add the backend `ModeConfigDto` entry in [app/src-tauri/src/modes/config.rs](../app/src-tauri/src/modes/config.rs).
3. Add the frontend fallback entry in [app/src/lib/types/mode.ts](../app/src/lib/types/mode.ts).
4. Add any mode-specific UI copy or labels that depend on a hardcoded mode map.

Current direct-chat split:

- `code` does **not** use `assistant_send`
- [app/src/App.svelte](../app/src/App.svelte) sends `code` through the direct engine generation path
- tool-backed modes use `assistantSend(...)` and stream provider events instead

If your new mode should behave like `code`, document that clearly and update the send-path branching in [app/src/App.svelte](../app/src/App.svelte).

Naming reminder:

- enum ids and user-facing labels do not need to match exactly
- current example: `AppMode::Impress` is presented to users as `Slides`

### Case 2: Add A Mode That Reuses An Existing Provider Family

Use this when the mode is a new shell identity, but it should share an existing connector/provider implementation.

Current example:

- `Writer` and `Impress` are separate mode ids
- both map to one LibreOffice provider family
- mode-specific differences live in [connectors/libreoffice/src/profiles.rs](../connectors/libreoffice/src/profiles.rs)

For this shape, update:

1. Shared mode id in [crates/smolpc-assistant-types/src/mode.rs](../crates/smolpc-assistant-types/src/mode.rs).
2. Backend mode config in [app/src-tauri/src/modes/config.rs](../app/src-tauri/src/modes/config.rs).
3. Existing provider-family mapping in [app/src-tauri/src/modes/registry.rs](../app/src-tauri/src/modes/registry.rs).
4. Any profile or allowlist logic inside the connector crate.
5. Host-launch and status behavior in [app/src-tauri/src/commands/modes.rs](../app/src-tauri/src/commands/modes.rs).
6. Frontend fallback config and any mode-specific UI copy maps.

What to copy from the LibreOffice pattern:

- mode-specific labels, subtitles, suggestions, and allowed tools can live in a profile table
- the app backend can keep one provider family while exposing multiple user-facing modes
- host launch often still branches per mode, even when the provider family is shared

Do not stop at mode config alone. Shared-family modes usually need updates in:

- host app launch routing
- setup-to-mode availability gating
- help/welcome copy maps
- any recommended-model or mode-specific notice maps in the app shell

### Case 3: Add A Brand-New Provider Family

Use this when the mode needs a new tool/runtime integration instead of reusing `Code`, `Gimp`, `Blender`, or `LibreOffice`.

On current `main`, this means a new connector crate under `connectors/`, not a new provider file under `app/src-tauri/src/modes/`.

Required updates:

1. Add the new `AppMode` enum variant.
2. Add a new connector crate under `connectors/<name>/`.
3. Export the provider and any executor/planner types from that crate’s `lib.rs`.
4. Add the connector crate dependency to [app/src-tauri/Cargo.toml](../app/src-tauri/Cargo.toml).
5. Extend [app/src-tauri/src/modes/registry.rs](../app/src-tauri/src/modes/registry.rs):
   - add a new `ProviderFamily`
   - add a registry field
   - instantiate the provider in `ModeProviderRegistry::new(...)`
   - map the new mode to that family
6. Extend [app/src-tauri/src/commands/assistant.rs](../app/src-tauri/src/commands/assistant.rs) so the new mode actually executes through the correct connector flow.
7. Add backend mode config and frontend fallback config.

For host-backed families, also expect setup work:

- shared setup item ids and host detection often live in `crates/smolpc-connector-common`
- app setup aggregation lives under `app/src-tauri/src/setup/`
- mode availability and host-launch labels are also mirrored in [app/src/App.svelte](../app/src/App.svelte)

## Real Current Touch Points

These are the places contributors most often miss on current `main`.

### Shared Contract

- [crates/smolpc-assistant-types/src/mode.rs](../crates/smolpc-assistant-types/src/mode.rs)
  - add or update `AppMode`
  - keep DTO shape aligned across frontend/backend

### App Backend

- [app/src-tauri/src/modes/config.rs](../app/src-tauri/src/modes/config.rs)
  - mode list returned by `list_modes`
  - labels, subtitles, icons, suggestions, provider kind, capabilities
- [app/src-tauri/src/modes/registry.rs](../app/src-tauri/src/modes/registry.rs)
  - provider-family mapping
  - provider instantiation
- [app/src-tauri/src/commands/assistant.rs](../app/src-tauri/src/commands/assistant.rs)
  - tool-backed execution routing
- [app/src-tauri/src/commands/modes.rs](../app/src-tauri/src/commands/modes.rs)
  - mode status
  - tool refresh
  - open-host-app behavior
- [app/src-tauri/src/lib.rs](../app/src-tauri/src/lib.rs)
  - managed state construction and command registration

### Frontend

- [app/src/lib/types/mode.ts](../app/src/lib/types/mode.ts)
  - fallback mode config only
- [app/src/lib/stores/mode.svelte.ts](../app/src/lib/stores/mode.svelte.ts)
  - active mode
  - status loading
  - persistence
- [app/src/App.svelte](../app/src/App.svelte)
  - direct chat vs `assistantSend` split
  - setup-to-mode availability maps
  - host-launch labels
  - recommended-model maps and other mode-specific shell constants
- [app/src/lib/components/chat/WelcomeState.svelte](../app/src/lib/components/chat/WelcomeState.svelte)
  - mode-specific empty-state copy
- [app/src/lib/components/ModeHelpDrawer.svelte](../app/src/lib/components/ModeHelpDrawer.svelte)
  - mode-specific help content

### Setup / Host-Backed Integration

- [app/src-tauri/src/setup/status.rs](../app/src-tauri/src/setup/status.rs)
  - host detection items
  - provider-owned repair state
- [app/src-tauri/src/setup/provision.rs](../app/src-tauri/src/setup/provision.rs)
  - provisioning hooks for connector-owned setup
- [crates/smolpc-connector-common/src/lib.rs](../crates/smolpc-connector-common/src/lib.rs)
  - shared setup item ids such as `host_gimp`, `host_blender`, and `host_libreoffice`

If your new mode talks to a host app, check all three layers:

1. connector/provider behavior
2. setup/status/provisioning
3. frontend availability and host-launch UX

## Recommended Workflow

When adding a new mode on current `main`, work in this order:

1. Decide which onboarding shape you are implementing.
2. Add the shared mode id.
3. Update backend mode config and provider-family routing.
4. Update command routing or connector wiring if needed.
5. Update frontend fallback config and mode-specific UI maps.
6. Update setup/host-launch wiring for host-backed modes.
7. Verify `list_modes`, `mode_status`, and message-send behavior all match the intended mode shape.

## Verification Checklist

Before you call a mode onboarding change done, verify the actual runtime path as well as the file wiring:

1. `list_modes` returns the new mode with the expected id, label, and capability metadata.
2. `mode_status` resolves for the new mode without falling back to a misleading generic state.
3. Sending a message reaches the intended execution path:
   - direct engine chat for `code`-style modes
   - `assistant_send` plus the intended provider family for tool-backed modes
4. Host-backed modes stay unavailable until the required setup items pass, then become available without extra manual toggles.
5. `open_host_app` and any mode-specific host-launch labels still point to the intended desktop app.
6. The empty state, help drawer, and other mode-specific shell copy maps do not still assume the previous mode list.

## Definition Of Done

A unified mode onboarding change is complete when:

1. The mode exists in shared contract, backend config, and frontend fallback config.
2. The mode resolves to the intended execution path:
   - direct engine chat, or
   - tool-backed assistant/provider flow
3. Host-backed modes report correct status, host-launch behavior, and setup availability.
4. Mode-specific UI text and shell maps do not still assume the old set of modes.
5. The change can be understood without reverse-engineering the older pre-unification `apps/codehelper/...` layout.
