# OpenVINO GenAI Rust FFI: Design & Contribution

This document describes SmolPC's Rust wrapper for the OpenVINO GenAI C API — why it was necessary, how it works, and why it constitutes a novel contribution to the field.

## Why This Wrapper Was Necessary

The OpenVINO ecosystem exposes two distinct APIs:

| | OpenVINO Inference API | OpenVINO GenAI API |
|---|---|---|
| **Library** | `openvino_c.dll` | `openvino_genai_c.dll` |
| **Rust crate** | `openvino` (Intel official, v0.9.1) | **None exists** |
| **Scope** | Raw tensor I/O: load model, set inputs, infer, read outputs | Complete LLM pipeline: tokenization, chat templates, streaming, sampling, KV-cache management |

The official `openvino-rs` crate wraps only the Inference API. Using it for LLM text generation would require reimplementing tokenization, KV-cache management, sampling strategies, chat template processing, and streaming — essentially rebuilding what GenAI already provides.

The GenAI C API (introduced in OpenVINO GenAI 2025.1, April 2024) is a C-ABI interop layer over the C++ GenAI library. As of March 2026, **no Rust wrapper for the OpenVINO GenAI C API has been published** — not on crates.io, not on GitHub, not in the official OpenVINO ecosystem. SmolPC's implementation is the first known production-grade Rust binding.

Additionally, the NPU `StaticLLMPipeline` has constraints that the C API exposes but does not enforce:

- Exceeding `MAX_PROMPT_LEN` crashes with "unknown exception" (no graceful error)
- `min_new_tokens >= 1` permanently suppresses EOS detection on OpenVINO GenAI 2026.0.0
- `extra_context` for thinking control does not work on NPU
- `presence_penalty` is incompatible with greedy-only NPU decoding
- No tokenizer is exposed through the C API, so token counting must happen host-side

These require a smart, constraint-aware wrapper — not just a thin binding.

## Architecture

The FFI implementation spans four layers across ~1,700 lines:

```
engine/crates/smolpc-engine-core/src/inference/
├── runtime_loading.rs                ← DLL loading, fingerprinting, single-init guarantee
└── genai/
    ├── openvino_ffi.rs  (470 lines)  ← C symbol definitions, RAII wrapper, API cache
    ├── openvino.rs      (1214 lines) ← Pipeline management, streaming, NPU constraints
    ├── whisper_ffi.rs                ← Whisper STT C symbol definitions
    └── whisper.rs                    ← Whisper pipeline wrapper
```

### Layer 1: Runtime Loading

DLLs are loaded via `libloading` at runtime (not link-time), in a strict 14-step dependency order:

```
tbb12 → tbbbind → tbbmalloc → tbbmalloc_proxy
  → openvino → openvino_c → openvino_ir_frontend → openvino_intel_cpu_plugin
  → [npu_compiler → npu_plugin]
  → icu_data → icu_uc → openvino_tokenizers
  → openvino_genai → openvino_genai_c
```

Security: Uses `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32` — absolute paths only, no PATH/CWD search, preventing DLL hijacking.

Each runtime bundle is fingerprinted (hash of file paths, sizes, timestamps, versions). A single-init guard via `OnceLock<Mutex<State>>` ensures one fingerprint per process — mixing runtime versions is a hard error requiring process restart.

### Layer 2: FFI Symbol Table

`OpenVinoGenAiApi` is a struct holding 65+ C function pointers, all resolved at runtime from the loaded DLLs:

```rust
pub(super) struct OpenVinoGenAiApi {
    _openvino_c: RetainedLibrary,        // Prevents DLL unload
    _openvino_genai_c: RetainedLibrary,  // Prevents DLL unload

    // Error handling
    pub(super) get_error_info: unsafe extern "C" fn(OvStatus) -> *const c_char,
    pub(super) get_last_err_msg: unsafe extern "C" fn() -> *const c_char,

    // Pipeline lifecycle
    pub(super) create_pipeline: unsafe extern "C" fn(...) -> OvStatus,
    pub(super) destroy_pipeline: unsafe extern "C" fn(*mut OvGenAiLlmPipeline),

    // Generation
    pub(super) pipeline_generate: unsafe extern "C" fn(...) -> OvStatus,
    pub(super) pipeline_generate_with_history: unsafe extern "C" fn(...) -> OvStatus,

    // 23 generation config setters, metrics, chat history, JSON containers...
    // Optional symbols (e.g., presence_penalty) loaded via try_load_symbol
}

unsafe impl Send for OpenVinoGenAiApi {} // Immutable after construction
unsafe impl Sync for OpenVinoGenAiApi {}
```

