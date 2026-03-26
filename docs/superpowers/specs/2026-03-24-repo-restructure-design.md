# Repository Restructure: Connector-First Architecture

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restructure the monorepo so that the product identity ("SmolPC 2.0"), the app, and the three connectors (Blender, GIMP, LibreOffice) are immediately visible from the root directory listing.

**Decision record:** Single app with modes that load connectors. No separate apps. Product name: "SmolPC 2.0".

---

## 0. Review Amendments (resolved blockers)

### 0a. Dev-mode resource resolution — option (d): per-connector resource roots

**Problem:** `app_paths.rs` validates dev-mode by checking that `CARGO_MANIFEST_DIR/resources` contains at least one of `["python", "models", "gimp", "blender", "libreoffice"]`. After extraction, `gimp/`, `blender/`, `libreoffice/` won't be there.

**Solution:** Change `KNOWN_BUNDLED_RESOURCE_ROOTS` to only contain app-owned resources. Connector resources are resolved by each connector crate independently.

```rust
// app_paths.rs — AFTER
const KNOWN_BUNDLED_RESOURCE_ROOTS: [&str; 2] = ["python", "models"];
```

Each connector already receives `resource_dir: Option<PathBuf>` at construction and probes for its own resources with dev-fallback candidates:
- `resource_dir.join("blender")` (production: Tauri bundle maps connector resources here)
- `env!("CARGO_MANIFEST_DIR")/resources/blender` (legacy dev path, still works if symlinked)

In production builds, Tauri's `bundle.resources` maps `../../connectors/X/resources/ → X/` so the connector resources appear at the same paths they do today. In dev mode, the Tauri dev server handles the resource mapping via `tauri.conf.json` — no code change needed beyond trimming the validation constant.

The connector resource resolution code in each connector's `resources.rs` / `provider.rs` already has multi-candidate fallback logic. No new abstraction needed.

### 0b. sysinfo version — promote to workspace dependency

**Problem:** App pins `sysinfo = "=0.32.1"` with an exact version. `launch.rs` (moving to connector-common) also needs sysinfo.

**Solution:** Promote to `[workspace.dependencies]` in root `Cargo.toml`:
```toml
sysinfo = { version = "=0.32.1" }
```
Both `app/src-tauri/Cargo.toml` and `crates/smolpc-connector-common/Cargo.toml` use `sysinfo.workspace = true`.

### 0c. Executor signatures — concrete CancellationToken changes

All three executor functions change their `state` parameter:

```rust
// BEFORE (all three executors):
pub async fn execute_blender_request<F>(
    ...,
    state: &AssistantState,
    ...
)

// AFTER:
pub async fn execute_blender_request<F>(
    ...,
    cancel: &dyn CancellationToken,
    ...
)
```

Same pattern for `execute_gimp_request` and `execute_libreoffice_request`.

### 0d. MockCancellationToken for connector tests

Tests in extracted crates call `state.mark_cancelled()` which isn't on the `CancellationToken` trait. Add a test utility to connector-common:

```rust
// In smolpc-connector-common, #[cfg(test)] or behind a "test-utils" feature:
pub struct MockCancellationToken {
    cancelled: std::sync::atomic::AtomicBool,
}

impl MockCancellationToken {
    pub fn new() -> Self {
        Self { cancelled: AtomicBool::new(false) }
    }
    pub fn mark_cancelled(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

impl CancellationToken for MockCancellationToken {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}
```

Used at: `blender/executor.rs:423`, `libreoffice/executor.rs:902`, `text_generation.rs:119`.

### 0e. MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION — complete consumer list

In addition to the connector providers, these app-side files also consume the constant:
- `modes/code.rs:4` — `use crate::assistant::MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION`
- `commands/assistant.rs:2` — same

After move: both import from `smolpc_connector_common::MODE_UNDO_NOT_SUPPORTED`. Added to Section 5 rewiring table.

### 0f. Additional script/config path updates

| File | Reference | Change |
|------|-----------|--------|
| `scripts/build-windows-local-bundle.ps1:195` | `"smolpc-code-helper.exe"` | → `"smolpc-desktop.exe"` |
| `scripts/run-tauri-dev.ps1:54,59` | `"smolpc-code-helper"` | → `"smolpc-desktop"` |
| `.github/workflows/ci.yml:24` | `--workspace apps/codehelper` | → `--workspace app` |
| `.github/workflows/ci.yml:133-140` | `apps/codehelper/*` filter | → `app/*` |

### 0g. TextStreamer error message — make generic

`text_generation.rs:95` hardcodes `"Blender generation failed: {message}"`. Change to `"Text generation failed: {message}"` during extraction since this is now shared infrastructure.

### 0h. Tauri identifier — keep unchanged

Changing `com.smolpc.codehelper` to `com.smolpc.desktop` would break NSIS upgrade detection. **Keep the identifier as `com.smolpc.codehelper`** — only the `productName` changes to "SmolPC 2.0". This preserves upgrade path for existing installs.

