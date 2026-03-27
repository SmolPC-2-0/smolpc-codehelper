# Hardware Detection and Model Selection

This document covers how the engine detects available hardware, selects the best inference backend, chooses the right model for the machine's RAM, and how the user can override these decisions from the frontend.

---

## Hardware Detection

### GPU Detection (DXGI)

GPU detection uses the Windows DXGI API (`IDXGIFactory6`) to enumerate graphics adapters. This runs in ~14ms — orders of magnitude faster than WMI queries, which can hang for 60+ seconds on some machines.

The probe:
1. Creates an `IDXGIFactory1`, then attempts to cast to `IDXGIFactory6` for high-performance GPU preference ordering
2. Enumerates all adapters, extracting per-adapter: name, vendor ID, device ID, dedicated VRAM, and software/hardware classification
3. Filters out software adapters (WARP)
4. Classifies as discrete if VRAM > 512 MB and not a software adapter
5. Selects the best candidate: discrete GPUs first, then by VRAM descending

**Integrated GPU rejection:** If the best candidate is not discrete (e.g., Intel UHD integrated graphics), it is rejected entirely. DirectML inference on integrated GPUs produces garbage output (runaway generation, no EOS detection). These machines fall through to OpenVINO or CPU.

The selected candidate provides a `DirectMlCandidate` with `device_id` (adapter index), `device_name`, `adapter_identity` (format: `0xVENDOR:0xDEVICE`), and `vram_mb`.

### NPU Detection

NPU detection is handled by the OpenVINO startup probe (`probe_openvino_startup` in `openvino.rs`), not by the DXGI probe. It checks whether the Intel NPU device is visible to the OpenVINO runtime and whether the driver supports the required operations.

### RAM Detection

System RAM is queried via the Windows `GlobalMemoryStatusEx` API, returning total physical memory in GB. This determines which model the engine auto-selects.

### App-Side vs Engine-Side Detection

The Tauri app has its own hardware detector (`app/src-tauri/src/hardware/detector.rs`), but GPU detection is disabled there because `sysinfo` v0.32.1 lacks GPU support. The engine's DXGI probe is the authoritative source for GPU information.

---

## Backend Selection

The engine supports three inference backends, selected in priority order:

| Priority | Backend | Hardware Required | Use Case |
|----------|---------|-------------------|----------|
| 1 | DirectML | Discrete GPU (NVIDIA, AMD) | Fastest decode speed |
| 2 | OpenVINO NPU | Intel Core Ultra NPU | Low power, good TTFT |
| 3 | CPU | Any | Universal fallback |

### Selection Algorithm

Backend selection follows a multi-stage decision process:

**1. Forced override (highest priority)**

If `SMOLPC_FORCE_EP` is set (or the user selected a mode in the UI), that backend is used unconditionally. The decision is not persisted.

**2. Persisted decision**

The engine caches successful backend decisions in `backend_decisions.v2.json`. The cache key is a fingerprint of: model ID, artifact fingerprint, app version, runtime versions, GPU adapter identity, NPU identity, and tuning parameters. If a matching record exists with a persisted decision, it is reused — skipping the full probe and preflight.

**3. Failure-based demotion**

If DirectML has failed 3 or more consecutive times (init or runtime failures), it is demoted. The engine falls through to OpenVINO NPU or CPU. Consecutive failure count resets to zero on any successful DirectML load.

**4. Default candidate priority**

Without a forced override, persisted decision, or demotion, the engine uses default priority: DirectML → OpenVINO NPU → CPU, subject to each backend passing its gates.

### Gate Chain

Each backend must pass a chain of gates before it can be selected:

**DirectML gates:**
1. `detected` — DXGI found a discrete GPU
2. `artifact_available` — model has `dml/model.onnx`, `dml/genai_config.json`, `dml/tokenizer.json`
3. `bundle_ready` — ONNX Runtime + DirectML DLLs loaded successfully
4. `preflight_validated` — `build_directml_runtime_adapter()` succeeds within 60 seconds

**OpenVINO NPU gates:**
1. `detected` — NPU device visible to OpenVINO runtime
2. `artifact_available` — model has `openvino/manifest.json` and IR artifacts (`.xml` + `.bin`)
3. `bundle_ready` — all 14 OpenVINO DLLs loaded in dependency order
4. `startup_probe_ready` — OpenVINO startup probe completed (not still pending)
5. `preflight_validated` — OpenVINO preflight succeeds within 300 seconds (5 minutes, includes first-run NPU compilation)

**CPU gates:**
1. Always available (no hardware requirement)
2. `artifact_available` — same OpenVINO IR artifacts as NPU
3. `bundle_ready` — OpenVINO CPU plugin DLLs loaded
4. `preflight_validated` — `build_openvino_cpu_runtime_adapter()` succeeds within 30 seconds

### Preflight Validation

Preflights are timeout-protected via `spawn_blocking` to prevent hung GPU drivers from blocking the entire load path:

| Backend | Timeout | What it does |
|---------|---------|-------------|
| DirectML | 60 seconds | Creates an ORT GenAI session on the selected GPU adapter |
| OpenVINO NPU | 300 seconds | Compiles and loads the model on the NPU (slow on first run, fast when cached) |
| CPU | 30 seconds | Loads the OpenVINO CPU runtime adapter |

### Temporary Fallback

A preflight timeout does **not** permanently demote a backend. It is recorded as `TemporaryFallback` — the decision is not persisted to disk, and the backend will be retried on the next startup. This is critical because:
- NPU compilation on first run can take minutes (but is cached for subsequent runs)
- GPU drivers can occasionally hang but work fine after a restart

Only explicit failures (not timeouts) are persisted as negative decisions.

### Decision Persistence

Successful decisions are cached in `backend_decisions.v2.json` (in the engine data directory under `inference/`). The cache key fingerprints hardware identity, runtime versions, and model artifacts — if any of these change, the cache entry is invalidated and a fresh probe runs.

The file is written atomically (temp file + rename) to prevent corruption.

---

## Model Selection

### Registered Models

| Model | ID | Parameters | Disk Size | Min RAM | Runtime RAM |
|-------|-----|-----------|-----------|---------|-------------|
| Qwen2.5 1.5B Instruct | `qwen2.5-1.5b-instruct` | 1.5B | 0.9 GB | 3.0 GB | ~1.5 GB |
| Qwen3 4B | `qwen3-4b` | 4B | 2.5 GB | 15.0 GB | ~4.0 GB |

### RAM-Based Auto-Selection

On startup, if no model is explicitly requested, the engine selects the best model for the available RAM:

1. Filter models where `min_ram_gb <= total_system_ram`
2. Sort by `min_ram_gb` descending (prefer the larger model)
3. Select the first model whose directory actually exists on disk
4. If the best model's directory is missing, try the next eligible model
5. Last resort: first registered model whose directory exists

In practice:
- **16 GB+ RAM** → `qwen3-4b` (if provisioned)
- **3-15 GB RAM** → `qwen2.5-1.5b-instruct`
- **< 3 GB RAM** → smallest available model (may not work well)

### Model Artifact Format

Models are stored at `%LOCALAPPDATA%\SmolPC\models\{model_id}\`:

**OpenVINO lane** (`openvino/` subdirectory):
- `.xml` + `.bin` — IR model artifacts (not ONNX)
- `manifest.json` — readiness gate (must exist and be valid)
- `tokenizer.json` — used for host-side token counting
- Used by both CPU and NPU backends

**DirectML lane** (`dml/` subdirectory):
- `model.onnx` — ONNX model
- `genai_config.json` — generation config
- `tokenizer.json` — tokenizer
- All three files must exist for DirectML to be available

### Artifact Fingerprinting

The engine computes a fingerprint from model artifact file paths, sizes, and modification times. This fingerprint is part of the backend decision cache key — if model files change, cached decisions are invalidated.

---

## Frontend Runtime Mode Switching

Users can override the automatic backend selection from the UI:

### Flow

1. **User selects mode** in the `InferenceModeSelector` dropdown (Auto, DirectML, CPU, NPU)
2. **Store validates** — rejects if generation is in progress
3. **Tauri command** `set_inference_runtime_mode` parses the mode string and calls `supervisor.set_runtime_mode()`
4. **Supervisor** sends `SetRuntimeMode` command, which triggers an engine restart with the new `SMOLPC_FORCE_EP` environment variable
5. **Model reload** — after the engine restarts, the command reloads the previously active model on the new backend
6. **Status return** — the final `BackendStatus` is returned to the frontend, updating the store

### Mode Availability

The dropdown disables modes that aren't available on the current hardware. Each mode shows a reason when unavailable (e.g., "DirectML: no discrete GPU detected", "NPU: runtime not installed").

### Persistence

The user's mode preference is saved in settings. When the app restarts, it sends the saved preference to the supervisor as part of the `Start` command. The engine uses it as a forced override — it takes priority over the automatic selection algorithm.

---

## Known Constraints

| Constraint | Details |
|-----------|---------|
| DirectML on Intel iGPU | Probe succeeds but output is garbage. Only discrete GPUs accepted. |
| Qwen3-4B INT4 on NPU | Crashes the NPU pipeline. Only INT8_SYM quantization works. |
| NPU fixed context window | `MAX_PROMPT_LEN` (2048) + `MIN_RESPONSE_LEN` (1024). Exceeding input limit crashes with "unknown exception". |
| NPU greedy decoding only | `do_sample=false` enforced. No temperature, top_p, or top_k on NPU. |
| NPU first-run compilation | Can take several minutes. Subsequent runs use cached compilation blobs (`CACHE_DIR`). |
| `SMOLPC_FORCE_EP` in shell | Does NOT reach the engine process. The supervisor explicitly controls env vars via `cmd.env()`. Use the frontend mode selector or the dev script's `-ForceEp` parameter instead. |
| Qwen3 thinking mode on NPU | NPU's `StaticLLMPipeline` does not support `extra_context`. Thinking is suppressed via `/nothink` injected into the system message. Template must be patched to default to non-thinking. |
