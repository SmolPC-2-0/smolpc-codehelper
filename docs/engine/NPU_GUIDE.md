# NPU Guide

## What Is the Intel NPU?

The Neural Processing Unit (NPU) is a dedicated AI accelerator built into Intel Core Ultra processors (codenamed Meteor Lake and later). It is designed for sustained, low-power inference workloads — running AI models without taxing the CPU or GPU.

For SmolPC, the NPU matters because:

- **It exists on the target hardware.** Intel Core Ultra laptops are the primary deployment target for the project's school partner.
- **No discrete GPU required.** Budget school laptops rarely have dedicated GPUs. The NPU provides hardware acceleration without one.
- **Low power draw.** The NPU runs inference without the thermal overhead of CPU or GPU acceleration, which matters for fanless and ultra-thin laptops.

## How It Works

SmolPC uses the Intel NPU through the **OpenVINO GenAI** framework. The path from Rust to the NPU is:

1. Rust calls the OpenVINO GenAI C API through our custom FFI wrapper (`openvino.rs`, `openvino_ffi.rs`)
2. OpenVINO GenAI creates a **StaticLLMPipeline** — a compiled, fixed-shape inference pipeline optimized for the NPU
3. The pipeline compiles the model to an NPU-specific binary blob on first load
4. The compiled blob is cached to disk via the `CACHE_DIR` property
5. On subsequent loads, the cached blob is loaded directly (~seconds vs. minutes)
6. During inference, the NPU executes the model graph while the CPU handles tokenization and I/O

## StaticLLMPipeline

The NPU uses OpenVINO's `StaticLLMPipeline`, which differs fundamentally from the CPU's `LLMPipeline`:

- **Compiled graph.** The model is compiled to a fixed-shape NPU program at pipeline creation time. This compilation is slow (3-5 minutes for Qwen at MAX_PROMPT_LEN=2048) but only happens once.
- **Fixed context window.** The input (MAX_PROMPT_LEN) and output (MIN_RESPONSE_LEN) budgets are set at compilation time and cannot be changed without recompiling.
- **Pre-allocated KV cache.** Memory is allocated upfront based on the context window size. There is no dynamic memory growth.

### Compilation Caching

The `CACHE_DIR` property tells OpenVINO where to store compiled blobs:

```
ov_genai_llm_pipeline_create(
    &mut pipeline_ptr,
    model_dir, "NPU", 6,
    "CACHE_DIR", cache_path,
    "MAX_PROMPT_LEN", "2048",
    "MIN_RESPONSE_LEN", "1024"
)
```

Note: the actual C API call writes to a `&mut pipeline_ptr` output parameter (the first argument) rather than returning the pipeline as a value.

- **First load:** compilation takes 3-5 minutes (for Qwen at MAX_PROMPT_LEN=2048). The engine's 300-second **preflight budget** (`OPENVINO_PREFLIGHT_BUDGET` in `types.rs`) accommodates this — it wraps the entire pipeline creation and warmup in a `tokio::time::timeout`. If the budget expires, the result is `OpenVinoPreflightResult::Timeout`, classified as `openvino_npu_preflight_timeout`, and the engine falls back to CPU. This is a `temporary_fallback` — not persisted, so the next engine restart will retry NPU. The separate **90-second token watchdog** (`TOKEN_WATCHDOG_SECS`) applies only during inference streaming, not during pipeline creation — it fires if no tokens arrive within 90 seconds between token deliveries.
- **Subsequent loads:** the cached blob loads in seconds.
- **Cache invalidation:** changing MAX_PROMPT_LEN or MIN_RESPONSE_LEN invalidates the cache, triggering recompilation. Changing the model or OpenVINO version also invalidates.