---

## 1. Target Directory Structure

```
smolpc-codehelper/                 # repo root (GitHub repo name unchanged)
  app/                             # THE desktop application
    src/                           # Svelte 5 frontend (unchanged internally)
    src-tauri/
      src/
        engine/                    # EngineSupervisor, handle (unchanged)
        commands/                  # Tauri command handlers (imports updated)
        assistant/                 # AssistantState (implements CancellationToken)
        security/                  # Token validation (unchanged)
        hardware/                  # Hardware detection wrappers (unchanged)
        setup/                     # App-level setup orchestration (trimmed)
        modes/                     # Slim: code.rs, config.rs, registry.rs
        app_paths.rs
        lib.rs
        main.rs
      resources/
        launcher/                  # App launcher manifest
        model-installer/           # Model installer
        python/                    # Shared Python runtime
        models/                    # Model manifest
      Cargo.toml                   # package: smolpc-desktop
      tauri.conf.json              # productName: "SmolPC 2.0"
    package.json
    vite.config.ts
    tsconfig.*.json

  connectors/
    blender/                       # crate: smolpc-connector-blender
      src/
        lib.rs
        provider.rs                # BlenderProvider (implements ToolProvider)
        executor.rs                # execute_blender_request
        bridge.rs                  # Blender socket bridge + internal axum server
        prompts.rs                 # Scene-aware system prompts
        rag.rs                     # Blender-docs indexing/retrieval
        response.rs                # Result formatting
        state.rs                   # Scene state caching
        setup.rs                   # Addon provisioning (from app/setup/blender.rs)
      resources/                   # Blender addon, RAG data
        addon/
        rag_system/
      Cargo.toml
    gimp/                          # crate: smolpc-connector-gimp
      src/
        lib.rs
        provider.rs                # GimpProvider (implements ToolProvider)
        executor.rs                # execute_gimp_request
        heuristics.rs              # Image post-processing
        macros.rs                  # Macro recording/replay
        planner.rs                 # Pre-execution planning
        response.rs                # Result formatting
        runtime.rs                 # GIMP bridge process lifecycle
        transport.rs               # Message passing
        setup.rs                   # Plugin provisioning (from app/setup/gimp.rs)
      resources/                   # GIMP plugin, bridge scripts
        bridge/
        plugin/
        upstream/
      Cargo.toml
    libreoffice/                   # crate: smolpc-connector-libreoffice
      src/
        lib.rs
        provider.rs                # LibreOfficeProvider (implements ToolProvider)
        executor.rs                # execute_libreoffice_request
        profiles.rs                # Per-app config (Writer, Calc, Impress)
        resources.rs               # MCP server bundling + path resolution
        response.rs                # Result formatting
        runtime.rs                 # LibreOffice MCP server lifecycle
        state.rs                   # Session state
      resources/                   # Python MCP server
        mcp_server/
      Cargo.toml

  crates/
    smolpc-assistant-types/        # EXISTING (extended with connector traits)
    smolpc-mcp-client/             # EXISTING (unchanged)
    smolpc-connector-common/       # NEW: shared connector infrastructure

  engine/                          # UNCHANGED
    crates/
      smolpc-engine-core/
      smolpc-engine-host/
      smolpc-engine-client/
      smolpc-tts-server/

  .claude/                         # Rules (unchanged, paths in rules updated)
  .github/workflows/               # CI (paths updated)
  docs/
  installers/
  scripts/
```

### What gets deleted

| Path | Reason |
|------|--------|
| `apps/` | Replaced by `app/` (codehelper promoted) |
| `launcher/` | Empty placeholder, never used |
| `codex/WORKING_ISSUES.md` | Stale coordination doc from old workflow |
| `benchmark-results/` | Old data, not part of the product |

### What does NOT change

- `engine/` — entirely unchanged
- `crates/smolpc-mcp-client/` — unchanged
- Frontend Svelte code (`app/src/`) — unchanged internally (same components, stores, types)
- All gitignored directories (`libs/`, `models/`, `target/`, `node_modules/`, `cache_dir/`, `dist/`)

---

## 2. New Crate: `smolpc-connector-common`

Shared infrastructure that all connectors depend on. Lives at `crates/smolpc-connector-common/`.

### Exports

