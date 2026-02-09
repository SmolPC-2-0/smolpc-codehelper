# Phase 2: GPU & OpenVINO Acceleration

**Goal:** Enable hardware acceleration for Intel NPU (OpenVINO) and NVIDIA GPU (CUDA).

**Prerequisites:**
- Read `CONTEXT.md` for overall project understanding
- Phase 1 must be complete (CPU inference working)

---

## Objectives

1. Implement Execution Provider abstraction
2. Add OpenVINO EP for Intel NPU acceleration
3. Add CUDA EP for NVIDIA GPU acceleration
4. Implement automatic hardware detection and EP selection
5. Add EP fallback chain with graceful degradation
6. Bundle required runtime DLLs

---

## Deliverables Checklist

### EP Abstraction

- [ ] Define `ExecutionProvider` trait
- [ ] Refactor CPU provider to use trait
- [ ] Implement provider registry/selection

### Cache Backend Abstraction

- [ ] Define `CacheBackend` trait for device-agnostic KV caching
- [ ] Implement `CpuCacheBackend` using current `Vec<f32>` implementation
- [ ] Implement `GpuCacheBackend` using ORT IoBinding for zero-copy
- [ ] Add cache backend selection based on EP

### Hardware Detection

- [ ] Detect Intel NPU (Core Ultra)
- [ ] Detect NVIDIA GPU
- [ ] Detect available RAM
- [ ] Cache detection results

### OpenVINO Integration

- [ ] Add OpenVINO EP configuration
- [ ] Implement input shape bucketing (128, 256, 512, 1024)
- [ ] Bundle OpenVINO DLLs
- [ ] Test on Intel Core Ultra system

### CUDA Integration

- [ ] Add CUDA EP configuration
- [ ] Bundle CUDA/cuDNN DLLs
- [ ] Set GPU device selection
- [ ] Test on NVIDIA GPU system

### Fallback Chain

- [ ] Implement EP selection priority
- [ ] Handle EP initialization failures gracefully
- [ ] Cache successful EP for next startup
- [ ] Show user notification on fallback

### Enhanced Model

- [ ] Add 7B model support for 16GB+ systems
- [ ] Implement model download (if not bundled)
- [ ] Model switching in settings UI

### Status UI

- [ ] Show active accelerator in status bar
- [ ] Show EP name (Intel AI Boost, NVIDIA GPU, CPU)
- [ ] Memory usage display

---

## Technical Specifications

### ExecutionProvider Trait

```rust
pub trait ExecutionProvider: Send + Sync {
    /// Provider name for display
    fn name(&self) -> &str;

    /// Check if hardware is available
    fn is_available(&self) -> bool;

    /// Initialize the provider
    fn initialize(&mut self) -> Result<(), EngineError>;

    /// Get ORT session builder with provider config
    fn configure_session(&self, builder: SessionBuilder) -> Result<SessionBuilder, EngineError>;

    /// Prepare inputs (e.g., padding for OpenVINO)
    fn prepare_inputs(&self, tokens: &[i64], max_len: usize) -> Result<InputTensors, EngineError>;

    /// Whether warmup is needed (e.g., QNN context binary)
    fn requires_warmup(&self) -> bool { false }

    /// Perform warmup if needed
    fn warmup(&mut self, _session: &Session) -> Result<(), EngineError> { Ok(()) }
}
```

### Provider Implementations

```rust
pub struct CpuProvider;
pub struct OpenVinoProvider {
    device_type: String,  // "NPU" or "GPU"
    input_buckets: Vec<usize>,
}
pub struct CudaProvider {
    device_id: usize,
}
```

### CacheBackend Trait

The KV cache implementation needs to support both CPU (Vec-based) and GPU (IoBinding) storage to avoid expensive device-to-host copies during generation.

