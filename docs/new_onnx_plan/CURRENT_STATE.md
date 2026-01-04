# ONNX Migration - Current State

**Last Updated:** January 2026
**Branch:** `feature/ort_setup`
**Phase:** 1.5 Complete (Frontend Integration)

---

## Summary

Phase 1.5 is complete. The ONNX Runtime inference is now integrated with the chat UI:
- Model loads and runs via chat interface
- Streaming tokens display in real-time in chat messages
- KV Cache with Attention Sinks for efficient long-context inference
- ~8 tok/s performance on CPU
- Ollama dependency removed from chat flow (still available as fallback)

---

## What's Working

### Backend (Rust)

| Component | File | Status |
|-----------|------|--------|
| ONNX Runtime init | `src/inference/mod.rs` | ✅ `init_onnx_runtime()` with programmatic DLL path |
| Session wrapper | `src/inference/session.rs` | ✅ `InferenceSession` wraps `ort::Session` |
| Tokenizer | `src/inference/tokenizer.rs` | ✅ `TokenizerWrapper` with encode/decode |
| Generator | `src/inference/generator.rs` | ✅ Autoregressive loop with KV cache and sampling |
| KV Cache | `src/inference/kv_cache.rs` | ✅ Pre-allocated buffer with Attention Sinks |
| Types | `src/inference/types.rs` | ✅ `GenerationResult`, `GenerationConfig`, etc. |
| Model registry | `src/models/registry.rs` | ✅ Hardcoded model definitions |
| Model loader | `src/models/loader.rs` | ✅ Path utilities for model files |
| Tauri commands | `src/commands/inference.rs` | ✅ `load_model`, `unload_model`, `generate_text`, `list_models`, `get_current_model`, `check_model_exists` |

### Frontend (TypeScript/Svelte)

| Component | File | Status |
|-----------|------|--------|
| Types | `src/lib/types/inference.ts` | ✅ Matches Rust types |
| Store | `src/lib/stores/inference.svelte.ts` | ✅ Full store with streaming, cancel, model loading |
| Chat Integration | `src/App.svelte` | ✅ Uses ONNX inference for chat messages |
| Model Selector | `src/lib/components/ModelSelector.svelte` | ✅ Lists/loads ONNX models |
| Status Indicator | `src/lib/components/StatusIndicator.svelte` | ✅ Shows model load status |

### Model Files

| File | Location | Status |
|------|----------|--------|
| model.onnx | `src-tauri/models/qwen2.5-coder-1.5b/` | ✅ Present |
| tokenizer.json | `src-tauri/models/qwen2.5-coder-1.5b/` | ✅ Present |
| onnxruntime.dll | `ort-extracted/onnxruntime-win-x64-1.22.1/lib/` | ✅ v1.22.1 |

### Tests Passing

```bash
cargo test test_load_model -- --ignored        # ✅ Model loads
cargo test test_tokenizer_encode_decode -- --ignored  # ✅ Tokenizer works
cargo test test_generate_simple -- --ignored   # ✅ Generation works
```

---

## What's NOT Working / Missing

### ✅ Priority 1: COMPLETED

#### ✅ 1. Streaming Generation - DONE
**Implementation:**
- Added `generate_stream` method to Generator with callback parameter
- Added `inference_generate` Tauri command that emits events
- Frontend store has `generateStream()` method with event listeners

**Events emitted:**
- `inference_token` (String) - Each generated token
- `inference_done` (GenerationMetrics) - Generation complete
- `inference_error` (String) - On error
- `inference_cancelled` () - When cancelled

#### ✅ 2. Cancellation Support - DONE
**Implementation:**
- Added `cancelled: Arc<AtomicBool>` to `InferenceState`
- Generator checks cancellation flag in generation loop
- Added `inference_cancel` Tauri command
- Frontend store has `cancel()` method

#### ✅ 3. Sampling Methods (Temperature/Top-k/Top-p) - DONE
**Implementation:**
- Added `sample()` method to Generator supporting:
  - Temperature scaling
  - Top-k filtering
  - Top-p (nucleus) sampling
  - Fallback to greedy for temperature=0 or top_k=1
- Uses `rand` crate for random sampling

### ✅ Priority 2: COMPLETED

#### ✅ 4. KV Cache Reuse - DONE
**Implementation:**
- Created `src/inference/kv_cache.rs` with pre-allocated buffer management
- Implemented Attention Sinks (StreamingLLM) for efficient long-context handling:
  - Preserves first `sink_size` tokens (default: 4) as attention anchors
  - Uses sliding window for remaining context
  - Enables infinite-length generation without OOM
- Generator rewritten with prefill/decode separation:
  - `run_prefill()`: Processes entire prompt, builds initial cache
  - `run_decode()`: Processes single token using cached KV