```rust
// --- Connector trait (from app/modes/provider.rs) ---
pub trait ToolProvider: Send + Sync { ... }
pub fn provider_state(...) -> ProviderStateDto { ... }
pub const FOUNDATION_PROVIDER_EXECUTION_NOT_IMPLEMENTED: &str = ...;
pub const MODE_UNDO_NOT_SUPPORTED: &str = ...;

// --- Cancellation abstraction (new, replaces direct AssistantState dep) ---
pub trait CancellationToken: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

// --- Text generation (from app/modes/text_generation.rs) ---
pub trait TextStreamer: Send + Sync {
    async fn generate_stream(
        &self,
        messages: &[EngineChatMessage],
        cancel: &dyn CancellationToken,
        on_token: &mut (dyn FnMut(String) + Send),
    ) -> Result<String, String>;
}
pub struct EngineTextStreamer { ... }

// --- Resource manifest utilities (from app/setup/manifests.rs) ---
pub mod manifests;   // ResourceManifest, load_manifest(), missing_expected_paths(), resource_root()

// --- Setup item constants (from app/setup/types.rs) ---
pub const SETUP_ITEM_HOST_GIMP: &str = ...;
pub const SETUP_ITEM_HOST_BLENDER: &str = ...;
pub const SETUP_ITEM_HOST_LIBREOFFICE: &str = ...;
pub const SETUP_ITEM_BLENDER_ADDON: &str = ...;
pub const SETUP_ITEM_GIMP_PLUGIN_RUNTIME: &str = ...;
pub const SETUP_ITEM_BUNDLED_PYTHON: &str = ...;
// Note: SETUP_ITEM_ENGINE_RUNTIME, SETUP_ITEM_BUNDLED_MODEL, DEFAULT_BUNDLED_MODEL_ID stay in app

// --- Shared setup utilities (from app/setup/) ---
pub mod host_apps;   // detect_blender(), detect_gimp(), HostAppDetection
pub mod launch;      // is_matching_blender_process_running(), etc.
pub mod python;      // resolve_prepared_python_command(), prepared_python_root()
```

### Dependencies

```toml
[dependencies]
smolpc-assistant-types = { path = "../smolpc-assistant-types" }
smolpc-engine-core = { path = "../../engine/crates/smolpc-engine-core" }
smolpc-engine-client = { path = "../../engine/crates/smolpc-engine-client" }
async-trait.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
log.workspace = true
dirs.workspace = true
sysinfo = "0.33"   # used by launch.rs for process detection
tempfile = "3"      # used by manifests.rs tests
```

### Design rationale

- `ToolProvider` trait moves here (not into `smolpc-assistant-types`) because it depends on `serde_json::Value` and `async_trait` — heavier than a types crate should be.
- `CancellationToken` is a 3-line trait that breaks the `AssistantState` coupling. `AssistantState` implements it in the app. Connectors only see the trait.
- `TextStreamer` depends on `EngineClient` from `smolpc-engine-client` and `GenerationConfig` from `smolpc-engine-core`, so it cannot live in the types crate.
- `manifests.rs` moves here because `setup/blender.rs`, `setup/gimp.rs`, and `python.rs` all depend on it. Without this, connector crates cannot compile.
- Connector-specific SETUP_ITEM constants move here because `host_apps.rs` and per-connector setup modules reference them. Engine/model constants stay in the app (not connector concerns).
- Shared setup utilities (`host_apps`, `launch`, `python`) move here because both GIMP and LibreOffice connectors need them, and they have no Tauri dependencies.

---

## 3. Connector Crate Dependencies

Each connector has specific dependencies based on what it uses:

**Blender** (`connectors/blender/Cargo.toml`):
```toml
[dependencies]
smolpc-connector-common = { path = "../../crates/smolpc-connector-common" }
smolpc-assistant-types = { path = "../../crates/smolpc-assistant-types" }
smolpc-engine-client = { path = "../../engine/crates/smolpc-engine-client" }
smolpc-engine-core = { path = "../../engine/crates/smolpc-engine-core" }
async-trait.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
log.workspace = true
axum.workspace = true       # bridge.rs internal HTTP server
rand.workspace = true       # bridge.rs port selection
serde-pickle = "1"          # rag.rs index loading
```

**GIMP** (`connectors/gimp/Cargo.toml`):
```toml
[dependencies]
smolpc-connector-common = { path = "../../crates/smolpc-connector-common" }
smolpc-assistant-types = { path = "../../crates/smolpc-assistant-types" }
smolpc-mcp-client = { path = "../../crates/smolpc-mcp-client" }
smolpc-engine-client = { path = "../../engine/crates/smolpc-engine-client" }
smolpc-engine-core = { path = "../../engine/crates/smolpc-engine-core" }
async-trait.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
log.workspace = true
```

**LibreOffice** (`connectors/libreoffice/Cargo.toml`):
```toml
[dependencies]
smolpc-connector-common = { path = "../../crates/smolpc-connector-common" }
smolpc-assistant-types = { path = "../../crates/smolpc-assistant-types" }
smolpc-mcp-client = { path = "../../crates/smolpc-mcp-client" }
smolpc-engine-client = { path = "../../engine/crates/smolpc-engine-client" }
smolpc-engine-core = { path = "../../engine/crates/smolpc-engine-core" }
async-trait.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
log.workspace = true
```

### Public API per connector (what the app imports)

**Blender:**
```rust
pub use provider::BlenderProvider;
pub use executor::execute_blender_request;
pub use setup::{ensure_blender_addon_prepared, BlenderAddonPrepareOutcome};
```

