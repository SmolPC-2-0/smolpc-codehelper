# Phase 1: CPU Engine MVP

**Goal:** Working inference on CPU using ONNX Runtime, replacing Ollama completely.

**Prerequisites:** Read `CONTEXT.md` first for overall project understanding.

---

## Objectives

1. Create the `smolpc-engine` Rust crate with CPU-only inference
2. Integrate engine into Tauri app
3. Remove all Ollama code
4. Ship with bundled Qwen 2.5 Coder 1.5B model
5. Maintain existing chat UI functionality

---

## Deliverables Checklist

### Engine Core

- [ ] Create `smolpc-engine` crate structure
- [ ] Implement `Engine` struct with public API
- [ ] Implement `EngineConfig` for configuration
- [ ] Implement `EngineError` types using `thiserror`

### Inference

- [ ] ONNX session management via `ort` crate
- [ ] Model loading from disk
- [ ] Autoregressive generation loop
- [ ] KV cache management
- [ ] Streaming token output via callback

### Tokenization

- [ ] Integrate `tokenizers` crate (HuggingFace)
- [ ] Load Qwen tokenizer from model directory
- [ ] Encode (text → tokens) and decode (tokens → text)
- [ ] Handle special tokens (BOS, EOS, PAD)

### Sampling

- [ ] Temperature scaling
- [ ] Top-k filtering
- [ ] Top-p (nucleus) sampling
- [ ] Stop sequence detection

### Memory Management

- [ ] Model unloading after inactivity timeout
- [ ] Context window limiting (2048 tokens)
- [ ] RAM monitoring and threshold checks
- [ ] Graceful refusal when memory too low

### Tauri Integration

- [ ] New `inference.rs` commands module
- [ ] `generate` command with streaming events
- [ ] `cancel_generation` command
- [ ] `get_engine_status` command
- [ ] `load_model` / `unload_model` commands
- [ ] Remove `ollama.rs` and all Ollama dependencies

### Model & Installer

- [ ] Bundle Qwen 2.5 Coder 1.5B INT4 ONNX model
- [ ] Update installer to include model
- [ ] Offline/USB installation support

---

## Technical Specifications

### Engine Crate Structure

```
smolpc-engine/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API
│   ├── engine.rs           # Engine struct
│   ├── config.rs           # EngineConfig, GenerationParams
│   ├── error.rs            # EngineError
│   ├── inference/
│   │   ├── mod.rs
│   │   ├── session.rs      # ONNX session wrapper
│   │   ├── generation.rs   # Generation loop
│   │   └── kv_cache.rs     # KV cache
│   ├── tokenizer/
│   │   ├── mod.rs
│   │   └── hf_tokenizer.rs
│   ├── sampling/
│   │   ├── mod.rs
│   │   ├── temperature.rs
│   │   ├── top_k.rs
│   │   └── top_p.rs
│   ├── providers/
│   │   ├── mod.rs
│   │   ├── traits.rs       # ExecutionProvider trait
│   │   └── cpu.rs          # CPU provider (Phase 1 only)
│   └── memory/
│       ├── mod.rs
│       ├── watchdog.rs
│       └── context.rs
└── tests/
```

### Public API

```rust
// Core types
pub struct Engine { ... }
pub struct EngineConfig {
    pub model_dir: PathBuf,
    pub max_context: usize,           // Default: 2048
    pub unload_timeout_secs: u64,     // Default: 300 (5 min)
    pub min_free_ram_bytes: u64,      // Default: 500MB
}
pub struct GenerationParams {
    pub temperature: f32,             // Default: 0.7
    pub top_k: usize,                 // Default: 40
    pub top_p: f32,                   // Default: 0.9
    pub max_tokens: usize,            // Default: 1024
    pub stop_sequences: Vec<String>,
}
pub struct Message {
    pub role: Role,
    pub content: String,
}
pub enum Role { System, User, Assistant }
pub enum TokenEvent {
    Token(String),
    Done(GenerationStats),
    Error(EngineError),
}

// Engine methods
impl Engine {
    pub fn new(config: EngineConfig) -> Result<Self, EngineError>;
    pub fn load_model(&mut self, path: &Path) -> Result<(), EngineError>;
    pub fn unload_model(&mut self);
    pub fn is_model_loaded(&self) -> bool;
    pub fn generate<F>(&self, messages: &[Message], params: GenerationParams, on_token: F)
        -> Result<GenerationResult, EngineError>
    where F: FnMut(TokenEvent) -> ControlFlow<(), ()>;
    pub fn cancel(&self);
    pub fn hardware_info(&self) -> &HardwareInfo;
    pub fn memory_stats(&self) -> MemoryStats;
    pub fn active_provider(&self) -> &str;
}
```

