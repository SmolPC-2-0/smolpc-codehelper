# Single-App Workflows

Use this guide when you want to work on, run, or deploy just one piece of the SmolPC system in isolation.

## See Also

- [ENGINE_STANDALONE.md](./ENGINE_STANDALONE.md) — running the shared engine by itself
- [APP_ONBOARDING_PLAYBOOK.md](./APP_ONBOARDING_PLAYBOOK.md) — onboarding a new app to the shared engine
- [UNIFIED_MODE_ONBOARDING.md](./UNIFIED_MODE_ONBOARDING.md) — adding a mode to the unified desktop shell

## Current App Roots

The repo has one primary app and two staged app roots. Only the primary app is actively developed and shipped.

### `app/` — Unified CodeHelper Shell (active)

This is the current production app. It is a Tauri 2 + Svelte 5 desktop application that bundles all mode experiences (Code, Blender, GIMP, Writer, Slides) into a single unified shell.

- **Frontend:** `app/src/`
- **Tauri backend:** `app/src-tauri/`
- **Dev command:** `cd app && npm run tauri:dev`
- **Frontend-only dev:** `cd app && npm run dev`
- **Lint/check:** `cd app && npm run check && npm run lint`

Connector logic for Blender, GIMP, and LibreOffice lives under `connectors/`, not inside the app root. The app imports connectors as crate dependencies.

### `apps/blender-assistant/` — Standalone Blender Helper (staged)

A placeholder for a future standalone Blender assistant app. Contains only a `src-tauri/` skeleton with a lockfile and generated files. No frontend code, no active development.

Do not build or run this root. It exists to reserve the directory structure for future standalone app extraction.

### `apps/codehelper/` — Legacy CodeHelper Root (staged)

A leftover from the pre-unification era when CodeHelper was one of several separate app roots under `apps/`. It contains old build artifacts, scripts, and temp files from debugging sessions.

The unified shell now lives at `app/`. Do not develop against `apps/codehelper/`. It is not part of the active build.

## Connector Crates

Connector crates under `connectors/` are self-contained Rust crates that own all tool/runtime integration for a specific host app. Each connector provides:

- A `ToolProvider` implementation
- An executor for tool calls
- Setup/detection logic for the host app
- Bundled resources (prompts, profiles, etc.)

| Crate | Host App | Notes |
|---|---|---|
| `connectors/blender/` | Blender | Bridge script, RAG, Python execution |
| `connectors/gimp/` | GIMP | Script-Fu transport, heuristic planner |
| `connectors/libreoffice/` | LibreOffice | Writer + Impress profiles via single provider family |

Connectors are consumed by the unified shell (`app/`) as crate dependencies. They do not run standalone.

## Engine-Only Workflow

To work on just the inference engine without running the Tauri app:

```bash
# From repo root
cargo run -p smolpc-engine-host
```

See [ENGINE_STANDALONE.md](./ENGINE_STANDALONE.md) for full details on token auth, environment overrides, and curl testing.

## CodeHelper App-Only Workflow

To work on just the CodeHelper app (frontend + Tauri shell) without modifying the engine or connectors:

```bash
# Frontend only (no Tauri, no engine)
cd app && npm run dev

# Full app with engine lifecycle
cd app && npm run tauri:dev

# DirectML-forced mode
cd app && npm run tauri:dml
```

Pre-commit checks for app-only changes:

```bash
cd app && npm run check && npm run lint
```

If your changes touch connector crates or Tauri backend code, also run:

```bash
cargo check --workspace && cargo clippy --workspace
```

## Connector-Only Workflow

To work on a single connector crate without running the full app:

```bash
# Check a specific connector compiles
cargo check -p smolpc-connector-blender
cargo check -p smolpc-connector-gimp
cargo check -p smolpc-connector-libreoffice

# Run tests for a connector (if present)
cargo test -p smolpc-connector-blender
```

To test a connector end-to-end, you still need to run the full app (`cd app && npm run tauri:dev`) because connectors are invoked through the unified shell's mode routing.

## Which Root to Use

| I want to... | Use |
|---|---|
| Develop or ship the desktop app | `app/` |
| Work on Blender/GIMP/LibreOffice integration | `connectors/<name>/` + test via `app/` |
| Work on the inference engine | `engine/` (see [ENGINE_STANDALONE.md](./ENGINE_STANDALONE.md)) |
| Add a new mode to the unified shell | `app/` + maybe `connectors/` (see [UNIFIED_MODE_ONBOARDING.md](./UNIFIED_MODE_ONBOARDING.md)) |
| Onboard a new external app | `apps/<name>/` (see [APP_ONBOARDING_PLAYBOOK.md](./APP_ONBOARDING_PLAYBOOK.md)) |