**GIMP:**
```rust
pub use provider::GimpProvider;
pub use executor::execute_gimp_request;
pub use planner::EngineTextGenerator;
pub use setup::{ensure_gimp_plugin_runtime_prepared, validate_supported_gimp, GimpPluginRuntimePrepareOutcome, GIMP_BRIDGE_HOST, GIMP_BRIDGE_PORT};
```

**LibreOffice:**
```rust
pub use provider::LibreOfficeProvider;
pub use executor::{execute_libreoffice_request, EngineTextPlanner};
pub use profiles::{libreoffice_profile, LibreOfficeModeProfile};
```

---

## 4. File Move Manifest

### Phase 1: Rename app directory

| Source | Destination |
|--------|-------------|
| `apps/codehelper/` | `app/` |

**Side effects:** Every path reference in the repo changes. This is the "big bang" move.

### Phase 2: Create shared crate

| Source | Destination |
|--------|-------------|
| `app/src-tauri/src/modes/provider.rs` | `crates/smolpc-connector-common/src/provider.rs` |
| `app/src-tauri/src/modes/text_generation.rs` | `crates/smolpc-connector-common/src/text_generation.rs` |
| `app/src-tauri/src/setup/manifests.rs` | `crates/smolpc-connector-common/src/manifests.rs` |
| `app/src-tauri/src/setup/host_apps.rs` | `crates/smolpc-connector-common/src/host_apps.rs` |
| `app/src-tauri/src/setup/launch.rs` | `crates/smolpc-connector-common/src/launch.rs` |
| `app/src-tauri/src/setup/python.rs` | `crates/smolpc-connector-common/src/python.rs` |
| (new) | `crates/smolpc-connector-common/src/lib.rs` |
| (new) | `crates/smolpc-connector-common/Cargo.toml` |

**Constants migration from `setup/types.rs`:** Move `SETUP_ITEM_HOST_GIMP`, `SETUP_ITEM_HOST_BLENDER`, `SETUP_ITEM_HOST_LIBREOFFICE`, `SETUP_ITEM_BLENDER_ADDON`, `SETUP_ITEM_GIMP_PLUGIN_RUNTIME`, `SETUP_ITEM_BUNDLED_PYTHON` into connector-common `lib.rs`. The app's `types.rs` keeps only `SETUP_ITEM_ENGINE_RUNTIME`, `SETUP_ITEM_BUNDLED_MODEL`, `DEFAULT_BUNDLED_MODEL_ID`, and the unused traits.

**App-side fixup:** `setup/mod.rs` removes `pub mod manifests; pub mod host_apps; pub mod launch; pub mod python;`. Files that previously imported these via `crate::setup::*` switch to `smolpc_connector_common::*`. `modes/mod.rs` removes `pub mod provider;` and `pub mod text_generation;`.

### Phase 3: Extract connector crates

**Blender:**

| Source | Destination |
|--------|-------------|
| `app/src-tauri/src/modes/blender/bridge.rs` | `connectors/blender/src/bridge.rs` |
| `app/src-tauri/src/modes/blender/executor.rs` | `connectors/blender/src/executor.rs` |
| `app/src-tauri/src/modes/blender/prompts.rs` | `connectors/blender/src/prompts.rs` |
| `app/src-tauri/src/modes/blender/provider.rs` | `connectors/blender/src/provider.rs` |
| `app/src-tauri/src/modes/blender/rag.rs` | `connectors/blender/src/rag.rs` |
| `app/src-tauri/src/modes/blender/response.rs` | `connectors/blender/src/response.rs` |
| `app/src-tauri/src/modes/blender/state.rs` | `connectors/blender/src/state.rs` |
| `app/src-tauri/src/setup/blender.rs` | `connectors/blender/src/setup.rs` |
| `app/src-tauri/resources/blender/` | `connectors/blender/resources/` |
| (new) | `connectors/blender/src/lib.rs` |
| (new) | `connectors/blender/Cargo.toml` |

**GIMP:**

| Source | Destination |
|--------|-------------|
| `app/src-tauri/src/modes/gimp/executor.rs` | `connectors/gimp/src/executor.rs` |
| `app/src-tauri/src/modes/gimp/heuristics.rs` | `connectors/gimp/src/heuristics.rs` |
| `app/src-tauri/src/modes/gimp/macros.rs` | `connectors/gimp/src/macros.rs` |
| `app/src-tauri/src/modes/gimp/planner.rs` | `connectors/gimp/src/planner.rs` |
| `app/src-tauri/src/modes/gimp/provider.rs` | `connectors/gimp/src/provider.rs` |
| `app/src-tauri/src/modes/gimp/response.rs` | `connectors/gimp/src/response.rs` |
| `app/src-tauri/src/modes/gimp/runtime.rs` | `connectors/gimp/src/runtime.rs` |
| `app/src-tauri/src/modes/gimp/transport.rs` | `connectors/gimp/src/transport.rs` |
| `app/src-tauri/src/setup/gimp.rs` | `connectors/gimp/src/setup.rs` |
| `app/src-tauri/resources/gimp/` | `connectors/gimp/resources/` |
| (new) | `connectors/gimp/src/lib.rs` |
| (new) | `connectors/gimp/Cargo.toml` |