```rust
use ndarray::Array4;

/// Trait for device-agnostic KV cache storage
pub trait CacheBackend: Send + Sync {
    /// Append a single token's KV values for a layer
    fn append(&mut self, layer: usize, key: &[f32], value: &[f32]) -> Result<(), EngineError>;

    /// Extend cache with multiple tokens (used during prefill)
    fn extend(&mut self, layer: usize, keys: &[f32], values: &[f32]) -> Result<(), EngineError>;

    /// Get key tensor for a layer (for ORT input binding)
    fn get_key_tensor(&self, layer: usize) -> CacheTensor;

    /// Get value tensor for a layer (for ORT input binding)
    fn get_value_tensor(&self, layer: usize) -> CacheTensor;

    /// Get current sequence length in cache
    fn len(&self) -> usize;

    /// Check if Attention Sinks shift is needed
    fn needs_shift(&self) -> bool;

    /// Perform Attention Sinks shift (preserve sink tokens, slide window)
    fn shift(&mut self) -> Result<(), EngineError>;
}

/// Tensor reference that works with IoBinding
pub enum CacheTensor<'a> {
    /// CPU: owned Array4 created on demand
    Cpu(Array4<f32>),
    /// GPU: reference to pre-allocated CUDA tensor via IoBinding
    Gpu(&'a ort::Value),
}
```

### CPU Cache Backend

Uses the current `Vec<f32>` implementation from Phase 1:

```rust
pub struct CpuCacheBackend {
    key_caches: Vec<LayerCache>,
    value_caches: Vec<LayerCache>,
    position: usize,
    sink_size: usize,
    max_context: usize,
}

impl CacheBackend for CpuCacheBackend {
    fn get_key_tensor(&self, layer: usize) -> CacheTensor {
        // Uses bulk copy with extend_from_slice (Phase 1 optimized)
        CacheTensor::Cpu(self.key_caches[layer].to_array())
    }
    // ... other methods wrap existing LayerCache/KVCache logic
}
```

### GPU Cache Backend (IoBinding)

For CUDA/OpenVINO, tensors stay on device:

```rust
pub struct GpuCacheBackend {
    /// Pre-allocated key tensors on GPU [layer][batch, heads, max_seq, head_dim]
    key_tensors: Vec<ort::Value>,
    /// Pre-allocated value tensors on GPU
    value_tensors: Vec<ort::Value>,
    /// Current valid sequence length
    current_length: usize,
    /// Attention sink configuration
    sink_size: usize,
    max_context: usize,
    /// IoBinding for zero-copy input/output
    io_binding: ort::IoBinding,
}

impl CacheBackend for GpuCacheBackend {
    fn get_key_tensor(&self, layer: usize) -> CacheTensor {
        // Zero-copy: returns reference to GPU tensor
        CacheTensor::Gpu(&self.key_tensors[layer])
    }

    fn append(&mut self, layer: usize, key: &[f32], value: &[f32]) -> Result<(), EngineError> {
        // Use CUDA copy to append at current_length position
        // This avoids GPU->CPU->GPU round trip
        self.copy_to_device_at_offset(layer, key, value, self.current_length)?;
        Ok(())
    }

    fn shift(&mut self) -> Result<(), EngineError> {
        // Launch CUDA kernel to shift data in-place
        // Preserves sink tokens, slides window
        self.launch_shift_kernel()?;
        Ok(())
    }
}
```

### Generator Integration

```rust
impl Generator {
    pub fn new_with_backend<B: CacheBackend>(
        session: Arc<Mutex<InferenceSession>>,
        tokenizer: Arc<TokenizerWrapper>,
        config: GenerationConfig,
        cache_backend: B,
    ) -> Self {
        // ...
    }

    async fn run_decode_with_backend<B: CacheBackend>(
        &self,
        token_id: i64,
        cache: &mut B,
    ) -> Result<Array2<f32>, String> {
        // Build inputs using cache.get_key_tensor() / get_value_tensor()
        // For GPU: these bind directly to device memory via IoBinding
        // For CPU: these create Array4 copies (current behavior)
    }
}
```

### Migration Path

1. **Phase 2a**: Extract current KVCache logic into `CpuCacheBackend`
2. **Phase 2b**: Add `CacheBackend` trait, make Generator generic over backend
3. **Phase 2c**: Implement `GpuCacheBackend` with IoBinding for CUDA EP
4. **Phase 2d**: Extend to OpenVINO (may need different approach for NPU)

