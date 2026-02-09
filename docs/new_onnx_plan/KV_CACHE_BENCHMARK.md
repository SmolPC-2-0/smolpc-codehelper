# KV Cache Performance Benchmark Report

**Date:** January 2025
**Branch:** `feature/ort_setup`
**Hardware:** Windows x64, CPU-only inference

---

## Executive Summary

The KV Cache implementation is **functional and provides significant performance improvement**:

| Metric         | Without Cache | With Cache | Improvement     |
| -------------- | ------------- | ---------- | --------------- |
| Tokens/second  | ~2.44         | **~8.0**   | **3.3x faster** |
| Time per token | ~410ms        | **~125ms** | **3.3x faster** |
| TTFT (warm)    | ~420ms        | ~420ms     | Same (expected) |

The cache is working correctly. There are optimization opportunities for Phase 2+.

---

## Benchmark Results

### 1. Pure KV Cache Operations (No Model)

```
Operation                              Time
────────────────────────────────────────────
Cache creation (2048 ctx)              0.240 ms
100 appends (no shift)                 2.013 ms
56× to_array() calls                   0.613 ms
100 appends (WITH shift)             701.822 ms
────────────────────────────────────────────
Per-operation breakdown:
  Single append (no shift):       20.12 µs
  Single append (with shift):   7018.21 µs  ⚠️
  Single to_array():              10.95 µs
────────────────────────────────────────────
KV Cache memory: 112 MB (for 2048 context)
```

**Critical Finding:** Append-with-shift is **350x slower** than regular append due to moving ~111 MB of data.

### 2. Decode Step Overhead (Without ONNX Inference)

```
Cache size: 100 tokens
Overhead per decode step (building inputs only):
  Average:    2.471 ms
  Min:        2.034 ms
  Max:        3.179 ms
```

This overhead is ~2% of total token time (~125ms). Acceptable.

### 3. Full Generation Performance

```
Token-by-token timing (512 context, 30 tokens generated):

Token  Time (ms)
─────────────────
    1     277.51  ← TTFT (includes prefill)
    2     127.34
    3     126.32
    ...
   20     134.93

Decode Statistics (excluding TTFT):
  Average:   129.45 ms/token
  Min:       123.48 ms/token
  Max:       141.74 ms/token

  Implied tok/s: 7.72
```

### 4. Context Size Scaling

```
Context size:   128 | tok/s:   7.80 | TTFT:  411ms
Context size:   256 | tok/s:   8.04 | TTFT:  423ms
Context size:   512 | tok/s:   8.03 | TTFT:  423ms
Context size:  1024 | tok/s:   8.03 | TTFT:  409ms
Context size:  2048 | tok/s:   8.03 | TTFT:  417ms
```

**Good news:** Larger cache allocation doesn't impact performance until cache fills.

---

## Analysis

### What's Working Well

1. **Cache reuse is effective** - 3.3x speedup achieved
2. **Pre-allocation** - No runtime memory fragmentation
3. **Attention Sinks logic** - Correctly preserves first N tokens
4. **to_array() is fast** - Only ~0.6ms for all 56 layer arrays

### Performance Bottlenecks

#### 1. Shift Operation When Cache Full (P1 - Critical for long conversations)

When cache reaches max_context, EVERY new token triggers:

```
56 caches × (2044 positions × 128 dims × 2 heads × 4 bytes) = ~111 MB moved
```

**Impact:** 7ms overhead per token after cache fills. Reduces tok/s from 8.0 to ~7.2.

**Solution:** Implement ring buffer instead of shift-copy.

#### 2. Data Copying in Decode Loop (P2 - Moderate)

Each decode step:

```
56 × to_array() allocations
56 × Value::from_array() tensor creations
58 × HashMap insertions
56 × output tensor extractions
```

**Impact:** ~2-3ms overhead per token.

**Solution:** Use ONNX Runtime IoBinding for zero-copy I/O.

#### 3. HashMap Rebuilding (P3 - Minor)

```rust
let mut inputs: HashMap<...> = HashMap::new();  // Every token!
```

**Impact:** <0.5ms per token.

**Solution:** Pre-allocate HashMap or use Vec-based inputs.

---

## Recommended Improvements

### Phase 2 Optimizations (Recommended)

| Priority | Optimization         | Actual Gain     | Status      |
| -------- | -------------------- | --------------- | ----------- |
| P3       | Pre-allocated inputs | **+9.8% tok/s** | ✅ **DONE** |

### Pre-Allocated Input Containers (Implemented)

**Before:** 129.45 ms/token, 7.72 tok/s
**After:** 117.93 ms/token, **8.48 tok/s**
**Improvement:** 11.5ms per token, 9.8% faster

The `InputBuilder` struct pre-allocates:

- All 58 input key strings (created once at generation start)
- HashMap with capacity for 58 entries (no rehashing)
- Reuses HashMap via `clear()` across decode steps

See `src-tauri/src/inference/input_builder.rs` for implementation.

---

### Phase 2 Optimizations (Planned)

| Priority | Optimization | Estimated Gain | Complexity |
| -------- | ------------ | -------------- | ---------- |
| P2       | IoBinding    | +10-20% tok/s  | High       |

Note: Ring buffer was considered but rejected - ONNX requires contiguous memory,
so linearization would negate any gains. IoBinding with GPU memory is the proper solution.

### P2: IoBinding (Phase 2 - GPU required)

```rust
// Current: copy on every call
let value = Value::from_array(cache.to_array())?;

// With IoBinding: zero-copy
io_binding.bind_output_to_device("present.0.key", &cache_buffer)?;
```

**Benefit:** Eliminates most data copying. Requires GPU memory management.

### P3: Pre-allocated Input Containers

```rust
// Current
let mut inputs: HashMap<String, Value> = HashMap::new();

// Optimized
struct PreallocatedInputs {
    input_ids: Value,
    attention_mask: Value,
    kv_cache_keys: [Value; NUM_LAYERS],
    kv_cache_values: [Value; NUM_LAYERS],
}
```

**Benefit:** Eliminates HashMap allocation overhead.

---

## Verification

Run benchmarks:

```bash
cd src-tauri

# Pure cache benchmarks (no model needed)
cargo test bench_kv_cache_pure -- --nocapture
cargo test bench_decode_step_overhead -- --nocapture

# Full benchmarks (requires model)
cargo test bench_single_decode_timing -- --ignored --nocapture
cargo test bench_cache_fill_levels -- --ignored --nocapture
cargo test bench_full_generation -- --ignored --nocapture
```

---

## Conclusion

The KV Cache implementation is **production-ready for Phase 1**:

- ✅ Achieves 3.3x speedup (target was >2x)
- ✅ 8 tok/s exceeds target of >2 tok/s
- ✅ Memory usage is bounded (~112 MB for 2048 context)
- ✅ Attention Sinks work correctly for long contexts

**Recommendation:** Proceed to frontend integration. Ring buffer optimization can be added in Phase 2 when long-context usage becomes common.

---