**LibreOffice:**

| Source | Destination |
|--------|-------------|
| `app/src-tauri/src/modes/libreoffice/executor.rs` | `connectors/libreoffice/src/executor.rs` |
| `app/src-tauri/src/modes/libreoffice/profiles.rs` | `connectors/libreoffice/src/profiles.rs` |
| `app/src-tauri/src/modes/libreoffice/provider.rs` | `connectors/libreoffice/src/provider.rs` |
| `app/src-tauri/src/modes/libreoffice/resources.rs` | `connectors/libreoffice/src/resources.rs` |
| `app/src-tauri/src/modes/libreoffice/response.rs` | `connectors/libreoffice/src/response.rs` |
| `app/src-tauri/src/modes/libreoffice/runtime.rs` | `connectors/libreoffice/src/runtime.rs` |
| `app/src-tauri/src/modes/libreoffice/state.rs` | `connectors/libreoffice/src/state.rs` |
| `app/src-tauri/resources/libreoffice/` | `connectors/libreoffice/resources/` |
| (new) | `connectors/libreoffice/src/lib.rs` |
| (new) | `connectors/libreoffice/Cargo.toml` |

### Phase 4: Cleanup

| Action | Target |
|--------|--------|
| Delete | `apps/` (empty after move) |
| Delete | `launcher/` (empty placeholder) |
| Delete | `app/src-tauri/src/modes/blender/` (moved) |
| Delete | `app/src-tauri/src/modes/gimp/` (moved) |
| Delete | `app/src-tauri/src/modes/libreoffice/` (moved) |

---

## 5. Import Rewiring

### Pattern: `use crate::modes::provider::*` becomes `use smolpc_connector_common::*`

Every connector file has `use crate::` imports that must change:

| Old import | New import | Used by |
|------------|-----------|---------|
| `crate::modes::provider::{ToolProvider, provider_state}` | `smolpc_connector_common::{ToolProvider, provider_state}` | All connectors |
| `crate::modes::text_generation::TextStreamer` | `smolpc_connector_common::TextStreamer` | Blender, LibreOffice |
| `crate::assistant::MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION` | `smolpc_connector_common::MODE_UNDO_NOT_SUPPORTED` | Blender, LibreOffice |
| `crate::assistant::state::AssistantState` | `dyn smolpc_connector_common::CancellationToken` | All executors |
| `crate::setup::blender::*` | `crate::setup::*` (same crate, just moved) | Blender provider |
| `crate::setup::gimp::*` | `crate::setup::*` (same crate, just moved) | GIMP provider |
| `crate::setup::host_apps::*` | `smolpc_connector_common::host_apps::*` | Blender, GIMP providers |
| `crate::setup::launch::*` | `smolpc_connector_common::launch::*` | Blender, GIMP providers |
| `crate::setup::python::*` | `smolpc_connector_common::python::*` | GIMP runtime, LO runtime |

### App-side import changes

In `app/src-tauri/src/`:

| File | Old import | New import |
|------|-----------|-----------|
| `modes/registry.rs` | `super::blender::BlenderProvider` | `smolpc_connector_blender::BlenderProvider` |
| `modes/registry.rs` | `super::gimp::GimpProvider` | `smolpc_connector_gimp::GimpProvider` |
| `modes/registry.rs` | `super::libreoffice::LibreOfficeProvider` | `smolpc_connector_libreoffice::LibreOfficeProvider` |
| `modes/registry.rs` | `super::provider::ToolProvider` | `smolpc_connector_common::ToolProvider` |
| `modes/config.rs` | `super::libreoffice::libreoffice_profile` | `smolpc_connector_libreoffice::libreoffice_profile` |
| `modes/code.rs` | `super::provider::{ToolProvider, provider_state}` | `smolpc_connector_common::{ToolProvider, provider_state}` |
| `commands/assistant.rs` | `crate::modes::blender::execute_blender_request` | `smolpc_connector_blender::execute_blender_request` |
| `commands/assistant.rs` | `crate::modes::gimp::execute_gimp_request` | `smolpc_connector_gimp::execute_gimp_request` |
| `commands/assistant.rs` | `crate::modes::libreoffice::execute_libreoffice_request` | `smolpc_connector_libreoffice::execute_libreoffice_request` |
| `commands/assistant.rs` | `crate::modes::text_generation::*` | `smolpc_connector_common::*` |
| `commands/assistant.rs` | `crate::modes::provider::*` | `smolpc_connector_common::*` |
| `commands/setup.rs` | `crate::setup::blender::*` | `smolpc_connector_blender::setup::*` |
| `commands/setup.rs` | `crate::setup::gimp::*` | `smolpc_connector_gimp::setup::*` |
| `commands/modes.rs` | `crate::setup::host_apps::*` | `smolpc_connector_common::host_apps::*` |
| `commands/modes.rs` | `crate::setup::launch::*` | `smolpc_connector_common::launch::*` |
| `setup/provision.rs` | `super::blender::*` | `smolpc_connector_blender::setup::*` |
| `setup/provision.rs` | `super::gimp::*` | `smolpc_connector_gimp::setup::*` |
| `setup/provision.rs` | `super::manifests::*` | `smolpc_connector_common::manifests::*` |
| `setup/provision.rs` | `super::python::*` | `smolpc_connector_common::python::*` |
| `setup/status.rs` | `super::host_apps::*` | `smolpc_connector_common::host_apps::*` |

