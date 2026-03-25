# SmolPC Platform Monorepo

Modular monorepo for SmolPC desktop AI apps.

The repository is organized into three zones with explicit ownership boundaries:

- `engine/`: shared inference runtime and API contract
- `launcher/`: app-suite launcher shell and manifest conventions
- `apps/`: individual product apps (CodeHelper now, others staged)

## Start Here

1. Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
2. Read [docs/ENGINE_API.md](docs/ENGINE_API.md).
3. Read [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md).

## Active Layout

```text
.
├── engine/
│   └── crates/
│       ├── smolpc-engine-core
│       ├── smolpc-engine-host
│       └── smolpc-engine-client
├── launcher/
├── app/                    (CodeHelper)
├── apps/
│   ├── libreoffice-assistant/
│   ├── gimp-assistant/
│   └── blender-assistant/
└── docs/
```

## Development

```bash
npm ci
cargo check --workspace
```

CodeHelper app flows from repo root:

```bash
npm run tauri:dev
npm run tauri:dml
npm run check
```

Shared model bootstrap:

```bash
npm run runtime:setup:openvino
npm run runtime:setup:python
npm run model:setup:qwen25-instruct
npm run model:setup:qwen3-4b
```

## Boundary Rules

- Engine owns startup/readiness/inference lifecycle.
- Apps consume `smolpc-engine-client` and the documented engine API contract.
- App-owned inference engines and legacy Ollama command paths are removed.
- Boundary checks run via:

```bash
npm run boundary:check
```

## Zone Docs

- [engine/README.md](engine/README.md)
- [launcher/README.md](launcher/README.md)
- [apps/README.md](apps/README.md)
- [app/README.md](app/README.md)