### EP Selection Logic

```rust
pub fn select_provider(hardware: &HardwareInfo, cached: Option<&str>) -> Box<dyn ExecutionProvider> {
    // Try cached provider first
    if let Some(cached_name) = cached {
        if let Some(provider) = try_provider(cached_name, hardware) {
            return provider;
        }
    }

    // Priority order
    let priority = [
        ("openvino_npu", || has_intel_npu(hardware)),
        ("cuda", || has_nvidia_gpu(hardware)),
        ("openvino_gpu", || has_intel_gpu(hardware)),
        ("cpu", || true),
    ];

    for (name, check) in priority {
        if check() {
            if let Some(provider) = try_provider(name, hardware) {
                return provider;
            }
        }
    }

    // Ultimate fallback
    Box::new(CpuProvider)
}
```

### Intel NPU Detection

```rust
fn has_intel_npu(hardware: &HardwareInfo) -> bool {
    // Check for Core Ultra (Meteor Lake) or newer
    // CPU model contains "Core Ultra" or specific model numbers
    hardware.cpu.name.contains("Core Ultra") ||
    hardware.cpu.name.contains("Ultra 5") ||
    hardware.cpu.name.contains("Ultra 7") ||
    hardware.cpu.name.contains("Ultra 9")
}
```

### OpenVINO Input Bucketing

OpenVINO NPU works better with static shapes. Pad inputs to nearest bucket:

```rust
impl OpenVinoProvider {
    fn get_bucket_size(&self, actual_len: usize) -> usize {
        for &bucket in &self.input_buckets {
            if actual_len <= bucket {
                return bucket;
            }
        }
        *self.input_buckets.last().unwrap()
    }

    fn prepare_inputs(&self, tokens: &[i64], _max_len: usize) -> Result<InputTensors, EngineError> {
        let bucket = self.get_bucket_size(tokens.len());
        let mut padded = vec![0i64; bucket];
        padded[..tokens.len()].copy_from_slice(tokens);
        // Create attention mask: 1 for real tokens, 0 for padding
        let attention_mask: Vec<i64> = (0..bucket)
            .map(|i| if i < tokens.len() { 1 } else { 0 })
            .collect();
        Ok(InputTensors { input_ids: padded, attention_mask })
    }
}
```

---

## DLL Bundling

### OpenVINO DLLs (Windows)

Required files (~200MB total):
```
runtime/openvino/
├── openvino.dll
├── openvino_c.dll
├── openvino_onnx_frontend.dll
├── openvino_intel_npu_plugin.dll
├── openvino_intel_cpu_plugin.dll
├── tbb12.dll
└── plugins.xml
```

### CUDA DLLs (Windows)

Required files (~400MB total):
```
runtime/cuda/
├── cudart64_12.dll
├── cublas64_12.dll
├── cublasLt64_12.dll
├── cudnn64_8.dll
└── cudnn_ops_infer64_8.dll
```

### Loading Strategy

```rust
// In Engine::new()
fn load_runtime_libraries(config: &EngineConfig) -> Result<(), EngineError> {
    let runtime_dir = config.app_dir.join("runtime");

    // Try OpenVINO
    if runtime_dir.join("openvino/openvino.dll").exists() {
        std::env::set_var("OV_FRONTEND_PATH", runtime_dir.join("openvino"));
        // Load via ort
    }

    // Try CUDA
    if runtime_dir.join("cuda/cudart64_12.dll").exists() {
        std::env::set_var("PATH", format!("{};{}",
            runtime_dir.join("cuda").display(),
            std::env::var("PATH").unwrap_or_default()
        ));
    }

    Ok(())
}
```

---

## Research Tasks

### 1. IoBinding for KV Cache

**Questions:**
- How to use `ort::IoBinding` to bind pre-allocated GPU tensors?
- Can we bind only a slice of a tensor (for growing cache)?
- What's the memory allocation pattern for dynamic sequence lengths?