### Intra-connector import rewiring (within each extracted crate)

After extraction, `crate::setup::gimp::*` and `crate::modes::blender::*` paths become local `crate::*` references:

| File (in connector crate) | Old import | New import |
|---------------------------|-----------|-----------|
| GIMP `runtime.rs` | `crate::setup::gimp::{resolve_gimp_resource_layout, GIMP_PLUGIN_SOCKET_HOST, GIMP_PLUGIN_SOCKET_PORT}` | `crate::setup::{resolve_gimp_resource_layout, GIMP_PLUGIN_SOCKET_HOST, GIMP_PLUGIN_SOCKET_PORT}` |
| GIMP `runtime.rs` | `crate::setup::python::resolve_prepared_python_command` | `smolpc_connector_common::python::resolve_prepared_python_command` |
| GIMP `transport.rs` | `crate::setup::gimp::{GIMP_BRIDGE_HOST, GIMP_BRIDGE_PORT}` | `crate::setup::{GIMP_BRIDGE_HOST, GIMP_BRIDGE_PORT}` |
| Blender `bridge.rs` (test) | `crate::modes::blender::state::*` | `crate::state::*` |
| LO `runtime.rs` | `crate::setup::python::resolve_prepared_python_command` | `smolpc_connector_common::python::resolve_prepared_python_command` |

---

## 6. Config File Updates

### `Cargo.toml` (workspace root)

```toml
[workspace]
members = [
  "app/src-tauri",
  "crates/smolpc-assistant-types",
  "crates/smolpc-mcp-client",
  "crates/smolpc-connector-common",
  "connectors/blender",
  "connectors/gimp",
  "connectors/libreoffice",
  "engine/crates/smolpc-engine-core",
  "engine/crates/smolpc-engine-host",
  "engine/crates/smolpc-engine-client",
]
```

### `app/src-tauri/Cargo.toml`

```toml
[package]
name = "smolpc-desktop"
# ... rest unchanged

[dependencies]
smolpc-connector-common = { path = "../../crates/smolpc-connector-common" }
smolpc-connector-blender = { path = "../../connectors/blender" }
smolpc-connector-gimp = { path = "../../connectors/gimp" }
smolpc-connector-libreoffice = { path = "../../connectors/libreoffice" }
# ... existing deps unchanged
```

### `app/src-tauri/tauri.conf.json`

```json
{
  "productName": "SmolPC 2.0",
  "identifier": "com.smolpc.codehelper",  // KEEP unchanged — preserves NSIS upgrade path
  "bundle": {
    "resources": {
      "libs/": "libs/",
      "binaries/": "binaries/",
      "resources/launcher/": "launcher/",
      "resources/model-installer/": "model-installer/",
      "../../connectors/gimp/resources/": "gimp/",
      "../../connectors/blender/resources/": "blender/",
      "../../connectors/libreoffice/resources/": "libreoffice/",
      "resources/python/": "python/",
      "resources/models/": "models/"
    }
  }
}
```

### `package.json` (root)

```json
{
  "workspaces": ["app"]
}
```

### `.github/workflows/ci.yml`

- `npm audit --workspace apps/codehelper` → `npm audit --workspace app`
- `cargo check -p smolpc-code-helper` → `cargo check -p smolpc-desktop`
- Engine test commands — unchanged (engine crate names unchanged)
- Add connector crate test jobs: `cargo test -p smolpc-connector-common -p smolpc-connector-blender -p smolpc-connector-gimp -p smolpc-connector-libreoffice`

### `.github/workflows/release.yml` (14 path references)