- Memory efficient: ~224 MB for 4096 context (28 layers × 2 KV heads × 128 dim)

**Architecture:**
```rust
pub struct KVCache {
    key_caches: Vec<LayerCache>,    // 28 layers
    value_caches: Vec<LayerCache>,  // 28 layers
    position: usize,                // Grows indefinitely
    sink_size: usize,               // Attention sink tokens
    max_context: usize,             // Physical buffer size
}

pub struct LayerCache {
    data: Vec<f32>,                 // Pre-allocated [heads, max_seq, head_dim]
    current_length: usize,          // Valid tokens in buffer
}
```

**Tests passing:** 11 KV cache unit tests covering basic operations, attention sinks, position tracking, and bulk copies

### Priority 3: Cleanup

#### 5. Remove Ollama Code
**Current:** Ollama commands still in codebase
**Needed:** Remove when ONNX inference is production-ready

**Files to remove/modify:**
- `src/commands/ollama.rs` - Remove entirely
- `src/commands/mod.rs` - Remove ollama module
- `src/lib.rs` - Remove ollama imports and handlers
- `src/lib/stores/` - Remove ollama-related stores

#### 6. Memory Management
**Current:** No automatic model unloading
**Needed:** Unload after inactivity timeout, RAM monitoring

**Files to create:**
- `src/inference/memory.rs` - Watchdog for auto-unload

---

## Architecture Notes

### Current Structure (Embedded)
```
src-tauri/src/
├── inference/
│   ├── mod.rs          # ONNX init, exports
│   ├── generator.rs    # Generation loop with prefill/decode
│   ├── kv_cache.rs     # KV cache with Attention Sinks
│   ├── session.rs      # Session wrapper
│   ├── tokenizer.rs    # Tokenizer wrapper
│   └── types.rs        # Shared types
├── models/
│   ├── mod.rs
│   ├── loader.rs       # Path utilities
│   └── registry.rs     # Model definitions
└── commands/
    └── inference.rs    # Tauri commands
```

### Planned Structure (Separate Crate) - Future
```
smolpc-engine/          # Separate crate
├── src/
│   ├── lib.rs
│   ├── engine.rs       # Main Engine struct
│   ├── inference/
│   ├── sampling/
│   └── memory/
```

**Decision:** Stay embedded for Phase 1. Refactor to separate crate in Phase 2+ if needed.

---

## Model Architecture Constants

Hardcoded in `kv_cache.rs` for Qwen2.5-Coder-1.5B:

```rust
const NUM_LAYERS: usize = 28;
const NUM_KV_HEADS: usize = 2;  // GQA (Grouped Query Attention)
const HEAD_DIM: usize = 128;
```

Default cache settings in `Generator`:
```rust
max_context: 4096,   // Physical buffer size
sink_size: 4,        // Attention sink tokens
```

These should be read from model config in future phases.

---

## Development Commands

```bash
# Run all inference tests
cd src-tauri
cargo test -- --ignored --nocapture

# Check compilation
cargo check

# Run the app
cd ..
npm run tauri dev
```

---

## Performance Benchmarks

### Phase 0 (Without KV Cache)

| Metric | Value | Target |
|--------|-------|--------|
| TTFT (warm) | 423ms | < 3s ✅ |
| Tokens/sec | 2.44 | > 2 ✅ |
| Model load | ~4s | < 30s ✅ |

### Phase 1 (With KV Cache) - VERIFIED ✅

| Metric | Value | Target | Status |
|--------|-------|--------|--------|
| TTFT (warm) | ~420ms | < 3s | ✅ Met |
| Tokens/sec | **8.0** | > 2 | ✅ **4x target!** |
| Decode ms/tok | ~125ms | - | - |
| Memory (cache) | ~112 MB | < 224 MB | ✅ Efficient |

**Improvement achieved: 3.3x speedup** (from 2.44 to 8.0 tok/s)

See `docs/new_onnx_plan/KV_CACHE_BENCHMARK.md` for detailed analysis.

---

## Next Session Checklist

### Phase 1 - COMPLETE
1. [x] Implement streaming generation with Tauri events ✅
2. [x] Add cancellation support ✅
3. [x] Implement temperature/top-k/top-p sampling ✅
4. [x] Implement KV cache with Attention Sinks ✅
5. [x] Run end-to-end performance benchmarks with KV cache ✅ (8 tok/s achieved!)

### Phase 1.5 - Frontend Integration - COMPLETE
6. [x] Integrate streaming with existing chat UI ✅
7. [ ] Remove Ollama code when ONNX inference is fully validated (kept as fallback)
8. [ ] Add memory management (model unload timeout) (deferred to Phase 2)

