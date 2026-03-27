# Separation Guide

SmolPC Code Helper is a monorepo, but it is designed so that components can be extracted and recombined. The engine runs as a standalone HTTP server. Connectors are self-contained crates. The desktop app imports everything but does not need to.

This guide covers how to build subsets of the system: Code Helper without connectors, a single-connector app, or the engine as an independent service.

## Workspace Structure

The workspace contains 11 crates plus a standalone TTS sidecar:

```
Cargo.toml (workspace root)
├── app/src-tauri/                          smolpc-desktop (Tauri app binary)
├── crates/smolpc-assistant-types/          Shared type definitions (leaf crate)
├── crates/smolpc-connector-common/         Connector infrastructure
├── crates/smolpc-mcp-client/              MCP client library
├── connectors/blender/                     Blender connector
├── connectors/gimp/                        GIMP connector
├── connectors/libreoffice/                 LibreOffice connector
├── engine/crates/smolpc-engine-core/       Engine foundation (no workspace deps)
├── engine/crates/smolpc-engine-host/       Engine HTTP server binary
├── engine/crates/smolpc-engine-client/     Engine consumer library
├── engine/crates/smolpc-engine-benchmark/  Benchmarking CLI
└── engine/crates/smolpc-tts-server/        TTS sidecar (own workspace)
```

`smolpc-tts-server` has its own `[workspace]` in its `Cargo.toml` due to an `ort` crate version conflict with the main workspace. It must be built with `--manifest-path`, not `-p`.

## Dependency Graph

```
smolpc-assistant-types (no dependencies)
        │
        ▼
smolpc-engine-core (no workspace dependencies)
        │
        ├──────────────────────┐
        ▼                      ▼
smolpc-engine-client    smolpc-engine-host (binary)
        │
        ├───────────────────────────────────┐
        ▼                                   ▼
smolpc-connector-common              smolpc-engine-benchmark
        │
        ├──────────────┬────────────────────┐
        ▼              ▼                    ▼
smolpc-connector-  smolpc-connector-  smolpc-connector-
  blender            gimp              libreoffice
        │              │                    │
        └──────────────┴────────────────────┘
                       │
                       ▼
              smolpc-desktop (Tauri app binary)
```

Key properties:

- `smolpc-assistant-types` is the leaf — zero dependencies, safe to import anywhere
- `smolpc-engine-core` has no workspace dependencies — it is the foundation
- `smolpc-engine-host` only depends on `smolpc-engine-core` — it does not import connectors
- Each connector depends on `smolpc-connector-common`, which depends on `smolpc-engine-client` and `smolpc-engine-core`
- `smolpc-desktop` is the top-level binary that imports everything

## Building Code Helper Only (No Connectors)

To produce a Code Helper app that only has the Code mode (no Blender, GIMP, or LibreOffice):

### 1. Remove connector workspace members

In the root `Cargo.toml`, remove:

```toml
# Remove these lines:
"connectors/blender",
"connectors/gimp",
"connectors/libreoffice",
"crates/smolpc-mcp-client",
```

### 2. Remove connector dependencies from the desktop app

In `app/src-tauri/Cargo.toml`, remove the dependencies on `smolpc-connector-blender`, `smolpc-connector-gimp`, `smolpc-connector-libreoffice`, and `smolpc-mcp-client`.

### 3. Simplify ModeProviderRegistry

In `app/src-tauri/src/modes/registry.rs`:

- Remove `ProviderFamily::Gimp`, `ProviderFamily::Blender`, `ProviderFamily::LibreOffice`
- Remove the `gimp`, `blender`, `libreoffice` fields from `ModeProviderRegistry`
- In `provider_for_mode`, make `AppMode::Gimp`, `AppMode::Blender`, `AppMode::Writer`, `AppMode::Impress` return the Code provider (or remove them)

### 4. Simplify mode config

In `app/src-tauri/src/modes/config.rs`, remove or comment out the `AppMode::Gimp`, `AppMode::Blender`, `AppMode::Writer`, and `AppMode::Impress` entries from `list_mode_configs()`.

### 5. Remove connector resources from bundle

In `app/src-tauri/tauri.conf.json`, remove the connector resource mappings:

```json
// Remove these:
"../../connectors/gimp/resources/": "gimp/",
"../../connectors/blender/resources/": "blender/",
"../../connectors/libreoffice/resources/": "libreoffice/"
```

### 6. Remove connector setup items

In `app/src-tauri/src/setup/status.rs`, remove the host app detection calls and addon/plugin setup items for Blender, GIMP, and LibreOffice.

The result is a smaller binary that only includes the engine and the Code mode chat interface.

## Building a Single-Connector App

To build an app with only one connector (e.g., GIMP-only):

### 1. Keep only the required crates

Workspace members:

```toml
members = [
  "app/src-tauri",
  "crates/smolpc-assistant-types",
  "crates/smolpc-connector-common",
  "crates/smolpc-mcp-client",          # needed by GIMP connector
  "connectors/gimp",
  "engine/crates/smolpc-engine-core",
  "engine/crates/smolpc-engine-host",
  "engine/crates/smolpc-engine-client",
]
```