All `apps/codehelper/` references must become `app/`:
- `apps/codehelper/src-tauri/libs` → `app/src-tauri/libs` (lines 51-52)
- `apps/codehelper/scripts/setup-directml-runtime.ps1` → `app/scripts/setup-directml-runtime.ps1` (line 59)
- `apps/codehelper/scripts/setup-openvino-runtime.ps1` → `app/scripts/setup-openvino-runtime.ps1` (line 65)
- `apps/codehelper/scripts/setup-bundled-python-runtime.ps1` → `app/scripts/setup-bundled-python-runtime.ps1` (line 71)
- `apps/codehelper/src-tauri/binaries/` → `app/src-tauri/binaries/` (lines 79, 85, 98)
- `apps/codehelper/src-tauri/libs/` → `app/src-tauri/libs/` (lines 91-96)
- `apps/codehelper/src-tauri/resources/python/` → `app/src-tauri/resources/python/` (line 97)
- `projectPath: apps/codehelper` → `projectPath: app` (line 111)

### `scripts/check-boundaries.ps1` (8 path references)

Every assertion and search path uses `apps/codehelper/` (lines 21-46). Must be updated to `app/`. This runs in the CI `boundary-enforcement` job.

### Root `package.json` scripts (14 entries)

Every `--workspace apps/codehelper` becomes `--workspace app`. All script lines must be updated.

### `app/scripts/*.ps1`

`run-tauri-dev.ps1:54` checks `Get-Process -Name "smolpc-code-helper"`. After the Cargo package rename to `smolpc-desktop`, the binary name changes. Update the process name check to match.

### `CLAUDE.md`

Update all path references from `apps/codehelper/` to `app/`. Update quick reference commands. Update pre-commit check paths.

### `.claude/rules/*.md`

Check and update any path references to `apps/codehelper/`.

---

## 7. The CancellationToken Pattern

Current: `TextStreamer::generate_stream` takes `&AssistantState` directly.

After:

```rust
// In smolpc-connector-common:
pub trait CancellationToken: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

// TextStreamer signature changes:
async fn generate_stream(
    &self,
    messages: &[EngineChatMessage],
    cancel: &dyn CancellationToken,
    on_token: &mut (dyn FnMut(String) + Send),
) -> Result<String, String>;
```

```rust
// In app/src-tauri/src/assistant/state.rs:
impl smolpc_connector_common::CancellationToken for AssistantState {
    fn is_cancelled(&self) -> bool {
        self.is_cancelled()  // delegates to existing method
    }
}
```

All executor call sites change from `&state` (AssistantState) to `&state` (dyn CancellationToken) — no behavioral change, just a type widening.

---

## 8. Setup Module Split

### What stays in `app/src-tauri/src/setup/`

| File | Lines | Reason |
|------|-------|--------|
| `state.rs` | 335 | Tauri-managed SetupState |
| `status.rs` | 242 | Aggregates readiness (imports from connector-common + connector crates) |
| `provision.rs` | 496 | Orchestrates resource preparation (imports from connector-common + connector crates) |
| `models.rs` | 142 | Model setup logic (engine concern, not connector) |
| `types.rs` | 41 | Trimmed: keeps `SETUP_ITEM_ENGINE_RUNTIME`, `SETUP_ITEM_BUNDLED_MODEL`, `DEFAULT_BUNDLED_MODEL_ID`, unused traits |
| `mod.rs` | 15 | Module declarations (trimmed: removes manifests, host_apps, launch, python, blender, gimp) |

### What moves to `smolpc-connector-common`

| File | Lines | Reason |
|------|-------|--------|
| `manifests.rs` | 125 | Required by blender.rs, gimp.rs, python.rs — all of which move out of the app |
| `host_apps.rs` | 409 | Shared: detect_blender(), detect_gimp(), HostAppDetection |
| `launch.rs` | 168 | Shared: process running checks (uses `sysinfo` crate) |
| `python.rs` | 380 | Shared: GIMP + LibreOffice both need Python resolution |
| Constants from `types.rs` | ~6 lines | `SETUP_ITEM_HOST_*`, `SETUP_ITEM_BLENDER_ADDON`, `SETUP_ITEM_GIMP_PLUGIN_RUNTIME`, `SETUP_ITEM_BUNDLED_PYTHON` |

### What moves to connector crates

| File | Lines | Destination |
|------|-------|-------------|
| `blender.rs` | 659 | `connectors/blender/src/setup.rs` |
| `gimp.rs` | 737 | `connectors/gimp/src/setup.rs` |

The app's `provision.rs` and `commands/setup.rs` will import per-connector setup functions from the connector crates. No circular dependency — the app depends on connectors, never the reverse.

---

## 9. Dependency Graph (No Cycles)

```
                    smolpc-assistant-types (DTOs)
                            |
                    smolpc-engine-core (GenerationConfig)
                            |
                    smolpc-engine-client
                            |
                  smolpc-connector-common
          (traits, shared setup, manifests, TextStreamer)
                   /        |        \
    connector-blender  connector-gimp  connector-libreoffice
                   \        |        /
                     smolpc-desktop (app)
                            |
                    EngineSupervisor
                            |
                    smolpc-engine-host (separate process)
```

All arrows point downward. No cycles.

---

## 10. Verification Checklist

After implementation, every one of these must pass:

