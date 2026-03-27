# Engine

The SmolPC inference engine is a standalone local HTTP server that runs large language models on the student's hardware. It is a separate process from the desktop app — it survives app restarts, can be shared across multiple consumers, and isolates native FFI crashes from the UI.

The engine listens on `localhost:19432` with token-based authentication. Any HTTP client can consume it — the Tauri app connects via the `smolpc-engine-client` library, but `curl` works just as well.

## Crate Breakdown

The engine is split across 5 Rust crates:

### smolpc-engine-core

Library crate. The foundation layer with no workspace dependencies.

- **Hardware detection** — GPU enumeration via DXGI (~14ms), NPU driver probing, CPU/RAM/storage detection
- **Backend abstraction** — `InferenceBackend` enum: `Cpu`, `DirectML`, `OpenVinoNpu`
- **FFI wrappers** — OpenVINO GenAI C API (`OpenVinoGenAiGenerator`), ONNX Runtime GenAI (`GenAiDirectMlGenerator`), Whisper STT (`WhisperPipeline`)
- **Centralized DLL loading** — all `Library::new()` calls confined to `runtime_loading.rs`, CI-enforced. OpenVINO requires 14 DLLs in strict dependency order; ORT requires 2
- **Model registry** — `ModelDefinition` with RAM thresholds, artifact paths, backend compatibility

### smolpc-engine-host

The binary. Axum HTTP server with 13 endpoints, semaphore-based concurrency, background probe, and idle timeout management.

See [ENGINE_API.md](ENGINE_API.md) for the full API reference.

### smolpc-engine-client

Library crate for engine consumers. Handles process spawning, health polling, reconnection, auth token management, and streaming SSE parsing.

Key exports:
- `spawn_engine()` — launch engine as detached process with PID file and spawn lock
- `wait_for_healthy()` — poll until ready (default 60s timeout, 250ms interval)
- `kill_stale_processes()` — clean up orphaned engines with PID identity verification
- `EngineClient` — HTTP client with streaming support
- `RuntimeModePreference` — Auto, Cpu, Dml, Npu

### smolpc-tts-server

Standalone TTS sidecar on port 19433. Lives in its own workspace (separate `Cargo.toml` root) due to an `ort` crate version conflict. Must be built with `--manifest-path`, not `-p`.

Spawned and monitored by the engine host. Health-checked at 200ms intervals during startup (15s budget).

### smolpc-benchmark

CLI tool for performance testing across backends. Measures TTFT, tokens/sec, memory, and CPU utilization. Supports multi-backend comparison with warmup/cooldown and CSV/JSON output.

## Running Standalone

### Start the engine

```bash
cargo run -p smolpc-engine-host
```

The engine needs a token to authenticate requests. Set one via environment variable:

```powershell
$env:SMOLPC_ENGINE_TOKEN = "my-dev-token"
cargo run -p smolpc-engine-host
```

### Verify it's running

```bash
curl -H "Authorization: Bearer my-dev-token" http://localhost:19432/engine/health
```

Expected response:
```json
{"ok": true, "state": "idle"}
```

### Start the engine with a model

```bash
curl -X POST http://localhost:19432/engine/ensure-started \
  -H "Authorization: Bearer my-dev-token" \
  -H "Content-Type: application/json" \
  -d '{"mode": "auto", "startup_policy": {}}'
```

This triggers the background probe: hardware detection, backend selection, and model loading. Poll `/engine/status` until `state` is `"ready"`.

### Run a chat completion

```bash
curl -X POST http://localhost:19432/v1/chat/completions \
  -H "Authorization: Bearer my-dev-token" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "smolpc-engine",
    "stream": true,
    "messages": [{"role": "user", "content": "Write a Python hello world"}]
  }'
```

Responses stream as Server-Sent Events (SSE).

### Force a specific backend

```powershell
$env:SMOLPC_FORCE_EP = "cpu"        # or "dml" or "openvino_npu"
cargo run -p smolpc-engine-host
```

