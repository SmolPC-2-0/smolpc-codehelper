# Hardware Detection System

**Version:** 2.2.0
**Status:** Production Ready
**Platform Support:** Windows, macOS, Linux (x86_64, ARM64)

---

## Overview

The Hardware Detection System is a comprehensive, offline hardware profiling feature that automatically detects and analyzes your system's CPU, GPU, Memory, Storage, and NPU capabilities. This information enables intelligent optimization decisions for running local AI models efficiently.

**Key Benefits:**

- 🔍 **Automatic Detection** - No manual configuration required
- 🌐 **100% Offline** - Uses native OS APIs, no internet needed
- ⚡ **Instant Insights** - Hardware info available on app startup
- 🎯 **Optimization Ready** - Provides data for llama.cpp compiler flags and model selection
- 🔄 **Cross-Platform** - Consistent API across Windows, macOS, and Linux
- 💾 **Cached Results** - Detection runs once, results cached for performance

---

## What Hardware is Detected?

### 1. CPU (Central Processing Unit)

**Information Collected:**

| Field           | Description                                    | Example                          |
| --------------- | ---------------------------------------------- | -------------------------------- |
| Vendor          | CPU manufacturer                               | "Apple", "Intel", "AMD"          |
| Brand           | Full CPU model name                            | "Apple M3 Pro"                   |
| Architecture    | Instruction set architecture                   | "ARM", "x86_64"                  |
| Physical Cores  | Number of physical CPU cores                   | 12                               |
| Logical Cores   | Number of logical cores (with hyperthreading) | 12                               |
| Base Frequency  | Base clock speed in MHz                        | 2400.0                           |
| Max Frequency   | Boost/turbo clock speed in MHz                 | 4050.0                           |
| Cache Sizes     | L1, L2, L3 cache in KB                         | { l1: 192, l2: 16384, l3: 0 }    |
| Features        | Instruction set extensions                     | ["NEON", "SVE"] or ["AVX2"]      |

**CPU Features Explained:**

- **AVX2** (x86): Advanced Vector Extensions 2 - SIMD instructions for fast math
- **AVX512** (x86): 512-bit wide vector operations for extreme performance
- **FMA** (x86): Fused Multiply-Add for efficient neural network operations
- **NEON** (ARM): ARM's SIMD technology for parallel processing
- **SVE** (ARM): Scalable Vector Extension for advanced ARM optimization

**Why It Matters:**
- Determines optimal llama.cpp compilation flags (`-DLLAMA_AVX2=ON`, `-DLLAMA_NEON=ON`)
- Influences CPU thread count for inference
- Helps select appropriate quantization levels (Q4, Q5, Q8)

---

### 2. GPU (Graphics Processing Unit)

**Information Collected:**

| Field                  | Description                      | Example                  |
| ---------------------- | -------------------------------- | ------------------------ |
| Name                   | GPU model name                   | "Apple M3 Pro"           |
| Vendor                 | GPU manufacturer                 | "Apple"                  |
| Backend                | Graphics API in use              | "Metal", "DirectX 12"    |
| Device Type            | GPU category                     | "Integrated", "Discrete" |
| VRAM (MB)              | Video memory in megabytes        | 18432                    |
| CUDA Compute Capability| CUDA version (NVIDIA only)       | "8.6", "7.5"             |

**Graphics Backends:**

- **Metal** (macOS): Apple's native GPU framework - optimal for M1/M2/M3 chips
- **DirectX 12** (Windows): Microsoft's graphics API
- **Vulkan** (Cross-platform): Low-overhead, high-performance API
- **CUDA** (NVIDIA): NVIDIA's parallel computing platform
- **OpenCL** (Cross-platform): Open standard for heterogeneous computing

**CUDA Compute Capability:**

NVIDIA GPU architecture versions that determine available features:

- **8.6**: RTX 30 series (Ampere) - Best performance
- **7.5**: RTX 20 series, GTX 16 series (Turing)
- **6.1**: GTX 10 series (Pascal)

Used for: Selecting compatible CUDA kernels, enabling Tensor Cores, optimizing layer offloading.

**Why It Matters:**
- Determines GPU layer offloading capability (Metal, CUDA, Vulkan)
- VRAM size dictates maximum model size (e.g., 7B models need ~4-6GB VRAM)
- Backend selection for llama.cpp compilation
- CUDA compute capability for kernel optimization