```bash
# Compile
cargo check --workspace
cargo clippy --workspace

# Tests
cargo test -p smolpc-engine-core
cargo test -p smolpc-engine-host
cargo test -p smolpc-connector-common
cargo test -p smolpc-connector-blender
cargo test -p smolpc-connector-gimp
cargo test -p smolpc-connector-libreoffice
cargo test -p smolpc-desktop

# Frontend
cd app && npm run check && npm run lint

# Format
cargo fmt -- --check

# Verify connector resources resolve
# (run app in dev mode, switch to each mode, verify resources found)
```

---

## 11. Risk Mitigations

| Risk | Mitigation |
|------|------------|
| Git loses rename history | Use `git mv` for all moves. Single structural commit per phase. |
| Tauri can't find resources at new paths | Test `../../connectors/X/resources/` paths in dev mode before committing. Fallback: copy resources in beforeBuildCommand. |
| Dev-mode resource fallback breaks | `app_paths.rs` uses `CARGO_MANIFEST_DIR/resources` as dev fallback — after extraction, connector resources won't be there. Update `default_dev_bundled_resource_dir()` to also probe `CARGO_MANIFEST_DIR/../../connectors/X/resources/`, or ensure Tauri's resource map handles dev mode correctly. |
| Connector crate won't compile (missing import) | `cargo check --workspace` after each phase. Fix before moving to next phase. |
| NSIS installer path changes | `productName` change means install path changes from `%LOCALAPPDATA%\SmolPC Code Helper\` to `%LOCALAPPDATA%\SmolPC 2.0\`. Document for existing users — old install dir becomes orphaned. |
| CI breaks on crate rename | Update CI in the same commit as the Cargo.toml rename. |
| `setup/provision.rs` can't find connector setup functions | Provision.rs imports from connector crates (app depends on them). Verified by cargo check. |
| CSP `connect-src` references stale Ollama port | Opportunistic cleanup: update CSP in same commit as tauri.conf.json changes. Remove `http://localhost:11434` reference, replace with engine port 19432 if needed. |
| PowerShell process name check stale after rename | Update `run-tauri-dev.ps1` process name check in Phase 6. |

---

## 12. Implementation Phases (Execution Order)

**Phase 1: Structural moves** (blocking — must be one commit)
1. `git mv apps/codehelper app`
2. Update root `Cargo.toml` workspace member path
3. Update root `package.json` workspace path
4. Update `app/src-tauri/tauri.conf.json` schema path
5. `cargo check --workspace` — must pass before continuing
6. Commit: `refactor: rename apps/codehelper to app`

**Phase 2: Create smolpc-connector-common crate**
1. Create `crates/smolpc-connector-common/` with Cargo.toml and lib.rs
2. Move `provider.rs`, `text_generation.rs` from app modes/
3. Move `host_apps.rs`, `launch.rs`, `python.rs` from app setup/
4. Introduce `CancellationToken` trait
5. Update app imports to use new crate
6. Add crate to workspace members
7. `cargo check --workspace`
8. Commit: `refactor: extract smolpc-connector-common crate`

**Phase 3: Extract Blender connector**
1. Create `connectors/blender/` with Cargo.toml and lib.rs
2. Move 8 source files from `app/src-tauri/src/modes/blender/`
3. Move `setup/blender.rs` → `connectors/blender/src/setup.rs`
4. Move `app/src-tauri/resources/blender/` → `connectors/blender/resources/`
5. Update all `use crate::` imports in moved files
6. Update app registry, config, commands to import from new crate
7. Update `tauri.conf.json` resource path
8. Add crate to workspace members
9. `cargo check --workspace`
10. Commit: `refactor: extract smolpc-connector-blender crate`

**Phase 4: Extract GIMP connector** (same pattern as Phase 3)

**Phase 5: Extract LibreOffice connector** (same pattern as Phase 3)

**Phase 6: Cleanup and identity**
1. Delete empty `app/src-tauri/src/modes/blender/`, `gimp/`, `libreoffice/`
2. Delete `launcher/`, stale files
3. Rename Cargo package to `smolpc-desktop`
4. Update `tauri.conf.json` productName to "SmolPC 2.0"
5. Update CI workflow paths
6. Update CLAUDE.md, .claude/rules/ path references
7. `cargo check --workspace` + full test suite + frontend checks
8. Commit: `refactor: finalize SmolPC 2.0 identity and cleanup`

---

## 13. Estimated Effort

| Phase | Effort | Risk |
|-------|--------|------|
| Phase 1: Structural move | 15 min | Low (mechanical) |
| Phase 2: Connector-common crate | 45 min | Medium (trait boundary design) |
| Phase 3: Blender extraction | 30 min | Low (mechanical + imports) |
| Phase 4: GIMP extraction | 30 min | Low (mechanical + imports) |
| Phase 5: LibreOffice extraction | 30 min | Low (mechanical + imports) |
| Phase 6: Cleanup + identity | 30 min | Low (config updates) |
| **Total** | **~3 hours** | |
