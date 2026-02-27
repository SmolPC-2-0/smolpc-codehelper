# Phase 2A: DirectML Acceleration

## Goal

Add GPU acceleration via DirectML to the existing CPU-only ONNX inference pipeline. DML requires a **separately exported model** with a different input schema — this is not a drop-in EP swap.

## Baseline Assumption

The CPU inference pipeline is stable and consists of:

- `generator.rs` — autoregressive loop (prefill -> decode) with streaming
- `kv_cache.rs` — dynamic KV cache with Attention Sinks
- `input_builder.rs` — ordered input construction (`input_ids`, `attention_mask`, `past_key_values.*`)
- `session.rs` — ORT session wrapper
- `runtime_spec.rs` — per-model IO tensor contracts
- `models/registry.rs` + `loader.rs` — model lookup and path resolution
- `tokenizer.rs` — shared tokenizer (same for both backends)

All existing DML workarounds (dummy prefill, backend selection, backend store, benchmark gating) should be considered reverted/removable.

---

## Why a Separate Model is Required

The CPU model (exported via HuggingFace Optimum) uses `GroupQueryAttention` contrib ops with:

- `attention_mask` input (dynamic, grows each step)
- Dynamically-sized `past_key_values` tensors (seq_len dimension changes every step)
- Present outputs that are larger than past inputs

DirectML's GQA kernel expects a fundamentally different format:

- `seqlens_k` + `total_sequence_length` inputs (replaces `attention_mask`)
- Fixed-size `past_key_values` tensors (pre-allocated at `max_sequence_length`)
- `past_present_share_buffer=true` (present outputs alias past inputs)

Attempting to run the CPU model on DML EP produces `E_INVALIDARG` (0x80070057) at the first GQA node because the DML kernel's output shape validation fails against the dynamic tensor format.

### References

