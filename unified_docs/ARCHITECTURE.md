# SmolPC Unified Assistant -- Architecture Specification

**Version:** 1.0
**Last Updated:** 2026-03-13
**Status:** Implementation reference -- canonical source of truth for all AI-assisted development sessions

---

## How to Use This Document

This specification is the primary reference for any AI agent (Claude, Codex, or similar) implementing features in the SmolPC Unified Assistant project. It assumes **zero prior context**. Every architectural decision, file path, data structure, communication protocol, and invariant is documented here so that an implementer can read this document alone and build or modify any component correctly.

The document is organized top-down: product overview first, then system architecture, then each subsystem in detail with exact file paths, type signatures, and protocol formats.

---

## Table of Contents

1. [Product Overview](#1-product-overview)
2. [System Architecture](#2-system-architecture)
3. [Process Architecture](#3-process-architecture)
4. [Engine Subsystem (smolpc-engine-host + smolpc-engine-core)](#4-engine-subsystem)
5. [Engine Client (smolpc-engine-client)](#5-engine-client)
6. [Tauri Desktop App (smolpc-codehelper)](#6-tauri-desktop-app)
7. [MCP Integration](#7-mcp-integration)
8. [VS Code Extension (Code Mode)](#8-vs-code-extension)
9. [Model Tiering and Hardware Detection](#9-model-tiering-and-hardware-detection)
10. [Communication Protocols](#10-communication-protocols)
11. [Mode Switching](#11-mode-switching)
12. [Inference Pipeline (End-to-End Request Flow)](#12-inference-pipeline)
13. [Plan Validation Pattern](#13-plan-validation-pattern)
14. [Security Model](#14-security-model)
15. [Build System and Toolchain](#15-build-system-and-toolchain)
16. [Design Principles and Constraints](#16-design-principles-and-constraints)
17. [Critical Invariants](#17-critical-invariants)
18. [File Path Reference](#18-file-path-reference)

---

## 1. Product Overview

### What It Is

SmolPC Code Helper is an **offline AI coding and creative assistant** designed for secondary school students (ages 11--18) on budget Windows laptops with 8 GB RAM. It provides AI assistance across six application modes:

| Mode | Target Application | Product |
|------|--------------------|---------|
| **Code** | VS Code | VS Code extension |
| **GIMP** | GIMP 2.10+ | Unified Tauri app |
| **Blender** | Blender 3.x/4.x | Unified Tauri app |
| **Writer** | LibreOffice Writer | Unified Tauri app |
| **Calc** | LibreOffice Calc | Unified Tauri app |
| **Impress** | LibreOffice Impress | Unified Tauri app |

### Two Products, One Engine

The architecture produces two user-facing products that share a single inference engine:

1. **Unified Tauri App** (`smolpc-codehelper.exe`) -- Desktop application with a mode dropdown for GIMP, Blender, Writer, Calc, and Impress.
2. **VS Code Extension** -- For Code mode only, providing ghost-text autocomplete and a chat panel.

Both connect to the same **smolpc-engine-host** inference server. The engine is never duplicated.

### Key Constraints

- **Offline-only**: No cloud services, no telemetry, no accounts. All data stays on the student's device.
- **Privacy-first**: GDPR and FERPA compliant by design -- no data ever leaves the machine.
- **Budget hardware**: Primary target is 8 GB RAM Intel laptops. Models must fit in memory alongside the target application.
- **Education-focused**: "Hint don't solve" philosophy. The assistant explains, guides, and teaches rather than producing solutions directly.

---

## 2. System Architecture

### Full System Diagram

```
+--------------------------------------------------------------------+
|                    Student's PC (8 GB+ RAM, Windows)               |
|                                                                    |
|  +----------------------------+     +---------------------------+  |
|  | Unified Tauri App          |     | VS Code Extension         |  |
|  | (smolpc-codehelper.exe)    |     | (Code mode only)          |  |
|  |                            |     |                           |  |
|  | Modes:                     |     | Features:                 |  |
|  |   GIMP                     |     |   Ghost text FIM          |  |
|  |   Blender                  |     |   Chat webview panel      |  |
|  |   Writer                   |     |   Error explanation       |  |
|  |   Calc                     |     |   Code hints              |  |
|  |   Impress                  |     |                           |  |
|  |                            |     |                           |  |
|  | Components:                |     |                           |  |
|  |   Chat UI (Svelte 5)       |     |                           |  |
|  |   Mode dropdown            |     |                           |  |
|  |   MCP client (Rust)        |     |                           |  |
|  +------------+---------------+     +-----------+---------------+  |
|               | HTTP + SSE                      | HTTP + SSE       |
|               |                                 |                  |
|  +------------+---------------------------------+---------------+  |
|  |            smolpc-engine-host (Axum, port 19432)             |  |
|  |                                                              |  |
|  |  +--------------------------------------------------------+ |  |
|  |  | Model Registry + Hardware Detection + Backend Selection | |  |
|  |  +----------------------------+---------------------------+ |  |
|  |                               |                              |  |
|  |  +----------------+  +-------+--------+  +---------------+  |  |
|  |  | onnxruntime-   |  | openvino_genai |  | ort (raw)     |  |  |
|  |  | genai (DML)    |  | (NPU + CPU?)   |  | DEPRECATED    |  |  |
|  |  | ONNX INT4      |  | OV IR INT4     |  |               |  |  |
|  |  +----------------+  +----------------+  +---------------+  |  |
|  +--------------------------------------------------------------+  |
|                                                                    |
|  +--------------------------------------------------------------+  |
|  |                  MCP Servers (Python via uv)                 |  |
|  |  +-----------+  +---------------+  +--------------------+   |  |
|  |  | gimp-mcp  |  | blender-mcp   |  | mcp-libre          |   |  |
|  |  | TCP:10008 |  | TCP:9876      |  | stdio               |   |  |
|  |  |           |  | + HTTP:5179   |  | Writer/Calc/Impress |   |  |
|  |  +-----------+  +---------------+  +--------------------+   |  |
|  +--------------------------------------------------------------+  |
|                                                                    |
|  +----------+  +----------+  +--------------+  +---------------+  |
|  |   GIMP   |  |  Blender |  | LibreOffice  |  |   VS Code     |  |
|  +----------+  +----------+  +--------------+  +---------------+  |
+--------------------------------------------------------------------+
```

### Component Responsibilities

| Component | Responsibility |
|-----------|---------------|
| `smolpc-engine-host` | Axum HTTP server. Model loading, hardware detection, backend selection, inference execution, SSE streaming. Runs as a standalone process. |
| `smolpc-engine-core` | Rust library crate. Runtime adapters (ORT CPU, DirectML GenAI, OpenVINO GenAI NPU), model registry, hardware detection types, generation config, tokenizer wrapper. |
| `smolpc-engine-client` | Rust HTTP client crate. Wraps engine API. Provides `connect_or_spawn()` to auto-start the engine. Used by both Tauri app and VS Code extension backend. |
| Unified Tauri App | Tauri 2 desktop app. Svelte 5 frontend, Rust backend. Chat UI, mode switching, MCP client routing. |
| VS Code Extension | TypeScript extension. Webview chat panel, InlineCompletionItemProvider for ghost text. |
| MCP Servers | Python processes. One per target application. Provide tool definitions and execute actions via JSON-RPC 2.0. |

---

## 3. Process Architecture

Three categories of processes run on the student's machine:

### Process 1: smolpc-codehelper.exe (Tauri 2 App)

- **Binary**: `smolpc-codehelper.exe`
- **Framework**: Tauri 2.6.2 (Rust backend + webview frontend)
- **Frontend**: Svelte 5 with Tailwind CSS 4
- **Roles**: Chat UI rendering, mode switching, MCP client, engine client (via `smolpc-engine-client`)
- **Ports**: None (webview is embedded, not a web server)
- **Lifetime**: Started by user, runs until closed

### Process 2: smolpc-engine-host.exe (Inference Server)

- **Binary**: `smolpc-engine-host.exe`
- **Framework**: Axum 0.8 HTTP server
- **Port**: `19432` (TCP, localhost only)
- **Auth**: Bearer token (random alphanumeric, generated on first spawn, stored in `%LOCALAPPDATA%/SmolPC/engine-token.txt`)
- **Roles**: Model loading/unloading, hardware detection, backend selection (NPU > GPU > CPU), inference execution, SSE token streaming
- **Lifetime**: Spawned by `smolpc-engine-client::connect_or_spawn()` on first client connection. Optionally auto-exits after idle timeout.
- **Concurrency**: Single active generation at a time (guarded by `Semaphore(1)` + queue). Multiple status/health queries allowed concurrently.

### Process 3: MCP Servers (Python)

- **Runtime**: Python via `uv` sidecar (vendored with the app)
- **Protocol**: JSON-RPC 2.0 (MCP standard)
- **Transport**: stdio (primary) or TCP (GIMP, Blender)
- **Lifetime**: Spawned per-mode by the Tauri app's MCP client. Killed on mode switch or app exit.
- **One per application**: GIMP has its own server, Blender has its own, LibreOffice has one shared across Writer/Calc/Impress.

---

## 4. Engine Subsystem

The engine is the core of the system. It is split across two Rust crates plus a host binary.

### 4.1 Crate Structure

```
engine/
  crates/
    smolpc-engine-host/     # Axum HTTP server binary
      src/
        main.rs             # Server setup, routes, backend selection orchestration
        openvino.rs          # OpenVINO startup probe, artifact inspection, preflight
        runtime_bundles.rs   # DLL bundle path resolution, validation
      Cargo.toml

    smolpc-engine-core/     # Library crate (no binary)
      src/
        lib.rs              # Re-exports: BackendStatus, GenerationConfig, GenerationMetrics, etc.
        hardware/
          mod.rs
          detector.rs        # Hardware fingerprinting (CPU, GPU, NPU, RAM)
          types.rs           # HardwareInfo, CpuInfo, GpuInfo, NpuInfo, MemoryInfo, StorageInfo
          errors.rs          # HardwareError type
        inference/
          mod.rs             # Re-exports for inference subsystem
          runtime_adapter.rs # InferenceRuntimeAdapter enum (dispatch to backend)
          runtime_loading.rs # DLL loading (OrtRuntimeLoader, OpenVinoRuntimeLoader, RetainedLibrary)
          generator.rs       # ORT CPU autoregressive loop (Generator struct)
          session.rs         # ORT InferenceSession wrapper
          tokenizer.rs       # TokenizerWrapper (HuggingFace tokenizers crate)
          input_builder.rs   # Tensor input construction for ORT
          kv_cache.rs        # KV cache management for ORT CPU path
          benchmark.rs       # Backend benchmark runner
          backend.rs         # InferenceBackend enum, BackendStatus, BackendDecision, DecisionReason
          backend_store.rs   # Persisted backend decision records (JSON file)
          types.rs           # GenerationConfig, GenerationMetrics, GenerationResult, InferenceChatMessage
          genai/
            mod.rs           # Re-exports GenAiDirectMlGenerator, OpenVinoGenAiGenerator
            directml.rs      # DirectML GenAI native C FFI generator
            openvino.rs      # OpenVINO GenAI native C FFI generator
        models/
          mod.rs
          registry.rs        # ModelRegistry, ModelDefinition (hard-coded model list)
          loader.rs          # ModelLoader (path resolution for model artifacts)
          runtime_spec.rs    # ModelRuntimeSpec, ModelArchitecture, ModelIoSpec, KvInputSchema

    smolpc-engine-client/   # HTTP client library crate
      src/
        lib.rs              # EngineClient, connect_or_spawn(), streaming
        test_utils.rs       # Test helpers
      Cargo.toml
```

### 4.2 HTTP API

Base URL: `http://127.0.0.1:19432`

All endpoints require: `Authorization: Bearer <token>`

#### Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/engine/health` | Health check. Returns `{ "ok": true }` |
| `GET` | `/engine/meta` | Protocol/runtime metadata (version, PID, busy state) |
| `GET` | `/engine/status` | Full readiness state, active backend, model info, lane diagnostics |
| `POST` | `/engine/ensure-started` | Trigger startup sequence (resolve assets, probe hardware, load model) |
| `POST` | `/engine/load` | Load/switch to a specific model by ID |
| `POST` | `/engine/unload` | Unload current model (optionally force-cancel active generation) |
| `POST` | `/engine/cancel` | Cancel in-flight generation |
| `POST` | `/engine/shutdown` | Graceful shutdown |
| `POST` | `/engine/check-model` | Check model artifact readiness per lane without loading |
| `GET` | `/v1/models` | OpenAI-compatible model list |
| `POST` | `/v1/chat/completions` | OpenAI-compatible chat completions (SSE streaming) |

#### POST /v1/chat/completions -- Request Format

```json
{
  "model": "qwen3-4b-instruct-2507",
  "messages": [
    { "role": "system", "content": "You are a helpful GIMP assistant." },
    { "role": "user", "content": "How do I resize an image?" }
  ],
  "stream": true,
  "max_tokens": 2048,
  "temperature": 0.7,
  "top_k": 50,
  "top_p": 0.9,
  "repetition_penalty": 1.1,
  "repetition_penalty_last_n": 64
}
```

#### POST /v1/chat/completions -- SSE Response Format

When `stream: true`, the response is `Content-Type: text/event-stream`. Each event follows the OpenAI SSE format:

```
data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1710000000,"model":"qwen3-4b-instruct-2507","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1710000000,"model":"qwen3-4b-instruct-2507","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1710000000,"model":"qwen3-4b-instruct-2507","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":12,"completion_tokens":50,"total_tokens":62}}

data: [DONE]
```

When `stream: false`, a single JSON response is returned with the full generated text.

#### POST /engine/ensure-started -- Request Format

```json
{
  "mode": "auto",
  "startup_policy": {
    "default_model_id": "qwen3-4b-instruct-2507"
  }
}
```

`mode` values: `"auto"` (default, engine selects best backend), `"directml_required"` (force GPU).

#### GET /engine/status -- Response Structure

```json
{
  "ok": true,
  "ready": true,
  "attempt_id": "a1b2c3d4",
  "state": "ready",
  "state_since": "2026-03-13T10:00:00Z",
  "active_backend": "directml",
  "active_model_id": "qwen3-4b-instruct-2507",
  "error_code": null,
  "error_message": null,
  "retryable": null,
  "engine_version": "0.1.0",
  "engine_api_version": "1.0.0",
  "current_model": "qwen3-4b-instruct-2507",
  "generating": false,
  "backend_status": {
    "runtime_engine": "genai_dml",
    "selection_state": "ready",
    "selection_reason": "persisted_decision",
    "decision_persistence_state": "persisted",
    "available_backends": ["cpu", "directml", "openvino_npu"],
    "selected_device": {
      "backend": "directml",
      "device_id": 0,
      "device_name": "Intel(R) UHD Graphics"
    },
    "runtime_bundles": {
      "load_mode": "production",
      "ort": { "root": "...", "fingerprint": "...", "validated": true, "failure": null },
      "directml": { "root": "...", "fingerprint": "...", "validated": true, "failure": null },
      "openvino": { "root": "...", "fingerprint": "...", "validated": false, "failure": "missing_root" }
    },
    "lanes": {
      "cpu": { "detected": true, "bundle_ready": true, "artifact_ready": true, "startup_probe_state": "ready", "preflight_state": "not_started" },
      "directml": { "detected": true, "bundle_ready": true, "artifact_ready": true, "startup_probe_state": "ready", "preflight_state": "ready" },
      "openvino_npu": { "detected": true, "bundle_ready": false, "artifact_ready": false, "startup_probe_state": "not_started", "preflight_state": "not_started" }
    }
  }
}
```

**Readiness states** (enum `ReadinessState`): `idle`, `starting`, `resolving_assets`, `probing`, `loading_model`, `ready`, `failed`.

**Runtime engine values**: `ort_cpu`, `genai_dml`, `ov_genai_npu`, `null`.

### 4.3 Runtime Adapters

The engine supports three inference backends through the `InferenceRuntimeAdapter` dispatch enum.

**File**: `engine/crates/smolpc-engine-core/src/inference/runtime_adapter.rs`

```rust
pub enum InferenceRuntimeAdapter {
    Ort { generator: Generator },
    GenAiDirectMl { generator: GenAiDirectMlGenerator },     // Windows only
    OpenVinoGenAiNpu { generator: OpenVinoGenAiGenerator },  // Windows only
}
```

All three variants implement the same interface:

```rust
pub async fn generate_stream<F>(
    &self,
    prompt: &str,
    config: Option<GenerationConfig>,
    cancelled: Arc<AtomicBool>,
    on_token: F,
) -> Result<GenerationMetrics, String>
where
    F: FnMut(String);
```

The OpenVINO variant also supports structured chat messages (role/content pairs passed to the native chat template):

```rust
pub async fn generate_stream_messages<F>(
    &self,
    messages: &[InferenceChatMessage],
    config: Option<GenerationConfig>,
    cancelled: Arc<AtomicBool>,
    on_token: F,
) -> Result<GenerationMetrics, String>
where
    F: FnMut(String);
```

#### Backend Comparison

| Property | `Ort` (CPU) | `GenAiDirectMl` (GPU) | `OpenVinoGenAiNpu` (NPU) |
|----------|-------------|----------------------|--------------------------|
| Runtime | Raw `ort` crate (v2.0.0-rc.11) | onnxruntime-genai C FFI | openvino_genai C FFI |
| DLLs | `onnxruntime.dll`, `onnxruntime_providers_shared.dll` | + `onnxruntime-genai.dll`, `DirectML.dll` | `openvino.dll`, `openvino_c.dll`, `openvino_genai.dll`, `openvino_genai_c_api.dll`, `openvino_tokenizers.dll`, `openvino_intel_npu_plugin.dll`, `openvino_intel_cpu_plugin.dll`, `openvino_ir_frontend.dll`, `tbb12.dll`, + supporting ICU/TBB DLLs |
| Model format | ONNX | ONNX INT4 | OpenVINO IR INT4 (`.xml` + `.bin`) |
| Tokenization | Rust `tokenizers` crate (`TokenizerWrapper`) | GenAI pipeline (built-in) | GenAI pipeline (built-in) |
| KV cache | Manual Rust implementation (`kv_cache.rs`) | GenAI pipeline (built-in) | GenAI pipeline (built-in) |
| Sampling | Manual Rust implementation (top-k, top-p, temp) | GenAI pipeline (built-in) | GenAI pipeline (built-in) |
| Chat template | Manual ChatML formatting in Rust | GenAI handles via `genai_config.json` | OpenVINO handles via model directory config |
| Status | **DEPRECATED -- migrating away** | Active | Active |
| `runtime_engine` value | `ort_cpu` | `genai_dml` | `ov_genai_npu` |

#### Migration Plan (3 backends to 2)

The raw `ort` backend is the weakest -- it requires manual KV cache management, tokenization, and sampling. Both GenAI-based backends handle these automatically. The migration target is:

**Option A (preferred)**: onnxruntime-genai for CPU + GPU, openvino_genai for NPU only
**Option B**: openvino_genai for CPU + NPU, onnxruntime-genai for GPU only

The decision depends on benchmark results comparing onnxruntime-genai's CPU EP (with OpenVINO acceleration) vs openvino_genai's CPU device.

### 4.4 DLL Loading Architecture

**Critical invariant**: All `Library::new()` and `load_with_flags()` calls MUST live in `runtime_loading.rs`. This is enforced by the `runtime_loading_is_centralized` test, which scans all `.rs` files in the engine crates and fails if any other file calls these functions.

**File**: `engine/crates/smolpc-engine-core/src/inference/runtime_loading.rs`

Key types:

```rust
pub enum RuntimeFamily { Ort, OpenVino }

pub struct OrtRuntimeBundle {
    pub bundle_root: PathBuf,
    pub onnxruntime_dll: PathBuf,
    pub onnxruntime_providers_shared_dll: PathBuf,
    pub onnxruntime_genai_dll: PathBuf,
    pub directml_dll: PathBuf,
    pub fingerprint: RuntimeBundleFingerprint,
    // validation failure fields...
}

pub struct OpenVinoRuntimeBundle {
    pub bundle_root: PathBuf,
    pub openvino_dll: PathBuf,
    pub openvino_c_dll: PathBuf,
    pub openvino_genai_dll: PathBuf,
    pub openvino_genai_c_dll: PathBuf,
    pub openvino_tokenizers_dll: PathBuf,
    pub openvino_intel_npu_plugin_dll: PathBuf,
    pub openvino_intel_cpu_plugin_dll: PathBuf,
    pub openvino_ir_frontend_dll: PathBuf,
    pub tbb_dll: PathBuf,
    // + tbbbind, tbbmalloc, tbbmalloc_proxy, icudt, icuuc
    pub fingerprint: RuntimeBundleFingerprint,
    // validation failure fields...
}

pub struct OrtRuntimeLoader;   // ensure_initialized(bundle) -> Result<OrtRuntimeHandle>
pub struct OpenVinoRuntimeLoader;  // ensure_initialized(bundle) -> Result<OpenVinoRuntimeHandle>
```

**DLL hijacking prevention**: On Windows, `load_with_flags()` uses `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32` to restrict DLL search to the bundle directory and System32. All paths must be absolute (validated before loading).

**OpenVINO DLL load order** (dependency chain, must be loaded in this order):

```
tbb12.dll
tbbbind.dll
tbbmalloc.dll
tbbmalloc_proxy.dll
openvino.dll
openvino_c.dll
openvino_ir_frontend.dll
openvino_intel_cpu_plugin.dll
openvino_intel_npu_compiler.dll
openvino_intel_npu_plugin.dll
icudt75.dll
icuuc75.dll
openvino_tokenizers.dll
openvino_genai.dll
openvino_genai_c_api.dll
```

**Caching**: Both loaders use `OnceLock<Mutex<State>>` for thread-safe initialization. Once a runtime family is initialized with a specific fingerprint, attempting to initialize with a different fingerprint returns an error (process restart required). Same-fingerprint re-initialization returns the cached handle.

### 4.5 Bundle Resolution

**File**: `engine/crates/smolpc-engine-host/src/runtime_bundles.rs`

Resolution modes:

| Mode | When | ORT candidates | OpenVINO candidates |
|------|------|----------------|---------------------|
| **Production** | Release build | `<exe_dir>/libs/` only | `<exe_dir>/libs/openvino/` only |
| **Development** | Debug build | 1. `SMOLPC_ORT_BUNDLE_ROOT` env, 2. Production path, 3. Workspace dev paths | 1. `SMOLPC_OPENVINO_BUNDLE_ROOT` env, 2. Production `libs/openvino/`, 3. Workspace dev paths |

Pinned runtime versions (constants in `runtime_bundles.rs`):
- `OPENVINO_RUNTIME_VERSION = "2026.0.0"`
- `OPENVINO_GENAI_VERSION = "2026.0.0"`
- `OPENVINO_TOKENIZERS_VERSION = "2026.0.0"`
- `ORT_GENAI_VERSION = "bundled"`

All three OpenVINO versions must match -- mixing versions breaks ABI.

### 4.6 Bundle Fingerprinting

**Struct**: `RuntimeBundleFingerprint`

Fingerprint hashes:
- `RuntimeFamily` (ort or openvino)
- Canonical root path (lowercased)
- For each required file: logical name, path (lowercased), existence boolean, file size, mtime (seconds + nanoseconds)
- Version metadata (component name + version string)

**Effect**: Updating, replacing, or removing any DLL automatically changes the fingerprint, which invalidates all cached backend decisions. No manual cache clearing is needed.

### 4.7 Backend Selection

**Priority order**: `openvino_npu` > `directml` > `cpu`

The engine host orchestrates backend selection during the `ensure-started` sequence:

1. **Resolve runtime bundles** -- find and validate ORT + OpenVINO DLL sets
2. **Probe hardware** -- detect GPU adapters (DirectML device enumeration), NPU presence (OpenVINO C API probe)
3. **Check persisted decisions** -- look up `BackendDecisionRecord` by fingerprint in `backend_decisions.v2.json`
4. **OpenVINO lane** (if NPU detected and bundle ready):
   a. **Startup probe** (30s budget): load OpenVINO C API, enumerate devices, check NPU driver version
   b. **Preflight** (300s budget): compile model on NPU, run first-token smoke test
   c. If preflight passes, generator is retained in memory, decision persisted
   d. If preflight times out: `temporary_fallback` (not persisted as negative), fall through
   e. If preflight fails: `OpenVinoPreflightFailed`, fall through
5. **DirectML lane** (if GPU detected and bundle ready):
   a. Startup probe: create GenAI model on DirectML device
   b. Optional benchmark: compare DirectML decode speed vs CPU
   c. Benchmark must show >= 1.30x decode speedup and <= 1.15x TTFT regression
   d. If benchmark fails: fall through to CPU
6. **CPU lane**: always available as final fallback

**Decision persistence** (`BackendDecisionRecord` in `backend_decisions.v2.json`):

```json
{
  "version": 2,
  "records": {
    "<fingerprint>": {
      "key": {
        "model_id": "qwen3-4b-instruct-2507",
        "app_version": "0.1.0",
        "selector_engine_id": "smolpc-engine-host",
        "ort_bundle_fingerprint": "ort:c:/path:abc123",
        "openvino_bundle_fingerprint": "openvino:c:/path:def456",
        "gpu_adapter_identity": "Intel(R) UHD Graphics",
        "selection_profile": "openvino_native_v1"
      },
      "persisted_decision": {
        "backend": "directml",
        "reason": "benchmark_passed"
      },
      "failure_counters": { "directml_failures": 0, "openvino_failures": 0 },
      "updated_at": "2026-03-13T10:00:00Z"
    }
  }
}
```

**Fingerprint key fields**: model_id, model_artifact_fingerprint, app_version, selector_engine_id, ort_bundle_fingerprint, openvino_bundle_fingerprint, gpu_adapter_identity, gpu_driver_version, gpu_device_id, npu_adapter_identity, npu_driver_version, selection_profile, openvino_npu_max_prompt_len, openvino_npu_min_response_len, openvino_message_mode.

Any change to any of these fields produces a new fingerprint, forcing a fresh backend evaluation.

**Decision reason codes** (`DecisionReason` enum):
- `default_cpu`, `default_openvino_candidate`, `default_directml_candidate`
- `forced_override`, `persisted_decision`
- `benchmark_passed`, `benchmark_directml_decode_too_slow`, `benchmark_ttft_too_high`, `benchmark_budget_exceeded`
- `no_directml_candidate`, `directml_initialization_failed`, `directml_preflight_failed`
- `no_openvino_candidate`, `openvino_startup_probe_pending`, `openvino_preflight_failed`, `openvino_preflight_timeout`, `openvino_runtime_unavailable`
- `runtime_failure_fallback`, `demoted_after_failures`

### 4.8 OpenVINO Subsystem

**File**: `engine/crates/smolpc-engine-host/src/openvino.rs`

Functions:
- `probe_openvino_startup()` -- load OpenVINO C API, enumerate devices, check NPU presence, read driver version, compare against floor (`32.0.100.3104`)
- `inspect_openvino_artifact()` -- check model directory for `manifest.json` and required files
- `run_openvino_preflight()` -- create `LlmPipeline` on NPU device, run a short generation, return generator if successful
- `resolve_openvino_npu_tuning()` -- resolve NPU tuning status (prompt length limits, response length minimums)
- `is_blocking_openvino_probe_failure()` -- classify probe failures as blocking vs non-blocking

**NPU driver floor**: `32.0.100.3104` is a recommended minimum, classified as `openvino_npu_driver_recommended_update` (non-blocking warning, not a hard gate).

**NPU compilation caching**: The pipeline constructor receives a `CACHE_DIR` property. First compilation is slow (can take minutes); subsequent loads reuse the compiled blob.

**OpenVINO selection profile**: `OPENVINO_SELECTION_PROFILE = "openvino_native_v1"`. Changing this constant invalidates all existing cached decisions, forcing full re-evaluation.

### 4.9 DirectML GenAI Subsystem

**File**: `engine/crates/smolpc-engine-core/src/inference/genai/directml.rs`

Uses native C FFI to `onnxruntime-genai.dll`:
- Creates `OgaConfig` from model directory
- Clears providers, appends `"dml"` provider
- Sets hardware device ID for multi-GPU systems
- Creates `OgaModel` from config
- Creates `OgaTokenizer` + `OgaTokenizerStream` from model
- Creates `OgaGenerator` with params for streaming

Key FFI symbols loaded from `onnxruntime-genai.dll`:
- `OgaCreateConfig`, `OgaCreateModel`, `OgaCreateTokenizer`
- `OgaCreateGeneratorParams`, `OgaCreateGenerator`
- `OgaGenerator_IsDone`, `OgaGenerator_ComputeLogits`, `OgaGenerator_GenerateNextToken`
- Token decoding via `OgaTokenizerStreamDecode`

**DirectML demotion**: If a DirectML device accumulates >= 3 runtime failures (`DIRECTML_DEMOTION_THRESHOLD`), it is demoted and CPU becomes the active backend.

### 4.10 ORT CPU Subsystem (DEPRECATED)

**Files**:
- `engine/crates/smolpc-engine-core/src/inference/generator.rs` -- autoregressive loop
- `engine/crates/smolpc-engine-core/src/inference/session.rs` -- ORT session wrapper
- `engine/crates/smolpc-engine-core/src/inference/tokenizer.rs` -- HuggingFace tokenizer
- `engine/crates/smolpc-engine-core/src/inference/kv_cache.rs` -- KV cache management
- `engine/crates/smolpc-engine-core/src/inference/input_builder.rs` -- tensor construction

The ORT CPU path manually manages:
- Tokenization (via `tokenizers` crate)
- KV cache allocation and rotation
- Position IDs and attention mask construction
- Top-k / top-p sampling with temperature
- Stop token detection (model-specific: Qwen2.5 has TWO stop tokens: 151643 + 151645)
- ChatML template formatting (`<|im_start|>system\n...<|im_end|>`)

This entire subsystem is being replaced by GenAI pipeline backends.

---

## 5. Engine Client

**File**: `engine/crates/smolpc-engine-client/src/lib.rs`

### 5.1 Key Types

```rust
pub struct EngineClient {
    http: Client,           // reqwest HTTP client
    base_url: String,       // "http://127.0.0.1:19432"
    token: String,          // Bearer token
}

pub struct EngineConnectOptions {
    pub port: u16,                       // Default: 19432
    pub shared_runtime_dir: PathBuf,     // %LOCALAPPDATA%/SmolPC/
    pub data_dir: PathBuf,               // %LOCALAPPDATA%/SmolPC/
    pub runtime_mode: RuntimeModePreference, // Auto, Cpu, Dml
    pub force_respawn: bool,
}

pub enum RuntimeModePreference { Auto, Cpu, Dml }
```

### 5.2 connect_or_spawn()

```rust
pub async fn connect_or_spawn(
    options: EngineConnectOptions,
) -> Result<EngineClient, EngineClientError>;
```

Flow:
1. Create directories (`shared_runtime_dir`, `data_dir`)
2. Load or generate bearer token from `engine-token.txt`
3. Build `EngineClient` with base URL and token
4. Check if engine is already running (health check)
5. If running and compatible: return client
6. If running but incompatible (wrong force mode): restart engine
7. If not running: acquire filesystem-based spawn lock, spawn `smolpc-engine-host.exe` as detached child process, poll `/engine/health` until ready (60s timeout, 250ms poll interval)

**Spawn lock**: `engine-spawn.lock` file in runtime dir. Prevents multiple clients from spawning duplicate engine processes. Stale lock detection at 30s.

**Engine binary resolution**: Searches for `smolpc-engine-host` or `smolpc-engine-host.exe` adjacent to the running binary.

### 5.3 Client API Methods

```rust
// Health and status
pub async fn health(&self) -> Result<bool, EngineClientError>;
pub async fn status(&self) -> Result<EngineStatus, EngineClientError>;
pub async fn ensure_started(&self, mode, policy) -> Result<EngineStatus, EngineClientError>;

// Model management
pub async fn load_model(&self, model_id: &str) -> Result<(), EngineClientError>;

// Generation (non-streaming)
pub async fn generate_text(&self, prompt, config) -> Result<GenerationResult, EngineClientError>;
pub async fn generate_text_messages(&self, messages, config) -> Result<GenerationResult, EngineClientError>;

// Generation (streaming via SSE)
pub async fn generate_stream<F>(&self, prompt, config, on_token: F) -> Result<GenerationMetrics, EngineClientError>;
pub async fn generate_stream_messages<F>(&self, messages, config, on_token: F) -> Result<GenerationMetrics, EngineClientError>;

// Control
pub async fn cancel(&self) -> Result<(), EngineClientError>;
```

The streaming methods parse SSE events from the engine, extract token text from the OpenAI-format delta, and call `on_token(token_text)` for each token. They return `GenerationMetrics` after the stream completes.

### 5.4 Environment Variable Overrides

| Variable | Purpose | Values |
|----------|---------|--------|
| `SMOLPC_FORCE_EP` | Force a specific backend | `cpu`, `dml`, `directml` |
| `SMOLPC_DML_DEVICE_ID` | Force a specific DirectML device | Integer device ID |
| `SMOLPC_MODELS_DIR` | Override model directory | Absolute path |
| `SMOLPC_ORT_BUNDLE_ROOT` | Override ORT DLL directory (dev only) | Absolute path |
| `SMOLPC_OPENVINO_BUNDLE_ROOT` | Override OpenVINO DLL directory (dev only) | Absolute path |
| `SMOLPC_ENGINE_DEFAULT_MODEL_ID` | Override default model selection | Model ID string |

---

## 6. Tauri Desktop App

### 6.1 Overview

The unified Tauri app serves five of the six modes (GIMP, Blender, Writer, Calc, Impress). Code mode lives in the VS Code extension.

**App location**: `apps/codehelper/` (note: named "codehelper" for historical reasons; it is the unified app)

```
apps/codehelper/
  src/                    # Svelte 5 frontend
    App.svelte            # Main application component
    lib/
      stores/
        inference.svelte.ts   # Inference state (Svelte 5 runes)
      types/
        inference.ts          # TypeScript type definitions
  src-tauri/              # Rust backend
    src/
      main.rs             # Tauri app setup
      commands/
        engine_client_adapter.rs  # Tauri commands bridging to engine client
    libs/                 # Vendored runtime DLLs
      onnxruntime.dll
      onnxruntime_providers_shared.dll
      onnxruntime-genai.dll
      DirectML.dll
      openvino/           # NOT YET PRESENT -- needs OpenVINO 2026.0.0 DLLs
    Cargo.toml
  package.json
  svelte.config.js
  vite.config.js
  tsconfig.json
```

### 6.2 Frontend Framework Rules

**Svelte 5 runes only** -- do NOT use Svelte 4 store patterns:

```typescript
// CORRECT -- Svelte 5
let data = $state<T>(initialValue);
export const store = {
    get data() { return data; },
    method() { data = newValue; }
};

// WRONG -- Svelte 4 (DO NOT USE)
import { writable } from 'svelte/store';  // FORBIDDEN
```

**Tailwind CSS 4** -- do NOT use `@apply` (unsupported in Tailwind 4). Use utility classes directly in templates.

### 6.3 Tauri IPC: Channel Streaming Pattern

Token streaming from the engine to the frontend uses **Tauri Channels** (`tauri::ipc::Channel<T>`), NOT Tauri Events.

**Why Channels, not Events**:
- Channels are command-scoped (bound to a single invocation)
- Channels guarantee ordered delivery
- Channels auto-cleanup when the command completes
- Events are global broadcast with potential race conditions on listener registration

**Rust backend** (Tauri command):

```rust
#[tauri::command]
async fn inference_generate(
    on_token: Channel<String>,
    prompt: String,
    // ...
) -> Result<GenerationMetrics, String> {
    let engine_client = get_engine_client().await?;
    engine_client.generate_stream(&prompt, config, |token| {
        let _ = on_token.send(token);
    }).await.map_err(|e| e.to_string())
}
```

**TypeScript frontend**:

```typescript
const channel = new Channel<string>();
channel.onmessage = (token: string) => {
    // Append token to chat bubble in real-time
    appendToken(token);
};
const metrics = await invoke('inference_generate', {
    onToken: channel,
    prompt: userMessage,
});
// invoke() resolves AFTER all tokens have been sent
```

### 6.4 Mode Switching

The app has a mode dropdown in the header. Switching modes changes:

1. **System prompt** -- Each mode has a different system prompt tailored to its application domain
2. **MCP server connection** -- Routes tool calls to the appropriate MCP server
3. **Suggestion chips** -- Quick-action suggestions change per mode
4. **Available tools** -- Each MCP server exposes different tools
5. **Fast paths** -- Some modes have macro-style shortcuts

Mode switching does **NOT**:
- Restart the engine or reload models
- Change the inference backend
- Require reconnecting to the engine
- Clear conversation history (that is a separate user action)

---

## 7. MCP Integration

### 7.1 Architecture

```
+-----------------------------------------------------------+
|                 Unified Tauri App                          |
|  +-------------------------------------------------+      |
|  |  MCP Client (Rust)                              |      |
|  |    stdio transport (spawn process)              |      |
|  |    TCP transport (connect to socket)            |      |
|  |    HTTP transport (future)                      |      |
|  |                                                 |      |
|  |  Per-mode routing:                              |      |
|  |    GIMP     -> maorcc/gimp-mcp   (TCP :10008)  |      |
|  |    Blender  -> blender-mcp       (TCP :9876)   |      |
|  |               + HTTP bridge      (Axum :5179)  |      |
|  |    Writer   -> mcp-libre         (stdio)       |      |
|  |    Calc     -> mcp-libre         (stdio)       |      |
|  |    Impress  -> mcp-libre         (stdio)       |      |
|  |    Code     -> VS Code extension (separate)    |      |
|  +-------------------------------------------------+      |
+-----------------------------------------------------------+
```

### 7.2 MCP Protocol

All MCP servers communicate via JSON-RPC 2.0. Example request:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "crop_image",
    "arguments": { "x": 0, "y": 0, "width": 800, "height": 600 }
  }
}
```

Example response:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      { "type": "text", "text": "Image cropped to 800x600" }
    ]
  }
}
```

### 7.3 Per-Mode MCP Servers

#### GIMP: maorcc/gimp-mcp

- **Repository**: github.com/maorcc/gimp-mcp (58 stars)
- **Transport**: TCP, port 10008
- **Key tools**: Standard GIMP operations plus `call_api` escape hatch for arbitrary GIMP API calls
- **Status**: Already integrated
- **Connection**: GIMP runs a script-fu server plugin that the MCP server connects to

#### Blender: Hybrid Approach

Two integrations running simultaneously:

1. **Existing HTTP bridge** (Axum server on port 5179):
   - Token-authenticated HTTP API
   - Keyword-based RAG for Blender documentation
   - Production features already built and tested
   - Maintained by the SmolPC team

2. **ahujasid/blender-mcp** (17.7K stars):
   - TCP transport, port 9876
   - Primary tool: `execute_blender_code` (runs arbitrary Python in Blender)
   - Broad ecosystem compatibility
   - Added alongside (not replacing) the HTTP bridge

#### LibreOffice: patrup/mcp-libre

- **Repository**: github.com/patrup/mcp-libre
- **Transport**: stdio (spawned by Tauri app)
- **Coverage**: 14 tools standalone, 73 in extension mode
- **Scope**: Covers Writer, Calc, and Impress with a single server
- **Listed on**: Official MCP server listing

### 7.4 MCP Server Lifecycle

1. On mode switch, Tauri app checks if the target mode's MCP server is already running
2. If not running, spawns it via `uv` sidecar (Python package manager bundled with the app)
3. MCP client sends `initialize` handshake
4. Server responds with capabilities and tool list
5. Tools are displayed to the user and available for the model to invoke
6. On mode switch away, the previous MCP server is optionally kept alive (for quick switching back) or terminated

---

## 8. VS Code Extension (Code Mode)

### 8.1 Architecture

Code mode is NOT in the Tauri app. It is a separate VS Code extension.

```
+---------------------------------------------------------+
|                VS Code Extension                        |
|  +-------------------------+  +----------------------+  |
|  | InlineCompletionProvider|  | Webview Chat Panel   |  |
|  | (ghost text FIM)        |  | (React or Svelte)    |  |
|  +-----------+-------------+  +----------+-----------+  |
|              |                            |              |
|  +-----------+----------------------------+-----------+  |
|  |             Engine Client (HTTP)                   |  |
|  |   -> smolpc-engine-host:19432                      |  |
|  |   -> /v1/chat/completions (SSE streaming)          |  |
|  +----------------------------------------------------+  |
|                                                          |
|  Additional VS Code APIs used:                          |
|    vscode.workspace.fs       (file I/O)                 |
|    createTerminal            (terminal access)          |
|    getDiagnostics            (LSP error reading)        |
|    InlineCompletionItemProvider (ghost text)            |
+---------------------------------------------------------+
```

### 8.2 Key Design Decisions

- **Webview for chat, NOT Chat Participant API**: The Chat Participant API requires GitHub Copilot to be installed. SmolPC does not depend on Copilot. A webview panel is used instead.
- **InlineCompletionItemProvider for ghost text**: Same API that Copilot uses for inline suggestions. Works without Copilot installed.
- **TypeScript, not Rust**: VS Code extensions must be TypeScript/JavaScript. The extension uses an HTTP client to talk to the engine (same endpoints as the Tauri app).

### 8.3 Reference Implementations

| Project | LOC | Architecture | Relevance |
|---------|-----|-------------|-----------|
| Continue | ~50K | Full IDE agent | Feature reference (too large for SmolPC scope) |
| Twinny | ~3.6K | FIM + chat | Closest size match (archived) |
| Cline | ~30K | Agentic coding | Tool-use patterns reference |

**SmolPC MVP estimate**: 3--6K LOC TypeScript.

### 8.4 Education Features

- **Hint mode**: When a student asks for code, the extension provides hints and explanations rather than complete solutions
- **Error explanation**: Reads LSP diagnostics (`getDiagnostics`), sends error context to the model, displays plain-language explanations
- **Guided completion**: Ghost text suggests the next few tokens, not entire functions

---

## 9. Model Tiering and Hardware Detection

### 9.1 Model Registry

**File**: `engine/crates/smolpc-engine-core/src/models/registry.rs`

```rust
pub struct ModelDefinition {
    pub id: String,           // Unique identifier
    pub name: String,         // Display name
    pub size: String,         // "1.5B", "4B", etc.
    pub disk_size_gb: f32,    // On-disk size
    pub min_ram_gb: f32,      // Minimum RAM to run
    pub directory: String,    // Model directory name
    pub description: String,
}
```

Current registered models (ordered by priority -- first entry is default):

| ID | Size | Min RAM | Format | Purpose |
|----|------|---------|--------|---------|
| `qwen3-4b-instruct-2507` | 4B | 8 GB | ONNX INT4 | Primary instruct model |
| `qwen2.5-coder-1.5b` | 1.5B | 2 GB | ONNX | Lightweight coding model |
| `qwen3-4b-int4-ov` | 4B | 8 GB | OpenVINO IR INT4 | OpenVINO CPU/NPU testing |
| `qwen3-4b-int4-ov-npu` | 4B | 8 GB | OpenVINO IR INT4 (FluidInference) | NPU-optimized variant |

### 9.2 Model Tiering Strategy

| Tier | RAM | Model Size | Candidates |
|------|-----|-----------|------------|
| Tier 1 (budget) | 8 GB | 0.5--3B params, ~1--2 GB INT4 | Qwen3-1.7B, Qwen3.5-2B, Qwen2.5-Coder-1.5B |
| Tier 2 (capable) | 16+ GB | 4--8B params, ~3--6 GB INT4 | Qwen3-4B, Qwen3.5-4B, Phi-4-mini, Qwen3-8B |

The engine selects the best model for available hardware at startup using `min_ram_gb` from the registry. The frontend shows the engine's recommendation but allows manual override.

### 9.3 Model Artifact Layout

Models are stored in `%LOCALAPPDATA%/SmolPC/models/<model_id>/`:

```
%LOCALAPPDATA%/SmolPC/models/qwen3-4b-instruct-2507/
  cpu/
    model.onnx               # ONNX model for CPU backend
    tokenizer.json            # HuggingFace tokenizer
  dml/
    model.onnx               # ONNX INT4 model for DirectML
    genai_config.json         # GenAI pipeline config
    tokenizer.json
  openvino_npu/
    manifest.json             # Required files list (readiness gate)
    model.xml                 # OpenVINO IR graph definition
    model.bin                 # Model weights
    tokenizer.json
    generation_config.json
    config.json
```

**File**: `engine/crates/smolpc-engine-core/src/models/loader.rs`

Resolution order for models directory:
1. `SMOLPC_MODELS_DIR` environment variable (if set and non-empty)
2. `%LOCALAPPDATA%/SmolPC/models/` (when it exists)
3. Development fallback: `<CARGO_MANIFEST_DIR>/models`

**OpenVINO artifact gate**: The `openvino_npu/manifest.json` file is the readiness gate. If it does not exist, the OpenVINO lane considers this model's artifact as not ready. The manifest enumerates required files so the engine can verify completeness.

**Important**: OpenVINO expects `.xml` + `.bin` IR artifacts, NOT ONNX files. Do not point the OpenVINO lane at an ONNX model.

### 9.4 Hardware Detection

**File**: `engine/crates/smolpc-engine-core/src/hardware/detector.rs`

Uses the `hardware-query` crate (v0.2.1) to detect:

```rust
pub struct HardwareInfo {
    pub cpu: CpuInfo,           // vendor, brand, cores, frequency, features, caches
    pub gpus: Vec<GpuInfo>,     // name, vendor, backend, driver, VRAM, PCI device ID
    pub npu: Option<NpuInfo>,   // detected, confidence, identifier, driver version
    pub memory: MemoryInfo,     // total_gb, available_gb, used_gb
    pub storage: StorageInfo,   // disk info
    pub detected_at: String,
}
```

NPU detection uses a confidence ranking:
- **High**: PCI/USB hardware device ID present
- **Medium**: Driver or TOPS rating present but no stable device ID
- **Low**: Generic heuristic match

GPU vendor classification: Nvidia, AMD, Intel, Apple, Qualcomm, Unknown.

### 9.5 Runtime Spec (ORT CPU + DirectML only)

**File**: `engine/crates/smolpc-engine-core/src/models/runtime_spec.rs`

For models using the raw ORT backend, the registry provides detailed I/O specs:

```rust
pub struct ModelRuntimeSpec {
    pub model_id: &'static str,
    pub backend_target: RuntimeBackendTarget,  // Cpu or DirectML
    pub architecture: ModelArchitecture {
        pub num_layers: usize,    // e.g., 36 for Qwen3-4B
        pub num_kv_heads: usize,  // e.g., 8
        pub head_dim: usize,      // e.g., 128
    },
    pub io: ModelIoSpec {
        pub input_ids: &'static str,
        pub position_ids: Option<&'static str>,  // DirectML needs this, CPU does not
        pub logits: &'static str,
        pub kv_input_schema: KvInputSchema,      // AttentionMask (CPU) or SeqlensK (DML)
        pub past_key_template: &'static str,     // "past_key_values.{layer}.key"
        pub past_value_template: &'static str,
        pub present_key_template: &'static str,  // "present.{layer}.key"
        pub present_value_template: &'static str,
    },
    pub stop_token_ids: &'static [i64],
}
```

This spec is only used by the ORT CPU backend. GenAI backends read their configuration from the model directory (`genai_config.json` or OpenVINO's built-in config).

---

## 10. Communication Protocols

### 10.1 Overview of All Communication Paths

```
Frontend (Svelte 5) --(Tauri IPC + Channel)--> Tauri Backend (Rust)
Tauri Backend --(HTTP + SSE)--> smolpc-engine-host (port 19432)
Tauri Backend --(JSON-RPC 2.0 / stdio or TCP)--> MCP Servers (Python)
VS Code Extension (TypeScript) --(HTTP + SSE)--> smolpc-engine-host (port 19432)
MCP Servers --(application-specific protocol)--> Target Apps (GIMP, Blender, etc.)
```

### 10.2 Tauri IPC

- **Method**: `invoke()` calls from frontend to Rust commands
- **Streaming**: `Channel<T>` (not Events)
- **Serialization**: Tauri's built-in serde JSON serialization
- **Error handling**: Rust `Result<T, String>` maps to TypeScript promise rejection

### 10.3 Engine HTTP Protocol

- **Transport**: HTTP/1.1 over TCP, localhost only
- **Port**: 19432 (hardcoded default, configurable via CLI args)
- **Auth**: `Authorization: Bearer <token>` header on every request
- **Streaming**: SSE (`text/event-stream`) for `/v1/chat/completions` with `stream: true`
- **Content type**: `application/json` for request/response bodies
- **Concurrency**: Single generation at a time (Semaphore(1)), multiple read-only requests allowed

### 10.4 MCP Protocol

- **Standard**: Model Context Protocol (MCP) by Anthropic
- **Wire format**: JSON-RPC 2.0
- **Transports**:
  - **stdio**: Tauri spawns MCP server as child process, communicates via stdin/stdout
  - **TCP**: Client connects to server socket (GIMP on :10008, Blender on :9876)
  - **HTTP**: Future option, not yet implemented
- **Key methods**: `initialize`, `tools/list`, `tools/call`

### 10.5 Engine SSE Event Format

Each SSE event is a single `data:` line containing JSON:

```
data: {"id":"chatcmpl-<uuid>","object":"chat.completion.chunk","created":<epoch>,"model":"<model_id>","choices":[{"index":0,"delta":{"content":"<token>"},"finish_reason":null}]}
```

Final event includes `finish_reason` and optional `usage`:

```
data: {"id":"chatcmpl-<uuid>","object":"chat.completion.chunk","created":<epoch>,"model":"<model_id>","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":<n>,"completion_tokens":<n>,"total_tokens":<n>}}
```

Stream terminator:

```
data: [DONE]
```

---

## 11. Mode Switching

### 11.1 What Changes Per Mode

| Aspect | GIMP | Blender | Writer | Calc | Impress |
|--------|------|---------|--------|------|---------|
| System prompt | Image editing expert | 3D modeling expert | Document editing expert | Spreadsheet expert | Presentation expert |
| MCP server | gimp-mcp (TCP:10008) | blender-mcp (TCP:9876) + HTTP bridge (:5179) | mcp-libre (stdio) | mcp-libre (stdio) | mcp-libre (stdio) |
| Tool examples | crop, resize, draw_line, call_api | execute_blender_code, scene operations | insert_text, format_paragraph | insert_formula, format_cells | add_slide, set_layout |
| Suggestion chips | "Resize image", "Add filter" | "Create cube", "Set material" | "Format heading", "Insert table" | "Sum column", "Create chart" | "Add slide", "Insert image" |

### 11.2 What Does NOT Change

- Engine process (stays running, model stays loaded)
- Inference backend (CPU/DML/NPU decision is persistent)
- Engine connection (HTTP client is reused)
- Bearer token
- Model selection

### 11.3 Mode Switch Sequence

1. User selects new mode from dropdown
2. Frontend notifies Tauri backend of mode change
3. Tauri backend updates system prompt for new mode
4. Tauri backend checks if target MCP server is already connected
5. If not connected: spawn MCP server process, perform initialize handshake
6. Tauri backend queries `tools/list` from new MCP server
7. Frontend updates UI: suggestion chips, available tool indicators
8. User continues chatting -- next inference request uses new system prompt and tool set

---

## 12. Inference Pipeline

### 12.1 End-to-End Request Flow

Step-by-step walkthrough from user keystroke to displayed response:

**Phase 1: Frontend**
1. User types a message in the chat input and presses Enter
2. Frontend creates a `Channel<string>` for receiving streamed tokens
3. Frontend calls `invoke("assistant_chat_stream", { prompt, onToken: channel })`
4. Frontend adds an empty assistant message bubble to the chat view

**Phase 2: Tauri Backend**
5. Tauri command handler receives the invocation
6. Handler builds the messages array: system prompt (based on current mode) + conversation history + user message
7. Handler calls `engine_client.generate_stream_messages(messages, config, |token| { channel.send(token) })`

**Phase 3: Engine Client**
8. Client constructs HTTP POST to `http://127.0.0.1:19432/v1/chat/completions` with `stream: true`
9. Client opens SSE connection and begins reading events

**Phase 4: Engine Host**
10. Host validates bearer token
11. Host acquires generation semaphore (or queues if busy)
12. Host retrieves loaded `InferenceRuntimeAdapter`
13. Host formats messages for the active backend:
    - OpenVINO NPU: passes `InferenceChatMessage` structs directly (native chat template)
    - DirectML: formats into single prompt string via GenAI config
    - ORT CPU: formats as ChatML manually
14. Host calls `adapter.generate_stream()` or `adapter.generate_stream_messages()`

**Phase 5: Runtime Adapter**
15. Backend-specific generator produces tokens one at a time
16. Each token is sent via the `on_token` callback
17. Callback creates an SSE event in OpenAI chunk format and sends it over the HTTP response stream

**Phase 6: Streaming Back**
18. Engine client parses each SSE `data:` event, extracts `delta.content`
19. Client calls `on_token(content)` for each token
20. Tauri backend receives each token and sends it through the Channel
21. Frontend's `channel.onmessage` handler appends the token to the chat bubble in real-time
22. Chat bubble re-renders with the accumulated text

**Phase 7: Completion**
23. Generator finishes (stop token or max length reached)
24. Engine host sends final SSE event with `finish_reason: "stop"` and `usage` metrics
25. Engine host sends `data: [DONE]`
26. Engine client returns `GenerationMetrics` to the Tauri backend
27. Tauri command returns `GenerationMetrics` as the `invoke()` result
28. Frontend receives the resolved promise, finalizes the chat bubble

**Phase 8: Tool Calls (if applicable)**
29. After generation, if the response contains tool call markup:
    a. Tauri backend parses tool calls from the model's output
    b. Routes each tool call to the appropriate MCP server (based on current mode)
    c. Sends JSON-RPC `tools/call` request to MCP server
    d. Collects tool result
    e. Feeds result back to the model as a new message for the next turn
    f. Or displays result directly to the user (depending on the tool type)

### 12.2 Cancellation

At any point during generation:
1. User clicks "Stop" button in the frontend
2. Frontend calls `invoke("cancel_generation")`
3. Tauri backend calls `engine_client.cancel()`
4. Engine host sets `AtomicBool` cancellation flag
5. Runtime adapter checks flag between token generations and stops early
6. Partial response is displayed as-is

### 12.3 Generation Config

```rust
pub struct GenerationConfig {
    pub max_length: usize,            // Default: 2048
    pub temperature: f32,             // Default: 1.0
    pub top_k: Option<usize>,         // Default: None
    pub top_p: Option<f32>,           // Default: None
    pub repetition_penalty: f32,      // Default: 1.1
    pub repetition_penalty_last_n: usize,  // Default: 64
}
```

These map to the OpenAI-compatible request fields: `max_tokens`, `temperature`, `top_k`, `top_p`, `repetition_penalty`, `repetition_penalty_last_n`.

### 12.4 Generation Metrics

```rust
pub struct GenerationMetrics {
    pub total_tokens: usize,           // Completion tokens only
    pub time_to_first_token_ms: Option<u64>,  // TTFT
    pub tokens_per_second: f64,        // Decode throughput
    pub total_time_ms: u64,            // Wall clock time
}
```

---

## 13. Plan Validation Pattern

### 13.1 Overview

The GIMP assistant established a proven pattern for structured tool execution that should be generalized across all modes in the unified app.

### 13.2 Flow

1. **User request**: "Resize this image to 800x600 and add a gaussian blur"
2. **Model generates a plan**: Structured list of steps, each mapping to a tool call
3. **Plan validation**: Each step is checked against the available tool list from the active MCP server
4. **Sequential execution**: Steps are executed one at a time via MCP `tools/call`
5. **Result collection**: Each step's result is collected
6. **Summary**: Results are summarized and presented to the user
7. **Undo support**: User can undo the last operation (where the target application supports it)

### 13.3 Implementation Approach

The plan can be represented as:

```typescript
interface PlanStep {
  tool_name: string;
  arguments: Record<string, unknown>;
  description: string;  // Human-readable explanation
}

interface Plan {
  steps: PlanStep[];
  summary: string;
}
```

Validation checks:
- Each `tool_name` exists in the current mode's tool list
- Required arguments are present
- Argument types match the tool's schema

### 13.4 Generalization for All Modes

Each mode can use the same plan pattern with mode-specific tools:

| Mode | Example Plan Steps |
|------|-------------------|
| GIMP | `crop_image`, `apply_filter`, `resize_canvas` |
| Blender | `execute_blender_code` (create object), `execute_blender_code` (set material) |
| Writer | `insert_text`, `format_paragraph`, `insert_table` |
| Calc | `insert_formula`, `format_cells`, `create_chart` |
| Impress | `add_slide`, `insert_image`, `set_layout` |

---

## 14. Security Model

### 14.1 Network Security

- Engine listens on `127.0.0.1:19432` (loopback only -- no remote access)
- All requests require `Authorization: Bearer <token>` header
- Token is randomly generated (alphanumeric) on first engine spawn
- Token is stored in `%LOCALAPPDATA%/SmolPC/engine-token.txt`
- Token is shared between engine host and clients via filesystem

### 14.2 DLL Security

- All DLL paths must be absolute (validated before loading)
- `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32` flags prevent DLL hijacking
- DLL loading is centralized in `runtime_loading.rs` (enforced by test)
- No DLLs are loaded from PATH or current directory

### 14.3 Data Privacy

- No network connections except loopback
- No telemetry or analytics
- No user accounts or authentication against external services
- All model inference runs locally
- Conversation history stored locally only (if persistence is implemented)
- GDPR and FERPA compliant by design

### 14.4 MCP Server Security

- MCP servers run as local child processes
- stdio transport has no network exposure
- TCP transport binds to localhost only
- MCP servers are spawned by the trusted Tauri app, not by external code

---

## 15. Build System and Toolchain

### 15.1 Workspace Configuration

**File**: `Cargo.toml` (workspace root)

```toml
[workspace]
members = [
  "apps/codehelper/src-tauri",
  "apps/gimp-assistant/src-tauri",
  "engine/crates/smolpc-engine-core",
  "engine/crates/smolpc-engine-host",
  "engine/crates/smolpc-engine-client",
]
resolver = "2"

[workspace.package]
edition = "2021"
rust-version = "1.88"
license = "MIT"
```

### 15.2 Key Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `axum` | 0.8 | HTTP server framework |
| `tokio` | 1 (full features) | Async runtime |
| `reqwest` | 0.12 (json, stream) | HTTP client |
| `ort` | 2.0.0-rc.11 (pinned) | ONNX Runtime Rust bindings |
| `libloading` | 0.8 | Dynamic library loading |
| `tokenizers` | 0.20 | HuggingFace tokenizer |
| `serde` / `serde_json` | 1.0 | Serialization |
| `hardware-query` | 0.2.1 | Hardware fingerprinting |
| `half` | 2.4 | f16 support (DirectML) |
| `ndarray` | 0.17 | Tensor operations |
| `windows-sys` | 0.59.0 | Windows API (MoveFileExW for atomic writes) |
| `chrono` | 0.4 | Timestamps |
| `dirs` | 6 | Platform directory resolution |

### 15.3 Build Commands

```bash
# Check all crates compile
cargo check --workspace

# Run all tests
cargo test -p smolpc-engine-core
cargo test -p smolpc-engine-host
cargo test -p smolpc-engine-client

# Lint
cargo clippy --workspace

# Run engine server (development)
cargo run -p smolpc-engine-host

# Run Tauri app (development)
cd apps/codehelper && npm run tauri dev

# Frontend only
cd apps/codehelper && npm run dev

# TypeScript checks
cd apps/codehelper && npm run check && npm run lint
```

### 15.4 Tauri Version

Tauri 2.6.2 (Tauri 2.x, NOT Tauri 1.x). Uses:
- `tauri::ipc::Channel<T>` for streaming (Tauri 2 API)
- `#[tauri::command]` attribute macros
- Webview2 on Windows

---

## 16. Design Principles and Constraints

### 16.1 Core Principles

1. **Offline-first**: No cloud dependency, no network calls beyond loopback. All AI inference runs on the student's device.

2. **Privacy-first**: No data leaves the device. No telemetry. No accounts. GDPR and FERPA compliant by design.

3. **Budget hardware**: The primary target is 8 GB RAM Intel laptops commonly issued by schools. Models must fit in memory alongside the target application (GIMP, Blender, etc.). The 8 GB tier uses 1.5--3B parameter models.

4. **Shared engine**: One inference server process serves both the Tauri app and the VS Code extension. Model loading happens once, not per-client.

5. **Mode-agnostic core**: The chat UI, streaming infrastructure, and engine connection are identical across all modes. Only the system prompt, available tools, and MCP routing change.

6. **Education-focused**: "Hint don't solve." The assistant explains concepts, provides guided hints, and teaches rather than producing ready-made solutions.

### 16.2 Hardware Constraints

- **Minimum RAM**: 8 GB total system
- **Target CPU**: Intel Core (11th gen+), Intel Core Ultra (for NPU)
- **GPU**: Intel UHD/Iris Xe (iGPU) via DirectML; discrete GPUs supported but not required
- **NPU**: Intel Core Ultra Neural Processing Unit via OpenVINO GenAI
- **Storage**: SSD preferred for model loading speed; HDD supported
- **OS**: Windows 10/11

### 16.3 Performance Targets

- **Time to first token (TTFT)**: < 2 seconds on 8 GB RAM device
- **Decode throughput**: > 5 tokens/second on CPU, > 10 on GPU, > 15 on NPU
- **Model load time**: < 10 seconds (cold), < 2 seconds (warm with NPU cache)
- **Mode switch**: < 500ms (model stays loaded)

---

## 17. Critical Invariants

These are rules that MUST NOT be violated. Breaking any of these will cause test failures, security issues, or architectural regressions.

### 17.1 DLL Loading Centralization

All `Library::new()` and `load_with_flags()` calls must be in `runtime_loading.rs`. The test `runtime_loading_is_centralized` scans every `.rs` file in the `engine/crates/` directory tree and fails if any other file contains these patterns.

### 17.2 Absolute DLL Paths

`validate_runtime_library_path()` rejects relative paths. Every DLL path passed to the loader must be absolute.

### 17.3 OpenVINO Version Tuple

`OPENVINO_RUNTIME_VERSION`, `OPENVINO_GENAI_VERSION`, and `OPENVINO_TOKENIZERS_VERSION` must all be the same version string (`"2026.0.0"`). Mixing versions breaks ABI.

### 17.4 Preflight Timeout is Temporary

An OpenVINO preflight timeout produces a `temporary_fallback` decision, NOT a persisted negative decision. It does not overwrite a prior successful decision. The timeout may be transient (first NPU compilation is slow).

### 17.5 Engine Owns Backend Selection

The engine host (`smolpc-engine-host`) owns all backend selection logic. The launcher, Tauri app, and VS Code extension must NOT rank backends or override engine decisions. They consume status only.

### 17.6 Svelte 5 Only

No Svelte 4 patterns. No `writable()`, `derived()`, or `import { ... } from 'svelte/store'`. Use `$state`, `$derived`, `$effect` runes.

### 17.7 Tailwind 4 Only

No `@apply` directives. Use utility classes directly in component templates.

### 17.8 ChatML Required (ORT CPU Path)

Without `<|im_start|>` / `<|im_end|>` ChatML formatting, Qwen models regurgitate pretraining data instead of following instructions. The ORT CPU path must format all prompts as ChatML.

### 17.9 Qwen Stop Tokens

Qwen2.5 models have TWO stop tokens: `<|endoftext|>` (151643) and `<|im_end|>` (151645). Both must be checked. Qwen3 models use only `<|im_end|>` (151645).

### 17.10 OpenVINO Uses IR, Not ONNX

The OpenVINO lane expects `.xml` + `.bin` IR artifacts. Do not point it at ONNX files. The `openvino_npu/manifest.json` must exist for artifact readiness.

### 17.11 ORT Crate Version Pinned

`ort` is pinned to exactly `2.0.0-rc.11` via `version = "=2.0.0-rc.11"` in Cargo.toml and `ORT_CRATE_VERSION` constant. Do not upgrade without updating both and re-validating the DML lane.

### 17.12 Bearer Token Required

Every HTTP request to the engine must include `Authorization: Bearer <token>`. Unauthenticated requests are rejected.

---

## 18. File Path Reference

### 18.1 Repository Structure

```
unified-assistant/                    # Workspace root
  Cargo.toml                          # Workspace Cargo.toml
  Cargo.lock
  rust-toolchain.toml                 # Rust 1.88
  package.json                        # Root npm config
  claude.md                           # AI session instructions (CLAUDE.md)

  apps/
    codehelper/                       # Unified Tauri app
      src/                            # Svelte 5 frontend
      src-tauri/                      # Rust backend
        src/
          main.rs
          commands/
            engine_client_adapter.rs
        libs/                         # Vendored DLLs
          onnxruntime.dll
          onnxruntime_providers_shared.dll
          onnxruntime-genai.dll
          DirectML.dll
          openvino/                   # NOT YET PRESENT
        Cargo.toml
      package.json
      svelte.config.js
      vite.config.js
      tsconfig.json

    gimp-assistant/                   # Legacy GIMP-specific app (being merged into codehelper)
      src/
      src-tauri/
      package.json

    blender-assistant/                # Legacy Blender-specific app (being merged into codehelper)
    libreoffice-assistant/            # Legacy LibreOffice-specific app (being merged into codehelper)

  engine/
    crates/
      smolpc-engine-host/            # Axum HTTP server binary
        src/
          main.rs                    # Routes, backend selection, startup orchestration
          openvino.rs                # OpenVINO probe, preflight, artifact checks
          runtime_bundles.rs         # DLL bundle resolution
        Cargo.toml

      smolpc-engine-core/            # Core inference library
        src/
          lib.rs                     # Public re-exports
          hardware/
            mod.rs
            detector.rs              # Hardware fingerprinting
            types.rs                 # HardwareInfo, CpuInfo, GpuInfo, NpuInfo
            errors.rs
          inference/
            mod.rs
            runtime_adapter.rs       # InferenceRuntimeAdapter dispatch enum
            runtime_loading.rs       # Centralized DLL loading (SOLE LOCATION)
            generator.rs             # ORT CPU autoregressive loop (DEPRECATED)
            session.rs               # ORT InferenceSession wrapper
            tokenizer.rs             # HuggingFace tokenizer wrapper
            input_builder.rs         # Tensor input construction
            kv_cache.rs              # KV cache management
            benchmark.rs             # Backend benchmark runner
            backend.rs               # InferenceBackend, BackendStatus, decisions
            backend_store.rs         # Persisted decision records (JSON)
            types.rs                 # GenerationConfig, GenerationMetrics, etc.
            genai/
              mod.rs                 # Re-exports
              directml.rs            # DirectML GenAI FFI
              openvino.rs            # OpenVINO GenAI FFI
          models/
            mod.rs
            registry.rs             # ModelRegistry, ModelDefinition
            loader.rs               # Model path resolution
            runtime_spec.rs         # ModelRuntimeSpec (ORT-specific)
        Cargo.toml

      smolpc-engine-client/          # HTTP client library
        src/
          lib.rs                     # EngineClient, connect_or_spawn()
          test_utils.rs
        Cargo.toml

  docs/
    ENGINE_API.md                    # Engine HTTP API documentation
    ARCHITECTURE.md                  # High-level architecture overview
    unified-assistant-spec/          # This specification directory
      ARCHITECTURE.md                # THIS FILE
    openvino-native-genai/
      PLAN.md                        # OpenVINO implementation plan
      MODEL_STRATEGY.md              # Model selection strategy
      ENGINE_SURFACE_TARGET.md       # Engine API surface targets
    adr/                             # Architecture Decision Records
    audit/                           # Production audit results

  launcher/                          # App launcher/installer
  scripts/                           # Build and development scripts
  codex/                             # Codex agent configuration
```

### 18.2 Runtime File Locations (on student's machine)

| Path | Contents |
|------|----------|
| `%LOCALAPPDATA%/SmolPC/` | Shared runtime directory |
| `%LOCALAPPDATA%/SmolPC/engine-token.txt` | Bearer token for engine auth |
| `%LOCALAPPDATA%/SmolPC/engine-spawn.lock` | Spawn lock file (prevents duplicate engine starts) |
| `%LOCALAPPDATA%/SmolPC/models/` | Model artifacts directory |
| `%LOCALAPPDATA%/SmolPC/models/<model_id>/cpu/` | CPU model artifacts |
| `%LOCALAPPDATA%/SmolPC/models/<model_id>/dml/` | DirectML model artifacts |
| `%LOCALAPPDATA%/SmolPC/models/<model_id>/openvino_npu/` | OpenVINO IR artifacts |
| `%LOCALAPPDATA%/SmolPC/backend_decisions.v2.json` | Persisted backend selection records |

### 18.3 Key Constants

| Constant | Value | Location |
|----------|-------|----------|
| Engine port | `19432` | `engine-client/lib.rs`, `engine-host/main.rs` |
| Engine protocol version | `"1.0.0"` | Both engine-host and engine-client |
| ORT crate version | `"2.0.0-rc.11"` | `backend.rs`, `Cargo.toml` |
| OpenVINO version tuple | `"2026.0.0"` (all three) | `runtime_bundles.rs` |
| Selection profile | `"openvino_native_v1"` | `main.rs` |
| Backend store version | `2` | `backend_store.rs` |
| Backend store filename | `"backend_decisions.v2.json"` | `backend_store.rs` |
| Spawn lock filename | `"engine-spawn.lock"` | `engine-client/lib.rs` |
| Spawn lock stale age | `30 seconds` | `engine-client/lib.rs` |
| Default wait ready timeout | `60 seconds` | `engine-client/lib.rs` |
| Wait ready poll interval | `250 ms` | `engine-client/lib.rs` |
| DirectML min decode speedup | `1.30x` | `backend.rs` |
| DirectML max TTFT regression | `1.15x` | `backend.rs` |
| DirectML demotion threshold | `3 failures` | `backend.rs` |
| OpenVINO startup probe budget | `30 seconds` | `main.rs` |
| OpenVINO preflight budget | `300 seconds` | `main.rs` |
| OpenVINO max tokens hard cap | `8192` | `main.rs` |
| Default max_length | `2048` | `types.rs` |
| Default repetition penalty | `1.1` | `types.rs` |
| Default repetition penalty last N | `64` | `types.rs` |

---

## Appendix A: Glossary

| Term | Definition |
|------|-----------|
| **Backend** | An inference execution path: CPU (ORT), DirectML (GPU), or OpenVINO NPU |
| **Lane** | Synonym for backend in the readiness model (e.g., "OpenVINO lane") |
| **Bundle** | A set of DLL files for a specific runtime family (ORT bundle, OpenVINO bundle) |
| **Fingerprint** | A hash of a bundle's files (paths, sizes, mtimes) used to detect changes |
| **Preflight** | A test run on a backend to verify it works before committing to it |
| **Startup probe** | Hardware detection phase that identifies available devices |
| **GenAI pipeline** | High-level API provided by onnxruntime-genai and openvino_genai that handles tokenization, KV cache, and sampling automatically |
| **IR** | Intermediate Representation -- OpenVINO's native model format (.xml graph + .bin weights) |
| **INT4** | 4-bit integer quantization, reducing model size by ~4x vs FP16 |
| **TTFT** | Time To First Token -- latency from request to first generated token |
| **FIM** | Fill In the Middle -- code completion technique used for ghost text |
| **MCP** | Model Context Protocol -- Anthropic's standard for tool-use with AI models |
| **SSE** | Server-Sent Events -- HTTP streaming protocol used for token delivery |
| **ChatML** | Chat Markup Language -- `<|im_start|>role\ncontent<|im_end|>` template format |
| **Runes** | Svelte 5's reactivity primitives (`$state`, `$derived`, `$effect`) |
| **Sidecar** | A bundled binary shipped with the Tauri app (e.g., `uv` for Python) |

## Appendix B: Environment Variables Reference

| Variable | Scope | Default | Purpose |
|----------|-------|---------|---------|
| `SMOLPC_FORCE_EP` | Engine client | `auto` | Force backend: `cpu`, `dml` |
| `SMOLPC_DML_DEVICE_ID` | Engine client | auto-detect | Force specific DirectML device |
| `SMOLPC_MODELS_DIR` | Engine core | `%LOCALAPPDATA%/SmolPC/models` | Override models directory |
| `SMOLPC_ORT_BUNDLE_ROOT` | Engine host (dev) | workspace paths | Override ORT DLL directory |
| `SMOLPC_OPENVINO_BUNDLE_ROOT` | Engine host (dev) | workspace paths | Override OpenVINO DLL directory |
| `SMOLPC_ENGINE_DEFAULT_MODEL_ID` | Engine host | first registry entry | Override default model |
| `SMOLPC_DEFAULT_MODEL_ID` | Engine host (legacy) | first registry entry | Legacy alias for above |
| `SMOLPC_OPENVINO_MAX_TOKENS_HARD_CAP` | Engine host | `8192` | Hard cap on OpenVINO max tokens |

## Appendix C: Error Codes

Backend selection error codes (used in `BackendStatus.error_code` and `LastStartupError.code`):

| Code | Meaning |
|------|---------|
| `no_models_available` | No models found in the models directory |
| `model_not_found` | Requested model ID not in registry |
| `model_files_missing` | Model artifact files not found on disk |
| `ort_init_failed` | ONNX Runtime initialization failed |
| `directml_init_failed` | DirectML device creation failed |
| `directml_model_load_failed` | Failed to load model on DirectML |
| `openvino_probe_failed` | OpenVINO startup probe failed |
| `openvino_preflight_failed` | OpenVINO preflight test failed |
| `openvino_preflight_timeout` | OpenVINO preflight exceeded budget |
| `openvino_artifact_missing` | OpenVINO IR model files not found |
| `generation_failed` | Inference generation failed at runtime |
| `cancelled` | Generation was cancelled by user |

---

*End of Architecture Specification*