The cache directory is `%LOCALAPPDATA%\SmolPC 2.0\engine\inference\openvino-cache\{model}\{artifact_fingerprint}\` (managed by the engine host). The model component and artifact fingerprint are sanitized via `sanitize_cache_component()` to produce filesystem-safe directory names. This means different model versions or artifact builds get separate compilation caches.

## NPU Constraints

These constraints are enforced in the FFI wrapper (`openvino.rs`) and engine host (`openvino.rs`):

### Greedy Decoding Only

The NPU StaticLLMPipeline only supports greedy decoding:

- `do_sample` is always forced to `false`
- Temperature, top_k, top_p are ignored (the wrapper logs a warning if the caller requests temperature > 0)
- `presence_penalty` is incompatible with greedy decoding and is skipped on NPU

If you need sampling (creative text, diverse outputs), use the CPU backend instead.

### Model-Specific Generation Tuning

The `openvino_model_tuning_for_model()` function in `engine/crates/smolpc-engine-host/src/openvino.rs` provides per-model request defaults:

| Parameter | Qwen 2.5 (1.5B) | Qwen 3 (4B) |
|---|---|---|
| temperature | 0.7 | 0.7 |
| top_p | 0.8 | 0.8 |
| top_k | 20 | 20 |
| presence_penalty | 1.5 | 1.5 |
| disable_thinking | false | true |

Both models receive identical sampling parameters. On NPU, these sampling parameters are silently overridden to greedy decoding (`do_sample=false`) by the FFI layer — they exist for CPU backend compatibility where sampling is used. The `disable_thinking` flag controls whether `/nothink` injection and template patching are applied: Qwen 2.5 has no thinking mode to suppress (`false`), while Qwen 3 has thinking mode that must be suppressed (`true`).

### Fixed Context Window

The context window is set at pipeline creation and cannot grow:

- **MAX_PROMPT_LEN** (default 2048 tokens) — maximum input tokens. Must be a multiple of the NPU prefill chunk size (default 1024).
- **MIN_RESPONSE_LEN** (default 1024 tokens) — maximum output tokens. `max_new_tokens` is clamped to this value by `clamp_max_new_tokens()`.

**Exceeding MAX_PROMPT_LEN crashes the pipeline with "unknown exception" — there is no graceful error.** The NPU does not validate input length before processing. The engine host humanizes this crash to: "This conversation has gotten too long for the current inference mode. Try starting a new chat, or switch to CPU mode for longer conversations."

These values can be overridden via environment variables for development:

```powershell
$env:SMOLPC_OPENVINO_NPU_MAX_PROMPT_LEN = "2048"
$env:SMOLPC_OPENVINO_NPU_MIN_RESPONSE_LEN = "1024"
```

Increasing MAX_PROMPT_LEN allows longer conversations but invalidates the cached blob and triggers recompilation.

### No Tokenizer in C API

The OpenVINO GenAI C API does not expose a tokenizer. You cannot tokenize text from Rust to count tokens before sending to the NPU. Two workarounds:

1. **Tokenizers crate:** Load the model's `tokenizer.json` using the Rust `tokenizers` crate. Accurate but adds a dependency.
2. **Character heuristic:** For Qwen models, ~3.5 characters per token. Overestimates for English (safe), underestimates for code with many short tokens (unsafe — may crash).

The engine host applies a hard cap to `max_tokens` in chat completion requests to limit the risk: `OPENVINO_MAX_TOKENS_HARD_CAP_DEFAULT = 8192` tokens (defined in `types.rs`). This cap is applied in `max_tokens_hard_cap()` in `chat.rs` and can be overridden via the `SMOLPC_OPENVINO_MAX_TOKENS_HARD_CAP` environment variable. Any `max_tokens` value in a chat completion request is clamped to this cap before being passed to the backend.

### No extra_context API

The CPU pipeline supports thinking mode control via `ov_genai_chat_history_set_extra_context({"enable_thinking": false})`. The NPU StaticLLMPipeline does not support this API.

Instead, thinking control on NPU is injected by prepending `/nothink` to the system message content:

```json
{"role": "system", "content": "/nothink\nYou are a helpful coding assistant."}
```

The `inject_nothink_into_messages()` function handles this: it prepends `/nothink` to an existing system message, or creates one if absent. The operation is idempotent.

### KV Cache Overflow

If `max_new_tokens` exceeds the MIN_RESPONSE_LEN budget, the KV cache overflows. The pipeline wraps around and replays context tokens as output instead of generating new tokens. The `clamp_max_new_tokens()` function prevents this by capping the request to the pipeline's response budget.

## Quantization

### Qwen 2.5 (1.5B Instruct)

Works with **INT4** quantization on NPU. This is the standard quantization used for both CPU and NPU.

### Qwen 3 (4B)

**INT4 crashes on NPU.** Only **INT8_SYM** (symmetric per-channel quantization via `nncf.compress_weights`) works. This was discovered empirically — INT4 produces garbage output or pipeline crashes.

- INT4 — CPU only
- INT8_SYM — NPU (and CPU)
- FP16 — too large for NPU memory on 16 GB machines

The model registry enforces this: `qwen3-4b` has `min_ram_gb = 15.0` and uses the INT8_SYM OpenVINO artifact. Do not lower `min_ram_gb` below 15.0 until DirectML backend selection is verified working (to prevent NPU-only machines from trying to load a model that doesn't fit).

## Chat Template Patching

Qwen3 ships with a Jinja chat template that defaults to **thinking mode enabled** when `enable_thinking` is undefined:

```jinja
{%- if enable_thinking is defined and enable_thinking is false %}
    {{- '<think>\n\n</think>\n\n' }}
{%- endif %}
```

On the NPU, where `extra_context` is not available to set `enable_thinking`, this means thinking mode is always on. Thinking tokens consume the entire response budget, producing runaway generation.

The `ensure_qwen3_nothink_template()` function patches the condition to:

```jinja
{%- if not enable_thinking is defined or enable_thinking is false %}
    {{- '<think>\n\n</think>\n\n' }}
{%- endif %}
```

Now the condition is true when `enable_thinking` is undefined, defaulting to non-thinking mode.

**This patch is a hard gate.** If it fails, NPU model loading is aborted. An un-patched template on NPU produces runaway generation on every request.

The function also handles the case where `chat_template.jinja` doesn't exist as a standalone file — it extracts the template from `tokenizer_config.json` (which can contain the template as a string or an array of `{name, template}` objects) and writes it as a standalone file that OpenVINO GenAI will use.

## Thinking Suppression: Defense in Depth

Qwen3's thinking mode is suppressed through three layers, each acting as a safety net for the one above:

1. **Template patch** (`ensure_qwen3_nothink_template()`) — modifies the Jinja template so `enable_thinking` defaults to false when undefined. This is the primary gate.
2. **`/nothink` injection** (`inject_nothink_into_messages()`) — prepends `/nothink` to the system message, which Qwen3 recognizes as a directive to skip chain-of-thought reasoning.
3. **`ThinkingFilter`** (in `chat.rs`) — a streaming filter that strips `<think>...</think>` blocks from generated text token-by-token. Even with the template patch and `/nothink` injection, Qwen3 may still emit thinking tokens in some edge cases. The filter buffers incoming tokens, detects `<think>` open tags, suppresses all content until the matching `</think>` close tag, and emits only non-thinking content to the user. It handles partial tag matches across token boundaries and consumes trailing newlines after `</think>`. The filter is instantiated in the streaming route (`routes.rs`) when `disable_thinking` is true.

## Driver Requirements

- **Recommended minimum:** NPU driver version 32.0.100.3104
- **Known-good:** driver .3717
- The engine probes the driver version via `ov_core_get_property(NPU, "NPU_DRIVER_VERSION")` and logs an advisory if the version is below the recommended floor. An unreadable driver version produces a different advisory but does not block startup.

## Recovery from DEVICE_LOST

There is **no in-process DEVICE_LOST recovery** in the codebase. The string "DEVICE_LOST" does not appear in any engine source file. If the NPU encounters an unrecoverable error (driver crash, device reset), the engine process crashes.

Recovery is handled at the **process level**: the `EngineSupervisor` in the Tauri app detects the crash and auto-restarts the engine with exponential backoff (1s, 2s, 4s), up to 3 restarts per 5-minute window. This is a pragmatic choice — NPU driver state after DEVICE_LOST is unpredictable, so a clean process restart (with fresh pipeline creation and compilation cache reuse) is safer than attempting in-process pipeline destruction and recreation.

## Startup Probe

The engine host runs an authoritative NPU probe at startup (`probe_openvino_startup()`):

1. Validate the OpenVINO runtime bundle (all required DLLs present)
2. Load the OpenVINO C API and enumerate available devices
3. Check if an NPU device is exposed
4. Read the NPU's full device name and driver version
5. Classify the driver version against the recommended floor
6. Verify the OpenVINO GenAI runtime can be activated (load the C API symbols)
7. Resolve NPU tuning parameters (MAX_PROMPT_LEN, MIN_RESPONSE_LEN)

If any step fails, the probe returns a failure with a classified error (e.g., `openvino_npu_driver_missing`, `openvino_npu_plugin_unavailable`, `openvino_genai_c_api_missing`). The engine falls back to CPU.

## NPU Preflight

After the startup probe succeeds, the engine runs a preflight generation:

1. Apply Qwen3 template patch if the model is qwen3-*
2. Resolve generation controls (stop tokens, stop strings, presence penalty)
3. Create the pipeline with NPU tuning parameters and CACHE_DIR
4. Generate a single warmup response ("Warmup preflight")
5. If preflight succeeds → NPU is ready for production use
6. If preflight fails → classified error, fall back to CPU

The preflight catches issues that the probe cannot: model incompatibility, compilation failures, and KV cache configuration problems.

### Preflight Budget

The entire preflight (pipeline creation + warmup generation) runs under `OPENVINO_PREFLIGHT_BUDGET` = **300 seconds** (defined in `types.rs`). This is implemented as a `tokio::time::timeout` wrapping the `spawn_blocking` task in `run_openvino_preflight_with_timeout()`.

If the budget expires:

- Result: `OpenVinoPreflightResult::Timeout`
- Failure class: `openvino_npu_preflight_timeout`
- Persistence: `temporary_fallback` — **not persisted** to the backend decision store
- Behavior: engine falls back to CPU for this startup; next engine restart will retry NPU

The 300-second budget is what accommodates first-time NPU compilation (3-5 minutes). This is distinct from the 90-second `TOKEN_WATCHDOG_SECS` in the streaming path, which applies during inference after the pipeline is already created.

## Streaming Safety Mechanisms

The stream callback in `engine/crates/smolpc-engine-core/src/inference/genai/openvino.rs` (`StreamCallbackState`) implements multiple layers of protection against degenerate output:

### Degenerate Dot Detection

If the model emits 64 or more consecutive dots (with only whitespace between them), generation is forcibly stopped and the result is marked as truncated. This catches quantization artifacts that cause the model to emit repetitive garbage output — a known failure mode with aggressive quantization on NPU.

### Stop String Matching

In addition to `stop_token_ids` (which operate at the token ID level), the callback maintains an `accumulated` text buffer and checks for stop strings after each token: `["<|im_end|>", "<|endoftext|>"]`. This catches stop markers even when the NPU `StaticLLMPipeline` does not honor `stop_token_ids` reliably. The two-layer stop mechanism (token IDs + string matching) provides redundancy.

### Safety-Net Callback Counter

The `StreamCallbackState` tracks a `callback_count` against a `max_callback_count` limit. If the C-level `max_new_tokens` is not honored for any reason (ABI mismatch, pipeline bug), the counter forcibly stops generation. This is a third line of defense after `max_new_tokens` and stop token/string detection.

## Known Limitations and Workarounds

### NPU Device Corruption After Heavy Usage

Repeated NPU pipeline creation and destruction (e.g., running benchmarks, switching backends rapidly, or repeated model loads) can leave the NPU driver in a corrupt state. Symptoms:

- `ov_genai_llm_pipeline_create: unknown exception` on model load
- Empty NPU blob cache directory (compilation started but never finished)
- Generation failures on a previously working pipeline

**Workaround:** Reboot the machine to reset the NPU device state. After reboot, the cache will recompile on first load and the NPU will function normally. There is no software-level recovery short of a full device reset.

### "Context Too Long" Error on Fresh Chats

The engine maps all `ov_genai` + `unknown exception` errors during generation to the user-friendly message: *"This conversation has gotten too long for the current inference mode."* This is correct when the tokenized input exceeds MAX_PROMPT_LEN, but it is a **misclassification** when the real cause is NPU device corruption (see above). If a fresh chat with a short message triggers this error, the cause is almost certainly a corrupt NPU state, not an actual context overflow.

### First-Time Compilation Is Slow

The first load of any model on NPU compiles the model to an NPU-specific blob. This takes 3-5 minutes for Qwen at MAX_PROMPT_LEN=2048 and there is no way to skip it. The compiled blob is cached to disk so subsequent loads take seconds. Changing MAX_PROMPT_LEN, MIN_RESPONSE_LEN, the model, or the OpenVINO version invalidates the cache and triggers recompilation.

Users will see a long "Loading AI model..." screen on first launch with NPU. The engine's 300-second preflight budget accommodates this. If compilation exceeds the budget, the engine falls back to CPU temporarily.

### Qwen3-4B INT4 Is Broken on NPU

INT4 quantization for Qwen3-4B produces garbage output or crashes on NPU. Only INT8_SYM works. The model artifacts shipped for NPU use INT8_SYM (the 3.8 GB `openvino_model.bin`). Do not substitute INT4 artifacts for NPU use.

### No Graceful Context Window Enforcement

The NPU does not validate input length before processing. If the tokenized chat history exceeds MAX_PROMPT_LEN (2048 tokens), the pipeline crashes with "unknown exception" instead of returning an error. The engine host attempts to humanize this, but cannot distinguish it from other "unknown exception" causes (see above).

**Practical impact:** Multi-turn conversations on NPU are limited to approximately 2048 input tokens total (system prompt + all messages). Long conversations must either be trimmed or switched to CPU mode.

## Performance Characteristics

Benchmarked on Core Ultra 7 155H / 15.4 GB RAM / RTX 4050 (2026-03-27, 3 runs per prompt, 10 prompts):

| Metric | Qwen 2.5 1.5B | | | Qwen3 4B | | |
|---|---|---|---|---|---|---|
| | **CPU** | **DirectML** | **NPU** | **CPU** | **DirectML** | **NPU** |
| Median TTFT | 63 ms | 152 ms | 2,279 ms | 161 ms | 260 ms | 4,091 ms |
| Median tok/s | 51.7 | 108.6 | 16.5 | 15.1 | 52.5 | 7.9 |
| Median TPOT | 19.4 ms | 8.5 ms | 60.5 ms | 66.3 ms | 17.7 ms | 127.0 ms |
| Peak RSS | 1,966 MB | 508 MB | 2,611 MB | 5,838 MB | 593 MB | 5,031 MB |

**Key observations:**

- **DirectML is fastest** on this machine (RTX 4050 discrete GPU). Machines without a discrete GPU will not have this option.
- **CPU has the lowest TTFT** — good for perceived responsiveness on the first token.
- **NPU has high TTFT** (~2.3s for 1.5B, ~4.1s for 4B) due to StaticLLMPipeline overhead per generation call, but steady throughput once generating.
- **NPU's advantage is power efficiency**, not raw speed. On battery-powered laptops without a discrete GPU, it provides hardware acceleration without the thermal overhead of CPU inference.
- **Qwen3-4B is ~3x slower and uses ~3x more memory** than Qwen 2.5 1.5B across all backends.

## OpenVINO Version

The engine pins OpenVINO 2026.0.0 for all three components:

- `openvino` runtime: 2026.0.0
- `openvino_genai`: 2026.0.0
- `openvino_tokenizers`: 2026.0.0

All three must match. Mixing versions breaks ABI compatibility. The runtime bundle fingerprinting system enforces this — loading DLLs from a different version produces a fingerprint mismatch error.

### Known Bug: min_new_tokens

On OpenVINO GenAI 2026.0.0, setting `min_new_tokens >= 1` permanently suppresses EOS token detection. Generation runs until `max_new_tokens` is exhausted, producing garbage trailing output. The FFI wrapper explicitly skips the `set_min_new_tokens` call and logs a warning.

## Environment Variables

| Variable | Default | Purpose |
|---|---|---|
| `SMOLPC_OPENVINO_NPU_MAX_PROMPT_LEN` | 2048 | Input token budget |
| `SMOLPC_OPENVINO_NPU_MIN_RESPONSE_LEN` | 1024 | Output token budget |
| `SMOLPC_OPENVINO_BUNDLE_ROOT` | Auto-detected | OpenVINO DLL directory (dev mode only) |
| `SMOLPC_OPENVINO_MAX_TOKENS_HARD_CAP` | 8192 | Hard cap on `max_tokens` in chat completion requests |
| `SMOLPC_FORCE_EP` | None | Force a specific backend (cpu, directml, openvino_npu) |

## Key Files

| File | Purpose |
|---|---|
| `engine/crates/smolpc-engine-core/src/inference/genai/openvino.rs` | NPU pipeline creation, generation, streaming |
| `engine/crates/smolpc-engine-core/src/inference/genai/openvino_ffi.rs` | OpenVINO GenAI C API bindings |
| `engine/crates/smolpc-engine-core/src/inference/runtime_loading.rs` | DLL loading, NPU-conditional load order |
| `engine/crates/smolpc-engine-host/src/openvino.rs` | NPU tuning, template patching, startup probe, preflight |
| `engine/crates/smolpc-engine-host/src/chat.rs` | Thinking filter, token counting, context window management |
