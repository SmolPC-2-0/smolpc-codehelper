# Inference Deep Dive

This document covers the lowest layer of the SmolPC engine: the custom FFI wrappers that bridge Rust to native inference libraries. No Rust bindings exist for the OpenVINO GenAI C API or the ONNX Runtime GenAI C API, so we wrote our own — loading DLLs at runtime, resolving function pointers by name, and managing opaque C handles with RAII guards.

This is the most technically dense part of the codebase. Everything here lives in `engine/crates/smolpc-engine-core/src/inference/` and `engine/crates/smolpc-engine-host/src/`.

## Why Custom FFI Wrappers

The OpenVINO GenAI C API and ONNX Runtime GenAI C API are C libraries with no published Rust crate bindings. The `openvino` Rust crate exists but does not cover the GenAI extension (LLM pipeline, streaming, chat history). The `ort` Rust crate covers ONNX Runtime but not the GenAI extension that provides the `OgaGenerator` step-by-step decoding API needed for DirectML streaming.

Our approach: load the native DLLs at runtime using `libloading`, resolve every function pointer by name, store them in a struct, and call them through `unsafe` FFI. This means:

- **No build-time linking** — the engine binary compiles without any native library present. DLLs are resolved at runtime from a configurable bundle directory.
- **Version-pinned** — we bind to specific C API symbol names from OpenVINO 2026.0.0 and ORT GenAI. If the C API changes, our `load_symbol` calls fail with a clear error naming the missing symbol.
- **Two separate API families** — OpenVINO GenAI uses `extern "C"` calling convention; ORT GenAI uses `extern "system"` (stdcall on Windows). Each FFI module matches the convention of its target library.

## Centralized DLL Loading

**Source:** `engine/crates/smolpc-engine-core/src/inference/runtime_loading.rs`

All native library loading is confined to a single file. A CI test (`runtime_loading_is_centralized`) scans every `.rs` file in the engine workspace and fails if `Library::new()` or `load_with_flags()` appears anywhere else. This is critical because:

1. **Windows DLL dependency order is fragile.** Loading `openvino_genai_c.dll` before `tbb12.dll` causes a silent load failure because the GenAI DLL has an implicit dependency on TBB. The order must be maintained in exactly one place.
2. **Load flags matter.** We use `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32` to restrict dependency resolution to the DLL's own directory plus System32. Without these flags, Windows searches `PATH` and the working directory, which can pick up wrong versions of shared DLLs.
3. **Fingerprinting prevents version skew.** Each bundle gets a hash-based fingerprint computed from file paths, sizes, modification times, and version metadata. If the engine is already initialized with one bundle, attempting to load a different one returns an error rather than silently mixing DLL versions.

### OpenVINO DLL Load Order

OpenVINO requires 14 DLLs (15 with NPU) loaded in strict dependency order. The `ensure_initialized_internal` function loads them sequentially:

1. `tbb12.dll` — Intel Threading Building Blocks runtime
2. `tbbbind_2_5.dll` — TBB binding library
3. `tbbmalloc.dll` — TBB memory allocator
4. `tbbmalloc_proxy.dll` — TBB malloc proxy
5. `openvino.dll` — OpenVINO core runtime
6. `openvino_c.dll` — OpenVINO C API wrapper
7. `openvino_ir_frontend.dll` — IR model format parser
8. `openvino_intel_cpu_plugin.dll` — CPU inference plugin
9. `openvino_intel_npu_compiler.dll` — NPU model compiler (NPU only)
10. `openvino_intel_npu_plugin.dll` — NPU inference plugin (NPU only)
11. `icudt70.dll` — ICU data (Unicode support for tokenizers)
12. `icuuc70.dll` — ICU common library
13. `openvino_tokenizers.dll` — tokenizer extension
14. `openvino_genai.dll` — GenAI LLM pipeline
15. `openvino_genai_c.dll` — GenAI C API (the API we actually call)

Steps 9-10 are conditionally included only when NPU is the target device. CPU-only initialization skips these, which avoids loading NPU driver dependencies on machines without an NPU.