---

### 3. Memory (RAM)

**Information Collected:**

| Field         | Description                       | Example |
| ------------- | --------------------------------- | ------- |
| Total (GB)    | Total system RAM in gigabytes     | 36.0    |
| Available (GB)| Currently available RAM           | 21.4    |

**Why It Matters:**
- **Critical for model selection** - Determines which model sizes can run:
  - **7B model (Q4)**: ~4-6GB RAM required
  - **13B model (Q4)**: ~8-10GB RAM required
  - **34B model (Q4)**: ~20-24GB RAM required
- Enables automatic model recommendations based on available memory
- Prevents out-of-memory crashes during inference
- Helps decide between full-model loading vs streaming

---

### 4. Storage

**Information Collected:**

| Field           | Description                        | Example           |
| --------------- | ---------------------------------- | ----------------- |
| Total (GB)      | Total storage capacity             | 494.4             |
| Available (GB)  | Free storage space                 | 89.3              |
| Type            | Storage medium type                | SSD (true/false)  |
| Device Name     | Storage device model               | "APPLE SSD"       |

**Why It Matters:**
- **Model downloads** - AI models are large (4-40GB each):
  - Qwen 2.5 Coder 7B: ~4.7GB
  - DeepSeek 33B: ~19GB
  - Llama 3 70B: ~40GB
- **SSD detection** - SSDs load models 3-5× faster than HDDs
- Prevents failed downloads due to insufficient space
- Enables automatic cleanup suggestions when storage is low

---

### 5. NPU (Neural Processing Unit)

**Information Collected:**

| Field       | Description                        | Example                   |
| ----------- | ---------------------------------- | ------------------------- |
| Detected    | Whether NPU was found              | true/false                |
| Confidence  | Detection confidence level         | "High", "Medium", "Low"   |
| Details     | NPU model/type                     | "Apple Neural Engine"     |
| Method      | How NPU was detected               | "Platform", "Hardware Query" |

**Supported NPUs:**

- **Apple Neural Engine** (M1/M2/M3/M4 chips) - High confidence detection
- **Intel AI Boost** (Meteor Lake/Lunar Lake) - Hardware query detection
- **AMD Ryzen AI** (Phoenix/Hawk Point) - Hardware query detection
- **Qualcomm Hexagon** (Snapdragon X Elite/Plus) - Hardware query detection

**Confidence Levels:**

- **High**: 100% certain (e.g., Apple Neural Engine on M-series Macs)
- **Medium**: Likely present based on CPU model patterns
- **Low**: Possible but uncertain detection

**Why It Matters:**
- **Future-proofing** - NPUs will enable faster, more efficient AI inference
- Preparing for llama.cpp NPU support (CoreML, DirectML backends)
- Educational transparency - users see what hardware they have
- Currently informational; will enable NPU offloading in future versions

---

## How It Works

### Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    Frontend (Svelte 5)                  │
├─────────────────────────────────────────────────────────┤
│  HardwareIndicator.svelte    │   HardwarePanel.svelte   │
│  (Status bar widget)          │   (Full hardware view)   │
└──────────────┬────────────────┴─────────────┬───────────┘
               │                              │
               │  hardware.svelte.ts (Store)  │
               │  - State management          │
               │  - Tauri command invocation  │
               └────────────┬─────────────────┘
                            │
               Tauri IPC (JSON serialization)
                            │
┌───────────────────────────┴─────────────────────────────┐
│                   Backend (Rust/Tauri)                  │
├─────────────────────────────────────────────────────────┤
│  commands/hardware.rs                                   │
│  - detect_hardware()         (Tauri command)            │
│  - get_cached_hardware()     (Tauri command)            │
│  - HardwareCache             (In-memory cache)          │
├─────────────────────────────────────────────────────────┤
│  hardware/detector.rs                                   │
│  - detect_all()              (Main detection logic)     │
│  - convert_cpu_info()        (HardwareInfo → CpuInfo)   │
│  - convert_gpu_info()        (HardwareInfo → GpuInfo[]) │
│  - convert_memory_info()     (HardwareInfo → MemoryInfo)│
│  - convert_storage_info()    (HardwareInfo → StorageInfo)│
│  - convert_npu_info()        (HardwareInfo → NpuInfo)   │
├─────────────────────────────────────────────────────────┤
│  hardware/types.rs                                      │
│  - HardwareInfo, CpuInfo, GpuInfo, MemoryInfo, etc.     │
│  (Rust structs with Serialize/Deserialize)              │
└──────────────┬──────────────────────────────────────────┘
               │
      hardware-query v0.2.1 (Rust crate)
               │