Optional symbols are loaded with `try_load_symbol`, returning `Option<fn>` for graceful degradation when a function does not exist in a given runtime version.

### Layer 3: RAII Ownership

`OvOwned<T>` is a generic drop guard that pairs any C opaque pointer with its destructor:

```rust
pub(super) struct OvOwned<T> {
    pub(super) api: Arc<OpenVinoGenAiApi>,  // Prevents DLL unload
    pub(super) ptr: *mut T,
    pub(super) destroy: unsafe extern "C" fn(*mut T),
}

impl<T> Drop for OvOwned<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { (self.destroy)(self.ptr) };
        }
        // Arc<Api> drops last, so DLLs always outlive C objects
    }
}
```

This solves the classic FFI lifetime problem: C objects must be freed before the DLL that created them is unloaded. The `Arc<OpenVinoGenAiApi>` reference inside each `OvOwned` guarantees this ordering at compile time.

### Layer 4: Async Streaming Bridge

Bridges the synchronous C callback to Rust's async runtime:

```
C thread (spawn_blocking)        Rust async runtime
─────────────────────           ──────────────────
stream_callback()  ──────────→  UnboundedSender<String>
  (unsafe extern "C")                │
  checks cancellation                ↓
  checks stop strings           token_rx.recv()
  checks degenerate output           │
  checks token count cap             ↓
  returns Running/Stop           on_token(piece)
                                     │
                                     ↓
                                90s watchdog timer
                                (resets per token)
```

The `StreamCallbackState` carries the channel sender, an `Arc<AtomicBool>` for cross-thread cancellation, stop-string accumulation, degenerate output detection (64+ trailing dots), and a hard token count safety cap.

### Layer 5: NPU Constraint Enforcement

The wrapper encodes hardware-specific knowledge that the C API does not enforce:

| Constraint | Implementation |
|---|---|
| Greedy decoding only on NPU | `do_sample` forced `false`, regardless of request |
| `min_new_tokens` bug (2026.0.0) | Wrapper refuses to set any value >= 1, logs warning |
| No `presence_penalty` on NPU | Skipped when device is NPU (incompatible with greedy) |
| `max_new_tokens` clamped | Cannot exceed `MIN_RESPONSE_LEN` (the compiled KV cache budget) |
| Thinking control without `extra_context` | `/nothink` injected into system message content |
| Qwen3 template patching | Jinja condition fixed to default to non-thinking; hard error on patch failure |

## What the Wrapper Covers

### C API Functions Wrapped (34 symbols)

**From `openvino_c.dll`:**
- `ov_get_error_info`, `ov_get_last_err_msg`

**From `openvino_genai_c.dll`:**
- Pipeline: `create`, `free`, `generate`, `generate_with_history`, `get_generation_config`
- GenerationConfig: `create`, `free`, `validate`, plus 14 setters (`max_new_tokens`, `eos_token_id`, `min_new_tokens`, `stop_token_ids`, `stop_strings`, `ignore_eos`, `echo`, `do_sample`, `temperature`, `top_p`, `top_k`, `repetition_penalty`, `presence_penalty` [optional])
- ChatHistory: `create_from_json_container`, `set_extra_context`, `free`
- JSON: `create_from_json_string`, `free`
- DecodedResults: `free`, `get_perf_metrics`
- PerfMetrics: `get_num_generation_tokens`, `get_ttft`, `get_throughput`, `get_generate_duration`

**Whisper (separate FFI module):**
- `create`, `free`, `generate`, `results_get_string`

### C API Functions Intentionally Not Wrapped

