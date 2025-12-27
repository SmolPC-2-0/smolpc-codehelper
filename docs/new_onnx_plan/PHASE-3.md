# Phase 3: Qualcomm NPU & Cross-Platform

**Goal:** Add Qualcomm NPU support (QNN) and basic cross-platform support.

**Prerequisites:**
- Phase 1 complete (CPU inference)
- Phase 2 complete (EP abstraction, OpenVINO, CUDA)

---

## Objectives

1. Add QNN Execution Provider for Qualcomm Snapdragon X
2. Implement context binary caching (first-run optimization)
3. Handle QDQ model variant for Qualcomm
4. Add macOS support (CoreML EP)
5. Add Linux support (CPU baseline)
6. Implement context compression/summarization

---

## Deliverables

### QNN Integration
- [ ] QNN EP configuration
- [ ] Snapdragon X detection
- [ ] QDQ model variant handling
- [ ] Context binary generation (first-run)
- [ ] Context binary caching
- [ ] First-run optimization UI

### Cross-Platform
- [ ] macOS build configuration
- [ ] CoreML EP integration
- [ ] Linux build configuration
- [ ] Platform-specific DLL/dylib handling

### Context Compression
- [ ] Message summarization when context exceeded
- [ ] Preserve recent messages, compress older
- [ ] Visual indicator when compression occurs

---

## QNN-Specific Technical Details

### Context Binary Caching

QNN compiles model graph to NPU-specific format on first run (2-5 minutes). Must cache this:

```rust
impl QnnProvider {
    fn get_context_binary_path(&self, model_path: &Path) -> PathBuf {
        let cache_dir = dirs::cache_dir().unwrap().join("smolpc/qnn");
        let model_hash = hash_file(model_path);
        cache_dir.join(format!("{}.qnn_ctx", model_hash))
    }

    fn initialize(&mut self) -> Result<(), EngineError> {
        let ctx_path = self.get_context_binary_path(&self.model_path);

        if ctx_path.exists() {
            // Fast path: load cached context
            self.load_context_binary(&ctx_path)?;
        } else {
            // Slow path: compile and cache
            self.compile_context_binary(&ctx_path)?;
        }
        Ok(())
    }
}
```

### QDQ Model Requirement

Qualcomm NPU requires QDQ (Quantize-Dequantize) format, not standard INT4:

```
models/
├── qwen-2.5-coder-1.5b-int4/     # For CPU/GPU/OpenVINO
│   └── model.onnx
└── qwen-2.5-coder-1.5b-qdq/      # For Qualcomm NPU
    └── model.onnx
```

Installer detects Snapdragon and downloads appropriate variant.

### First-Run UI

```
┌─────────────────────────────────────────────────────────────┐
│              Optimizing AI for Your Computer                │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│              ████████████░░░░░░░░░░  55%                   │
│                                                             │
│              Setting up the AI accelerator...               │
│              This only happens once.                        │
│                                                             │
│              Estimated time: 3 minutes                      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Research Tasks

| Topic | Questions |
|-------|-----------|
| QNN SDK | Version requirements, DLL dependencies, licensing |
| Context binary | Format, caching strategy, invalidation |
| QDQ export | Qualcomm AI Hub workflow, manual export |
| CoreML EP | macOS integration, Metal acceleration |
| Cross-compilation | Build matrix for Windows/macOS/Linux |

---

## Success Criteria

| Criteria | Target |
|----------|--------|
| QNN works on Snapdragon X Elite | Yes |
| First-run optimization < 5 min | Yes |
| Subsequent launches fast | < 5s to ready |
| macOS build runs | Yes |
| Linux build runs | Yes |
| Context compression works | Yes |

---

*When Phase 3 is complete, proceed to PHASE-4.md for educational features.*