### Tauri Commands

```rust
#[tauri::command]
async fn generate(
    window: Window,
    state: State<'_, AppState>,
    messages: Vec<Message>,
    params: GenerationParams,
) -> Result<(), String>;

#[tauri::command]
async fn cancel_generation(state: State<'_, AppState>) -> Result<(), String>;

#[tauri::command]
async fn get_engine_status(state: State<'_, AppState>) -> Result<EngineStatus, String>;

#[tauri::command]
async fn load_model(state: State<'_, AppState>, model_name: String) -> Result<(), String>;
```

### Frontend Changes

Replace Ollama store calls with new engine commands:

```typescript
// Before (Ollama)
await invoke('generate_stream', { prompt, model });
listen('chat_chunk', handler);

// After (Engine)
await invoke('generate', { messages, params });
listen('inference_token', handler);
```

---

## Research Tasks (Do Before Coding)

### 1. Validate `ort` Crate

**Questions:**
- Does `ort` support CPU EP properly?
- What version should we use?
- How do we configure session options?
- What's the memory footprint?

**Actions:**
- Create minimal test project
- Load a small ONNX model
- Run inference and measure performance

### 2. Qwen 2.5 ONNX Availability

**Questions:**
- Are pre-exported ONNX models available on HuggingFace?
- What quantization formats are available?
- Do we need to export ourselves using `optimum`?

**Actions:**
- Search HuggingFace for Qwen 2.5 Coder ONNX
- If not available, document export process
- Verify tokenizer files are included

### 3. Memory Footprint

**Questions:**
- Actual RAM usage when loading 1.5B INT4 model?
- KV cache size at 2048 context?
- Can we run on 8GB system with headroom?

**Actions:**
- Load model in test environment
- Profile memory usage during inference
- Document findings

### 4. Generation Loop

**Questions:**
- How to properly implement autoregressive loop with `ort`?
- How to manage KV cache between iterations?
- Reference implementations to study?

**Actions:**
- Study `ort` examples
- Look at llama.cpp, candle, or other Rust LLM implementations
- Document the approach

---

## Model Directory Structure

```
models/qwen-2.5-coder-1.5b-int4/
├── model.onnx              # Model graph + weights
├── tokenizer.json          # HuggingFace tokenizer
├── tokenizer_config.json   # Tokenizer config
├── special_tokens_map.json # Special token mappings
└── config.json             # Model configuration
```

---

## Success Criteria

| Criteria | Target |
|----------|--------|
| App launches on 8GB system | Yes |
| Model loads without OOM | Yes |
| TTFT (cold start) | < 30 seconds |
| TTFT (warm) | < 3 seconds |
| Generation speed (i3, 8GB) | > 2 tok/s |
| Streaming works | Yes |
| Cancel works | Yes |
| No Ollama code remaining | Yes |

---

## Known Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `ort` doesn't work as expected | Medium | Validate early with test project |
| Model too slow on low-end CPU | High | Accept slower speeds; focus on NPU in Phase 2 |
| Memory too tight on 8GB | High | Aggressive unloading; test extensively |
| KV cache implementation complex | Medium | Study reference implementations |

---

## Dependencies

```toml
# smolpc-engine/Cargo.toml
[dependencies]
ort = { version = "2.0", features = ["load-dynamic"] }
tokenizers = "0.15"
thiserror = "1.0"
log = "0.4"
sysinfo = "0.30"  # For memory monitoring
```

---

## Testing Strategy

### Unit Tests
- Tokenizer encode/decode
- Sampling functions
- Config validation

### Integration Tests
- Full generation loop with small model
- Memory management behavior
- Cancellation

### Manual Testing
- Run on 8GB system
- Run on 16GB system
- Test all chat UI functionality

---

## Notes for Implementation

1. **Start with hardcoded CPU provider.** Don't implement EP abstraction yet—just get CPU working.

2. **Use existing frontend.** Minimal UI changes; just swap out the store calls.

3. **Test early and often on 8GB system.** Memory issues should be caught immediately.

4. **Log extensively.** Add debug logging for timing, memory, and generation stats.

5. **Don't optimize prematurely.** Get it working first, then profile and optimize.

---

*When Phase 1 is complete, proceed to PHASE-2.md for GPU/OpenVINO acceleration.*
