# Phase 2: llama.cpp Integration & Intelligent Optimization

**Project:** SmolPC Code Helper v2.3.0
**Status:** Planning
**Start Date:** TBD
**Target Completion:** Q1 2025
**Dependencies:** v2.2.0 (Hardware Detection System)

---

## Executive Summary

Phase 2 replaces Ollama with in-process llama.cpp integration, enabling hardware-optimized compilation, better performance, and intelligent model management based on detected hardware capabilities.

**Key Goals:**
1. Build llama.cpp with hardware-specific optimizations (CUDA, Metal, AVX2/512, NEON)
2. Implement intelligent model selection based on available RAM/VRAM
3. Enable GPU layer offloading for hybrid CPU+GPU inference
4. Create download manager with storage validation
5. Achieve 20-50% performance improvement over Ollama

**Current System Capabilities (from v2.2.0):**
- CPU: Intel Core Ultra 9 285H (16 cores, AVX2 supported)
- GPU: NVIDIA GeForce RTX 4050 Laptop (6GB VRAM, CUDA 8.9)
- NPU: Intel AI Boost (detected but not utilized)
- RAM: Available for model loading
- Performance: ~43 tokens/sec with Ollama (baseline)

---

## Table of Contents

1. [Technical Requirements](#technical-requirements)
2. [Architecture Overview](#architecture-overview)
3. [Implementation Plan](#implementation-plan)
4. [Module Structure](#module-structure)
5. [Testing Strategy](#testing-strategy)
6. [Performance Targets](#performance-targets)
7. [Risk Mitigation](#risk-mitigation)
8. [Success Criteria](#success-criteria)

---

## Technical Requirements

### Dependencies

**Rust Crates:**
```toml
# Core llama.cpp binding
llama-cpp-rs = "0.4"  # Or llama-cpp-2 (check latest)

# Model file handling
safetensors = "0.4"
hf-hub = "0.3"  # For HuggingFace model downloads

# Compression/decompression
flate2 = "1.0"
tar = "0.4"

# Progress tracking
indicatif = "0.17"

# Build system (for compiling llama.cpp)
cc = "1.0"
cmake = "0.1"
```

**System Requirements:**
- **CMake** 3.14+ (for building llama.cpp)
- **C++ Compiler:**
  - Windows: MSVC 2019+ or MinGW-w64
  - macOS: Xcode Command Line Tools
  - Linux: GCC 9+ or Clang 10+
- **CUDA Toolkit** 11.8+ (Windows, Linux - for NVIDIA GPU support)
- **Metal** (macOS - built-in)
- **Git** (for downloading llama.cpp)

### Build Configurations

**Windows (NVIDIA GPU):**
```cmake
-DLLAMA_CUDA=ON
-DCMAKE_CUDA_ARCHITECTURES=89  # RTX 4050 (Ada Lovelace)
-DLLAMA_CUBLAS=ON
-DLLAMA_AVX2=ON
```

**macOS (Apple Silicon):**
```cmake
-DLLAMA_METAL=ON
-DLLAMA_ACCELERATE=ON
-DLLAMA_NEON=ON
```

**Linux (NVIDIA GPU):**
```cmake
-DLLAMA_CUDA=ON
-DCMAKE_CUDA_ARCHITECTURES=89
-DLLAMA_CUBLAS=ON
-DLLAMA_AVX2=ON
```

---

## Architecture Overview

### Current Architecture (v2.2.0)

```
Frontend (Svelte) → Tauri IPC → Ollama HTTP API (localhost:11434)
                                      ↓
                                  AI Model (loaded by Ollama)
```

**Issues:**
- HTTP overhead (~40% CPU for client)
- No control over compilation flags
- Can't optimize for specific hardware
- Limited layer offloading control

### New Architecture (v2.3.0)

```
Frontend (Svelte) → Tauri IPC → llama.cpp (in-process)
                                      ↓
                      ┌───────────────┴───────────────┐
                      │                               │
                 GPU Layers                      CPU Layers
            (RTX 4050 - CUDA)              (Intel Ultra 9)
                      │                               │
                      └───────────────┬───────────────┘
                                      ↓
                                 AI Model (GGUF)
```

**Benefits:**
- No HTTP overhead (direct memory access)
- Hardware-optimized binary
- Configurable layer offloading
- Better resource utilization
- 20-50% performance gain expected

---

## Implementation Plan

### Milestone 1: llama.cpp Build System (Week 1-2)

#### 1.1 Create Build Module

**File:** `src-tauri/src/llama/builder.rs`

**Responsibilities:**
- Download llama.cpp from GitHub
- Detect system capabilities (CMake, compilers, CUDA)
- Generate CMake configuration based on hardware
- Build llama.cpp with optimizations
- Validate build artifacts

**Key Functions:**
```rust
pub async fn check_build_dependencies() -> Result<BuildCapabilities, String>
pub async fn download_llama_cpp(version: &str) -> Result<PathBuf, String>
pub fn generate_cmake_flags(hardware: &HardwareInfo) -> Vec<String>
pub async fn build_llama_cpp(flags: Vec<String>) -> Result<PathBuf, String>
pub async fn verify_build(binary_path: &PathBuf) -> Result<bool, String>
```

**CMake Flag Generation Logic:**
```rust
fn generate_cmake_flags(hardware: &HardwareInfo) -> Vec<String> {
    let mut flags = vec!["-DBUILD_SHARED_LIBS=OFF".to_string()];

    // CPU optimizations
    if hardware.cpu.features.contains(&"AVX2".to_string()) {
        flags.push("-DLLAMA_AVX2=ON".to_string());
    }
    if hardware.cpu.features.contains(&"AVX512".to_string()) {
        flags.push("-DLLAMA_AVX512=ON".to_string());
    }
    if hardware.cpu.features.contains(&"NEON".to_string()) {
        flags.push("-DLLAMA_NEON=ON".to_string());
    }

    // GPU optimizations
    if let Some(gpu) = hardware.gpus.first() {
        match gpu.backend.as_str() {
            "CUDA" => {
                flags.push("-DLLAMA_CUDA=ON".to_string());
                flags.push("-DLLAMA_CUBLAS=ON".to_string());

                if let Some(compute_cap) = &gpu.cuda_compute_capability {
                    let arch = compute_cap.replace(".", "");
                    flags.push(format!("-DCMAKE_CUDA_ARCHITECTURES={}", arch));
                }
            }
            "Metal" => {
                flags.push("-DLLAMA_METAL=ON".to_string());
            }
            "Vulkan" => {
                flags.push("-DLLAMA_VULKAN=ON".to_string());
            }
            _ => {}
        }
    }

    flags
}
```

**Build Process:**
```rust
pub async fn build_llama_cpp(flags: Vec<String>) -> Result<PathBuf, String> {
    // 1. Create build directory
    let build_dir = PathBuf::from("llama.cpp/build");
    std::fs::create_dir_all(&build_dir)?;

    // 2. Run CMake configure
    let cmake_output = Command::new("cmake")
        .args(&["..", "-G", "Ninja"]) // or "Unix Makefiles" / "Visual Studio"
        .args(&flags)
        .current_dir(&build_dir)
        .output()
        .await?;

    // 3. Run CMake build
    let build_output = Command::new("cmake")
        .args(&["--build", ".", "--config", "Release", "-j", "8"])
        .current_dir(&build_dir)
        .output()
        .await?;

    // 4. Return path to built library
    Ok(build_dir.join("libllama.so")) // or .dll / .dylib
}
```

#### 1.2 Create Build Status UI

**File:** `src/lib/components/BuildPanel.svelte`

**Features:**
- Show build progress (downloading, configuring, compiling)
- Display CMake flags being used
- Show compiler output (collapsible)
- Error handling and retry logic
- Build cache management

**Store:** `src/lib/stores/llama-build.svelte.ts`

```typescript
export const llamaBuildStore = {
    status: $state<'idle' | 'downloading' | 'configuring' | 'building' | 'complete' | 'error'>('idle'),
    progress: $state(0),
    currentStep: $state(''),
    error: $state<string | null>(null),
    buildPath: $state<string | null>(null),

    async startBuild() {
        this.status = 'downloading';
        // ... trigger Tauri command
    }
};
```

---

### Milestone 2: Model Management System (Week 3-4)

#### 2.1 Model Registry

**File:** `src-tauri/src/models/registry.rs`

**Purpose:** Track available models, their requirements, and compatibility.

**Data Structure:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub size: String, // "7b", "13b", "34b"
    pub quantization: String, // "Q4_K_M", "Q5_K_M", "Q8_0"
    pub file_size_gb: f64,
    pub min_ram_gb: f64,
    pub min_vram_gb: f64,
    pub recommended_ram_gb: f64,
    pub context_length: u32,
    pub download_url: String,
    pub sha256: String,
}

pub struct ModelRegistry {
    models: Vec<ModelInfo>,
}

impl ModelRegistry {
    pub fn get_compatible_models(&self, hardware: &HardwareInfo) -> Vec<ModelInfo> {
        self.models.iter()
            .filter(|model| {
                hardware.memory.available_gb >= model.min_ram_gb &&
                hardware.storage.available_gb >= model.file_size_gb
            })
            .cloned()
            .collect()
    }

    pub fn recommend_model(&self, hardware: &HardwareInfo) -> Option<ModelInfo> {
        let mut compatible = self.get_compatible_models(hardware);
        compatible.sort_by(|a, b| b.min_ram_gb.partial_cmp(&a.min_ram_gb).unwrap());
        compatible.first().cloned()
    }
}
```

**Default Models:**
```rust
pub fn default_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            name: "qwen2.5-coder:3b-q4".to_string(),
            size: "3b".to_string(),
            quantization: "Q4_K_M".to_string(),
            file_size_gb: 1.9,
            min_ram_gb: 4.0,
            min_vram_gb: 0.0,
            recommended_ram_gb: 6.0,
            context_length: 32768,
            download_url: "https://huggingface.co/...".to_string(),
            sha256: "...".to_string(),
        },
        ModelInfo {
            name: "qwen2.5-coder:7b-q4".to_string(),
            size: "7b".to_string(),
            quantization: "Q4_K_M".to_string(),
            file_size_gb: 4.4,
            min_ram_gb: 6.0,
            min_vram_gb: 0.0,
            recommended_ram_gb: 10.0,
            context_length: 32768,
            download_url: "https://huggingface.co/...".to_string(),
            sha256: "...".to_string(),
        },
        ModelInfo {
            name: "qwen2.5-coder:7b-q8".to_string(),
            size: "7b".to_string(),
            quantization: "Q8_0".to_string(),
            file_size_gb: 7.7,
            min_ram_gb: 10.0,
            min_vram_gb: 0.0,
            recommended_ram_gb: 12.0,
            context_length: 32768,
            download_url: "https://huggingface.co/...".to_string(),
            sha256: "...".to_string(),
        },
        ModelInfo {
            name: "qwen2.5-coder:14b-q4".to_string(),
            size: "14b".to_string(),
            quantization: "Q4_K_M".to_string(),
            file_size_gb: 8.6,
            min_ram_gb: 12.0,
            min_vram_gb: 0.0,
            recommended_ram_gb: 16.0,
            context_length: 32768,
            download_url: "https://huggingface.co/...".to_string(),
            sha256: "...".to_string(),
        },
    ]
}
```

#### 2.2 Download Manager

**File:** `src-tauri/src/models/downloader.rs`

**Features:**
- Resume interrupted downloads
- Progress tracking
- SHA256 verification
- Parallel chunk downloads (optional)
- Bandwidth throttling (optional)

**Key Functions:**
```rust
pub struct DownloadManager {
    client: reqwest::Client,
    download_dir: PathBuf,
}

impl DownloadManager {
    pub async fn download_model(
        &self,
        model: &ModelInfo,
        progress_callback: impl Fn(u64, u64)
    ) -> Result<PathBuf, String> {
        // 1. Check available storage
        let available_gb = check_available_storage(&self.download_dir)?;
        if available_gb < model.file_size_gb {
            return Err(format!(
                "Insufficient storage: {} GB required, {} GB available",
                model.file_size_gb, available_gb
            ));
        }

        // 2. Download with progress tracking
        let response = self.client.get(&model.download_url).send().await?;
        let total_size = response.content_length().unwrap_or(0);

        let file_path = self.download_dir.join(format!("{}.gguf", model.name));
        let mut file = File::create(&file_path).await?;
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            progress_callback(downloaded, total_size);
        }

        // 3. Verify SHA256
        let computed_hash = compute_sha256(&file_path).await?;
        if computed_hash != model.sha256 {
            return Err("SHA256 verification failed".to_string());
        }

        Ok(file_path)
    }

    pub async fn resume_download(
        &self,
        model: &ModelInfo,
        partial_file: &PathBuf
    ) -> Result<PathBuf, String> {
        let downloaded = std::fs::metadata(partial_file)?.len();
        // Use HTTP Range header to resume from downloaded bytes
        // Implementation details...
    }
}
```

**Frontend Integration:**

**File:** `src/lib/components/ModelDownloader.svelte`

```svelte
<script lang="ts">
    import { modelStore } from '$lib/stores/models.svelte';

    let downloading = $state(false);
    let progress = $state(0);

    async function downloadModel(modelName: string) {
        downloading = true;
        try {
            await modelStore.download(modelName, (downloaded, total) => {
                progress = (downloaded / total) * 100;
            });
        } finally {
            downloading = false;
        }
    }
</script>

<div class="model-downloader">
    {#if downloading}
        <progress value={progress} max="100"></progress>
        <p>{progress.toFixed(1)}% downloaded</p>
    {/if}
</div>
```

---

### Milestone 3: llama.cpp Runtime Integration (Week 5-6)

#### 3.1 llama.cpp Wrapper

**File:** `src-tauri/src/llama/runtime.rs`

**Purpose:** Safe Rust wrapper around llama.cpp C++ API.

**Structure:**
```rust
use llama_cpp_rs::*;

pub struct LlamaRuntime {
    context: LlamaContext,
    model: LlamaModel,
    config: RuntimeConfig,
}

pub struct RuntimeConfig {
    pub n_threads: u32,
    pub n_gpu_layers: u32,
    pub context_size: u32,
    pub batch_size: u32,
    pub seed: u32,
}

impl LlamaRuntime {
    pub fn new(model_path: &Path, config: RuntimeConfig) -> Result<Self, String> {
        let model_params = LlamaModelParams {
            n_gpu_layers: config.n_gpu_layers,
            use_mmap: true,
            use_mlock: false,
            ..Default::default()
        };

        let model = LlamaModel::load_from_file(model_path, model_params)?;

        let context_params = LlamaContextParams {
            n_ctx: config.context_size,
            n_batch: config.batch_size,
            n_threads: config.n_threads,
            seed: config.seed,
            ..Default::default()
        };

        let context = model.new_context(context_params)?;

        Ok(Self { context, model, config })
    }

    pub async fn generate(
        &mut self,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
        callback: impl Fn(&str)
    ) -> Result<String, String> {
        let tokens = self.model.tokenize(prompt, true)?;

        let mut output = String::new();
        let mut generated_tokens = 0;

        // Evaluate prompt
        self.context.eval(&tokens, 0)?;

        // Generate tokens
        while generated_tokens < max_tokens {
            let token = self.context.sample(temperature);

            if token == self.model.token_eos() {
                break;
            }

            let text = self.model.token_to_str(token)?;
            output.push_str(&text);
            callback(&text);

            self.context.eval(&[token], tokens.len() + generated_tokens)?;
            generated_tokens += 1;
        }

        Ok(output)
    }
}
```

#### 3.2 Configuration Optimizer

**File:** `src-tauri/src/llama/optimizer.rs`

**Purpose:** Automatically configure optimal settings based on hardware.

```rust
pub struct ConfigOptimizer;

impl ConfigOptimizer {
    pub fn optimize(hardware: &HardwareInfo, model: &ModelInfo) -> RuntimeConfig {
        // Calculate optimal thread count
        let n_threads = Self::calculate_threads(&hardware.cpu);

        // Calculate GPU layer offloading
        let n_gpu_layers = Self::calculate_gpu_layers(hardware, model);

        // Calculate context size based on available RAM
        let context_size = Self::calculate_context_size(hardware, model);

        RuntimeConfig {
            n_threads,
            n_gpu_layers,
            context_size,
            batch_size: 512,
            seed: 42,
        }
    }

    fn calculate_threads(cpu: &CpuInfo) -> u32 {
        // Use physical cores for CPU inference
        // Leave 2 cores for system
        (cpu.physical_cores.saturating_sub(2)).max(1)
    }

    fn calculate_gpu_layers(hardware: &HardwareInfo, model: &ModelInfo) -> u32 {
        if let Some(gpu) = hardware.gpus.first() {
            // Estimate layers per GB of VRAM
            let vram_gb = gpu.vram_mb as f64 / 1024.0;

            // Conservative estimate: 1GB VRAM per ~10 layers for 7B model
            let layers_per_gb = match model.size.as_str() {
                "3b" => 15,
                "7b" => 10,
                "13b" => 5,
                "34b" => 2,
                _ => 5,
            };

            let max_layers = (vram_gb * layers_per_gb as f64) as u32;

            // Reserve 1GB for OS/other apps
            max_layers.saturating_sub(layers_per_gb)
        } else {
            0 // CPU-only
        }
    }

    fn calculate_context_size(hardware: &HardwareInfo, model: &ModelInfo) -> u32 {
        let available_gb = hardware.memory.available_gb;

        // Rough estimate: each token needs ~2 bytes in context
        let max_tokens = ((available_gb - model.min_ram_gb) * 1024.0 * 1024.0 * 1024.0 / 2.0) as u32;

        // Cap at model's maximum context length
        max_tokens.min(model.context_length)
    }
}
```

#### 3.3 Tauri Commands

**File:** `src-tauri/src/commands/llama.rs`

```rust
#[tauri::command]
pub async fn load_model(
    model_name: String,
    state: State<'_, LlamaState>
) -> Result<(), String> {
    let hardware = get_hardware_info()?;
    let model_info = get_model_info(&model_name)?;
    let config = ConfigOptimizer::optimize(&hardware, &model_info);

    let model_path = get_model_path(&model_name)?;
    let runtime = LlamaRuntime::new(&model_path, config)?;

    let mut llama_state = state.runtime.lock().await;
    *llama_state = Some(runtime);

    Ok(())
}

#[tauri::command]
pub async fn generate_text(
    prompt: String,
    max_tokens: u32,
    temperature: f32,
    window: tauri::Window,
    state: State<'_, LlamaState>
) -> Result<(), String> {
    let mut llama_state = state.runtime.lock().await;
    let runtime = llama_state.as_mut()
        .ok_or("Model not loaded")?;

    runtime.generate(&prompt, max_tokens, temperature, |token| {
        window.emit("llama_token", token).ok();
    }).await?;

    window.emit("llama_done", ()).ok();
    Ok(())
}

#[tauri::command]
pub async fn unload_model(state: State<'_, LlamaState>) -> Result<(), String> {
    let mut llama_state = state.runtime.lock().await;
    *llama_state = None;
    Ok(())
}
```

---

### Milestone 4: Frontend Integration (Week 7)

#### 4.1 Model Selection UI

**File:** `src/lib/components/ModelSelector.svelte` (Update existing)

**New Features:**
- Show recommended model based on hardware
- Display model requirements vs. available resources
- Visual indicators (✓ compatible, ⚠️ tight, ✗ incompatible)
- Download button for models not yet installed

#### 4.2 Settings Panel

**File:** `src/lib/components/SettingsPanel.svelte` (New)

**Sections:**
- **Model Settings:** Currently loaded model, unload/reload
- **Performance:** Thread count, GPU layers (with explanations)
- **Advanced:** Context size, batch size, temperature, seed
- **Storage:** Show downloaded models, delete unused models

#### 4.3 Migration from Ollama

**Strategy:**
- Keep Ollama integration as fallback (Phase 2.5)
- Add feature flag: `use_llama_cpp` in settings
- Gradual migration: users can toggle between Ollama and llama.cpp
- UI indication of which backend is active

**File:** `src/lib/stores/settings.svelte.ts` (Update)

```typescript
export const settingsStore = {
    useLlamaCpp: $state(false), // New setting

    async toggleBackend() {
        this.useLlamaCpp = !this.useLlamaCpp;
        // Reload model with new backend
        await this.reloadModel();
    }
};
```

---

## Module Structure

```
src-tauri/src/
├── llama/                      # New llama.cpp integration
│   ├── mod.rs                  # Module exports
│   ├── builder.rs              # Build system
│   ├── runtime.rs              # llama.cpp wrapper
│   ├── optimizer.rs            # Configuration optimizer
│   └── bindings.rs             # FFI bindings (if needed)
├── models/                     # New model management
│   ├── mod.rs                  # Module exports
│   ├── registry.rs             # Model catalog
│   ├── downloader.rs           # Download manager
│   └── storage.rs              # Model file management
├── commands/
│   ├── llama.rs                # New llama.cpp commands
│   ├── models.rs               # New model commands
│   ├── ollama.rs               # Keep for backward compat
│   └── hardware.rs             # Existing hardware detection
└── ...

src/lib/
├── stores/
│   ├── llama-build.svelte.ts   # New build status store
│   ├── models.svelte.ts        # New model management store
│   └── ...
├── components/
│   ├── BuildPanel.svelte       # New build UI
│   ├── ModelDownloader.svelte  # New download UI
│   ├── SettingsPanel.svelte    # New settings UI
│   └── ...
└── types/
    ├── llama.ts                # New llama.cpp types
    ├── models.ts               # New model types
    └── ...
```

---

## Testing Strategy

### Unit Tests

**Rust Tests:**
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_cmake_flag_generation() {
        let hardware = create_test_hardware();
        let flags = generate_cmake_flags(&hardware);
        assert!(flags.contains(&"-DLLAMA_AVX2=ON".to_string()));
    }

    #[test]
    fn test_model_compatibility() {
        let hardware = create_test_hardware();
        let registry = ModelRegistry::default();
        let compatible = registry.get_compatible_models(&hardware);
        assert!(!compatible.is_empty());
    }

    #[test]
    fn test_layer_calculation() {
        let hardware = create_test_hardware_rtx4050();
        let model = create_test_model_7b();
        let layers = ConfigOptimizer::calculate_gpu_layers(&hardware, &model);
        assert!(layers > 0 && layers <= 60);
    }
}
```

### Integration Tests

1. **Build System:**
   - Test CMake detection
   - Test compiler detection
   - Test successful build on each platform

2. **Model Download:**
   - Test download progress tracking
   - Test resume functionality
   - Test SHA256 verification
   - Test insufficient storage handling

3. **Runtime:**
   - Test model loading
   - Test text generation
   - Test GPU layer offloading
   - Test multi-threading

### Performance Benchmarks

**Comparison Matrix:**

| Metric | Ollama (Baseline) | llama.cpp (Target) | Improvement |
|--------|-------------------|-------------------|-------------|
| Tokens/sec | 43 | 52-65 | +20-50% |
| First token (ms) | 39 | 25-35 | ~35% faster |
| CPU usage | 1% | 5-10% | Higher (expected) |
| Memory usage | 76 MB | 100-200 MB | Higher (in-process) |
| VRAM usage | ~4.5 GB | 4-5 GB | Similar |

**Benchmark Script:**
```bash
# Run with Ollama (baseline)
npm run benchmark -- --backend ollama --iterations 5

# Run with llama.cpp
npm run benchmark -- --backend llama-cpp --iterations 5

# Compare results
npm run benchmark-compare
```

---

## Performance Targets

### Your System (RTX 4050 + Ultra 9 285H)

**Current Performance (Ollama):**
- Tokens/sec: 43
- First token: 39ms
- CPU: 1%
- VRAM: ~4.5GB

**Target Performance (llama.cpp):**
- Tokens/sec: **52-60** (20-40% improvement)
- First token: **25-30ms** (35% improvement)
- CPU: **5-8%** (acceptable increase)
- VRAM: **4-5GB** (similar)

**Optimization Breakdown:**

1. **CUDA Optimization:** +15-20% (Ada architecture support)
2. **Remove HTTP overhead:** +10-15% (in-process)
3. **Optimized layer split:** +5-10% (better CPU/GPU balance)
4. **AVX2 instructions:** Already used by Ollama, no gain

**Expected Configuration:**
```
n_threads: 14 (16 physical cores - 2 for system)
n_gpu_layers: 50-55 (out of 60 total for 7B model)
context_size: 16384-32768 (based on RAM)
batch_size: 512
```

---

## Risk Mitigation

### Risk 1: Build Complexity

**Risk:** llama.cpp build fails on some systems.

**Mitigation:**
- Pre-build binaries for common configurations (Windows + NVIDIA, macOS + Metal)
- Fallback to Ollama if build fails
- Comprehensive error messages with troubleshooting steps
- Option to skip build and use pre-built binaries

### Risk 2: Performance Regression

**Risk:** llama.cpp performs worse than Ollama in some configurations.

**Mitigation:**
- Keep Ollama backend as fallback
- Add A/B testing mode for users
- Benchmark extensively before v2.3.0 release
- Allow per-model backend selection

### Risk 3: Model Compatibility

**Risk:** Downloaded models not compatible with llama.cpp version.

**Mitigation:**
- Pin llama.cpp version
- Maintain model registry with tested models
- SHA256 verification
- Clear error messages for incompatible models

### Risk 4: Storage Management

**Risk:** Users run out of storage with multiple models.

**Mitigation:**
- Pre-download checks
- Show storage usage in UI
- Easy model deletion
- Warn when storage < 10% free

### Risk 5: GPU Driver Issues

**Risk:** CUDA/Metal initialization fails.

**Mitigation:**
- Detect CUDA version before building
- Graceful fallback to CPU-only
- Clear error messages with driver update instructions
- Test on multiple GPU generations

---

## Success Criteria

### Must Have (v2.3.0 Release)

- [ ] llama.cpp builds successfully on Windows/macOS/Linux
- [ ] At least 3 models available in registry (3B, 7B, 14B)
- [ ] Model download works with progress tracking
- [ ] Text generation works with GPU acceleration
- [ ] Performance >= Ollama baseline (no regression)
- [ ] Settings UI for configuration
- [ ] Migration guide for users
- [ ] Comprehensive error handling
- [ ] Ollama fallback functional

### Should Have (v2.3.0)

- [ ] Performance 20%+ better than Ollama
- [ ] Automatic optimal configuration
- [ ] Model recommendation system
- [ ] SHA256 verification
- [ ] Resume interrupted downloads
- [ ] Storage management UI
- [ ] Build caching (don't rebuild every time)

### Nice to Have (v2.4.0+)

- [ ] Pre-built binaries for common configs
- [ ] NPU support (DirectML, CoreML)
- [ ] Multi-model support (load multiple models)
- [ ] Model quantization tool
- [ ] Advanced layer split configuration
- [ ] Benchmark comparison UI
- [ ] Model format converter (Ollama → GGUF)

---

## Implementation Timeline

### Week 1-2: Build System
- Set up llama.cpp download and build
- CMake flag generation from hardware
- Build verification
- Build status UI

### Week 3-4: Model Management
- Model registry implementation
- Download manager with progress
- SHA256 verification
- Model storage management

### Week 5-6: Runtime Integration
- llama.cpp wrapper
- Configuration optimizer
- Tauri commands
- GPU layer offloading

### Week 7: Frontend Integration
- Model selection UI updates
- Settings panel
- Migration from Ollama
- Error handling UI

### Week 8: Testing & Optimization
- Unit tests
- Integration tests
- Performance benchmarks
- Bug fixes

### Week 9: Documentation
- User migration guide
- Developer documentation
- API documentation
- Troubleshooting guide

### Week 10: Release Preparation
- Final testing
- Release notes
- Version bump to 2.3.0
- Deploy

---

## Additional Considerations

### Cross-Platform Compatibility

**Windows:**
- Test on Windows 10 and 11
- Test with MSVC and MinGW compilers
- Ensure CUDA paths are correctly detected
- Handle long path names (>260 chars)

**macOS:**
- Test on Intel and Apple Silicon
- Verify Metal framework linking
- Check Xcode Command Line Tools version
- Test on macOS 12+

**Linux:**
- Test on Ubuntu 22.04+, Fedora, Arch
- Verify GCC/Clang versions
- Check CUDA Toolkit installation
- Handle different library paths

### User Experience

**First-Time Setup:**
1. Detect system capabilities
2. Recommend optimal model
3. Offer one-click setup (build + download)
4. Show progress clearly
5. Provide time estimates

**Ongoing Use:**
- Model loads quickly on startup
- Clear indication of which backend is active
- Easy switching between models
- Transparent error messages
- Performance stats in UI

### Migration Path

**For Existing Users:**
1. Update to v2.3.0
2. See "Try llama.cpp" banner in UI
3. Click to see benefits (performance comparison)
4. One-click migration (builds llama.cpp, downloads model)
5. A/B test mode: toggle between Ollama and llama.cpp
6. Choose preferred backend permanently

**Rollback Plan:**
- Keep Ollama integration until v3.0
- Allow switching back if issues occur
- Export/import chat history between backends

---

## References

- llama.cpp: https://github.com/ggerganov/llama.cpp
- llama-cpp-rs: https://github.com/utilityai/llama-cpp-rs
- GGUF format: https://github.com/ggerganov/ggml/blob/master/docs/gguf.md
- HuggingFace Hub: https://huggingface.co/docs/huggingface_hub
- CMake documentation: https://cmake.org/documentation/

---

**Document Version:** 1.0
**Last Updated:** January 2025
**Next Review:** Before Phase 2 implementation begins