### 2. Simplify ModeProviderRegistry

Keep only `Code` and `Gimp` in the registry. The mode config list should return only those two modes.

### 3. Bundle only the needed resources

In `tauri.conf.json`, keep only the GIMP connector resources:

```json
"../../connectors/gimp/resources/": "gimp/"
```

### 4. Adjust setup

Remove Blender and LibreOffice from host app detection and setup item collection.

The same pattern applies for a Blender-only or LibreOffice-only build — swap which connector crate and resources are included.

## Engine as Standalone Service

The engine already works as a standalone service. No changes are needed.

### Running standalone

```bash
cargo run -p smolpc-engine-host
```

Set the auth token:

```powershell
$env:SMOLPC_ENGINE_TOKEN = "my-dev-token"
cargo run -p smolpc-engine-host
```

The engine listens on `localhost:19432` and accepts any HTTP client. See [ENGINE_API.md](ENGINE_API.md) for the full endpoint reference.

### Consuming from Rust

The `smolpc-engine-client` crate provides:

- `spawn_engine()` — launch the engine as a detached process
- `wait_for_healthy()` — poll until ready
- `EngineClient` — HTTP client with streaming SSE support
- `RuntimeModePreference` — override backend selection (Auto, Cpu, Dml, Npu)

Add to your `Cargo.toml`:

```toml
smolpc-engine-client = { path = "engine/crates/smolpc-engine-client" }
smolpc-engine-core = { path = "engine/crates/smolpc-engine-core" }
```

### Consuming from any language

The engine exposes a standard HTTP API. Any language with HTTP client support can consume it:

```bash
curl -s http://localhost:19432/engine/status \
  -H "Authorization: Bearer my-dev-token"

curl -s http://localhost:19432/v1/chat/completions \
  -H "Authorization: Bearer my-dev-token" \
  -H "Content-Type: application/json" \
  -d '{"messages":[{"role":"user","content":"Hello"}],"stream":true}'
```

### Engine-only workspace

For a minimal workspace that only includes the engine:

```toml
[workspace]
members = [
  "crates/smolpc-assistant-types",
  "engine/crates/smolpc-engine-core",
  "engine/crates/smolpc-engine-host",
  "engine/crates/smolpc-engine-client",
]
```

This compiles the engine server and client library without any Tauri, connector, or frontend dependencies.

## Offline Bundle Variants

The build system produces three pre-built offline bundle variants:

| Bundle | Contents | Target |
|---|---|---|
| Full (`usb/`) | Qwen 2.5 1.5B + Qwen 3 4B (all backends) | Mixed hardware, 8 GB+ USB |
| Lite (`qwen2.5-1.5b-instruct/`) | Qwen 2.5 1.5B only | 4 GB+ USB, 8 GB RAM machines |
| Standard (`qwen3-4b/`) | Qwen 3 4B only | 8 GB+ USB, 16 GB+ RAM machines |

Each bundle contains:

```
<bundle>/
  SmolPC Code Helper_x.x.x_x64-setup.exe   # NSIS installer
  Install-CodeHelper.cmd                    # Entry point (double-click)
  Install-CodeHelper.ps1                    # PowerShell orchestrator
  models/
    model-archives.json                     # Manifest (version 1)
    SHA256SUMS.txt                          # Combined checksums
    Install-Models.ps1                      # Model extraction script
    *.zip                                   # Per-backend model archives
```

To create a custom bundle variant, modify the model list passed to `provision_models` or build a custom `model-archives.json` that includes only the desired entries.

## NSIS Installer Customization

The installer is configured in `app/src-tauri/tauri.conf.json`:

```json
"bundle": {
  "targets": ["nsis"],
  "resources": {
    "libs/": "libs/",
    "resources/models/": "models/",
    "resources/python/": "python/",
    "../../connectors/gimp/resources/": "gimp/",
    "../../connectors/blender/resources/": "blender/",
    "../../connectors/libreoffice/resources/": "libreoffice/"
  }
}
```

To exclude a connector's resources, remove its entry from `bundle.resources`. To exclude the bundled Python runtime, remove the `resources/python/` entry (connectors that need Python will fall back to system Python or fail with a clear error).

The NSIS hooks file (`app/src-tauri/nsis/hooks.nsh`) handles:

- **Pre-install**: VC++ Redistributable detection and silent install
- **Post-install**: Creates `%LOCALAPPDATA%\SmolPC 2.0\` and writes `installer-source.txt` breadcrumb

## Key Files Reference

| File | Purpose |
|---|---|
| `Cargo.toml` | Workspace member list |
| `app/src-tauri/Cargo.toml` | Desktop app dependencies |
| `app/src-tauri/tauri.conf.json` | Bundle targets, resources, NSIS config |
| `app/src-tauri/nsis/hooks.nsh` | NSIS pre/post-install hooks |
| `app/src-tauri/src/modes/registry.rs` | Mode-to-provider dispatch |
| `app/src-tauri/src/modes/config.rs` | Mode configuration list |
| `app/src-tauri/src/setup/status.rs` | Setup item collection |
| `app/scripts/build-windows-local-bundle.ps1` | 5-phase build pipeline |