## CLI Arguments

```
smolpc-engine-host [OPTIONS]

  --port <PORT>              Server port (default: 19432)
  --data-dir <PATH>          Engine data directory (default: %LOCALAPPDATA%\SmolPC 2.0\engine)
  --resource-dir <PATH>      Bundled resource directory (DLLs, models)
  --app-version <VERSION>    App version string (default: "dev")
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SMOLPC_ENGINE_TOKEN` | Auth token (required) | None — must be set |
| `SMOLPC_FORCE_EP` | Force backend: `cpu`, `dml`, `openvino_npu` | Auto-detect |
| `SMOLPC_DML_DEVICE_ID` | Force GPU adapter index for DirectML | Auto-select |
| `SMOLPC_MODELS_DIR` | Override model directory | `%LOCALAPPDATA%\SmolPC\models` |
| `SMOLPC_ORT_BUNDLE_ROOT` | Override ONNX Runtime DLL path | Bundled |
| `SMOLPC_OPENVINO_BUNDLE_ROOT` | Override OpenVINO DLL path | Bundled |
| `SMOLPC_ENGINE_QUEUE_SIZE` | Max queued inference requests | `3` |
| `SMOLPC_ENGINE_QUEUE_TIMEOUT_SECS` | Queue wait timeout in seconds | `60` |
| `SMOLPC_ENGINE_MODEL_IDLE_UNLOAD_SECS` | Unload model after idle (0 = disabled) | Disabled |
| `SMOLPC_ENGINE_PROCESS_IDLE_EXIT_SECS` | Exit engine after idle (0 = disabled) | Disabled |
| `SMOLPC_TTS_PORT` | TTS sidecar port | `19433` |
| `SMOLPC_ENGINE_DEFAULT_MODEL_ID` | Override default model | RAM-based auto-select |
| `SMOLPC_OPENVINO_MAX_TOKENS_HARD_CAP` | Max tokens hard limit | `8192` |

## Data Directories

| Data | Path |
|------|------|
| Engine data | `%LOCALAPPDATA%\SmolPC 2.0\engine\` |
| AI models | `%LOCALAPPDATA%\SmolPC\models\{model_id}\{backend}\` |
| Engine log | `%LOCALAPPDATA%\SmolPC 2.0\engine\engine-spawn.log` |
| TTS log | `%LOCALAPPDATA%\SmolPC 2.0\engine\tts-spawn.log` |
| PID file | `%LOCALAPPDATA%\SmolPC 2.0\engine\engine-spawn.lock` |
| TTS PID | `%LOCALAPPDATA%\SmolPC 2.0\engine\tts.pid` |

## Concurrency Model

The engine enforces strict concurrency limits via semaphores:

- **Generation semaphore** (capacity 1) — only one LLM generation runs at a time
- **Queue semaphore** (configurable, default 3) — limits waiting requests; returns 429 when full
- **Voice semaphore** (capacity 1) — serializes STT and TTS operations

An atomic `generating` flag provides fast path checking. An atomic `model_transition_in_progress` flag prevents model unload during active transitions.

## Idle Timeout

An optional idle timeout loop runs every 30 seconds:

1. **Model idle unload** — if configured, unloads the model after no activity for the specified duration (disabled by default to avoid the "unhealthy after idle" bug)
2. **Process idle exit** — if configured, gracefully shuts down the engine after no activity for the specified duration

Both check the `generating` and `model_transition_in_progress` flags before acting.

## Related Documentation

- [ENGINE_API.md](ENGINE_API.md) — Full HTTP API reference with request/response schemas
- [ENGINE_LIFECYCLE.md](ENGINE_LIFECYCLE.md) — Startup flow, health checks, auto-restart
- [HARDWARE_AND_MODELS.md](HARDWARE_AND_MODELS.md) — Hardware detection and model selection
- [INFERENCE_DEEP_DIVE.md](INFERENCE_DEEP_DIVE.md) — OpenVINO and DirectML FFI wrappers