- [ORT Issue #24937](https://github.com/microsoft/onnxruntime/issues/24937) — Llama-3.2-1B fails identically on DirectML
- [ORT Issue #20575](https://github.com/microsoft/onnxruntime/issues/20575) — DirectML Exception 80070057
- [Past-present share buffer docs](https://onnxruntime.ai/docs/genai/howto/past-present-share-buffer.html)
- [GQA Discussion #22732](https://github.com/microsoft/onnxruntime/discussions/22732) — seqlens_k / total_sequence_length behavior

---

## Step 0: Export the DML Model (Manual, Outside Codebase)

```bash
pip install onnxruntime-genai
python -m onnxruntime_genai.models.builder \
  -m Qwen/Qwen2.5-Coder-1.5B-Instruct \
  -o ./qwen2.5-coder-1.5b-dml \
  -p int4 \
  -e dml
```

**Before writing any code**, inspect the exported model's actual input/output tensor names and shapes. The genai builder may use different naming conventions than the Optimum export. Document the exact schema — it drives every subsequent step.

### Expected DML Model Schema (Verify Against Actual Export)

| Input | Type | Shape | Notes |
|-------|------|-------|-------|
| `input_ids` | INT64 | `[batch, seq_len]` | Same as CPU |
| `seqlens_k` | INT32 | `[batch]` | Valid KV cache length per batch item |
| `total_sequence_length` | INT32 | scalar | Total position counter |
| `past_key_values.N.key` | FP16/FP32 | `[batch, kv_heads, max_seq_len, head_dim]` | **Fixed size**, pre-allocated |
| `past_key_values.N.value` | FP16/FP32 | `[batch, kv_heads, max_seq_len, head_dim]` | **Fixed size**, pre-allocated |

| Output | Type | Shape |
|--------|------|-------|
| `logits` | FP32 | `[batch, seq_len, vocab]` |
| `present.N.key` | — | Aliases past buffer (no extraction needed) |
| `present.N.value` | — | Aliases past buffer (no extraction needed) |

### Key Differences from CPU Model

- No `attention_mask` -> replaced by `seqlens_k` + `total_sequence_length`
- KV cache tensors are **fixed size** at `max_seq_len`, never resized
- `past_present_share_buffer=true` -> present outputs write into the same buffer as past inputs; no separate extraction/append step needed
- Weights are INT4 quantized (smaller file, GPU-optimized)

---

## Step 1: Ship DirectML-Capable ORT DLL

The current `onnxruntime.dll` is CPU-only. Replace it with the DML-capable build from `Microsoft.ML.OnnxRuntime.DirectML` NuGet package. This DLL supports **both** CPU EP and DML EP, so it's a drop-in replacement.

- Update `setup-libs.sh` / `setup-libs.ps1` to download the DML build
- Update offline installer script if applicable
- Verify CPU inference still works with the new DLL (regression check)

---

## Step 2: Model Directory Structure + Loader

```
models/qwen2.5-coder-1.5b/
├── cpu/
│   ├── model.onnx
│   └── model.onnx_data
├── dml/
│   ├── model.onnx
│   └── (model.onnx_data or embedded)
└── tokenizer.json          # Shared
```

Extend model loader to resolve path by backend:

- `resolve_model_path(model_id, backend) -> PathBuf`
- If DML model doesn't exist -> backend selection skips DML entirely

---

## Step 3: Extend ModelRuntimeSpec for Dual Schema

The CPU and DML models have different IO contracts. Encode this in the type system:

```rust
pub enum KvInputSchema {
    /// CPU: dynamic past tensors + attention_mask
    AttentionMask {
        attention_mask: &'static str,
    },
    /// DML: fixed-size past tensors + seqlens_k + total_sequence_length
    SeqlensK {
        seqlens_k: &'static str,
        total_sequence_length: &'static str,
        max_sequence_length: usize,
    },
}
```

`ModelIoSpec` replaces the current `attention_mask` field with `kv_schema: KvInputSchema`. The model registry provides a separate spec per (model, backend) pair.

---

## Step 4: Extend InputBuilder

Currently hardcoded to `input_ids` + `attention_mask` + `past_key_values.*`. Needs to support the DML schema's `seqlens_k` + `total_sequence_length` as alternative inputs.

- Factory method `InputBuilder::from_schema(kv_schema, ...)` constructs the right layout
- Add `set_seqlens_k()` and `set_total_sequence_length()` setters
- Input count differs: CPU has 2 + 2\*layers, DML has 3 + 2\*layers (`seqlens_k` + `total_seq_len` replace `attention_mask`)
- Validation logic in `with_names_and_input_order()` must accommodate both

---

## Step 5: DML KV Cache (Fixed-Buffer)

The CPU `KVCache` is dynamic — `to_array()` creates a new array sized to `current_length` each step. DML needs the opposite: a fixed-size buffer where only a length counter changes.

Create a separate `DmlKvCache`:

- Pre-allocates all layer buffers at `[1, kv_heads, max_seq_len, head_dim]` at init
- Tracks `valid_length: usize` (becomes the `seqlens_k` value)
- **No per-step array creation** — returns references/views to the existing buffers
- Attention Sinks: shifts data within the fixed buffer, decrements `valid_length`, same sink preservation logic
- Since `past_present_share_buffer=true`, present outputs write directly into the past buffer — the cache update after each step is just incrementing `valid_length` (no extraction from outputs needed)

---

## Step 6: Dual-Path Generator

The generation loop (tokenize -> prefill -> decode loop -> sample -> emit) is shared. Only the **input preparation** and **output extraction** differ per backend.

### Strategy Pattern via Enum

```rust
enum InferenceStrategy {
    Cpu {
        cache: KVCache,
        input_builder: InputBuilder,  // attention_mask variant
    },
    DirectML {
        cache: DmlKvCache,
        input_builder: InputBuilder,  // seqlens_k variant
    },
}
```

`Generator::generate_stream()` delegates to `strategy.prepare_prefill_inputs(...)`, `strategy.prepare_decode_inputs(...)`, and `strategy.process_outputs(...)`.

### Prefill — CPU Path (Existing)

- `attention_mask`: all 1s, length = seq_len
- `past_key_values.*`: zero-length on dim 2 (empty cache)
- Extract present outputs -> populate KVCache

### Prefill — DML Path

- `seqlens_k`: `[0]` (no cached tokens)
- `total_sequence_length`: `seq_len`
- `past_key_values.*`: pre-allocated zeros at max_seq_len
- No output extraction needed (buffer updated in-place), increment valid_length to seq_len

### Decode — CPU Path (Existing)

- `attention_mask`: all 1s, length = past_length + 1
- `past_key_values.*`: `to_array()` at current_length
- Extract last-position KV from present outputs -> `cache.append()`

### Decode — DML Path

- `seqlens_k`: `[valid_length]`
- `total_sequence_length`: `valid_length + 1`
- `past_key_values.*`: same fixed buffer (already contains previous state)
- No output extraction, increment valid_length

---

## Step 7: Backend Selection

At app startup:

1. Check env var `SMOLPC_FORCE_EP=cpu|dml` -> forced override (debug/testing)
2. Check if DML model exists on disk -> if not, CPU only
3. Check for persisted decision (keyed by model + GPU adapter + driver version + app version)
4. If no persisted decision -> run a short benchmark (e.g., 8 tokens on each backend)
5. **Gate:** DML chosen only if decode tok/s improves by >= 30% AND time-to-first-token doesn't regress by > 15%
6. Persist the decision
7. Track runtime failures — if DML fails 3 consecutive times, demote to CPU and persist

This needs:

- A `BackendSelector` that orchestrates the above flow
- A persistence file (JSON in app data dir) for cached decisions
- GPU adapter detection (DirectX/DXGI enumeration for adapter name + driver version)
- A benchmark runner that creates both sessions, runs a short generation, compares metrics

---

## Step 8: Session Creation

Extend `InferenceSession` to accept backend selection:

- **CPU:** `with_intra_threads(4)`, optimization level 3
- **DML:** `with_execution_providers([DirectML::default()])`, disable memory pattern, disable parallel execution (DML requirements)

The `ort` crate's `ep::DirectML` builder handles this. Only available on Windows (`#[cfg(target_os = "windows")]`).

---

## Step 9: Frontend Changes

Minimal — the streaming contract (`Channel<String>` for tokens, `GenerationMetrics` on completion) stays identical. The frontend doesn't need to know which backend is running.

**One addition:** Surface the active backend to the UI so the user knows if they're on CPU or DML:

- Add `active_backend: String` to the status/info command response
- Display as a subtle indicator in the chat UI (e.g., "CPU" or "DirectML" badge)

---

## Step 10: IoBinding Optimization (Stretch Goal)

After correctness is proven, the DML path's biggest remaining bottleneck is CPU<->GPU tensor copies every step. ORT's `IoBinding` API pre-binds tensors to GPU memory:

- Allocate KV cache buffers on GPU once
- Bind them as both inputs and outputs
- Each step: only `input_ids` (tiny) and `seqlens_k`/`total_sequence_length` (scalars) cross the bus

This is a meaningful performance win but not required for initial correctness. Defer until basic DML inference works end-to-end.

---

## Execution Order Summary

| Phase | What | Depends On |
|-------|------|-----------|
| 0 | Export DML model + document schema | Nothing |
| 1 | Swap ORT DLL to DML-capable build | Step 0 (verify CPU still works) |
| 2 | Model directory structure + loader | Step 0 |
| 3 | `KvInputSchema` enum + `ModelRuntimeSpec` extension | Step 0 (schema known) |
| 4 | `InputBuilder` dual-schema support | Step 3 |
| 5 | `DmlKvCache` fixed-buffer implementation | Step 3 |
| 6 | Generator dual-path (strategy pattern) | Steps 4 + 5 |
| 7 | Backend selection + benchmarking + persistence | Step 6 |
| 8 | Session creation with DML EP | Step 1 |
| 9 | Frontend backend indicator | Step 7 |
| 10 | IoBinding optimization | Step 6 (stretch) |

Steps 3-5 can be developed and unit-tested without DML hardware. Step 6 onward needs the DML model and a DX12 GPU.

---

## Testing Strategy

1. **Unit tests:** `InputBuilder` with DML schema, `DmlKvCache` buffer management, `seqlens_k` tracking
2. **Integration test:** Run same prompt on CPU and DML with greedy decoding, compare outputs (should be close but not identical due to INT4 quantization)
3. **Benchmark gate test:** Verify benchmark comparison logic with real measurements
4. **Failure/fallback test:** Kill DML session mid-generation -> verify fallback to CPU

---

## Out of Scope (Deferred)

- **OpenVINO / NPU** — separate runtime (not ONNX Runtime), separate model format (OpenVINO IR), much larger undertaking. Treat as Phase 3.
- **IoBinding** for zero-copy GPU tensor management — Phase 2A stretch goal
- **Multi-model memory budgeting** — loading both CPU and DML models simultaneously
- **Dynamic quantization selection** — INT4 vs FP16 based on GPU VRAM

---

## OpenVINO Note

The original plan assumed OpenVINO could be "just another EP" in ONNX Runtime. Research shows this is not viable for LLMs:

- OpenVINO EP does not support `GroupQueryAttention` contrib ops — attention layers fall back to CPU, defeating the purpose
- `onnxruntime-genai` model builder does not support OpenVINO as a target EP
- Intel's recommended path for LLM inference on NPU is **OpenVINO GenAI** — a completely separate runtime with its own model format (OpenVINO IR), KV cache management (PagedAttention), and generation loop
- OpenVINO 2025.3 validates Qwen2.5-1.5B-Instruct on NPU natively

OpenVINO/NPU support would require integrating a second inference runtime (e.g., via the `openvino` Rust crate or FFI), not just adding another execution provider. This is architecturally distinct from DML and should be scoped as its own phase.