┌──────────────┴──────────────────────────────────────────┐
│              Operating System APIs                      │
├─────────────────────────────────────────────────────────┤
│  Windows: WMI, DXGI, Windows API                        │
│  macOS:   IOKit, Metal, sysctl                          │
│  Linux:   /proc, /sys, lspci, Vulkan                    │
└─────────────────────────────────────────────────────────┘
```

### Detection Flow

**1. App Startup (Automatic Detection)**

```rust
// src-tauri/src/lib.rs
.setup(|app| {
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        match hardware::detect_all().await {
            Ok(info) => {
                log::info!("Hardware detected: CPU={}, GPUs={}",
                           info.cpu.brand, info.gpus.len());
                app_handle.state::<HardwareCache>().set(info).await;
            }
            Err(e) => {
                log::error!("Failed to detect hardware: {}", e);
            }
        }
    });
    Ok(())
})
```

- Runs asynchronously on app launch
- Non-blocking - UI loads immediately
- Results cached in memory (HardwareCache)
- Takes ~100-500ms depending on system

**2. Frontend Mount (Cache Retrieval)**

```typescript
// src/lib/stores/hardware.svelte.ts
async getCached(): Promise<void> {
    const cached = await invoke<HardwareInfo | null>('get_cached_hardware');
    if (cached) {
        hardware = cached;
    } else {
        // If backend hasn't finished yet, trigger detection
        await this.detect();
    }
}
```

- Frontend requests cached data on mount
- If cache empty (backend still detecting), triggers manual detection
- Handles race condition gracefully

**3. Detection Implementation**

```rust
// src-tauri/src/hardware/detector.rs
pub async fn detect_all() -> Result<HardwareInfo, String> {
    let hw_info = hardware_query::HardwareInfo::query()
        .map_err(|e| format!("Hardware query failed: {}", e))?;

    Ok(HardwareInfo {
        cpu: convert_cpu_info(&hw_info),
        gpus: convert_gpu_info(&hw_info),
        npu: convert_npu_info(&hw_info),
        memory: convert_memory_info(&hw_info),
        storage: convert_storage_info(&hw_info),
        detected_at: chrono::Utc::now().to_rfc3339(),
    })
}
```

- Single call to `hardware_query::HardwareInfo::query()`
- Converts to our internal types for flexibility
- Timestamps detection for cache invalidation (future feature)

---

## Using Hardware Information

### In the UI

**Hardware Panel** (`Settings → Hardware`):

- View all detected hardware in a clean, organized interface
- CPU details with instruction set features
- GPU list with VRAM and backend information
- Memory and storage capacity with availability
- NPU status (if detected)

**Hardware Indicator** (Status bar):

- Quick view of primary GPU or CPU
- Click to open full hardware panel
- Shows detection status (ready/detecting/error)

### Programmatically (Future Features)

The hardware data is available for optimization decisions:

```typescript
// Example: Model selection based on memory
import { hardwareStore } from '$lib/stores/hardware.svelte';

function recommendModel(): string {
    const availableGB = hardwareStore.info?.memory.available_gb ?? 0;

    if (availableGB >= 20) return "qwen2.5-coder:32b";
    if (availableGB >= 10) return "qwen2.5-coder:14b";
    if (availableGB >= 6) return "qwen2.5-coder:7b";
    return "qwen2.5-coder:3b";
}

