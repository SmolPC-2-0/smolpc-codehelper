# Model Strategy

> **Purpose:** Defines model selection criteria, hardware tiering, candidate models, quantization approach, export pipeline, runtime consolidation strategy, and model distribution plan for the SmolPC Unified Assistant.
>
> **Audience:** Any AI session (Claude or Codex) working on model selection, export, engine runtime adapters, or hardware detection.
>
> **Last Updated:** 2026-03-13

---

## Table of Contents

1. [Hardware Tiering](#hardware-tiering)
2. [Runtime Consolidation (CPU Migration)](#runtime-consolidation-cpu-migration)
3. [Model Selection Criteria](#model-selection-criteria)
4. [Tier 1 Candidates (8GB RAM)](#tier-1-candidates-8gb-ram)
5. [Tier 2 Candidates (16GB+ RAM)](#tier-2-candidates-16gb-ram)
6. [Qwen3.5 Assessment](#qwen35-assessment)
7. [NPU-Verified Models](#npu-verified-models)
8. [Pre-Quantized Models on HuggingFace](#pre-quantized-models-on-huggingface)
9. [INT4 Quantization Approach](#int4-quantization-approach)
10. [Model Export Pipeline](#model-export-pipeline)
11. [Model Artifact Layout](#model-artifact-layout)
12. [Model Distribution Strategy](#model-distribution-strategy)
13. [Auto-Selection Logic](#auto-selection-logic)
14. [CI Pipeline for Model Export](#ci-pipeline-for-model-export)
15. [Pending Decisions](#pending-decisions)

---

## Hardware Tiering

The target audience is secondary school students (ages 11-18) on budget laptops. Hardware varies significantly:

### Tier 1: Budget Hardware (8GB RAM)

- **Total RAM:** 8GB
- **Available for inference:** ~3-4GB after Windows (~2-3GB) + host application (~1-2GB)
- **Model budget:** ~1-2GB for weights + KV cache
- **Target models:** 0.5-3B parameters at INT4
- **Expected performance:** 6-12 tok/s on NPU, 4-8 tok/s on CPU
- **GPU:** Typically Intel integrated (UHD/Iris) — limited DirectML utility
- **NPU:** Intel NPU present on newer Intel Core Ultra laptops (Meteor Lake, Lunar Lake, Arrow Lake)

### Tier 2: Better Hardware (16GB+ RAM)

- **Total RAM:** 16GB+
- **Available for inference:** ~8-10GB
- **Model budget:** ~3-6GB for weights + KV cache
- **Target models:** 4-8B parameters at INT4
- **Expected performance:** 10-20 tok/s on NPU, 6-12 tok/s on CPU, 15-30 tok/s on DirectML GPU
- **GPU:** May have discrete NVIDIA GPU (GTX/RTX) or better Intel integrated
- **NPU:** Same Intel NPU support as Tier 1

### Memory Budget Breakdown (Tier 1 — Worst Case)

```
Total RAM:                    8192 MB
─ Windows 11 (idle):        ~2500 MB
─ Target app (GIMP/Blender): ~1500 MB
─ Tauri app:                  ~150 MB
─ Engine host process:        ~100 MB
─ Python MCP server:           ~80 MB
─ Headroom / other:           ~350 MB
═══════════════════════════════════════
Available for model:         ~3500 MB
─ Model weights (INT4 1.7B): ~1200 MB
─ KV cache (2K context):      ~200 MB
─ Activation memory:          ~100 MB
═══════════════════════════════════════
Remaining headroom:          ~2000 MB
```

This budget is tight but viable for sub-2B models. Models above 3B at INT4 (~2GB+) risk memory pressure on 8GB systems.

---

## Runtime Consolidation (CPU Migration)

### Current State (3 Backends — Being Deprecated)

| Backend | Rust Adapter | Runtime | DLLs | Model Format | Status |
|---------|-------------|---------|------|-------------|--------|
| `Ort` | `OrtAdapter` | Raw `ort` crate | `onnxruntime.dll` | ONNX | **DEPRECATED** |
| `GenAiDirectMl` | `GenAiDirectMlAdapter` | onnxruntime-genai C FFI | `onnxruntime_genai.dll` + `DirectML.dll` | ONNX INT4 | Active |
| `OpenVinoGenAiNpu` | `OpenVinoGenAiNpuAdapter` | openvino_genai C FFI | `openvino_genai.dll` + deps | OpenVINO IR INT4 | Active |

### Why Migrate Away from Raw `ort`

The raw `ort` crate is the weakest backend:

1. **No GenAI pipeline** — Must manually implement autoregressive generation loop
2. **Manual KV cache** — Must implement Attention Sinks and cache management by hand
3. **Manual tokenization** — Must load and run HuggingFace tokenizer separately
4. **Manual sampling** — Must implement temperature, top-k, top-p, repetition penalty
5. **No built-in stop token detection** — Must implement ChatML stop token logic
6. **Maintenance burden** — All the above is ~500+ lines of custom code that both onnxruntime-genai and openvino_genai provide out of the box

Both alternative runtimes provide a GenAI pipeline with built-in KV cache, tokenization, sampling, and stop token handling — matching what DirectML and NPU lanes already use.

### Target State Options

#### Option A: onnxruntime-genai for CPU + DirectML

```
onnxruntime-genai
├── CPU execution provider (+ OpenVINO EP for acceleration)
├── DirectML execution provider (GPU)
└── Model format: ONNX INT4

openvino_genai
├── NPU device only
└── Model format: OpenVINO IR INT4
```

**Pros:**
- Single runtime (onnxruntime-genai) covers both CPU and GPU
- One model format (ONNX INT4) for two execution providers
- OpenVINO EP within ORT gives CPU acceleration via oneDNN/AMX/VNNI without a separate runtime

**Cons:**
- onnxruntime-genai.dll is large (~150-200MB with DirectML)
- OpenVINO EP within ORT may not match native OpenVINO performance
- Still need openvino_genai separately for NPU

**Export targets:** ONNX INT4 (CPU+GPU), OpenVINO IR INT4 (NPU)

#### Option B: OpenVINO for CPU + NPU

```
openvino_genai
├── CPU device (via oneDNN/AMX/VNNI acceleration)
├── NPU device (via StaticLLMPipeline)
└── Model format: OpenVINO IR INT4

onnxruntime-genai
├── DirectML execution provider (GPU only)
└── Model format: ONNX INT4
```

**Pros:**
- openvino_genai handles both CPU and NPU with one runtime
- Native OpenVINO CPU is faster than OpenVINO EP within ORT
- Fewer total DLLs if GPU isn't present (skip ORT entirely)
- Intel partnership alignment

**Cons:**
- Need separate openvino_genai pipeline instances for CPU vs NPU (no runtime device switching)
- openvino_genai DLLs are large (~300-400MB)
- Less community support vs ONNX ecosystem

**Export targets:** OpenVINO IR INT4 (CPU+NPU), ONNX INT4 (GPU/DirectML)

#### Impact on Model Artifacts

Either option results in **2 export formats** (down from current 3):

| Scenario | Format 1 | Format 2 |
|----------|----------|----------|
| Option A | ONNX INT4 (CPU+GPU) | OpenVINO IR INT4 (NPU) |
| Option B | OpenVINO IR INT4 (CPU+NPU) | ONNX INT4 (GPU) |

### Decision Status

**NOT YET DECIDED.** Requires hands-on benchmarking:
1. onnxruntime-genai CPU (with OpenVINO EP) vs native openvino_genai CPU — measure tok/s, memory, latency
2. Evaluate DLL size tradeoffs
3. Consider Intel partnership alignment (favors OpenVINO)
4. Consider community/ecosystem (favors ORT)

---

## Model Selection Criteria

Models must satisfy ALL of the following:

### Hard Requirements

1. **OpenVINO NPU verified** — Must appear in OpenVINO 2025-2026 supported models list, or be manually verified to work on Intel NPU
2. **INT4 quantizable** — Must maintain acceptable quality at INT4 (symmetric for NPU)
3. **Fits memory budget** — INT4 weights + KV cache must fit within tier's available RAM
4. **Instruction-tuned** — Must be a chat/instruct model, not base
5. **Permissive license** — Apache 2.0, MIT, or similar (no research-only licenses)

### Soft Requirements (Ranked by Priority)

1. **Tool calling / function calling** — Critical for MCP integration. Model must reliably generate structured tool calls.
2. **Code generation quality** — HumanEval, MBPP scores. Important for Code mode.
3. **Instruction following** — IFEval, MT-Bench. Important for all modes.
4. **Multilingual** — Nice-to-have for international deployments.
5. **Pre-quantized availability** — Saves export effort if INT4 OV IR or ONNX already on HuggingFace.

### Why Tool Calling Matters Most

Every mode in the unified assistant relies on MCP tool calls:
- GIMP mode: `call_api`, `draw_line`, `apply_filter`, etc.
- Blender mode: `execute_blender_code`, scene queries
- LibreOffice: `create_document`, `insert_text`, `format_cells`, etc.
- Code mode: file operations, terminal commands

If the model can't reliably generate structured tool calls, the entire assistant is degraded. This is why BFCL (Berkeley Function Calling Leaderboard) scores are the most important benchmark.

**Problem:** BFCL scores for sub-8B models are not widely reported. Qwen3 small models don't have published BFCL numbers. This requires hands-on testing.

---

## Tier 1 Candidates (8GB RAM)

### Qwen3-1.7B (Recommended — Pending Benchmarking)

| Property | Value |
|----------|-------|
| Parameters | 1.7B |
| INT4 size | ~1.2GB |
| NPU verified | Yes (OpenVINO 2025 release notes) |
| Code quality | Good (Qwen3 family scores well) |
| Tool calling | Yes (Qwen3 native function calling) |
| License | Apache 2.0 |
| Pre-quantized OV IR | Check HuggingFace `OpenVINO/` namespace |

**Why recommended:** Best balance of tool calling capability + code quality + NPU support in the sub-2B range. Qwen3 family has native function calling support built into the model architecture.

### Qwen2.5-Coder-1.5B (Fallback — Proven)

| Property | Value |
|----------|-------|
| Parameters | 1.5B |
| INT4 size | ~1.1GB |
| NPU verified | Yes (validated in project) |
| Code quality | Strong (code-specialized) |
| Tool calling | Limited (not natively trained for it) |
| License | Apache 2.0 |
| Pre-quantized OV IR | `OpenVINO/Qwen2.5-1.5B-Instruct-int4-ov` |

**Why fallback:** Already in use in the current GIMP assistant. Proven to work. But limited tool calling capability is a significant weakness for the unified assistant's MCP-heavy architecture.

### Gemma-3-1B-it

| Property | Value |
|----------|-------|
| Parameters | 1B |
| INT4 size | ~0.7GB |
| NPU verified | Yes (OpenVINO 2025) |
| Code quality | Moderate |
| Tool calling | Unknown — needs testing |
| License | Apache 2.0 (Gemma terms) |
| Pre-quantized OV IR | Check HuggingFace |

**Assessment:** Very small, fits easily in 8GB. But code quality and tool calling are unproven at 1B scale. May be useful as an "ultra-light" option for extremely constrained hardware.

### SmolLM3-3B

| Property | Value |
|----------|-------|
| Parameters | 3B |
| INT4 size | ~2GB |
| NPU verified | Unknown (too new, HuggingFace latest) |
| Code quality | Strong (HuggingFace optimized for code) |
| Tool calling | Unknown |
| License | Apache 2.0 |
| Pre-quantized OV IR | Not yet available |

**Assessment:** Promising but risky. 2GB INT4 is tight for Tier 1 (leaves only ~1.5GB headroom on 8GB). NPU support unverified. Wait for OpenVINO compatibility confirmation.

### Qwen3.5-2B (Future Candidate)

| Property | Value |
|----------|-------|
| Parameters | 2B |
| INT4 size | <2GB |
| NPU verified | **NO — not yet verified** |
| Code quality | Strong (Intelligence Index 16, vs 13 for Qwen3-1.7B) |
| Tool calling | Expected (Qwen3.5 family) |
| License | Apache 2.0 |
| Pre-quantized OV IR | Not yet available (no first-party quantizations) |

**Assessment:** Potentially the best Tier 1 candidate IF NPU support lands. Intelligence Index 16 vs Qwen3-1.7B's 13 is a significant improvement. But too many unknowns today — no NPU verification, no pre-quantized models, high hallucination rates reported. Monitor and re-evaluate when OpenVINO adds support.

---

## Tier 2 Candidates (16GB+ RAM)

### Qwen3-4B (Recommended — Pending Benchmarking)

| Property | Value |
|----------|-------|
| Parameters | 4B |
| INT4 size | ~3GB |
| NPU verified | Yes (OpenVINO 2025) |
| Code quality | Strong |
| Tool calling | Yes (Qwen3 native) |
| License | Apache 2.0 |
| Pre-quantized OV IR | Check HuggingFace |

**Why recommended:** Strong all-around model with NPU support and native tool calling. 3GB INT4 fits comfortably in 16GB RAM with plenty of headroom.

### Phi-4-mini (Strong Alternative)

| Property | Value |
|----------|-------|
| Parameters | 3.8B |
| INT4 size | ~2.8GB |
| NPU verified | Yes (OpenVINO 2025) |
| Code quality | Strong (88.6% GSM-8K, strong math/reasoning) |
| Tool calling | Yes (supported) |
| License | MIT |
| Pre-quantized OV IR | `FluidInference/phi-4-mini-instruct-int4-ov-npu` (NPU-optimized) |

**Assessment:** Very competitive with Qwen3-4B. MIT license is more permissive. NPU-optimized pre-quantized model already available. Strong math/reasoning may benefit Code mode. Could also serve as a Tier 1 model on better 8GB systems (2.8GB INT4 is tight but possible).

### Qwen3-8B (Premium Option)

| Property | Value |
|----------|-------|
| Parameters | 8B |
| INT4 size | ~5GB |
| NPU verified | Yes (OpenVINO 2025) |
| Code quality | Very strong |
| Tool calling | Yes (Qwen3 native) |
| License | Apache 2.0 |
| Pre-quantized OV IR | `OpenVINO/Qwen3-8B-int4-ov` |

**Assessment:** Best quality in the range. Only viable on 16GB+ systems. Pre-quantized OV IR already available. Use when maximum quality is needed and hardware allows.

### Gemma-3-4B-it

| Property | Value |
|----------|-------|
| Parameters | 4B |
| INT4 size | ~3GB |
| NPU verified | Yes (OpenVINO 2025) |
| Code quality | Good |
| Tool calling | Unknown |
| License | Apache 2.0 (Gemma terms) |
| Pre-quantized OV IR | Check HuggingFace |

**Assessment:** Competitive with Qwen3-4B. Tool calling support unknown — needs testing.

### Qwen3.5-4B (Future Candidate)

| Property | Value |
|----------|-------|
| Parameters | 4B |
| INT4 size | ~3GB |
| NPU verified | **NO — not yet verified** |
| Code quality | Excellent (Intelligence Index 27, vs 18 for Qwen3-4B) |
| Tool calling | Expected |
| License | Apache 2.0 |
| Pre-quantized OV IR | Not yet available |

**Assessment:** Potentially the best sub-5B model (Intelligence Index 27 is a massive leap over Qwen3-4B's 18). Same caveats as Qwen3.5-2B — awaiting NPU verification and first-party quantizations.

---

## Qwen3.5 Assessment

### Overview

Qwen3.5 was released in February-March 2026. It represents a major architectural shift from Qwen3:
- **Hybrid architecture:** Gated DeltaNet (linear attention) + Sliding Window Attention + MoE
- **262K context length** (vs Qwen3's 32K-128K)
- **Native vision** support
- **Apache 2.0** license

### Model Lineup

| Model | Params | INT4 Size | Intelligence Index | vs Qwen3 Equivalent |
|-------|--------|-----------|-------------------|---------------------|
| Qwen3.5-0.8B | 0.8B | <1GB | 9 | +2.5 vs Qwen3-0.6B (6.5) |
| Qwen3.5-2B | 2B | <2GB | 16 | +3 vs Qwen3-1.7B (13) |
| Qwen3.5-4B | 4B | ~3GB | 27 | +9 vs Qwen3-4B (18) |
| Qwen3.5-9B | 9B | ~6GB | 32 | +15 vs Qwen3-8B (17) |
| Qwen3.5-35B-A3B | 35B (3B active) | ~24GB | — | **NOT viable** |

### Viability for SmolPC

- **Qwen3.5-2B** — Strong Tier 1 candidate. ~2GB INT4. Intelligence Index 16 vs Qwen3-1.7B's 13.
- **Qwen3.5-4B** — Excellent Tier 2 candidate. ~3GB INT4. Intelligence Index 27 — best sub-5B model available.
- **Qwen3.5-9B** — Most intelligent sub-10B model (32). For 16GB+ systems only (~6GB INT4).
- **Qwen3.5-35B-A3B** — **RULED OUT.** Despite only 3B active parameters (MoE), total weight size is ~24GB at Q4. Far too large for any target hardware.

### Risks

1. **No OpenVINO NPU verification** — Qwen3 is confirmed NPU-compatible. Qwen3.5 is NOT (too new as of March 2026). This is the biggest blocker.
2. **No first-party INT4 quantizations** — No official INT4 models on HuggingFace yet. Would need to export ourselves.
3. **High hallucination rates** — 80-82% on AA-Omniscience benchmark for 4B and 9B models. This is concerning for an educational tool.
4. **High token consumption** — 230-390M output tokens consumed during benchmarks. May indicate verbose/unfocused generation patterns.
5. **Hybrid architecture complexity** — Gated DeltaNet + MoE may have unexpected behavior on NPU (fixed-shape compilation, operator support).

### Recommendation

**Wait and watch.** Add Qwen3.5-2B and Qwen3.5-4B to the candidate list alongside Qwen3 equivalents. Do NOT commit to Qwen3.5 until:
1. OpenVINO NPU support is confirmed (check release notes, run manual tests)
2. First-party INT4 quantizations appear on HuggingFace
3. Hallucination rates are evaluated for our specific use cases

**Qwen3 is the safer choice today. Qwen3.5 is the better choice IF NPU support lands.**

---

## NPU-Verified Models

The following models are confirmed to work on Intel NPU via OpenVINO 2025-2026:

| Model | Params | Source |
|-------|--------|--------|
| Qwen3-1.7B | 1.7B | OpenVINO 2025 release notes |
| Qwen3-4B | 4B | OpenVINO 2025 release notes |
| Qwen3-8B | 8B | OpenVINO 2025 release notes |
| Gemma-3-1B-it | 1B | OpenVINO 2025 release notes |
| Gemma-3-4B-it | 4B | OpenVINO 2025 release notes |
| Phi-3-Mini-4K-Instruct | 3.8B | OpenVINO 2025 (sliding window fix applied) |
| Phi-4-mini-reasoning | 3.8B | OpenVINO 2025 release notes |
| AFM-4.5B | 4.5B | OpenVINO 2025 release notes |
| Qwen2.5-Coder-1.5B | 1.5B | Validated in project |

**NPU constraints:**
- INT4 **symmetric** quantization required (asymmetric not supported on NPU)
- Fixed input shapes (MAX_PROMPT_LEN, MIN_RESPONSE_LEN) — set at compile time
- 0.5-4B models work best on NPU. 8B is possible but slower.
- 6-12 tok/s typical for 1-3B models on NPU
- Blob cache in `<model_dir>/cache/` for faster NPU compilation (first load is slow)
- Use `compile_decoder_for_npu()` for optimized loading

---

## Pre-Quantized Models on HuggingFace

These models are ready to download and use without running the export pipeline:

| HuggingFace ID | Model | Format | Notes |
|----------------|-------|--------|-------|
| `OpenVINO/Qwen2.5-1.5B-Instruct-int4-ov` | Qwen2.5-1.5B | OpenVINO IR INT4 | Ready for CPU/NPU |
| `OpenVINO/Qwen3-8B-int4-ov` | Qwen3-8B | OpenVINO IR INT4 | Ready for CPU/NPU |
| `FluidInference/phi-4-mini-instruct-int4-ov-npu` | Phi-4-mini | OpenVINO IR INT4 | NPU-optimized |
| `OpenVINO/phi-2-int4-ov` | Phi-2 | OpenVINO IR INT4 | Older model |
| `Qwen/Qwen2.5-Coder-1.5B-Instruct-GPTQ-Int4` | Qwen2.5-Coder-1.5B | GPTQ INT4 | Needs conversion to OV IR |

**Note:** Pre-quantized models save significant effort. Always check the `OpenVINO/` namespace on HuggingFace before running manual export.

---

## INT4 Quantization Approach

### Why INT4

- **Memory reduction:** ~4x smaller than FP16 (8B model: ~16GB FP16 → ~5GB INT4)
- **NPU requirement:** Intel NPU only supports INT4/INT8 (no FP16 inference)
- **Speed:** INT4 is faster than FP16 on all backends (less memory bandwidth)
- **Quality:** Modern INT4 quantization (with group size 128) retains >95% of FP16 quality for instruction-tuned models

### Quantization Methods

| Method | Use For | Tool | Quality |
|--------|---------|------|---------|
| **NNCF** (Neural Network Compression Framework) | OpenVINO IR export | `optimum-cli export openvino` | Best for OV |
| **RTN** (Round-To-Nearest) | ONNX export (fast, lower quality) | `onnxruntime_genai.models.builder` | Good |
| **GPTQ** | ONNX export (slower, better quality) | `auto-gptq` + conversion | Better |
| **AWQ** | ONNX export (activation-aware) | `autoawq` + conversion | Best for ONNX |

### Recommended Settings

- **Group size:** 128 (standard, good quality/compression tradeoff)
- **Symmetric:** Required for NPU. Recommended for all backends for consistency.
- **Calibration dataset:** Optional but improves quality. Use ~100 samples from target domain (code, chat, tool calls).
- **Block size:** Default (varies by method)

---

## Model Export Pipeline

### ONNX Runtime GenAI Export (for CPU/DirectML)

```bash
# Install tools
pip install onnxruntime-genai

# Export with INT4 quantization
python -m onnxruntime_genai.models.builder \
  -m <huggingface-model-id> \
  -e cpu \
  -p int4 \
  -o ./output-dir

# For DirectML specifically
python -m onnxruntime_genai.models.builder \
  -m <huggingface-model-id> \
  -e dml \
  -p int4 \
  -o ./output-dir
```

**Output files:**
```
output-dir/
├── genai_config.json        # ORT GenAI configuration (sampling, tokenizer settings)
├── model.onnx               # ONNX model graph
├── model.onnx.data          # ONNX model weights (large file)
├── tokenizer.json           # HuggingFace tokenizer
├── tokenizer_config.json    # Tokenizer configuration
├── special_tokens_map.json  # Special token mappings
├── config.json              # HuggingFace model config (for reference)
└── generation_config.json   # Default generation parameters
```

### OpenVINO IR Export (for NPU + Potentially CPU)

```bash
# Install tools
pip install optimum[openvino] nncf

# Export with INT4 symmetric quantization
optimum-cli export openvino \
  --model <huggingface-model-id> \
  --weight-format int4 \
  --sym \
  --output ./output-dir

# With calibration dataset (better quality)
optimum-cli export openvino \
  --model <huggingface-model-id> \
  --weight-format int4 \
  --sym \
  --dataset wikitext2 \
  --num-samples 100 \
  --output ./output-dir
```

**Output files:**
```
output-dir/
├── openvino_model.xml       # OpenVINO IR graph definition
├── openvino_model.bin       # OpenVINO IR weights (large file)
├── tokenizer.json           # HuggingFace tokenizer
├── tokenizer_config.json    # Tokenizer configuration
├── special_tokens_map.json  # Special token mappings
├── config.json              # HuggingFace model config
└── generation_config.json   # Default generation parameters
```

### NPU-Specific Export Requirements

When exporting for NPU, additional considerations:

1. **INT4 symmetric only** — Pass `--sym` flag. Asymmetric quantization is not supported on Intel NPU.
2. **Fixed input shapes** — NPU compilation requires fixed tensor shapes. Set `MAX_PROMPT_LEN` and `MIN_RESPONSE_LEN` at model compile time.
3. **Blob cache** — First NPU load compiles the model to NPU-specific blob format. Cache this in `<model_dir>/cache/` to avoid recompilation on subsequent loads.
4. **StaticLLMPipeline** — NPU uses `StaticLLMPipeline` (not `LLMPipeline`). This is handled by openvino_genai automatically when targeting NPU device.

---

## Model Artifact Layout

### Per-Model Directory Structure

Each model gets its own directory under the models root, with subdirectories per format:

```
models/
├── qwen3-1.7b/
│   ├── onnx-int4/                 # For CPU (ORT GenAI) + DirectML
│   │   ├── genai_config.json
│   │   ├── model.onnx
│   │   ├── model.onnx.data
│   │   ├── tokenizer.json
│   │   └── ...
│   ├── openvino-int4/             # For NPU (+ potentially CPU via OpenVINO)
│   │   ├── openvino_model.xml
│   │   ├── openvino_model.bin
│   │   ├── tokenizer.json
│   │   └── ...
│   └── cache/                     # NPU blob cache (generated at runtime)
│       └── *.blob
├── qwen3-4b/
│   ├── onnx-int4/
│   ├── openvino-int4/
│   └── cache/
└── ...
```

### Model Registry Integration

The engine's model registry (`engine/crates/smolpc-engine-core/src/models/registry.rs`) tracks:

```rust
pub struct ModelEntry {
    pub id: String,               // e.g., "qwen3-1.7b"
    pub display_name: String,     // e.g., "Qwen3 1.7B"
    pub min_ram_gb: u32,          // e.g., 2 (minimum RAM to load this model)
    pub params_billions: f32,     // e.g., 1.7
    pub artifacts: Vec<ArtifactEntry>,  // per-format artifact paths
}

pub struct ArtifactEntry {
    pub format: ModelFormat,      // Onnx, OpenVinoIr
    pub path: PathBuf,            // relative to models root
    pub backend: BackendKind,     // GenAiDirectMl, OpenVinoGenAiNpu, etc.
}
```

---

## Model Distribution Strategy

### Decision: PENDING

Three approaches under consideration:

### Option 1: Bundle in Installer

- **Pros:** Works offline immediately. No first-run download. Schools with restricted internet are supported.
- **Cons:** Installer size: 1.5-4.5GB (Tier 1 + runtime DLLs). Large download. Slow install.
- **Mitigation:** Separate "models" download from "app" download. Or offer both.

### Option 2: Download on First Run

- **Pros:** Small installer (~500MB app + runtimes only). Models downloaded only for detected hardware tier.
- **Cons:** Requires internet on first run. Schools may have firewalls/bandwidth limits.
- **Mitigation:** Progress bar, resume support, offline fallback instructions.

### Option 3: Hybrid (Recommended)

- Bundle Tier 1 model (small, ~1-2GB) in installer for immediate offline use
- Download Tier 2 model on-demand if hardware supports it
- Provide manual model download instructions for fully offline schools
- Support sideloading from USB drive

### School-Specific Considerations

- Some schools have **no internet access** on student machines
- Some schools have **bandwidth caps** or **download blockers**
- Some schools use **SCCM/Intune** for software deployment — need silent install support
- **Recommendation:** Provide both an "everything bundled" installer for IT admins and a "slim" installer for internet-connected students

---

## Auto-Selection Logic

### Current Engine Capabilities

The engine already has the building blocks for auto-selection:
- `hardware_query` crate detects total RAM, GPU, NPU
- Model registry has `min_ram_gb` per model
- `POST /engine/load` can load/switch models (unloads previous first)
- Backend decision caching prevents re-selection on model switches

### Proposed Auto-Selection Algorithm

```
function select_best_model(available_ram, has_npu, has_dgpu):
    candidates = registry.models_sorted_by_quality_desc()

    for model in candidates:
        if model.min_ram_gb > available_ram:
            continue  # Too large

        # Select best backend for this model
        if has_npu and model.has_artifact(OpenVinoIr):
            backend = NPU
        elif has_dgpu and model.has_artifact(Onnx):
            backend = DirectML
        elif model.has_artifact(Onnx):  # or OpenVinoIr if Option B
            backend = CPU
        else:
            continue  # No compatible artifact

        return (model, backend)

    return Error("No compatible model found")
```

### Frontend Integration

- Engine returns recommended model + backend via `GET /engine/status`
- Frontend shows recommendation with manual override dropdown
- User can switch models at any time (triggers `POST /engine/load`)
- Current selection persists in `engine.db`

---

## CI Pipeline for Model Export

### GitHub Actions Workflow

```yaml
name: Export Models
on:
  workflow_dispatch:
    inputs:
      model_id:
        description: 'HuggingFace model ID'
        required: true
      formats:
        description: 'Export formats (onnx, openvino, both)'
        default: 'both'

jobs:
  export:
    runs-on: ubuntu-latest  # Needs ~16GB RAM for 4B models
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: '3.11'

      - name: Install export tools
        run: |
          pip install optimum[openvino] nncf onnxruntime-genai

      - name: Export OpenVINO IR INT4
        if: contains(inputs.formats, 'openvino') || inputs.formats == 'both'
        run: |
          optimum-cli export openvino \
            --model ${{ inputs.model_id }} \
            --weight-format int4 --sym \
            --output ./exported/openvino-int4

      - name: Export ONNX INT4
        if: contains(inputs.formats, 'onnx') || inputs.formats == 'both'
        run: |
          python -m onnxruntime_genai.models.builder \
            -m ${{ inputs.model_id }} \
            -e cpu -p int4 \
            -o ./exported/onnx-int4

      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: model-${{ inputs.model_id }}-int4
          path: ./exported/
```

**Resource requirements:**
- ~16GB RAM for 4B model export
- ~8GB RAM for 1-2B model export
- ~30 minutes per export (quantization is CPU-intensive)
- ~2-5GB artifact size per format

**Note:** Pre-quantized models on HuggingFace eliminate the need for CI export in most cases. Use CI only for models without pre-quantized versions.

---

## Pending Decisions

| Decision | Options | Blocker | Priority |
|----------|---------|---------|----------|
| Final model for Tier 1 | Qwen3-1.7B vs Qwen2.5-Coder-1.5B vs Qwen3.5-2B | Hands-on benchmarking + NPU testing | High |
| Final model for Tier 2 | Qwen3-4B vs Phi-4-mini vs Qwen3.5-4B | Same | High |
| CPU runtime | onnxruntime-genai (Option A) vs openvino_genai (Option B) | Performance benchmarking | High |
| Model distribution | Bundle vs download vs hybrid | School deployment testing | Medium |
| Qwen3.5 viability | Use Qwen3.5 or stick with Qwen3 | OpenVINO NPU verification | Low (wait) |

### Benchmarking Protocol

When benchmarking candidates:

1. **Setup:** Windows 11, 8GB RAM laptop (Tier 1) + 16GB laptop (Tier 2)
2. **Metrics:** tok/s, time-to-first-token, memory usage (peak RSS), quality (manual evaluation)
3. **Backends:** CPU, NPU (if available), DirectML (if GPU available)
4. **Tasks:**
   - Code completion (HumanEval-style)
   - Tool calling (generate MCP-compatible JSON)
   - Chat quality (instruction following)
   - Multi-turn conversation (memory/coherence)
5. **Compare:** Each candidate on each backend, record all metrics
6. **Decision criteria:** Prioritize tool calling quality > code quality > speed > memory