Each DLL is loaded via `RetainedLibrary::load()`, which calls `LoadLibraryExW` with the restricted search flags. The resulting `Library` handle is wrapped in an `Arc` and stored in a `RetainedLibrary` struct. The `_lib` field keeps the DLL loaded for the lifetime of the process — if the `Arc` is dropped, the DLL is unloaded and all function pointers become dangling.

### ORT DLL Load Order

ONNX Runtime requires only 2 support DLLs:

1. `onnxruntime.dll` — core ONNX Runtime
2. `onnxruntime_providers_shared.dll` — shared execution providers

The GenAI DLL (`onnxruntime-genai.dll`) and DirectML DLL (`DirectML.dll`) are loaded later when the GenAI API struct is constructed, not during the initial runtime initialization.

### Bundle Fingerprinting

Each runtime bundle gets a `RuntimeBundleFingerprint` computed from:

- Runtime family (ORT or OpenVINO)
- Canonical root path (lowercased)
- For each required file: logical name, file path (lowercased), existence flag, file size, and modification timestamp
- Version metadata strings (e.g., "openvino-runtime: 2026.0.0")

The fingerprint is a `DefaultHasher` digest formatted as a 16-character hex string. It is used as a cache key: `OnceLock<Mutex<HashMap<String, CachedInit>>>` ensures each bundle is loaded exactly once per process, and loading a second bundle with a different fingerprint returns an error. This prevents ABI mismatches from mixing DLL versions within a single engine process.

## OpenVINO GenAI Wrapper (CPU + NPU)

**Source:** `engine/crates/smolpc-engine-core/src/inference/genai/openvino.rs` (wrapper) and `openvino_ffi.rs` (C API bindings)

### C API Bindings

The `OpenVinoGenAiApi` struct holds function pointers resolved from two DLLs:

- `openvino_c.dll` — error reporting (`ov_get_error_info`, `ov_get_last_err_msg`)
- `openvino_genai_c.dll` — everything else: pipeline lifecycle, generation config, chat history, streaming, performance metrics

Key function signatures (simplified):

| C API Function | Purpose |
|---|---|
| `ov_genai_llm_pipeline_create` | Create pipeline from model dir + device + properties (variadic) |
| `ov_genai_llm_pipeline_generate` | Generate from a raw string prompt |
| `ov_genai_llm_pipeline_generate_with_history` | Generate from structured chat history |
| `ov_genai_generation_config_create` | Create empty generation config |
| `ov_genai_generation_config_set_*` | Set individual config parameters |
| `ov_genai_generation_config_validate` | Validate config before generation |
| `ov_genai_chat_history_create_from_json_container` | Create chat history from JSON messages |
| `ov_genai_chat_history_set_extra_context` | Set extra context (e.g., `enable_thinking: false`) |
| `ov_genai_decoded_results_get_perf_metrics` | Extract TTFT, throughput, duration from results |

All functions return an `OvStatus` integer (0 = OK). Errors are retrieved via `ov_get_error_info(status)` and `ov_get_last_err_msg()`, which return C strings. The `check_status` helper converts these into Rust `Result<(), String>`.

The `presence_penalty` symbol is loaded via `try_load_symbol` (returns `Option`) rather than `load_symbol` (returns `Result`) because it was added in a later OpenVINO version and may not be present in all builds.

### RAII for C Handles

Every opaque C pointer is wrapped in `OvOwned<T>`, a generic RAII guard:

```rust
pub(super) struct OvOwned<T> {
    pub(super) api: Arc<OpenVinoGenAiApi>,
    pub(super) ptr: *mut T,
    pub(super) destroy: unsafe extern "C" fn(*mut T),
}
```

On drop, it calls the stored destroy function. The `api` field keeps the `Arc<OpenVinoGenAiApi>` alive, which in turn keeps the `RetainedLibrary` handles alive, which keeps the DLLs loaded. This ownership chain — `OvOwned` -> `Arc<Api>` -> `RetainedLibrary` -> `Arc<Library>` — ensures that function pointers are never called after their DLL has been unloaded.

### Pipeline Creation

The `OpenVinoGenAiGenerator::new()` function creates an LLM pipeline:

**CPU path:**
1. Call `ov_genai_llm_pipeline_create(model_dir, "CPU", 0, &pipeline)` — the `0` means no extra properties
2. Set `max_new_tokens_cap = usize::MAX` (no cap on CPU)

**NPU path:**
1. Create the CACHE_DIR directory for compiled blob storage
2. Call `ov_genai_llm_pipeline_create(model_dir, "NPU", 6, &pipeline, "CACHE_DIR", cache_path, "MAX_PROMPT_LEN", "2048", "MIN_RESPONSE_LEN", "1024")` — the `6` is the number of variadic property arguments (3 key-value pairs)
3. Set `max_new_tokens_cap = min_response_len` (NPU has a fixed response budget)

The variadic C API uses `key, value, key, value, ...` string pairs. The count parameter tells the API how many strings follow. Getting this wrong causes undefined behavior — an incorrect count reads garbage from the stack.

### Generation Config

The `create_generation_config` function translates a Rust `GenerationConfig` into C API calls:

1. Try to inherit the pipeline's default config via `ov_genai_llm_pipeline_get_generation_config`. If this fails (some pipeline types don't support it), fall back to `ov_genai_generation_config_create` (empty config).
2. Set `echo = false` (never echo the prompt back)
3. Set `max_new_tokens` from the request config
4. Set stop controls: `eos_token_id`, `stop_token_ids`, `stop_strings`, `ignore_eos`
5. Handle the `min_new_tokens` bug: **any value >= 1 permanently suppresses EOS detection on OpenVINO GenAI 2026.0.0**, causing runaway generation. The wrapper logs a warning and skips the call.
6. Set sampling parameters based on device:
   - **NPU:** Force `do_sample = false` (greedy decoding only). Log a warning if the caller requested temperature > 0.
   - **CPU:** Set `do_sample = true` if temperature > 0, then set temperature, top_p, top_k.
7. Set `repetition_penalty` if finite and positive
8. Handle `presence_penalty`: skip on NPU (incompatible with greedy decoding), use the optional symbol on CPU (gracefully degrade if the symbol is missing)
9. Call `ov_genai_generation_config_validate` to catch invalid parameter combinations before generation starts

### NPU-Specific Constraints

The NPU uses OpenVINO's `StaticLLMPipeline`, which differs from the CPU `LLMPipeline` in several ways:

- **Fixed context window.** `MAX_PROMPT_LEN` (default 2048 tokens) + `MIN_RESPONSE_LEN` (default 1024 tokens) are set at pipeline creation time and cannot be changed. Exceeding `MAX_PROMPT_LEN` crashes with "unknown exception" — no graceful error. The engine host humanizes this to "conversation too long, try starting a new chat."
- **Greedy decoding only.** `do_sample` must be `false`. Temperature, top_k, top_p are ignored. `presence_penalty` is incompatible.
- **Compilation caching.** First load compiles the model to an NPU-specific blob (~3-5 minutes for Qwen at MAX_PROMPT_LEN=2048). The `CACHE_DIR` property stores this blob so subsequent loads take seconds. Changing `MAX_PROMPT_LEN` or `MIN_RESPONSE_LEN` invalidates the cache.
- **No `extra_context` API.** CPU can disable thinking mode via `ov_genai_chat_history_set_extra_context({"enable_thinking": false})`. The NPU StaticLLMPipeline does not support this — thinking control is injected by prepending `/nothink` to the system message content.
- **KV cache overflow.** `max_new_tokens` must be clamped to the `MIN_RESPONSE_LEN` budget. The `clamp_max_new_tokens` function handles this, logging when clamping occurs.

### Qwen3 Template Patching

Qwen3 ships with a Jinja chat template containing the condition:

```
enable_thinking is defined and enable_thinking is false
```

This means: if `enable_thinking` is **not** provided (i.e., undefined), the condition is false, and thinking mode is **enabled** by default. On the NPU, where we cannot pass `extra_context`, this causes runaway generation (thinking tokens consume the entire response budget).

The `ensure_qwen3_nothink_template` function patches this to:

```
not enable_thinking is defined or enable_thinking is false
```