// Example: llama.cpp compilation flags
function getCMakeFlags(): string[] {
    const cpu = hardwareStore.info?.cpu;
    const gpu = hardwareStore.info?.gpus[0];
    const flags = [];

    if (cpu?.features.includes('AVX2')) flags.push('-DLLAMA_AVX2=ON');
    if (cpu?.features.includes('AVX512')) flags.push('-DLLAMA_AVX512=ON');
    if (cpu?.features.includes('NEON')) flags.push('-DLLAMA_NEON=ON');

    if (gpu?.backend === 'Metal') flags.push('-DLLAMA_METAL=ON');
    if (gpu?.backend === 'CUDA') flags.push('-DLLAMA_CUDA=ON');
    if (gpu?.cuda_compute_capability) {
        flags.push(`-DCMAKE_CUDA_ARCHITECTURES=${gpu.cuda_compute_capability.replace('.', '')}`);
    }

    return flags;
}
```

---

## Technical Implementation Details

### Dependencies

**Backend (Rust):**

```toml
# src-tauri/Cargo.toml
hardware-query = "0.2.1"  # Cross-platform hardware detection
chrono = "0.4"            # Timestamp generation
```

**Why hardware-query?**

- Single dependency replacing 3+ crates (wgpu, raw-cpuid, windows)
- Unified API across all platforms
- Completely offline (no internet/API calls)
- Actively maintained and well-tested
- Reduced code from 447 lines to 87 lines

### Type Definitions

**Rust Backend:**

```rust
// src-tauri/src/hardware/types.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub cpu: CpuInfo,
    pub gpus: Vec<GpuInfo>,
    pub npu: Option<NpuInfo>,
    pub memory: MemoryInfo,
    pub storage: StorageInfo,
    pub detected_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub vendor: String,
    pub brand: String,
    pub architecture: String,
    pub physical_cores: u32,
    pub logical_cores: u32,
    pub base_frequency_mhz: f64,
    pub max_frequency_mhz: f64,
    pub cache_sizes: CacheInfo,
    pub features: Vec<String>,
}
// ... (MemoryInfo, StorageInfo, GpuInfo, NpuInfo)
```

**TypeScript Frontend:**

```typescript
// src/lib/types/hardware.ts
export interface HardwareInfo {
    cpu: CpuInfo;
    gpus: GpuInfo[];
    npu?: NpuInfo;
    memory: MemoryInfo;
    storage: StorageInfo;
    detected_at: string;
}
// ... (matching interfaces)
```

### Caching Strategy

```rust
// src-tauri/src/commands/hardware.rs
pub struct HardwareCache {
    info: Arc<Mutex<Option<HardwareInfo>>>,
}

impl HardwareCache {
    pub async fn set(&self, info: HardwareInfo) {
        let mut cache = self.info.lock().await;
        *cache = Some(info);
    }