### Phase 2 - GPU/NPU Acceleration
9. [ ] Test on Mac to ensure cross-platform works
10. [ ] Begin GPU/NPU acceleration (see PHASE-2.MD)
11. [ ] Implement ring buffer cache for long-context optimization
12. [ ] Add IoBinding for zero-copy inference

---

## Files Changed (Session 1 - Phase 0)

### New Files
- `src-tauri/src/inference/mod.rs`
- `src-tauri/src/inference/generator.rs`
- `src-tauri/src/inference/session.rs`
- `src-tauri/src/inference/tokenizer.rs`
- `src-tauri/src/inference/types.rs`
- `src-tauri/src/models/mod.rs`
- `src-tauri/src/models/loader.rs`
- `src-tauri/src/models/registry.rs`
- `src-tauri/src/commands/inference.rs`
- `src/lib/types/inference.ts`
- `src/lib/stores/inference.svelte.ts`

### Modified Files
- `src-tauri/src/lib.rs` - Added inference module, init call, commands
- `src-tauri/src/commands/mod.rs` - Added inference module
- `src-tauri/Cargo.toml` - Added ort, tokenizers, ndarray dependencies
- `.gitignore` - Added ONNX/model exclusions

---

## Files Changed (Session 2 - Phase 1 Streaming)

### Modified Files
- `src-tauri/src/inference/generator.rs` - Added `generate_stream()` method, `sample()` with temperature/top-k/top-p
- `src-tauri/src/commands/inference.rs` - Added `inference_generate`, `inference_cancel`, `is_generating` commands
- `src-tauri/src/lib.rs` - Registered new commands
- `src/lib/stores/inference.svelte.ts` - Added `generateStream()`, `cancel()`, event listeners
- `src/lib/types/inference.ts` - Added `GenerationConfig` type

---

## Files Changed (Session 3 - Phase 1 KV Cache)

### New Files
- `src-tauri/src/inference/kv_cache.rs` - KV cache with Attention Sinks implementation

### Modified Files
- `src-tauri/src/inference/generator.rs` - Complete rewrite with prefill/decode separation, KV cache integration
- `src-tauri/src/inference/mod.rs` - Added `kv_cache` module export

### Key Implementation Details
- **LayerCache**: Pre-allocated buffer for single layer K or V values
- **KVCache**: Manages all 28 layers with Attention Sinks support
- **Prefill phase**: Processes entire prompt, populates cache from `present.*` outputs
- **Decode phase**: Processes single token, appends to cache with shift-and-sink logic
- **Memory layout**: `[heads, seq_len, head_dim]` flattened contiguously
- **Bulk copies**: Uses `extend_from_slice()` for efficient array generation

---

---

## Files Changed (Session 4 - KV Cache Benchmarks)

### New Files
- `src-tauri/src/inference/benchmark.rs` - Comprehensive benchmark suite for KV cache performance
- `docs/new_onnx_plan/KV_CACHE_BENCHMARK.md` - Benchmark results and analysis

### Modified Files
- `src-tauri/src/inference/mod.rs` - Added benchmark module (test-only)
- `src-tauri/src/inference/generator.rs` - Added tokenizer() getter for tests
- `docs/new_onnx_plan/CURRENT_STATE.md` - Updated with benchmark results

---

---

## Files Changed (Session 5 - Phase 1.5 Frontend Integration)

### Modified Files
- `src/App.svelte` - Replaced Ollama with ONNX inference:
  - Removed `ollamaStore` import, added `inferenceStore`
  - Changed event listeners from `ollama_*` to `inference_*`
  - Updated `handleSendMessage()` to use `inferenceStore.generateStream()`
  - Updated `handleCancelGeneration()` to use `inferenceStore.cancel()`
  - Changed status checks from `ollamaStore.isConnected` to `inferenceStore.isLoaded`
  - Added auto-load on mount for first available model
- `src/lib/components/ModelSelector.svelte` - ONNX model selection:
  - Shows available ONNX models from `inferenceStore`
  - Loads model on selection
  - Shows loading spinner during model load
- `src/lib/components/StatusIndicator.svelte` - ONNX status display:
  - Changed from `OllamaStatus` to `InferenceStatus` type
  - Shows model load state (loaded/loading/not loaded)
  - Shows current model name when loaded

### Key Implementation Details
- **Auto-load on startup**: First available model loads automatically via `initInference()`
- **Streaming callback pattern**: `inferenceStore.generateStream()` takes callback for token updates
- **Context building**: Conversation history formatted as `User: ... / Assistant: ...` prompt
- **Generation config**: Uses temperature from settings store

---

### Not Committed (in .gitignore)
- `ort-extracted/` - ONNX Runtime DLLs
- `src-tauri/models/` - Model files (~900MB)
- `onnxruntime-win-x64-1.22.1.zip` - Downloaded archive
