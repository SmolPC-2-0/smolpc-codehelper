# SmolPC Unified Assistant Self-Contained Architecture

**Last Updated:** 2026-03-17
**Status:** Target architecture for the self-contained delivery line

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

Expected responsibilities:

- `OwnedIntegration`: one provider-owned packaged integration bundle
- `Provisioner`: install/repair/update provider assets in user profile paths
- `HostAppLocator`: resolve installed host app executables and profiles
- `RuntimeSupervisor`: start/stop/check long-lived provider runtimes
- `BundledPythonRuntime`: resolve packaged interpreter, wheels, and environment

## 7. Mode-Specific Architecture

### 7.1 Code

- no external host app
- engine + bundled model only
- remains on current inference path

### 7.2 LibreOffice

- host app remains external: LibreOffice / Collabora
- bundled runtime scripts stay in unified resources
- packaged Python runtime replaces system Python dependency
- provider auto-launches `soffice` when needed
- Writer and Slides remain side-effectful single-tool-turn modes
- Calc stays scaffold-only

### 7.3 Blender

- bridge server stays inside the unified app
- addon payload becomes bundled unified-app-owned resource
- provider auto-installs/enables addon in Blender profile
- provider launches Blender when needed
- addon-facing token-file contract remains unchanged

### 7.4 GIMP

- provider transport stays TCP on `127.0.0.1:10008`
- self-contained line vendors a pinned `gimp-mcp` snapshot
- plugin/server payload becomes bundled unified-app-owned resource
- provider provisions plugin files into the GIMP profile
- provider launches both GIMP and the bundled GIMP MCP runtime when needed

## 8. Boot Flow

On app launch:

1. initialize setup subsystem
2. eagerly start engine
3. ensure bundled default model is ready
4. resolve packaged resource roots
5. collect host-app presence status in background
6. do not eagerly launch host apps unless explicitly configured later

## 9. Mode Activation Flow

On first use of a live external mode:

1. provider asks setup subsystem for host-app status
2. setup subsystem provisions missing provider-owned assets if required
3. setup subsystem launches host app if not already running
4. provider starts/attaches to the provider runtime
5. provider reports live status and available tools
6. assistant flow proceeds normally

## 10. Deferred Architecture

Not part of this line:

- Calc activation
- bundle identifier migration
- multi-installer/product split
- launcher reintroduction