    pub async fn get(&self) -> Option<HardwareInfo> {
        let cache = self.info.lock().await;
        cache.clone()
    }
}
```

- Thread-safe (Arc<Mutex<>>)
- Single detection on startup
- In-memory only (no disk persistence)
- Instant retrieval for UI

---

## Privacy & Security

**100% Offline Operation:**

- ✅ No network requests
- ✅ No telemetry or analytics
- ✅ No data sent to external servers
- ✅ All detection via native OS APIs
- ✅ No user identification or tracking

**Data Storage:**

- ✅ Cached in memory only (volatile)
- ✅ Cleared on app restart
- ✅ Not saved to disk
- ✅ No persistent storage of hardware info

**Security Considerations:**

- ✅ Read-only operations (no system modifications)
- ✅ Standard OS API calls (no privileged access needed)
- ✅ No sensitive data collection (serial numbers, MAC addresses, etc.)
- ✅ Hardware info is not personally identifiable

---

## Platform-Specific Behavior

### macOS

**Detection Method:**
- IOKit for CPU/GPU enumeration
- Metal framework for GPU capabilities
- sysctl for memory/storage info
- Apple Silicon: Native ARM detection, Apple Neural Engine detection

**Accuracy:** ⭐⭐⭐⭐⭐ (Excellent)

**Known Limitations:**
- L3 cache reported as 0 on M-series chips (hardware design)
- Temperature sensors not exposed by hardware-query

---

### Windows

**Detection Method:**
- WMI (Windows Management Instrumentation)
- DXGI (DirectX Graphics Infrastructure)
- Windows API for memory/storage
- Intel/AMD: NPU detection via hardware query

**Accuracy:** ⭐⭐⭐⭐ (Very Good)

**Known Limitations:**
- Requires Windows 10 or later
- Some older GPUs may not report VRAM correctly
- NPU detection limited to Meteor Lake+ (Intel) and Phoenix+ (AMD)

---

### Linux

**Detection Method:**
- /proc/cpuinfo for CPU info
- /sys/class for memory/storage
- lspci for GPU enumeration
- Vulkan for GPU capabilities

**Accuracy:** ⭐⭐⭐⭐ (Very Good)

**Known Limitations:**
- Requires lspci installed for GPU detection
- Some distros may need additional permissions for full hardware info
- NPU support very limited (Qualcomm laptops only)

---

## Future Enhancements

### Planned for v2.3.0+

1. **Cache Persistence** - Save hardware info to disk for faster startup
2. **Change Detection** - Detect hardware changes (e.g., external GPU connected)
3. **Temperature Monitoring** - Real-time CPU/GPU temperature tracking
4. **Power Metrics** - Battery status, power consumption estimation
5. **Benchmark Integration** - Use hardware info to estimate model performance
6. **Automatic Optimization** - One-click llama.cpp configuration based on hardware

### Long-Term Vision

1. **NPU Inference** - Leverage Neural Engines for faster AI processing
2. **Multi-GPU Support** - Distribute model layers across multiple GPUs
3. **Hardware Recommendations** - Suggest upgrades for better AI performance
4. **Performance Profiles** - Pre-configured settings per hardware tier

---

## Troubleshooting

### "Hardware detection failed"

**Possible causes:**

1. **Permissions issue** (Linux) - Try running with elevated privileges
2. **Missing dependencies** (Linux) - Install lspci: `sudo apt install pciutils`
3. **Corrupted hardware-query** - Clean rebuild: `cargo clean && npm run tauri dev`

**Solution:** Check logs (F12 → Console) for specific error messages.

---

### Hardware info shows "Unknown" or "0"

**Causes:**

- Some fields may not be available on all platforms
- Older hardware may not expose certain metrics
- VM/container environments have limited hardware access

**Expected behavior:** Application gracefully handles missing data.

---

### NPU not detected (but I have one)

**Causes:**

- NPU detection is still evolving
- Some NPUs require specific drivers/software
- Limited support for newer NPU models

**Workaround:** NPU detection is informational only (not required for current features).

---

### Detection is slow (>5 seconds)

**Causes:**

- First detection after cold boot
- Many storage devices connected
- Background antivirus scanning

**Solution:** Subsequent detections use cache (instant).

---

## Developer Guide

### Adding New Hardware Metrics

1. **Update Rust types** (`src-tauri/src/hardware/types.rs`):

```rust
pub struct CpuInfo {
    // ... existing fields
    pub new_field: String,  // Add new field
}
```

2. **Update detector** (`src-tauri/src/hardware/detector.rs`):

```rust
fn convert_cpu_info(hw_info: &hardware_query::HardwareInfo) -> CpuInfo {
    CpuInfo {
        // ... existing conversions
        new_field: hw_info.cpu().some_method().to_string(),
    }
}
```

3. **Update TypeScript types** (`src/lib/types/hardware.ts`):

```typescript
export interface CpuInfo {
    // ... existing fields
    new_field: string;
}
```

4. **Update UI** (`src/lib/components/HardwarePanel.svelte`):

```svelte
<div class="grid grid-cols-3 gap-2">
    <span class="text-muted-foreground">New Field:</span>
    <span class="col-span-2">{hardwareStore.info.cpu.new_field}</span>
</div>
```

### Testing Hardware Detection

```bash
# Build and run
npm run tauri dev

# Check logs
# macOS/Linux: ~/.local/share/smolpc-code-helper/logs/
# Windows: %APPDATA%\smolpc-code-helper\logs\

# Manual detection test
# Open DevTools (F12), run:
await window.__TAURI__.invoke('detect_hardware')
```

---

## Changelog

### v2.2.0 (January 2025)

- ✅ Initial hardware detection release
- ✅ CPU, GPU, Memory, Storage, NPU detection
- ✅ Cross-platform support (Windows/macOS/Linux)
- ✅ CUDA compute capability detection
- ✅ Auto-detection on startup with caching
- ✅ Hardware panel UI
- ✅ Fixed startup race condition
- ✅ Fixed NPU confidence badge display

---

## Credits

**Built with:**

- [hardware-query](https://crates.io/crates/hardware-query) - Cross-platform hardware detection
- [Tauri](https://tauri.app/) - Desktop framework
- [Svelte 5](https://svelte.dev/) - Reactive UI

**Thanks to:**

- SmolPC Team for development
- hardware-query contributors for the excellent crate
- Community testers on Mac/Windows/Linux

---

## License

MIT License - See [LICENSE](../LICENSE) for details.

---

**Questions or issues?** Open a GitHub issue: https://github.com/SmolPC-2-0/smolpc-codehelper/issues
