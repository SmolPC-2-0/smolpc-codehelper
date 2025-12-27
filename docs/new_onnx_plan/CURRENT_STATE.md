# ONNX Migration - Current State

**Last Updated:** December 2025
**Branch:** `feature/ort_setup`
**Phase:** 0 Complete, Phase 1 In Progress

---

## Summary

Phase 0 MVP is complete. The ONNX Runtime inference pipeline is functional:
- Model loads and runs
- Generation produces correct output
- Performance is acceptable (2.44 tok/s, 423ms TTFT)

---

## What's Working

### Backend (Rust)

| Component | File | Status |
|-----------|------|--------|
| ONNX Runtime init | `src/inference/mod.rs` | ✅ `init_onnx_runtime()` with programmatic DLL path |
| Session wrapper | `src/inference/session.rs` | ✅ `InferenceSession` wraps `ort::Session` |
| Tokenizer | `src/inference/tokenizer.rs` | ✅ `TokenizerWrapper` with encode/decode |
| Generator | `src/inference/generator.rs` | ✅ Autoregressive loop with greedy sampling |
| Types | `src/inference/types.rs` | ✅ `GenerationResult`, `GenerationConfig`, etc. |
| Model registry | `src/models/registry.rs` | ✅ Hardcoded model definitions |
| Model loader | `src/models/loader.rs` | ✅ Path utilities for model files |
| Tauri commands | `src/commands/inference.rs` | ✅ `load_model`, `unload_model`, `generate_text`, `list_models`, `get_current_model`, `check_model_exists` |

### Frontend (TypeScript/Svelte)

| Component | File | Status |
|-----------|------|--------|
| Types | `src/lib/types/inference.ts` | ✅ Matches Rust types |
| Store | `src/lib/stores/inference.svelte.ts` | ✅ Basic store with all commands |

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

### Priority 2: Performance Critical

#### 4. KV Cache Reuse
**Current:** Empty KV cache provided every step (recomputes entire sequence)
**Needed:** Store `present.*` outputs and feed as `past_key_values.*` inputs

**Impact:** Will improve generation speed by 5-10x

```rust
// Current (slow):
for step in 0..max_length {
    let kv_cache = Self::create_empty_kv_cache()?;  // Empty every time!
    // ... run inference with full sequence
}

// Needed (fast):
let mut kv_cache = None;
for step in 0..max_length {
    let inputs = if let Some(cache) = &kv_cache {
        // Use cached KV, only process new token
        Self::create_inputs_with_cache(new_token, cache)?
    } else {
        // First pass: process full prompt
        Self::create_inputs_without_cache(prompt_tokens)?
    };

    let outputs = session.run(inputs)?;
    kv_cache = Some(Self::extract_kv_cache(&outputs)?);
}
```

**Files to modify:**
- `src/inference/generator.rs` - Major refactor of generate loop
- Consider creating `src/inference/kv_cache.rs`

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
│   ├── generator.rs    # Generation loop
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

Hardcoded in `generator.rs` for Qwen2.5-Coder-1.5B:

```rust
const NUM_LAYERS: usize = 28;
const NUM_KV_HEADS: usize = 2;  // GQA
const HEAD_DIM: usize = 128;
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

## Performance Benchmarks (Phase 0)

| Metric | Value | Target (Phase 1) |
|--------|-------|------------------|
| TTFT (warm) | 423ms | < 3s ✅ |
| Tokens/sec | 2.44 | > 2 ✅ |
| Model load | ~4s | < 30s ✅ |

Note: Performance will improve significantly with KV cache reuse.

---

## Next Session Checklist

1. [x] Implement streaming generation with Tauri events ✅
2. [x] Add cancellation support ✅
3. [x] Implement temperature/top-k/top-p sampling ✅
4. [ ] (Optional) Start KV cache reuse for better performance
5. [ ] Test on Mac to ensure cross-platform works
6. [ ] Integrate streaming with existing chat UI
7. [ ] Remove Ollama code when ONNX inference is fully validated

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

### Not Committed (in .gitignore)
- `ort-extracted/` - ONNX Runtime DLLs
- `src-tauri/models/` - Model files (~900MB)
- `onnxruntime-win-x64-1.22.1.zip` - Downloaded archive