Now the condition is true when `enable_thinking` is undefined, defaulting to non-thinking mode. The function:

1. Checks for `chat_template.jinja` in the model directory
2. If missing, extracts the template from `tokenizer_config.json` (which may contain the template as a string or an array of `{name, template}` objects) and writes it as a standalone file
3. Searches for the broken condition string and replaces it
4. Is idempotent — a second call returns `Ok(false)`

Template patch failure is a **hard error** that aborts NPU model loading. An un-patched template produces runaway generation on every request.

### Streaming

Token streaming uses a C callback mechanism:

```rust
#[repr(C)]
pub(super) struct StreamerCallback {
    pub(super) callback_func: Option<
        unsafe extern "C" fn(*const c_char, *mut c_void) -> OvGenAiStreamingStatus,
    >,
    pub(super) args: *mut c_void,
}
```

The callback receives each generated token as a C string and returns a status:

- `Running (0)` — continue generation
- `Stop (1)` — stop gracefully
- `Cancel (2)` — abort immediately

The Rust `stream_callback` function:

1. Checks the `cancelled` AtomicBool (set by the user or timeout)
2. Increments a callback counter and enforces a safety-net token limit (matches `max_new_tokens`). This catches cases where the C-level limit is not honored due to pipeline bugs or ABI mismatches.
3. Accumulates text and checks for stop strings (e.g., `<|im_end|>`, `<|endoftext|>`). This is a second line of defense — the NPU StaticLLMPipeline does not always honor `stop_token_ids` reliably.
4. Detects degenerate output: if 64+ consecutive dots are generated, stops generation (a sign of model collapse).
5. Sends each token through a Tokio `mpsc::unbounded_channel` to the async layer.

The async wrapper (`generate_stream`) runs the blocking generation in `spawn_blocking` and reads tokens from the channel with a 90-second watchdog timer. If no token arrives within 90 seconds (which can happen during NPU compilation on first load), it sets the cancelled flag and returns a timeout error. The watchdog resets after each token.

### Thinking Filter

**Source:** `engine/crates/smolpc-engine-host/src/chat.rs`

Even with `/nothink` in the prompt, Qwen3 occasionally emits `<think>...</think>` blocks. The `ThinkingFilter` is a streaming text filter that strips these before tokens reach the user:

1. Buffers incoming tokens
2. When `<think>` is found: enter suppression mode, emit everything before the tag
3. When `</think>` is found: exit suppression mode, consume trailing newline
4. Handles partial tag matches at buffer boundaries (e.g., receiving `<thi` in one token and `nk>` in the next)
5. At end-of-stream, flushes remaining buffer (unless still inside a think block)

### Performance Metrics

After generation completes, `read_generation_metrics` extracts metrics from the `OvGenAiPerfMetrics` object:

- **`total_tokens`** — tokens generated (excluding prompt)
- **`time_to_first_token_ms`** — TTFT in milliseconds (mean value)
- **`tokens_per_second`** — decode throughput (mean value)
- **`total_time_ms`** — end-to-end generation duration (mean value)

The C API returns mean and standard deviation pairs for each metric; we use only the mean. If the DecodedResults pointer is null (which can happen on cancellation), we compute fallback metrics from wall-clock time.

## DirectML Wrapper (Discrete GPU)

**Source:** `engine/crates/smolpc-engine-core/src/inference/genai/directml.rs` (wrapper) and `directml_ffi.rs` (C API bindings)

### C API Bindings

The `GenAiApi` struct holds function pointers from `onnxruntime-genai.dll` and keeps `DirectML.dll` loaded. Key difference from OpenVINO: ORT GenAI uses `extern "system"` (stdcall) calling convention and an error pattern where functions return `*mut OgaResult` (null = success, non-null = error with embedded message).

Key function signatures:

| C API Function | Purpose |
|---|---|
| `OgaCreateConfig` | Create model config from directory path |
| `OgaConfigClearProviders` / `OgaConfigAppendProvider` | Select DirectML as the execution provider |
| `OgaConfigSetDecoderProviderOptionsHardwareDeviceId` | Target a specific GPU by DXGI adapter index |
| `OgaCreateModelFromConfig` | Load model with the configured provider |
| `OgaCreateTokenizer` | Create tokenizer from model |
| `OgaCreateGenerator` | Create step-by-step generator |
| `OgaGenerator_GenerateNextToken` | Generate one token |
| `OgaGenerator_GetNextTokens` | Read generated token IDs |
| `OgaGenerator_IsDone` | Check if generation is complete |
| `OgaGenerator_GetLogits` | Access logits tensor (for preflight validation) |

### Model Loading

`GenAiDirectMlGenerator::new()`:

1. Create an `OgaConfig` from the model directory
2. Clear default providers and append `"dml"`
3. If a `directml_device_id` is provided (the DXGI adapter index of the target GPU), set it via `OgaConfigSetDecoderProviderOptionsHardwareDeviceId`. This is how we target a specific discrete GPU when multiple GPUs are present.
4. Create the model from config — this triggers DirectML graph compilation
5. Create a tokenizer from the model
6. Read EOS token IDs from the tokenizer via `OgaTokenizerGetEosTokenIds`

### Token-by-Token Generation

Unlike OpenVINO (which uses a callback-based streaming API), ORT GenAI uses a step-by-step loop:

1. Encode the prompt with `OgaTokenizerEncode` into `OgaSequences`
2. Create `OgaGeneratorParams` with search options (max_length, temperature, top_k, top_p, repetition_penalty, do_sample)
3. Create an `OgaGenerator` and append the token sequences
4. Create an `OgaTokenizerStream` for incremental decoding
5. Loop:
   a. Check cancellation flag
   b. Check `OgaGenerator_IsDone` — if true, break
   c. Call `OgaGenerator_GenerateNextToken` — generates one token
   d. Call `OgaGenerator_GetNextTokens` — read the new token ID(s)
   e. Check each token against the EOS token ID list
   f. Decode each token via `OgaTokenizerStreamDecode` — returns the text piece
   g. Send the text through the mpsc channel
   h. Increment token counter, break if max_length reached

The async wrapper uses the same `spawn_blocking` + channel + watchdog pattern as OpenVINO, but with a 45-second timeout (DirectML TTFT on budget hardware can be 10-20 seconds, but should not reach 45).

### Preflight Validation

`run_preflight()` generates a single token and inspects the logits tensor to detect garbage output. This catches the known issue where DirectML on Intel integrated GPUs produces non-finite logits (NaN/Inf):

1. Encode a short prompt and generate one token with `max_length = prompt_tokens + 1`
2. Call `OgaGenerator_GetLogits` to get the logits tensor
3. Read the tensor's element type (Float32 or Float16), shape, and data pointer
4. Compute the bounds of the last row of logits
5. Scan the last row for non-finite values (NaN, Inf, -Inf)
6. If any non-finite values are found, return an error with the count and first offending value

This is how we detect that DirectML is producing garbage and should fall back to OpenVINO CPU — the logits check is more reliable than testing the decoded text, because a single non-finite logit can propagate through softmax and produce plausible-looking but wrong tokens.

### Hung Driver Protection

DirectML on some GPUs/drivers can hang indefinitely during `OgaGenerator_GenerateNextToken`. The `hung` flag tracks this:

- The async wrapper sets `hung = true` if the 45-second watchdog fires
- All subsequent calls to `generate_stream` or `run_preflight` immediately return an error
- Recovery requires reloading the model (destroying and recreating the generator)

## Whisper STT Wrapper

**Source:** `engine/crates/smolpc-engine-core/src/inference/genai/whisper.rs` (wrapper) and `whisper_ffi.rs` (C API bindings)

The Whisper wrapper transcribes audio using the OpenVINO GenAI WhisperPipeline. It always runs on CPU (no NPU path for Whisper). Windows-only, since it depends on the OpenVINO GenAI runtime.

### C API Bindings

The `WhisperApi` struct holds function pointers from the same `openvino_c.dll` and `openvino_genai_c.dll`:

| C API Function | Purpose |
|---|---|
| `ov_genai_whisper_pipeline_create` | Create pipeline from model dir + device (variadic) |
| `ov_genai_whisper_pipeline_generate` | Run transcription on raw audio samples |
| `ov_genai_whisper_decoded_results_get_string` | Extract transcribed text (two-call buffer pattern) |
| `ov_genai_whisper_decoded_results_free` | Free results |

### Transcription Flow

1. `WhisperPipeline::new()` creates the pipeline targeting CPU. This is a blocking call (~1 second cold start).
2. `transcribe()` receives 16kHz mono f32 audio samples and calls `ov_genai_whisper_pipeline_generate` with a null config (use defaults).
3. Text extraction uses the two-call buffer pattern:
   - First call with `output = null` returns the required buffer size
   - Allocate a byte buffer of that size
   - Second call fills the buffer with the transcribed string
   - Trim the null terminator and convert to a Rust String
4. An RAII `ResultsGuard` ensures the decoded results are freed even on early return.

The pipeline is created once (on first transcription request) and reused. Thread safety is handled at the caller level (the engine host wraps it in a voice semaphore).

## Token Counting

The NPU StaticLLMPipeline has a hard input token limit (`MAX_PROMPT_LEN`). Exceeding it crashes with no graceful error. Token counting must happen host-side before sending the prompt.

The OpenVINO GenAI C API does not expose a tokenizer — you cannot tokenize text from Rust. Two strategies:

1. **Tokenizers crate.** Load the model's `tokenizer.json` file using the `tokenizers` Rust crate and count tokens directly. This is accurate but adds a dependency and requires the tokenizer file to be present.
2. **Character heuristic.** For Qwen models, ~3.5 characters per token is a reasonable approximation. This is fast and dependency-free but imprecise — it overestimates for English text (safe, wastes context) and underestimates for code with many short tokens (unsafe, may crash).

The engine host applies a hard cap (`OPENVINO_MAX_TOKENS_HARD_CAP_DEFAULT`) to `max_tokens` in chat completion requests, clamping before the request reaches the backend.

## Qwen Model Specifics

### Qwen 2.5 (1.5B Instruct)

- Default model for machines with <16 GB RAM
- Two stop tokens: `<|endoftext|>` (151643) + `<|im_end|>` (151645)
- No thinking mode — `disable_thinking = false` in tuning (not applicable)
- Available in INT4 for both CPU and NPU
- OpenVINO IR format: `.xml` + `.bin` artifacts, not ONNX

### Qwen 3 (4B)

- Default model for machines with 16+ GB RAM
- Same stop tokens as Qwen 2.5 (ChatML format)
- Thinking mode enabled by default — must be disabled for production use
- Template patching required for NPU (see above)
- **INT4 crashes on NPU** — only INT8_SYM (symmetric per-channel quantization via `nncf.compress_weights`) works. INT4 is used only on CPU.
- Default sampling params (non-thinking mode): temperature=0.7, top_p=0.8, top_k=20, presence_penalty=1.5

### Chat Template Handling

HuggingFace `tokenizer_config.json` has a `chat_template` field that can be:

- **A string** — the Jinja template directly
- **An array** — `[{name: "default", template: "..."}, {name: "rag", template: "..."}]`

The `extract_chat_template_from_tokenizer_config` function handles both: for arrays, it finds the entry named `"default"` or falls back to the first entry.

### Generation Controls

The engine host sets explicit generation controls for both Qwen models:

| Parameter | Qwen 2.5 | Qwen 3 |
|---|---|---|
| `eos_token_id` | 151645 | 151645 |
| `stop_token_ids` | [151643, 151645] | [151643, 151645] |
| `stop_strings` | ["<\|im_end\|>", "<\|endoftext\|>"] | ["<\|im_end\|>", "<\|endoftext\|>"] |
| `ignore_eos` | false | false |
| `min_new_tokens` | None (skipped) | None (skipped) |
| `presence_penalty` | 1.5 | 1.5 (skipped on NPU) |
| `disable_thinking` | false | true |

These are explicit because the OpenVINO C config is built manually and does not inherit `generation_config.json` defaults automatically. The `stop_strings` provide a second line of defense — they operate on accumulated decoded text and catch `<|im_end|>` even when the NPU does not honor `stop_token_ids` reliably.

