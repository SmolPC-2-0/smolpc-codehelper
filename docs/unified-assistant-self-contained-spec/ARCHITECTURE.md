# SmolPC Unified Assistant Self-Contained Architecture

**Last Updated:** 2026-03-17
**Status:** Target architecture with Phase 2 foundation and Phase 3 LibreOffice runtime ownership landed

## 1. Product Shape

SmolPC Unified Assistant remains one Tauri desktop app, but the self-contained
line adds explicit ownership of setup, provisioning, and runtime supervision.

The final runtime model is:

- one unified shell
- one shared engine runtime
- one bundled default model
- one app-private Python runtime
- provider-owned bundled assets per external mode
- host apps launched on demand by the unified app

## 2. Modes

| Mode    | Internal id | Shipping state | Runtime owner                               |
| ------- | ----------- | -------------- | ------------------------------------------- |
| Code    | `code`      | live           | unified app only                            |
| GIMP    | `gimp`      | live           | unified app + external GIMP host app        |
| Blender | `blender`   | live           | unified app + external Blender host app     |
| Writer  | `writer`    | live           | unified app + external LibreOffice host app |
| Calc    | `calc`      | deferred       | scaffold only                               |
| Slides  | `impress`   | live           | unified app + external LibreOffice host app |

## 3. High-Level System

```text
+-------------------------------------------------------------------+
| SmolPC Unified Assistant                                          |
|                                                                   |
|  Frontend shell                                                   |
|    - mode selector                                                |
|    - per-mode chats                                               |
|    - setup status / repair surface                                |
|                                                                   |
|  Backend                                                          |
|    - assistant orchestrator                                       |
|    - mode provider registry                                       |
|    - setup/provisioning subsystem                                 |
|    - runtime supervisors                                          |
|                                                                   |
|  Shared local engine                                              |
|    - bundled default model                                        |
|    - auto-start on app launch                                     |
|                                                                   |
|  App-owned provider assets                                        |
|    - bundled Python runtime                                       |
|    - LibreOffice MCP scripts                                      |
|    - Blender addon payload                                        |
|    - GIMP plugin/server payload                                   |
+-------------------------------------------------------------------+
            |                  |                    |
            v                  v                    v
      External GIMP      External Blender      External LibreOffice
      app installation   app installation      / Collabora install
```

## 4. Core Backend Layers

### 4.1 Unified shell and assistant layer

Responsibilities:

- chat storage
- mode switching
- `assistant_send`
- streaming
- mode-specific actions

### 4.2 Engine supervisor

Responsibilities:

- eager engine startup
- bundled default model readiness
- engine health/status
- shared generation client resolution

### 4.3 Setup/provisioning subsystem

New subsystem added on the self-contained line.

Responsibilities:

- detect installed host apps
- provision bundled assets into user-owned host-app locations
- track provisioned versions
- expose setup health and repair state
- launch host apps on demand

Phase 2 foundation limits:

- the subsystem is app-level, not mode-level
- it reports readiness for the whole app
- it does not replace `mode_status`
- it does not launch host apps in Phase 2
- it does not provision Blender or GIMP user-profile integrations in Phase 2

Phase 2 implementation status:

- setup subsystem now exists in the implementation line
- app-level setup state and setup commands now exist
- host-app detection now exists
- packaged resource manifest validation now exists

Phase 3 first consumer:

- LibreOffice now consumes the prepared bundled-Python substrate from setup state
- `setup_prepare()` still remains foundation-only and does not launch LibreOffice
- Writer and Slides now use that prepared runtime at provider-use time
- the provider resolves LibreOffice through the shared host-app locator and passes the detected host path into the runtime

### 4.4 Mode providers

Responsibilities:

- mode-specific transport and tool execution
- provider runtime supervision
- integration-specific status reporting

## 5. Ownership Matrix

| Component                  | Owner in self-contained line                |
| -------------------------- | ------------------------------------------- |
| Engine process             | unified app                                 |
| Default model              | unified app                                 |
| Python runtime             | unified app                                 |
| LibreOffice scripts        | unified app                                 |
| Blender bridge server      | unified app                                 |
| Blender addon              | unified app provisions into Blender profile |
| GIMP plugin/server runtime | unified app provisions into GIMP profile    |
| Host creative/office apps  | external install, launched by unified app   |