| Function | Reason |
|---|---|
| `start_chat` / `finish_chat` | Uses explicit `ChatHistory` via `generate_with_history` instead |
| Beam search params (`num_beams`, etc.) | NPU is greedy-only; not applicable |
| `set_logprobs` | Not needed for chat interface |
| `set_rng_seed` | Not needed |
| Speculative decoding (`set_assistant_*`) | Not yet used |
| VLMPipeline (vision-language) | Project is text-only |
| `decoded_results_get_string` | Output comes via streaming callback, not post-hoc extraction |

## Why This Is a Contribution

### First Known Rust Wrapper for OpenVINO GenAI

As of March 2026, the only known language bindings for the OpenVINO GenAI C API are:

| Language | Project | Scope |
|---|---|---|
| C# | `openvino_ai_practice` (Intel demo) | Basic P/Invoke interop |
| Go | Ollama OpenVINO backend | LLM generation |
| C | Official samples | 3 sample programs |
| **Rust** | **SmolPC (this project)** | **Production-hardened, async-native, NPU-aware** |

No `openvino-genai` Rust crate exists on crates.io or GitHub. Intel's own `openvino-rs` wraps only the lower-level Inference API. SmolPC's wrapper is more comprehensive than any published binding in any language, with RAII ownership, async streaming, runtime fingerprinting, and NPU constraint enforcement.

### Safety Rails Intel's C API Does Not Provide

The C API is a thin interop layer — it exposes functions but does not protect callers from misuse. This wrapper adds an enforcement layer:

| What Intel's C API does | What the wrapper adds |
|---|---|
| Accepts `min_new_tokens=1` silently | Refuses to set it (prevents runaway generation bug) |
| Crashes on prompt > `MAX_PROMPT_LEN` | Clamps before sending, warns |
| Accepts `do_sample=true` on NPU | Forces `false`, logs override |
| Accepts `presence_penalty` on NPU | Skips (incompatible with greedy decoding) |
| No degenerate output detection | 64-dot trailing run detection with truncation |
| No streaming timeout | 90-second per-token watchdog |
| No token count safety net | Callback counter with hard cap |
| No DLL load order enforcement | 14-step dependency-ordered loading |
| No runtime version safety | Fingerprint-based single-init guard |

These are production safety rails that any serious consumer of the API would need. Intel ships the API; this project built the guardrails.

### Reusable Pattern

The architecture — `RetainedLibrary` -> symbol table struct -> `OvOwned<T>` RAII -> async streaming bridge — is a general pattern for wrapping any C API from async Rust. The same approach is already used within this project for ONNX Runtime GenAI (DirectML backend via `directml_ffi.rs`), demonstrating its generality.

### Novel Integration

No published project combines: Rust + Tauri desktop app + OpenVINO GenAI C API + Intel NPU + streaming + async cancellation + RAII safety, packaged for budget hardware (8 GB RAM, no discrete GPU). The individual technologies exist; the integration — especially the NPU constraint enforcement and async bridge — is original work.

## File Reference

| File | Lines | Purpose |
|---|---|---|
| `engine/crates/smolpc-engine-core/src/inference/runtime_loading.rs` | ~800 | DLL loading, fingerprinting, lifecycle |
| `engine/crates/smolpc-engine-core/src/inference/genai/openvino_ffi.rs` | ~470 | C symbol definitions, `OvOwned<T>`, API cache |
| `engine/crates/smolpc-engine-core/src/inference/genai/openvino.rs` | ~1214 | Pipeline wrapper, streaming, NPU constraints |
| `engine/crates/smolpc-engine-core/src/inference/genai/whisper_ffi.rs` | ~120 | Whisper C symbol definitions |
| `engine/crates/smolpc-engine-core/src/inference/genai/whisper.rs` | ~150 | Whisper pipeline wrapper |
| `engine/crates/smolpc-engine-host/src/openvino.rs` | ~980 | NPU startup probe, template patching, preflight |
| `engine/crates/smolpc-engine-host/src/runtime_bundles.rs` | ~500 | Bundle discovery, validation, version pinning |