**Actions:**
- Read `ort` crate IoBinding documentation
- Test with simple model on CUDA
- Benchmark zero-copy vs current copy approach

**Reference from Phase 1:**
The current CPU implementation uses bulk copies via `extend_from_slice()`. True zero-copy requires IoBinding with pre-allocated device tensors.

### 2. OpenVINO EP in ORT

**Questions:**
- How to configure OpenVINO EP in `ort` crate?
- What session options are needed?
- How to specify NPU vs GPU device?

**Actions:**
- Read `ort` documentation for OpenVINO
- Test with simple model
- Document configuration

### 3. OpenVINO DLL Requirements

**Questions:**
- Exact DLLs needed for NPU inference?
- Licensing terms for redistribution?
- Minimum version requirements?

**Actions:**
- Download OpenVINO runtime
- Identify minimal DLL set
- Verify licensing allows bundling

### 4. CUDA EP in ORT

**Questions:**
- How to configure CUDA EP?
- cuDNN version requirements?
- Memory management considerations?

**Actions:**
- Test CUDA EP on NVIDIA system
- Document DLL requirements
- Test memory usage

### 5. Input Bucketing Impact

**Questions:**
- Performance impact of padding?
- Optimal bucket sizes?
- Memory overhead of larger buckets?

**Actions:**
- Benchmark with different bucket sizes
- Measure latency difference
- Find optimal configuration

---

## UI Changes

### Status Bar Update

```svelte
<!-- StatusBar.svelte -->
<div class="status-bar">
  {#if engineStatus.activeProvider === 'openvino_npu'}
    <span class="accelerator">Intel AI Boost</span>
  {:else if engineStatus.activeProvider === 'cuda'}
    <span class="accelerator">NVIDIA GPU</span>
  {:else}
    <span class="accelerator">CPU</span>
  {/if}
  <span class="memory">{memoryUsed}GB / {memoryTotal}GB</span>
  <span class="speed">{tokensPerSecond} tok/s</span>
</div>
```

### Fallback Notification

```svelte
<!-- Show when EP fallback occurs -->
{#if showFallbackNotice}
  <Toast type="info">
    Could not enable Intel AI Boost. Running on CPU (may be slower).
  </Toast>
{/if}
```

---

## Success Criteria

| Criteria | Target |
|----------|--------|
| OpenVINO EP works on Core Ultra | Yes |
| NPU provides speedup over CPU | 5-10x |
| CUDA EP works on NVIDIA GPU | Yes |
| Fallback chain works | Yes |
| Status bar shows correct EP | Yes |
| 7B model works on 16GB system | Yes |

---

## Performance Targets

| Configuration | TTFT | Tokens/sec |
|---------------|------|------------|
| CPU (i3, 8GB) | < 3s (warm) | > 2 tok/s |
| Intel NPU | < 1s (warm) | > 15 tok/s |
| NVIDIA GTX 1060 | < 1s (warm) | > 10 tok/s |
| NVIDIA RTX 3060 | < 0.5s (warm) | > 30 tok/s |

---

## Known Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| OpenVINO EP not in `ort` | Low | Use raw ORT C API if needed |
| DLL bundling issues | Medium | Test on clean Windows install |
| Input bucketing hurts quality | Low | Test output quality |
| NPU not detected properly | Medium | Multiple detection methods |

---

## Testing Strategy

### Hardware Test Matrix

| System | Test |
|--------|------|
| Intel Core Ultra (NPU) | OpenVINO NPU |
| Intel 12th Gen (no NPU) | CPU fallback |
| NVIDIA GTX 1060 | CUDA |
| NVIDIA RTX 3060 | CUDA |
| AMD CPU + GPU | CPU fallback |
| 8GB RAM | Memory limits |
| 16GB RAM | 7B model |

### Integration Tests

- EP selection logic
- Fallback scenarios
- DLL loading
- Session configuration

---

*When Phase 2 is complete, proceed to PHASE-3.md for Qualcomm NPU and cross-platform support.*