## 6. Setup/Provisioning Contracts

New internal interfaces to establish in the implementation line:

- `OwnedIntegration`
- `Provisioner`
- `HostAppLocator`
- `RuntimeSupervisor`
- `BundledPythonRuntime`

Phase 2 public setup surface:

- `setup_status`
- `setup_prepare`

Phase 2 setup item ids:

- `engine_runtime`
- `bundled_model`
- `bundled_python`
- `host_gimp`
- `host_blender`
- `host_libreoffice`

Expected responsibilities:

- `OwnedIntegration`: one provider-owned packaged integration bundle
- `Provisioner`: install/repair/update provider assets in user profile paths
- `HostAppLocator`: resolve installed host app executables and profiles
- `RuntimeSupervisor`: start/stop/check long-lived provider runtimes
- `BundledPythonRuntime`: resolve packaged interpreter, wheels, and environment

## 7. Phase 2 Resource And State Layout

Phase 2 establishes these packaged resource roots:

- `resources/python/`
- `resources/gimp/`
- `resources/blender/`
- `resources/libreoffice/`
- `resources/models/`

Each resource root must have a tracked manifest with:

- `version`
- `source`
- `expectedPaths`
- `status`

Phase 2 also establishes app-local-data setup roots under:

- `setup/python/`
- `setup/state/`
- `setup/logs/`

Those roots are now part of the implementation contract on
`dev/unified-assistant-self-contained`.

## 8. Mode-Specific Architecture

### 8.1 Code

- no external host app
- engine + bundled model only
- remains on current inference path

### 8.2 LibreOffice

- host app remains external: LibreOffice / Collabora
- bundled runtime scripts stay in unified resources
- packaged builds use the prepared bundled Python runtime only
- packaged mode does not fall back to system `python` or `python3`
- provider auto-detects the LibreOffice host path and the bundled runtime auto-launches `soffice` when needed
- `mode_status(writer|impress)` and `mode_refresh_tools(writer|impress)` surface bundled-Python and LibreOffice readiness honestly
- Writer and Slides remain side-effectful single-tool-turn modes
- Calc stays scaffold-only

### 8.3 Blender

- bridge server stays inside the unified app
- addon payload becomes bundled unified-app-owned resource
- provider auto-installs/enables addon in Blender profile in Phase 4
- provider launches Blender when needed in Phase 4
- addon-facing token-file contract remains unchanged

### 8.4 GIMP

- provider transport stays TCP on `127.0.0.1:10008`
- self-contained line vendors a pinned `gimp-mcp` snapshot
- plugin/server payload becomes bundled unified-app-owned resource
- provider provisions plugin files into the GIMP profile in Phase 5
- provider launches both GIMP and the bundled GIMP MCP runtime when needed in Phase 5

## 9. Boot Flow

On app launch:

1. initialize setup subsystem
2. eagerly start engine
3. ensure bundled default model is ready
4. resolve packaged resource roots
5. collect host-app presence status in background
6. do not eagerly launch host apps unless explicitly configured later

Phase 2 stop-point:

- steps 1, 4, and 5 are introduced now
- step 2 stays as it already exists on the demo baseline
- step 3 becomes a formal packaged-resource contract in Phase 2, not a new model-selection change
- step 6 remains the rule

## 10. Mode Activation Flow

On first use of a live external mode:

1. provider asks setup subsystem for host-app status
2. setup subsystem provisions missing provider-owned assets if required
3. setup subsystem launches host app if not already running
4. provider starts/attaches to the provider runtime
5. provider reports live status and available tools
6. assistant flow proceeds normally

Phase 2 does not yet implement this full flow for every provider. Phase 3 now
implements the LibreOffice slice while leaving Blender and GIMP provisioning
for later phases.

## 11. Deferred Architecture

Not part of this line:

- Calc activation
- bundle identifier migration
- multi-installer/product split
- launcher reintroduction
