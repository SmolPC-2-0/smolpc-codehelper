# SmolPC Code Helper - Context for Claude

**Purpose:** This document provides essential context for Claude sessions working on SmolPC Code Helper. Include this file in every session.

---

## What Is This Project?

SmolPC Code Helper is an **offline AI coding assistant** for secondary school students (ages 11-18). It runs entirely on local hardware using ONNX Runtime for inference.

**Key Constraints:**
- 100% offline after installation (no cloud, no telemetry)
- Must work on 8GB RAM minimum (budget school laptops)
- Windows primary platform (Microsoft partnership)
- Intel NPU acceleration required (Intel/OpenVINO partnership)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                  Code Helper (Tauri)                    │
│  ┌─────────────────────────────────────────────────┐   │
│  │           Frontend (Svelte 5)                    │   │
│  │        Chat UI, Settings, Profiles               │   │
│  └─────────────────────┬───────────────────────────┘   │
│                        │ Tauri IPC                      │
│  ┌─────────────────────┴───────────────────────────┐   │
│  │              smolpc-engine (Rust Crate)          │   │
│  │  ┌─────────┐ ┌──────────┐ ┌─────────────────┐   │   │
│  │  │Inference│ │Tokenizer │ │Memory Management│   │   │
│  │  └─────────┘ └──────────┘ └─────────────────┘   │   │
│  │  ┌─────────────────────────────────────────────┐│   │
│  │  │      Execution Providers (CPU/OpenVINO)     ││   │
│  │  └─────────────────────────────────────────────┘│   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

**Key Components:**

1. **smolpc-engine** - Standalone Rust crate handling all inference. Reusable across other SmolPC apps.
2. **Tauri App** - Desktop application wrapping the engine with UI.
3. **Execution Providers** - Hardware backends (CPU, OpenVINO, CUDA, QNN).

---

## Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Inference Engine | ONNX Runtime via `ort` crate | Only framework supporting all target EPs |
| Model | Qwen 2.5 Coder 1.5B (INT4) | Good capability/size tradeoff for 8GB |
| Frontend | Svelte 5 + Tauri 2 | Existing stack, works well |
| Hardware Acceleration | OpenVINO priority | Intel partnership requirement |

---

## Implementation Phases

| Phase | Focus | Status |
|-------|-------|--------|
| **Phase 1** | CPU inference engine, basic functionality | Current |
| Phase 2 | GPU + OpenVINO acceleration | Planned |
| Phase 3 | Qualcomm NPU, cross-platform | Planned |
| Phase 4 | Educational features, multi-user | Planned |
| Phase 5 | SmolPC Launcher ecosystem | Planned |
| Phase 6 | VS Code Extension | Future |

---

## Key Technical Details

### Engine API (smolpc-engine crate)

```rust
pub struct Engine { ... }

impl Engine {
    pub fn new(config: EngineConfig) -> Result<Self, EngineError>;
    pub fn load_model(&mut self, path: &Path) -> Result<(), EngineError>;
    pub fn generate<F>(&self, messages: &[Message], params: GenerationParams, on_token: F)
        -> Result<GenerationResult, EngineError>;
    pub fn cancel(&self);
    pub fn hardware_info(&self) -> &HardwareInfo;
    pub fn memory_stats(&self) -> MemoryStats;
}
```

### EP Selection Flow

NPU (OpenVINO) → GPU (CUDA) → CPU (fallback)

Detected at startup, cached for next launch.

### Memory Management

- Model unloads after 5 min inactivity
- Context window: 2048 tokens max
- Refuse generation if RAM < 500MB
- Context compression when window exceeded

### Streaming

Tokens emitted via Tauri events (`inference_token`). Same pattern as current Ollama implementation.

---

## Performance Targets

| Metric | Target |
|--------|--------|
| TTFT (cold, CPU) | < 30s |
| TTFT (warm) | < 3s |
| Tokens/sec (CPU, i3) | > 2 tok/s |
| Tokens/sec (NPU) | > 15 tok/s |
| Peak RAM (1.5B model) | < 3 GB |

---

## What Claude Should Know

1. **Ollama is being removed.** The new engine replaces all Ollama code.
2. **The engine is a separate crate** that will be reused by other apps.
3. **Windows is primary platform.** Cross-platform is nice-to-have.
4. **OpenVINO is high priority** due to Intel partnership.
5. **Memory is tight.** Aggressive optimization required for 8GB systems.

---

## File Structure Reference

```
smolpc-codehelper/
├── src/                      # Svelte frontend
├── src-tauri/
│   ├── src/
│   │   ├── engine/           # NEW: smolpc-engine integration
│   │   ├── commands/         # Tauri commands (replace ollama.rs)
│   │   └── ...
│   └── Cargo.toml
├── docs/
│   ├── PRD.md                # Full PRD (detailed reference)
│   ├── claude/               # Phase-specific docs for Claude
│   │   ├── CONTEXT.md        # This file
│   │   ├── PHASE-1.md        # Phase 1 requirements
│   │   └── ...
│   └── onnx/                 # ONNX migration research
└── CLAUDE.md                 # Development conventions
```

---

## For Each Session

1. **Always read CONTEXT.md** (this file)
2. **Read the relevant PHASE-X.md** for current work
3. **Reference PRD.md** for detailed specifications if needed
4. **Follow CLAUDE.md** for code conventions

---

*Last updated: December 2025*
