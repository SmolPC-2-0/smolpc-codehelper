# Implementation Plan Analysis & Validation

**Date:** December 2, 2025
**Analyst:** Claude Code
**Source Document:** `docs/implementation_plan.txt` (Gemini's Research)

---

## Executive Summary

After extensive research and validation of Gemini's implementation plan, **the core strategy is sound and represents the only viable path forward** for NPU support across Intel and Qualcomm platforms. However, several critical updates, corrections, and additional considerations have been identified that must be incorporated into the implementation.

**Key Finding:** ONNX Runtime with Rust bindings (ort crate) is indeed the only framework that provides unified access to both Intel OpenVINO and Qualcomm QNN NPU backends while meeting the project's offline-first requirements.

---

## Validation Results

### ✅ CONFIRMED CLAIMS

#### 1. ONNX Runtime as the Only Viable Solution

**Status:** ✅ VALIDATED

- ONNX Runtime 1.22 (latest) supports both QNN EP and OpenVINO EP
- llama.cpp has NO native NPU support for Intel/Qualcomm (only experimental OpenVINO backend PR)
- DirectML does not support Qualcomm NPUs
- Alternative frameworks (TensorRT, TFLite) lack cross-vendor NPU support

**Sources:**

- [ONNX Runtime QNN EP Docs](https://onnxruntime.ai/docs/execution-providers/QNN-ExecutionProvider.html)
- [ONNX Runtime OpenVINO EP Docs](https://onnxruntime.ai/docs/execution-providers/OpenVINO-ExecutionProvider.html)
- [llama.cpp NPU Discussion](https://github.com/ggml-org/llama.cpp/discussions/15883)

#### 2. Context Binary Caching for QNN

**Status:** ✅ VALIDATED

QNN EP supports context binary caching via:

- `ep.context_enable = "1"` - Enable caching
- `ep.context_file_path` - Specify cache location
- `ep.context_embed_mode = "1"` - Embed in ONNX model

This WILL reduce model loading from minutes to seconds on subsequent runs.

**Source:** [QNN EP Context Caching](https://onnxruntime.ai/docs/execution-providers/EP-Context-Design.html)

#### 3. QDQ Quantization Requirements

**Status:** ✅ VALIDATED (with nuance)

NPUs achieve optimal performance with QDQ (Quantize-Dequantize) quantization:

- Qualcomm NPU: QDQ INT8 recommended
- Intel NPU: Supports both QDQ and standard INT8, FP16 only
- AMD NPU: AWQ/Group quantization with UINT4 weights + BF16 activations

**Source:** [AMD Ryzen AI Quantization](https://ryzenai.docs.amd.com/en/latest/model_quantization.html)

#### 4. Qwen2.5-Coder ONNX Compatibility

**Status:** ✅ VALIDATED

Qwen2.5 models ARE available in ONNX format with NPU optimization:

- AMD provides Qwen2.5 1.5B/3B/7B for Ryzen AI NPUs
- Community ONNX exports available via Hugging Face
- Export tools: optimum-cli, Olive, custom scripts

**Sources:**

- [AMD Qwen2.5-1.5B NPU](https://huggingface.co/amd/Qwen2.5-1.5B-Instruct-awq-g128-int4-asym-bf16-onnx-ryzen-strix)
- [AMD Qwen2.5-7B NPU](https://huggingface.co/amd/Qwen2.5-7B-Instruct-onnx-ryzenai-npu)

#### 5. Windows 11 Native Support

**Status:** ✅ VALIDATED (Recent Update!)

Microsoft just released KB5072095 (December 2, 2025) updating QNN EP to v1.8.21.0, confirming active development and Windows 11 integration.

**Source:** [KB5072095 Update](https://windowsforum.com/threads/kb5072095-qualcomm-qnn-update-for-onnx-runtime-on-windows-11-24h2-25h2.391747/)

---

#### 2. Qualcomm AI Hub Model Availability

**Status:** ⚠️ PARTIAL CORRECTION

**Finding:** Qualcomm AI Hub DOES have Qwen2.5-7B-Instruct, but it's optimized for **Snapdragon 8 Elite** (mobile), not explicitly for Snapdragon X Elite (desktop).

**Implications:**

- May need to export our own Qwen2.5 models for Snapdragon X Elite
- Qualcomm AI Hub allows custom model uploads and optimization
- Pre-optimized desktop models may not be readily available

**Recommendation:** Plan for custom model export pipeline using Qualcomm's tools. Contact Qualcomm developer relations to confirm Snapdragon X Elite model availability.

**Sources:**

- [Qwen2.5-7B AI Hub](https://aihub.qualcomm.com/compute/models/qwen2_5_7b_instruct)
- [Snapdragon X Elite Developer Blog](https://www.qualcomm.com/developer/blog/2025/05/deploy-ai-models-on-snapdragon-x-elite-with-qualcomm-ai-hub)

#### 3. Three-Model Strategy (Not Two)

**Status:** ⚠️ EXPANSION REQUIRED

**Finding:** The "two-model" approach may actually require THREE model variants:

1. **Generic (CPU/GPU):** INT4/INT8 block quantization (CUDA, DirectML, CPU)
2. **Intel NPU:** FP16 or QDQ INT8 (OpenVINO EP, NPU-specific)
3. **Qualcomm NPU:** QDQ INT8 (QNN EP, HTP-specific)

**Rationale:**

- Intel NPU supports FP16 ONLY (per OpenVINO EP docs)
- Qualcomm requires QDQ INT8
- AMD uses UINT4 AWQ quantization
- Generic path uses dynamic quantization or INT4 block

**Recommendation:** Implement flexible model loader that selects appropriate quantization variant based on detected hardware. Consider storage implications (3x model files).

**Source:** [OpenVINO NPU Precision](https://onnxruntime.ai/docs/execution-providers/OpenVINO-ExecutionProvider.html)

#### 4. ONNX Runtime Library Bundling

**Status:** ⚠️ SIMPLIFICATION POSSIBLE

**Finding:** Starting ONNX Runtime v1.18.0, QNN dependency libraries are **included in ORT packages**. The plan suggests bundling separate QNN SDK DLLs, which may be unnecessary.

**Implication:** Deployment may be simpler than anticipated. Verify if separate QNN SDK bundling is still required for Snapdragon X Elite.

**Recommendation:** Test with standard ONNX Runtime packages first before adding custom QNN SDK bundles.

**Source:** [QNN EP Requirements](https://onnxruntime.ai/docs/execution-providers/QNN-ExecutionProvider.html)

#### 5. Dynamic Shapes Limitation

**Status:** ⚠️ CRITICAL CONSTRAINT

**Finding:** QNN EP requires **fixed input shapes** - no dynamic dimensions allowed. OpenVINO NPU has improved dynamic shape support in 2025.2 but still benefits from static shapes.

**Implications:**

- Cannot use variable-length input tensors directly
- Must implement input bucketing (128, 256, 512, 1024 tokens)
- Padding strategy required
- Multiple model variants per quantization type OR dynamic reshaping

**Recommendation:** Implement token bucketing with padding. This adds complexity to the inference loop but is mandatory for QNN EP.

**Source:** [QNN Model Constraints](https://onnxruntime.ai/docs/execution-providers/QNN-ExecutionProvider.html)

---

## Additional Technical Considerations

### 1. Memory Management Complexity

The plan correctly identifies KV cache management as complex, but underestimates the Rust-specific challenges:

**Concern:** ONNX Runtime C API uses raw pointers. The ort crate wraps these, but:

- Tensor memory must be manually managed
- KV cache slicing requires unsafe FFI calls
- Memory leaks are fatal in long-running desktop apps

**Recommendation:**

- Implement comprehensive memory profiling during Phase 4
- Use Valgrind/ASAN during development
- Consider using `ndarray` crate for safer tensor operations

### 2. Tauri IPC Performance Bottleneck

The current architecture uses Tauri events for streaming tokens. This may introduce latency:

**Concern:**

- Each token emission = separate event = serialization overhead
- 1000+ tokens per generation = 1000+ IPC calls
- May cause UI jank on slower machines

**Recommendation:**

- Batch tokens (e.g., emit every 5-10 tokens)
- Use binary serialization instead of JSON for large tensors
- Profile IPC latency early in Phase 1

### 3. Model Download Strategy

The plan mentions a "file download system" but doesn't specify:

**Missing Details:**

- Where are models hosted? (Hugging Face? Self-hosted CDN?)
- How are downloads verified? (checksums, signatures)
- Resume capability for failed downloads?
- Bandwidth considerations (7B models = 4-5GB)

**Recommendation:**

- Use Hugging Face Hub API with `hf_hub` Rust crate
- Implement chunked downloads with resume support
- Verify SHA256 checksums
- Add download progress tracking to UI

### 4. Hardware Detection Expansion

Current `hardware.rs` detects CPU/GPU/Memory. Must expand for NPU detection:

**Required Additions:**

- **Qualcomm:** Check for Snapdragon X via CPUID or WMI queries
- **Intel:** Check for Core Ultra via CPUID (NPU = AI Boost)
- **AMD:** Check for Ryzen AI via CPUID
- **Fallback:** Attempt to load EP and gracefully fail if unsupported

**Recommendation:**

- Extend `HardwareDetector` with NPU detection
- Add NPU info to `HardwareInfo` struct (sync Rust/TS types!)
- Implement EP capability probing (attempt to create session with NPU EP)

### 5. Error Handling Strategy

NPU initialization can fail in many ways:

**Failure Modes:**

- NPU present but drivers not installed
- EP fails to load (DLL missing)
- Model incompatible with NPU
- Insufficient VTCM memory
- Thermal throttling

**Recommendation:**

- Implement fallback chain: NPU → GPU → CPU
- Provide clear user feedback (not just "inference failed")
- Add diagnostic logs for support
- Consider "NPU Test" button in settings to validate setup

---

## Alternative Approaches Considered

### Option 1: llama.cpp with OpenVINO Backend

**Status:** REJECTED

**Pros:**

- Simpler deployment (single executable)
- Better CPU performance
- Mature ecosystem

**Cons:**

- OpenVINO backend is experimental PR (not merged)
- No Qualcomm QNN support
- Cannot meet Snapdragon X Elite requirement

### Option 2: Multiple Backend Implementations

**Status:** REJECTED (Too Complex)

**Concept:** Separate implementations for each vendor:

- llama.cpp for CPU/GPU
- OpenVINO C++ SDK for Intel NPU
- QNN SDK for Qualcomm NPU

**Cons:**

- 3x maintenance burden
- Inconsistent behavior across platforms
- Increased binary size

### Option 3: WebAssembly with XNNPACK

**Status:** REJECTED (Performance)

**Concept:** Use WebAssembly for cross-platform compatibility

**Cons:**

- No NPU access from WASM
- Poor performance for LLMs
- Defeats purpose of hardware acceleration

---

## Recommended Implementation Roadmap

### Phase 0: Foundation & Validation (Weeks 1-2)

**Goal:** Validate technical assumptions before major refactoring

**Tasks:**

1. **Prototype ort Crate Integration**

   - Create minimal Rust project with ort crate
   - Test QNN EP and OpenVINO EP loading
   - Verify execution provider availability on target hardware
   - Document any API differences from plan assumptions

2. **Model Export Pipeline**

   - Export Qwen2.5-1.5B to ONNX (generic INT4)
   - Test quantization tools (Olive, optimum-cli)
   - Validate model loading in ONNX Runtime
   - Measure model file sizes

3. **Hardware Access Testing**
   - Test on Snapdragon X Elite device (if available)
   - Test on Intel Core Ultra device (if available)
   - Verify NPU detection methods
   - Document driver installation requirements

**Exit Criteria:**

- ort crate successfully loads and runs inference on CPU
- At least one NPU EP (QNN or OpenVINO) loads successfully
- Qwen2.5-1.5B ONNX model exists and runs

### Phase 1: Core Inference Engine (Weeks 3-5)

**Goal:** Implement CPU-based inference with manual generation loop

**Tasks:**

1. **Inference Loop Implementation** (`src-tauri/src/inference/`)

   - Create `InferenceSession` wrapper around ort
   - Implement tokenizer integration (tokenizers crate)
   - Build autoregressive generation loop:
     - Token-by-token generation
     - KV cache management (initial, update, swap)
     - Sampling logic (temperature, top-k, top-p)
   - Add stop token detection

2. **Model Manager** (`src-tauri/src/models/`)

   - Model file storage (AppData directory)
   - Model metadata (name, size, quantization, variant)
   - Model loading/unloading
   - Memory usage tracking

3. **Tauri IPC Commands**

   - `load_model(model_id, variant) -> Result<(), String>`
   - `unload_model() -> Result<(), String>`
   - `generate_text(prompt, options) -> Result<(), String>`
   - `cancel_generation() -> Result<(), String>`
   - Streaming via events: `generation_token`, `generation_done`, `generation_error`

4. **Frontend Integration**
   - Create `inference.svelte.ts` store
   - Update `chats.svelte.ts` to use new inference system
   - Add model selection UI
   - Display generation progress

**Exit Criteria:**

- Text generation works with CPU EP
- Streaming tokens appear in UI
- User can load/unload models
- Generation can be cancelled mid-stream

### Phase 2: GPU Acceleration (Weeks 6-7)

**Goal:** Add CUDA and DirectML support for GPU acceleration

**Tasks:**

1. **CUDA Support (NVIDIA)**

   - Bundle `onnxruntime_providers_cuda.dll`
   - Bundle CUDNN libraries
   - Detect NVIDIA GPU via hardware detection
   - Configure CUDA EP in session options

2. **DirectML Support (Windows GPU)**

   - Enable DirectML EP (included in standard ORT)
   - Detect DirectX 12 capable GPU
   - Test on Intel Arc, AMD Radeon, NVIDIA

3. **Execution Provider Selection Logic**

   ```rust
   Priority: CUDA (if NVIDIA) → DirectML (if Windows) → CPU
   ```

4. **Benchmarking**
   - Measure tokens/sec on each EP
   - Measure time-to-first-token (TTFT)
   - Memory usage per EP

**Exit Criteria:**

- GPU acceleration works on NVIDIA, Intel, AMD GPUs
- Performance metrics collected
- Automatic EP selection works correctly

### Phase 3: Intel NPU Integration (Weeks 8-10)

**Goal:** Enable OpenVINO EP for Intel Core Ultra NPUs

**Tasks:**

1. **OpenVINO Library Integration**

   - Bundle OpenVINO 2025.2+ libraries
   - Verify NPU device visibility

2. **Intel NPU Detection**

   - Extend `hardware.rs` with Core Ultra detection
   - Check for NPU via OpenVINO device enumeration
   - Display NPU info in UI

3. **Model Variant Management**

   - Export Qwen2.5-1.5B for Intel NPU (FP16 or QDQ INT8)
   - Create model variant selector
   - Implement automatic variant selection based on hardware

4. **OpenVINO EP Configuration**

   - Set `device_type = "NPU"`
   - Configure FP16 precision
   - Implement input bucketing for fixed shapes
   - Enable model caching

5. **Fallback Logic**
   - Detect unsupported ops
   - Graceful fallback to CPU if NPU fails
   - User notification of fallback

**Exit Criteria:**

- Inference runs on Intel Core Ultra NPU
- Performance comparable to or better than GPU
- Fallback to CPU works correctly

### Phase 4: Qualcomm NPU Integration (Weeks 11-14)

**Goal:** Enable QNN EP for Snapdragon X Elite NPUs

**Tasks:**

1. **QNN Library Integration**

   - Bundle QNN SDK libraries (if not included in ORT 1.18+)
   - Verify HTP backend availability

2. **Snapdragon X Detection**

   - Extend `hardware.rs` with Snapdragon X detection
   - Windows ARM64 platform detection
   - Check for QNN EP capability

3. **QDQ Model Variant**

   - Export Qwen2.5-1.5B for Qualcomm NPU (QDQ INT8)
   - Use Qualcomm AI Hub or qnn-onnx-converter
   - Validate model on actual hardware

4. **Context Binary Caching**

   - Implement "First Run Optimization" UI
   - Configure context binary generation:
     ```rust
     ep.context_enable = "1"
     ep.context_file_path = "models/qwen-1.5b-qnn-ctx.bin"
     ep.context_embed_mode = "1"
     ```
   - Show progress bar during initial compilation (2-5 minutes)
   - Detect cached context on subsequent runs

5. **HTP Configuration**

   - Set `backend_type = "htp"`
   - Configure performance mode:
     - `burst` during active generation
     - `sustained_high_performance` for long sessions
   - Tune VTCM size if needed

6. **Fixed Input Shapes**
   - Implement token bucketing: 128, 256, 512, 1024
   - Pad inputs to nearest bucket
   - Handle attention masking for padded tokens

**Exit Criteria:**

- Inference runs on Snapdragon X Elite NPU
- Context binary caching reduces startup time to <5 seconds
- Performance meets or exceeds expectations
- Fixed-shape bucketing works correctly

### Phase 5: Model Download & Management (Weeks 15-16)

**Goal:** Implement automatic model downloading and version management

**Tasks:**

1. **Download Manager** (`src-tauri/src/models/downloader.rs`)

   - Integrate Hugging Face Hub API (hf-hub crate)
   - Implement chunked downloads with resume
   - SHA256 verification
   - Progress tracking (bytes downloaded/total)

2. **Model Registry**

   - Define available models:

     ```rust
     pub struct ModelDefinition {
         id: String, // "qwen2.5-coder-1.5b"
         name: String,
         size_gb: f32,
         variants: Vec<ModelVariant>,
     }

     pub struct ModelVariant {
         hardware: HardwareType, // CPU, CUDA, OpenVINO, QNN
         quantization: String, // "INT4", "FP16", "QDQ-INT8"
         hf_repo: String,
         hf_file: String,
     }
     ```

   - Recommend model based on detected hardware + RAM

3. **UI Components**

   - Model browser/selector
   - Download progress indicator
   - Model info (size, variant, status)
   - Delete model option

4. **Storage Management**
   - Track disk space usage
   - Implement model cleanup (auto-delete unused)
   - Warn if insufficient disk space

**Exit Criteria:**

- User can download models from UI
- Downloads resume after interruption
- Correct model variant selected automatically
- Disk space managed appropriately

### Phase 6: Optimization & Polish (Weeks 17-20)

**Goal:** Refine performance, memory usage, and user experience

**Tasks:**

1. **KV Cache Optimization**

   - Profile memory allocation in generation loop
   - Minimize copies (use tensor views where possible)
   - Pre-allocate cache buffers
   - Benchmark: reduce memory allocations by >50%

2. **Prompt Caching**

   - Cache KV states for system prompts
   - Implement prompt prefix matching
   - Reduce time-to-first-token by ~30%

3. **Memory Safety Audit**

   - Review all unsafe FFI blocks
   - Run Valgrind/ASAN tests
   - Fix any memory leaks
   - Document unsafe code with safety invariants

4. **Low Resource Mode**

   - Detect available RAM on startup
   - If <2GB free:
     - Refuse to load 7B model
     - Force 1.5B model
     - Show warning to user
   - Unload model after N minutes of inactivity

5. **Performance Monitoring**

   - Add telemetry (local only, no phone home):
     - Tokens/sec
     - TTFT
     - Memory usage
     - Active EP
   - Display in UI
   - Export to logs for debugging

6. **Error Recovery**
   - Handle OOM gracefully
   - Detect NPU failures and fallback
   - Retry logic for transient errors
   - User-friendly error messages

**Exit Criteria:**

- Memory usage stable over long sessions
- No memory leaks detected
- Low-RAM devices handled gracefully
- Performance metrics visible to user

### Phase 7: Testing & Documentation (Weeks 21-22)

**Goal:** Comprehensive testing and documentation

**Tasks:**

1. **Test Suite**

   - Unit tests for inference components
   - Integration tests for full generation pipeline
   - Mock NPU testing (simulate EP behavior)
   - Benchmark regression tests

2. **Hardware Testing Matrix**

   - Test on target devices:
     - Snapdragon X Elite (Qualcomm NPU)
     - Core Ultra (Intel NPU)
     - AMD Ryzen AI (bonus)
     - NVIDIA GPU
     - Intel/AMD iGPU
     - CPU-only
   - Document performance on each

3. **User Documentation**

   - Installation guide
   - Model selection guide
   - Troubleshooting NPU issues
   - FAQ

4. **Developer Documentation**
   - Update CLAUDE.md
   - Architecture diagrams
   - API documentation
   - Contributing guide

**Exit Criteria:**

- Test coverage >60%
- All target hardware tested
- Documentation complete

---

## Critical Unknowns & Risk Mitigation

### Risk 1: ort Crate API Instability

**Probability:** MEDIUM
**Impact:** HIGH

**Mitigation:**

- Pin exact version (2.0.0-rc.10)
- Monitor upstream releases
- Abstract ort API behind internal trait
- Budget time for API migration

### Risk 2: Snapdragon X Elite Model Availability

**Probability:** MEDIUM
**Impact:** MEDIUM

**Mitigation:**

- Contact Qualcomm Developer Relations early
- Plan custom model export if pre-optimized unavailable
- Budget extra time for quantization calibration

### Risk 3: OOM on 8GB Devices

**Probability:** HIGH
**Impact:** MEDIUM

**Mitigation:**

- Implement low-resource mode (Phase 6)
- Force 1.5B model on <10GB RAM devices
- Aggressive model unloading
- Memory usage warnings

### Risk 4: NPU Driver Issues on User Machines

**Probability:** HIGH
**Impact:** LOW (graceful fallback)

**Mitigation:**

- Always fallback to CPU/GPU
- Clear error messages directing to driver updates
- Diagnostic tool in settings
- Document driver requirements

### Risk 5: Context Binary Caching Failure

**Probability:** MEDIUM
**Impact:** MEDIUM (slow startup)

**Mitigation:**

- Retry context generation on failure
- Allow user to skip optimization
- Fallback to CPU if context generation fails
- Show clear progress during initial optimization

### Risk 6: Performance Below Expectations

**Probability:** MEDIUM
**Impact:** HIGH

**Mitigation:**

- Benchmark early (Phase 2)
- Compare against Ollama/llama.cpp baselines
- Optimize hot paths (KV cache, sampling)
- Consider hybrid CPU+NPU execution for unsupported ops

---

## Open Questions for User

Before proceeding, I need clarification on several points:

### 1. Hardware Access

**Question:** Do you have access to the following devices for testing?

- Snapdragon X Elite device
- Intel Core Ultra device (Meteor Lake or Lunar Lake)
- AMD Ryzen AI device

**Impact:** Cannot validate NPU implementation without hardware. May need to partner with hardware vendors for testing.

### 2. Model Hosting

**Question:** Where will model files be hosted?

- Option A: Hugging Face (free, but user-dependent)
- Option B: Self-hosted CDN (costs money, more reliable)
- Option C: Hybrid (recommend HF, fallback to CDN)

**Impact:** Affects download reliability and bandwidth costs.

### 3. Multi-Model Support Priority

**Question:** Should Phase 1 support multiple models (1.5B AND 7B) or focus on 1.5B only?

**Recommendation:** Start with 1.5B only, add 7B in Phase 5. Reduces complexity.

### 4. Fallback Strategy

**Question:** If NPU fails, should we:

- Option A: Silent fallback to CPU/GPU
- Option B: Warn user and ask permission
- Option C: Refuse to run (force NPU)

**Recommendation:** Option A with notification banner. Never block functionality.

### 5. Code Signing & Distribution

**Question:** Are you planning code signing for Windows distribution?

**Impact:** Users will see "Unknown Publisher" warning without it. May affect enterprise adoption.

### 6. Telemetry (Local Only)

**Question:** Should we collect local performance metrics for debugging?

- No phone-home, stored locally only
- Helps diagnose issues ("share logs" feature)

**Recommendation:** Yes, but make it opt-out in settings.

---

## Immediate Next Steps

1. **Review this analysis** - Identify any disagreements or missing considerations
2. **Answer open questions** - Clarify priorities and constraints
3. **Hardware acquisition** - Obtain or arrange access to Snapdragon X Elite and Core Ultra devices
4. **Begin Phase 0** - Start with ort crate prototype and model export testing

Once Phase 0 validates our assumptions, we'll have high confidence in the approach and can proceed with the full implementation.

---

## Sources

All research sources are documented inline throughout this document. Key references:

- [ONNX Runtime QNN EP](https://onnxruntime.ai/docs/execution-providers/QNN-ExecutionProvider.html)
- [ONNX Runtime OpenVINO EP](https://onnxruntime.ai/docs/execution-providers/OpenVINO-ExecutionProvider.html)
- [ort Rust Crate](https://docs.rs/ort)
- [Qualcomm AI Hub](https://aihub.qualcomm.com/)
- [AMD Qwen2.5 NPU Models](https://huggingface.co/amd/Qwen2.5-1.5B-Instruct-awq-g128-int4-asym-bf16-onnx-ryzen-strix)
- [llama.cpp NPU Discussion](https://github.com/ggml-org/llama.cpp/discussions/15883)
- [Windows ML Blog Post](https://blogs.windows.com/windowsdeveloper/2025/09/23/windows-ml-is-generally-available-empowering-developers-to-scale-local-ai-across-windows-devices/)
- [KB5072095 QNN Update](https://windowsforum.com/threads/kb5072095-qualcomm-qnn-update-for-onnx-runtime-on-windows-11-24h2-25h2.391747/)