### OpenVINO 2026.0.0 Bug: min_new_tokens

Setting `min_new_tokens >= 1` on OpenVINO GenAI 2026.0.0 permanently suppresses EOS token detection, causing generation to run until `max_new_tokens` is exhausted. This produces garbage trailing output (repetitive text, degenerate dot runs). The wrapper explicitly skips the `set_min_new_tokens` call and logs a warning if a caller attempts to set it.

## Runtime Adapter

**Source:** `engine/crates/smolpc-engine-core/src/inference/runtime_adapter.rs`

The `InferenceRuntimeAdapter` enum unifies both backends behind a single interface:

```rust
pub enum InferenceRuntimeAdapter {
    GenAiDirectMl { generator: GenAiDirectMlGenerator },
    OpenVinoGenAi { generator: OpenVinoGenAiGenerator },
}
```

The adapter provides two generation methods:

- `generate_stream(prompt, config, cancelled, on_token)` — works with both backends. Takes a raw string prompt (pre-formatted ChatML).
- `generate_stream_messages(messages, config, cancelled, on_token)` — structured chat messages. **Only supported by OpenVINO GenAI** (which uses `ov_genai_llm_pipeline_generate_with_history`). DirectML returns an error because ORT GenAI does not have a native structured chat API.

The engine host checks `is_openvino_genai()` to decide whether to use structured messages or the legacy prompt path.

## Performance Characteristics

| | CPU (OpenVINO) | NPU (OpenVINO) | DirectML (discrete GPU) |
|---|---|---|---|
| TTFT | Slow (seconds) | Fast after compilation | Variable (depends on GPU) |
| Decode speed | Slowest | Moderate | Fastest on discrete GPU |
| First load | Fast | Slow (3-5 min compilation, then cached) | Fast |
| Context window | Unlimited (within RAM) | Fixed (MAX_PROMPT_LEN) | Unlimited (within VRAM) |
| Sampling | Full (temperature, top_k, top_p) | Greedy only | Full |
| Reliability | Most reliable | Driver-dependent | GPU-dependent |
| Quantization | INT4 | INT8_SYM (Qwen3), INT4 (Qwen2.5) | INT4 |

The NPU's fixed context window is the most impactful architectural constraint. Multi-turn conversations that exceed `MAX_PROMPT_LEN` tokens crash the pipeline, so the engine host must either truncate history or suggest the user start a new chat.

## Key Files Reference

| File | Purpose |
|---|---|
| `engine/crates/smolpc-engine-core/src/inference/runtime_loading.rs` | Centralized DLL loading, bundle fingerprinting, runtime initialization |
| `engine/crates/smolpc-engine-core/src/inference/genai/openvino_ffi.rs` | OpenVINO GenAI C API function pointer bindings |
| `engine/crates/smolpc-engine-core/src/inference/genai/openvino.rs` | OpenVINO pipeline wrapper, generation, streaming |
| `engine/crates/smolpc-engine-core/src/inference/genai/directml_ffi.rs` | ORT GenAI C API function pointer bindings |
| `engine/crates/smolpc-engine-core/src/inference/genai/directml.rs` | DirectML pipeline wrapper, token-by-token generation |
| `engine/crates/smolpc-engine-core/src/inference/genai/whisper_ffi.rs` | Whisper C API bindings |
| `engine/crates/smolpc-engine-core/src/inference/genai/whisper.rs` | Whisper STT pipeline wrapper |
| `engine/crates/smolpc-engine-core/src/inference/runtime_adapter.rs` | Unified enum adapter over both backends |
| `engine/crates/smolpc-engine-core/src/inference/types.rs` | GenerationConfig, GenerationMetrics, InferenceChatMessage |
| `engine/crates/smolpc-engine-host/src/openvino.rs` | NPU tuning, artifact checking, template patching, preflight |
| `engine/crates/smolpc-engine-host/src/chat.rs` | Request-to-prompt conversion, thinking filter, token cap |
| `engine/crates/smolpc-engine-host/src/runtime_bundles.rs` | Bundle path resolution (dev/prod), candidate selection |
