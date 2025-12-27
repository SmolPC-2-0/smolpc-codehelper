# SmolPC Code Helper - Product Requirements Document

**Version:** 1.0
**Last Updated:** December 2025
**Status:** Draft
**Authors:** SmolPC Team

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Background & Context](#2-background--context)
3. [Goals & Non-Goals](#3-goals--non-goals)
4. [User Personas](#4-user-personas)
5. [Product Requirements](#5-product-requirements)
6. [Technical Architecture](#6-technical-architecture)
7. [User Experience](#7-user-experience)
8. [Data Model](#8-data-model)
9. [Educational Features](#9-educational-features)
10. [Implementation Phases](#10-implementation-phases) (Phases 1-6)
11. [Risks & Mitigations](#11-risks--mitigations)
12. [Research Agenda](#12-research-agenda)
13. [Success Metrics](#13-success-metrics)
14. [Appendices](#14-appendices)

**For Claude Sessions:** See `docs/claude/` for phase-specific implementation guides.

---

## 1. Executive Summary

### 1.1 What We're Building

SmolPC Code Helper is an **offline, privacy-first AI coding assistant** designed for secondary school students (ages 11-18). The application runs entirely on local hardware without requiring internet connectivity after initial installation.

### 1.2 Why We're Building It

1. **Privacy:** Student data must never leave the device. Schools require GDPR/FERPA compliance.
2. **Accessibility:** Many schools have limited/no internet. Budget constraints mean older hardware.
3. **Educational Focus:** Students need explanatory, age-appropriate assistance, not enterprise code generation.
4. **Partnerships:** Strategic relationships with Microsoft (Windows focus) and Intel (OpenVINO acceleration) drive platform priorities.

### 1.3 Core Technical Decision

**ONNX Runtime** is the inference engine. This is the only framework that bridges Intel NPU (OpenVINO), Qualcomm NPU (QNN), NVIDIA GPU (CUDA), Apple Silicon (CoreML), and CPU under a single API. The alternative (Ollama/llama.cpp) cannot utilize NPU hardware required by partnership agreements.

### 1.4 The Engine Strategy

The inference engine will be a **standalone, reusable Rust crate** (`smolpc-engine`) that can be integrated into multiple applications:

- SmolPC Code Helper (this app)
- SmolPC GIMP Assistant
- SmolPC LibreOffice Assistant
- SmolPC Blender Assistant

Each application integrates the engine and connects to its host application via MCP (Model Context Protocol).

---

## 2. Background & Context

### 2.1 The Problem Space

Secondary schools face a dilemma:

- **Cloud AI services** (ChatGPT, Copilot) are powerful but raise privacy concerns, require internet, and cost money
- **Offline solutions** exist but are complex to set up (Ollama requires CLI knowledge) or don't run on budget hardware
- **Students need help** learning to code, but teachers can't provide 1:1 attention to every student

### 2.2 The Hardware Reality

Target deployment hardware:

| Tier            | Specs                                               | Prevalence         |
| --------------- | --------------------------------------------------- | ------------------ |
| **Minimum**     | 8GB RAM, Intel i3/Celeron, integrated graphics, HDD | Common in schools  |
| **Recommended** | 16GB RAM, Intel i5/Ryzen 5, SSD                     | Newer purchases    |
| **Optimal**     | 16GB+ RAM, Intel Core Ultra (NPU), SSD              | Future deployments |

The application **must run on Minimum tier** hardware, even if performance is degraded.

### 2.3 The NPU Opportunity

Modern Intel Core Ultra and Qualcomm Snapdragon X processors include dedicated NPUs (Neural Processing Units). These provide:

- 10-40x faster inference than CPU
- Lower power consumption
- Dedicated silicon (doesn't compete with user applications for resources)

Intel partnership requires we demonstrate NPU acceleration via OpenVINO.

### 2.4 Current State (MVP)

The existing MVP uses Ollama for inference:

- Requires separate Ollama installation
- No NPU support (CPU/GPU only via llama.cpp)
- Works but doesn't meet partnership requirements

This PRD defines the migration to a custom ONNX Runtime-based engine.

### 2.5 Relationship to Other SmolPC Apps

SmolPC is building a suite of AI-powered educational tools:

```
┌─────────────────────────────────────────────────────────────┐
│                    SmolPC Launcher                          │
│                   (User Authentication)                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Code Helper  │  │GIMP Assistant│  │LibreOffice   │ ...  │
│  │   (Tauri)    │  │   (Plugin)   │  │  Assistant   │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                 │                 │               │
│         │                 │      MCP        │      MCP      │
│         │                 │                 │               │
│  ┌──────┴─────────────────┴─────────────────┴───────-┐      │
│  │                  smolpc-engine                    │      │
│  │              (Rust Inference Crate)               │      │
│  └───────────────────────────────────────────────────┘      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

All apps share the same engine. User profiles persist across apps.

---

## 3. Goals & Non-Goals

### 3.1 Goals

| ID  | Goal                                                     | Priority |
| --- | -------------------------------------------------------- | -------- |
| G1  | Run 100% offline after installation                      | P0       |
| G2  | Work on 8GB RAM minimum hardware                         | P0       |
| G3  | Provide useful coding assistance for students ages 11-18 | P0       |
| G4  | Support Intel NPU acceleration via OpenVINO              | P0       |
| G5  | Automatic hardware detection and optimal EP selection    | P0       |
| G6  | Easy installation (no technical knowledge required)      | P0       |
| G7  | Support offline/USB installation for no-internet schools | P1       |
| G8  | Support NVIDIA GPU acceleration                          | P1       |
| G9  | Support Qualcomm NPU acceleration via QNN                | P2       |
| G10 | Cross-platform (macOS, Linux)                            | P2       |
| G11 | Teacher administration features                          | P2       |

### 3.2 Non-Goals

| ID  | Non-Goal                                   | Rationale                                                              |
| --- | ------------------------------------------ | ---------------------------------------------------------------------- |
| NG1 | Compete with GPT-4/Claude capability       | Impossible on local hardware; "good enough for learning" is the target |
| NG2 | Support arbitrary model loading in Phase 1 | Complexity; future feature for advanced users                          |
| NG3 | Real-time collaboration features           | Out of scope; focus on individual learning                             |
| NG4 | Cloud sync or backup                       | Violates offline-first principle                                       |
| NG5 | Mobile support (iOS/Android)               | Desktop focus for school computer labs                                 |
| NG6 | Fine-tuning or training models             | Too complex; use pre-trained models                                    |

---

## 4. User Personas

### 4.1 Primary: The Student

**Name:** Alex, 14 years old

**Context:** Learning Python in Year 9 Computer Science class

**Technical Skill:** Beginner to intermediate programmer

**Goals:**

- Understand why their code doesn't work
- Get explanations of programming concepts
- Complete homework assignments

**Pain Points:**

- Can't ask teacher every question (class of 30)
- Online resources are overwhelming or too advanced
- ChatGPT is blocked at school

**Needs from SmolPC:**

- Simple, clear explanations
- Quick responses (patience is limited)
- Works on school laptop without admin rights
- Doesn't require internet

### 4.2 Secondary: The Teacher

**Name:** Mrs. Thompson, Computer Science teacher

**Context:** Teaches Years 7-11, manages a computer lab

**Technical Skill:** Competent but not a developer

**Goals:**

- Help students learn without doing their work for them
- Monitor how students use AI assistance
- Align AI help with curriculum

**Pain Points:**

- Can't supervise 30 students simultaneously
- Worried about AI doing homework for students
- Limited IT support for software installation

**Needs from SmolPC:**

- Admin controls (adjust AI behavior)
- Visibility into student usage
- Easy deployment across lab computers
- Curriculum-aware responses

### 4.3 Tertiary: The IT Administrator

**Name:** Dave, School IT Manager

**Context:** Manages 500+ devices across the school

**Technical Skill:** Experienced with enterprise deployment

**Goals:**

- Deploy software to many machines efficiently
- Ensure security and compliance
- Minimize support tickets

**Pain Points:**

- Limited bandwidth for downloads
- Diverse hardware fleet
- Teachers requesting software constantly

**Needs from SmolPC:**

- Silent/automated installation
- Pre-configuration options
- Offline/USB deployment
- No ongoing maintenance

---

## 5. Product Requirements

### 5.1 Functional Requirements

#### FR1: Core Inference

| ID    | Requirement                                                                                                           | Priority |
| ----- | --------------------------------------------------------------------------------------------------------------------- | -------- |
| FR1.1 | The engine MUST perform text generation using locally stored ONNX models                                              | P0       |
| FR1.2 | The engine MUST stream tokens to the UI as they are generated                                                         | P0       |
| FR1.3 | The engine MUST support cancellation of in-progress generation                                                        | P0       |
| FR1.4 | The engine MUST implement the full autoregressive generation loop (tokenization, inference, sampling, detokenization) | P0       |
| FR1.5 | The engine MUST support temperature, top-k, and top-p sampling parameters                                             | P1       |
| FR1.6 | The engine SHOULD support multiple concurrent conversations (one active generation, others waiting)                   | P1       |

#### FR2: Hardware Acceleration

| ID    | Requirement                                                                   | Priority |
| ----- | ----------------------------------------------------------------------------- | -------- |
| FR2.1 | The engine MUST support CPU execution (baseline)                              | P0       |
| FR2.2 | The engine MUST support Intel NPU via OpenVINO Execution Provider             | P0       |
| FR2.3 | The engine SHOULD support NVIDIA GPU via CUDA Execution Provider              | P1       |
| FR2.4 | The engine SHOULD support Qualcomm NPU via QNN Execution Provider             | P2       |
| FR2.5 | The engine SHOULD support Apple Silicon via CoreML Execution Provider         | P2       |
| FR2.6 | The engine MUST automatically detect available hardware and select optimal EP | P0       |
| FR2.7 | The engine MUST gracefully fallback if preferred EP fails to initialize       | P0       |

#### FR3: Memory Management

| ID    | Requirement                                                                                              | Priority |
| ----- | -------------------------------------------------------------------------------------------------------- | -------- |
| FR3.1 | The engine MUST operate within available system memory without crashing                                  | P0       |
| FR3.2 | The engine MUST unload the model after configurable inactivity period (default: 5 minutes)               | P0       |
| FR3.3 | The engine MUST refuse generation if available RAM < 500MB                                               | P0       |
| FR3.4 | The engine MUST support context window limiting (configurable, default: 2048 tokens)                     | P0       |
| FR3.5 | The engine SHOULD implement context compression/summarization when window is exceeded                    | P1       |
| FR3.6 | The engine SHOULD monitor RAM and automatically take action (unload, compress) before critical threshold | P1       |

#### FR4: Model Management

| ID    | Requirement                                                                                   | Priority |
| ----- | --------------------------------------------------------------------------------------------- | -------- |
| FR4.1 | The application MUST ship with a pre-bundled model (Qwen 2.5 Coder 1.5B, INT4 quantized)      | P0       |
| FR4.2 | The application MUST support an optional enhanced model (Qwen 2.5 Coder 7B) for 16GB+ systems | P1       |
| FR4.3 | The installer MUST recommend appropriate model based on detected hardware                     | P0       |
| FR4.4 | Users MUST be able to switch between installed models                                         | P1       |
| FR4.5 | Advanced users SHOULD be able to install custom ONNX models                                   | P2       |

#### FR5: Chat Interface

| ID    | Requirement                                                    | Priority |
| ----- | -------------------------------------------------------------- | -------- |
| FR5.1 | Users MUST be able to send messages and receive AI responses   | P0       |
| FR5.2 | Users MUST be able to create, view, and delete conversations   | P0       |
| FR5.3 | Conversations MUST persist locally across application restarts | P0       |
| FR5.4 | Users MUST be able to cancel in-progress generation            | P0       |
| FR5.5 | Code blocks in responses MUST be syntax highlighted            | P1       |
| FR5.6 | Users SHOULD be able to copy code blocks with one click        | P1       |
| FR5.7 | Users SHOULD be able to export conversations to Markdown       | P2       |

#### FR6: User Profiles & Authentication

| ID    | Requirement                                                             | Priority |
| ----- | ----------------------------------------------------------------------- | -------- |
| FR6.1 | The application MUST support multiple user profiles on shared computers | P1       |
| FR6.2 | Each user's conversations and settings MUST be isolated                 | P1       |
| FR6.3 | User authentication MUST work offline (local accounts)                  | P1       |
| FR6.4 | Users MUST be able to clear all their data                              | P0       |

#### FR7: Installation & Deployment

| ID    | Requirement                                                                    | Priority |
| ----- | ------------------------------------------------------------------------------ | -------- |
| FR7.1 | Online installer MUST download and install the application and selected model  | P0       |
| FR7.2 | USB/offline installer MUST work without any internet connection                | P1       |
| FR7.3 | Installer MUST detect hardware and display recommendations                     | P0       |
| FR7.4 | Installation MUST NOT require administrator privileges for basic functionality | P0       |
| FR7.5 | School deployment mode MUST allow IT admins to pre-configure settings          | P2       |
| FR7.6 | Installer MUST support silent/unattended installation for mass deployment      | P2       |

### 5.2 Non-Functional Requirements

#### NFR1: Performance

| ID     | Requirement                                         | Target       | Priority |
| ------ | --------------------------------------------------- | ------------ | -------- |
| NFR1.1 | Time to First Token (TTFT) on CPU (8GB, cold start) | < 30 seconds | P0       |
| NFR1.2 | Time to First Token (TTFT) on CPU (model loaded)    | < 3 seconds  | P0       |
| NFR1.3 | Tokens per second on CPU (i3, 8GB)                  | > 2 tok/s    | P0       |
| NFR1.4 | Tokens per second on Intel NPU                      | > 15 tok/s   | P1       |
| NFR1.5 | Application startup time (without model)            | < 3 seconds  | P0       |
| NFR1.6 | Peak memory usage with 1.5B model                   | < 3 GB       | P0       |
| NFR1.7 | Peak memory usage with 7B model                     | < 6 GB       | P1       |

#### NFR2: Reliability

| ID     | Requirement                                                                      | Priority |
| ------ | -------------------------------------------------------------------------------- | -------- |
| NFR2.1 | The application MUST NOT crash when running out of memory (graceful degradation) | P0       |
| NFR2.2 | The application MUST recover from EP initialization failures                     | P0       |
| NFR2.3 | Conversation data MUST NOT be corrupted by unexpected shutdowns                  | P0       |
| NFR2.4 | The application MUST log errors for debugging                                    | P1       |

#### NFR3: Security & Privacy

| ID     | Requirement                                                                    | Priority |
| ------ | ------------------------------------------------------------------------------ | -------- |
| NFR3.1 | The application MUST NOT transmit any data over the network after installation | P0       |
| NFR3.2 | The application MUST NOT include telemetry or analytics                        | P0       |
| NFR3.3 | User data MUST be stored locally only                                          | P0       |
| NFR3.4 | Users MUST be able to delete all their data completely                         | P0       |

#### NFR4: Usability

| ID     | Requirement                                                               | Priority |
| ------ | ------------------------------------------------------------------------- | -------- |
| NFR4.1 | The application MUST be usable by students with no AI/CLI experience      | P0       |
| NFR4.2 | Error messages MUST be understandable by non-technical users              | P0       |
| NFR4.3 | The UI MUST be accessible (keyboard navigation, screen reader compatible) | P1       |

---

## 6. Technical Architecture

### 6.1 System Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         SmolPC Code Helper                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Frontend (Svelte 5)                       │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │   │
│  │  │  Chat View   │  │  Settings    │  │  Profiles    │       │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘       │   │
│  │                           │                                  │   │
│  │                    Tauri Events                              │   │
│  │                    (inference_token)                         │   │
│  └───────────────────────────┬─────────────────────────────────┘   │
│                              │                                      │
│                       Tauri IPC                                     │
│                              │                                      │
│  ┌───────────────────────────┴─────────────────────────────────┐   │
│  │              Tauri Backend (Rust)                            │   │
│  │  ┌──────────────────────────────────────────────────────┐   │   │
│  │  │                    Commands Layer                     │   │   │
│  │  │  - generate()     - cancel()     - get_hardware()    │   │   │
│  │  │  - load_model()   - get_status() - get_settings()    │   │   │
│  │  └──────────────────────────┬───────────────────────────┘   │   │
│  │                             │                                │   │
│  │  ┌──────────────────────────┴───────────────────────────┐   │   │
│  │  │              smolpc-engine (Crate)                    │   │   │
│  │  │                                                       │   │   │
│  │  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   │   │   │
│  │  │  │  Inference  │  │  Tokenizer  │  │   Sampler   │   │   │   │
│  │  │  └─────────────┘  └─────────────┘  └─────────────┘   │   │   │
│  │  │                                                       │   │   │
│  │  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   │   │   │
│  │  │  │  KV Cache   │  │   Memory    │  │  Hardware   │   │   │   │
│  │  │  │  Manager    │  │   Watchdog  │  │  Detector   │   │   │   │
│  │  │  └─────────────┘  └─────────────┘  └─────────────┘   │   │   │
│  │  │                                                       │   │   │
│  │  │  ┌─────────────────────────────────────────────────┐ │   │   │
│  │  │  │            Execution Providers                   │ │   │   │
│  │  │  │  ┌─────┐  ┌─────────┐  ┌──────┐  ┌─────┐       │ │   │   │
│  │  │  │  │ CPU │  │OpenVINO │  │ CUDA │  │ QNN │       │ │   │   │
│  │  │  │  └─────┘  └─────────┘  └──────┘  └─────┘       │ │   │   │
│  │  │  └─────────────────────────────────────────────────┘ │   │   │
│  │  └───────────────────────────────────────────────────────┘   │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    │                               │
           ┌────────┴────────┐             ┌───────┴───────┐
           │   ONNX Model    │             │   ORT Libs    │
           │   (.onnx)       │             │   (.dll/.so)  │
           └─────────────────┘             └───────────────┘
```

### 6.2 The smolpc-engine Crate

#### 6.2.1 Repository Structure

The engine is a separate Git repository: `github.com/SmolPC/smolpc-engine`

```
smolpc-engine/
├── Cargo.toml
├── README.md
├── LICENSE
│
├── src/
│   ├── lib.rs                    # Public API exports
│   ├── engine.rs                 # Main Engine struct
│   ├── config.rs                 # Configuration types
│   ├── error.rs                  # Error types (thiserror)
│   │
│   ├── inference/
│   │   ├── mod.rs
│   │   ├── session.rs            # ONNX session wrapper
│   │   ├── generation.rs         # Autoregressive loop
│   │   └── kv_cache.rs           # KV cache management
│   │
│   ├── tokenizer/
│   │   ├── mod.rs
│   │   └── hf_tokenizer.rs       # HuggingFace tokenizers
│   │
│   ├── sampling/
│   │   ├── mod.rs
│   │   ├── temperature.rs
│   │   ├── top_k.rs
│   │   └── top_p.rs
│   │
│   ├── providers/
│   │   ├── mod.rs
│   │   ├── traits.rs             # ExecutionProvider trait
│   │   ├── cpu.rs
│   │   ├── openvino.rs
│   │   ├── cuda.rs
│   │   └── qnn.rs
│   │
│   ├── hardware/
│   │   ├── mod.rs
│   │   ├── detection.rs          # Hardware probing
│   │   └── selection.rs          # EP selection logic
│   │
│   └── memory/
│       ├── mod.rs
│       ├── watchdog.rs           # RAM monitoring
│       └── context.rs            # Context compression
│
├── tests/
│   ├── integration/
│   └── unit/
│
└── examples/
    ├── basic.rs
    └── streaming.rs
```

#### 6.2.2 Public API

```rust
// lib.rs - Public API surface

pub struct Engine { /* ... */ }

impl Engine {
    /// Create engine with auto-detected hardware
    pub fn new(config: EngineConfig) -> Result<Self, EngineError>;

    /// Load a model from disk
    pub fn load_model(&mut self, path: &Path) -> Result<(), EngineError>;

    /// Unload current model to free memory
    pub fn unload_model(&mut self);

    /// Check if model is loaded
    pub fn is_model_loaded(&self) -> bool;

    /// Generate text with streaming
    pub fn generate<F>(
        &self,
        messages: &[Message],
        params: GenerationParams,
        on_token: F,
    ) -> Result<GenerationResult, EngineError>
    where
        F: FnMut(TokenEvent) -> ControlFlow<(), ()>;

    /// Cancel ongoing generation
    pub fn cancel(&self);

    /// Get hardware information
    pub fn hardware_info(&self) -> &HardwareInfo;

    /// Get current memory stats
    pub fn memory_stats(&self) -> MemoryStats;

    /// Get active execution provider
    pub fn active_provider(&self) -> &str;
}

pub struct EngineConfig {
    /// Model directory path
    pub model_dir: PathBuf,
    /// Maximum context window (tokens)
    pub max_context: usize,
    /// Inactivity timeout before unloading (seconds)
    pub unload_timeout: u64,
    /// Minimum free RAM to allow generation (bytes)
    pub min_free_ram: u64,
    /// Preferred execution providers (in order of preference)
    pub preferred_providers: Vec<ProviderType>,
    /// Enable context compression
    pub enable_context_compression: bool,
}

pub struct GenerationParams {
    pub temperature: f32,
    pub top_k: usize,
    pub top_p: f32,
    pub max_tokens: usize,
    pub stop_sequences: Vec<String>,
}

pub struct Message {
    pub role: Role,      // System, User, Assistant
    pub content: String,
}

pub enum TokenEvent {
    Token(String),
    Done(GenerationStats),
    Error(EngineError),
}

pub struct GenerationResult {
    pub text: String,
    pub tokens_generated: usize,
    pub time_to_first_token_ms: u64,
    pub total_time_ms: u64,
    pub tokens_per_second: f32,
}
```

#### 6.2.3 Execution Provider Abstraction

```rust
// providers/traits.rs

pub trait ExecutionProvider: Send + Sync {
    /// Provider name (for logging/display)
    fn name(&self) -> &str;

    /// Check if this provider can be used on current hardware
    fn is_available(&self) -> bool;

    /// Initialize the provider
    fn initialize(&mut self) -> Result<(), EngineError>;

    /// Get ONNX Runtime session options for this provider
    fn session_options(&self) -> SessionBuilder;

    /// Provider-specific input preparation (e.g., padding for OpenVINO)
    fn prepare_inputs(&self, tokens: &[i64]) -> Result<InputTensors, EngineError>;

    /// Whether this provider requires warmup (e.g., QNN context binary)
    fn requires_warmup(&self) -> bool;

    /// Perform warmup if required
    fn warmup(&mut self, model_path: &Path) -> Result<(), EngineError>;

    /// Preferred model variant for this provider
    fn preferred_model_variant(&self) -> ModelVariant;
}

pub enum ModelVariant {
    /// Standard INT4 quantization (CPU, GPU, OpenVINO)
    Int4,
    /// QDQ quantization (Qualcomm NPU)
    Qdq,
}
```

### 6.3 Tauri Integration

The Code Helper app integrates the engine via Tauri commands:

```rust
// src-tauri/src/commands/inference.rs

use smolpc_engine::{Engine, EngineConfig, GenerationParams, Message};

#[tauri::command]
pub async fn generate(
    window: Window,
    state: State<'_, AppState>,
    messages: Vec<Message>,
    params: GenerationParams,
) -> Result<(), String> {
    let engine = state.engine.lock().await;

    engine.generate(&messages, params, |event| {
        match event {
            TokenEvent::Token(text) => {
                window.emit("inference_token", text).ok();
                ControlFlow::Continue(())
            }
            TokenEvent::Done(stats) => {
                window.emit("inference_done", stats).ok();
                ControlFlow::Break(())
            }
            TokenEvent::Error(e) => {
                window.emit("inference_error", e.to_string()).ok();
                ControlFlow::Break(())
            }
        }
    }).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cancel_generation(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let engine = state.engine.lock().await;
    engine.cancel();
    Ok(())
}

#[tauri::command]
pub async fn get_hardware_info(
    state: State<'_, AppState>,
) -> Result<HardwareInfo, String> {
    let engine = state.engine.lock().await;
    Ok(engine.hardware_info().clone())
}

#[tauri::command]
pub async fn get_engine_status(
    state: State<'_, AppState>,
) -> Result<EngineStatus, String> {
    let engine = state.engine.lock().await;
    Ok(EngineStatus {
        model_loaded: engine.is_model_loaded(),
        active_provider: engine.active_provider().to_string(),
        memory_stats: engine.memory_stats(),
    })
}
```

### 6.4 Hardware Detection & EP Selection

#### 6.4.1 Detection Flow

```
Application Startup
        │
        ▼
┌───────────────────────┐
│   Probe Hardware      │
│   - CPU info          │
│   - GPU info          │
│   - NPU detection     │
│   - RAM available     │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│  Check Cached EP      │◄─── If previous session used EP successfully,
│  (from settings)      │     try that first
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│  EP Selection Order:  │
│  1. Cached (if valid) │
│  2. Intel NPU         │
│  3. NVIDIA GPU        │
│  4. Apple Silicon     │
│  5. CPU (fallback)    │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│  Try Initialize EP    │
└───────────┬───────────┘
            │
      ┌─────┴─────┐
      │           │
   Success     Failure
      │           │
      ▼           ▼
┌─────────┐  ┌──────────────┐
│  Use EP │  │ Show Warning │
│  Ready  │  │ Try Next EP  │
└─────────┘  └──────────────┘
```

#### 6.4.2 EP-Specific Considerations

| EP           | Detection Method                               | Initialization       | Special Handling                                 |
| ------------ | ---------------------------------------------- | -------------------- | ------------------------------------------------ |
| **CPU**      | Always available                               | Trivial              | None                                             |
| **OpenVINO** | Check for Intel CPU with AI Boost (Core Ultra) | Load OpenVINO DLLs   | Pad inputs to bucket sizes (128, 256, 512, 1024) |
| **CUDA**     | Check for NVIDIA GPU via NVML                  | Load CUDA/cuDNN DLLs | Set GPU device ID                                |
| **QNN**      | Check for Snapdragon X via CPUID               | Load QNN DLLs        | First-run context binary compilation (2-5 min)   |
| **CoreML**   | Check for Apple Silicon                        | Native on macOS      | Metal acceleration                               |

### 6.5 Memory Management

#### 6.5.1 Memory Budget (8GB System)

```
Total System RAM:           8,192 MB
├── Windows OS:            ~3,000 MB
├── Background Apps:         ~500 MB
├── SmolPC App (UI):         ~100 MB
├── Model Weights (1.5B):  ~1,200 MB
├── KV Cache (2048 ctx):     ~500 MB
├── Inference Buffers:       ~200 MB
└── Safety Margin:           ~500 MB
    ─────────────────────────────────
    Remaining:             ~2,192 MB (for other apps)
```

#### 6.5.2 Memory Management Strategies

**1. Automatic Model Unloading**

```rust
// After N seconds of inactivity, unload model
// Reload on next generation request
struct InactivityWatchdog {
    last_activity: Instant,
    timeout: Duration,
}
```

**2. Context Window Management**

```rust
// Hard limit on context tokens
// When exceeded, compress older messages
struct ContextManager {
    max_tokens: usize,
    messages: Vec<Message>,
    compressed_prefix: Option<String>,
}

impl ContextManager {
    fn add_message(&mut self, msg: Message) {
        self.messages.push(msg);
        if self.total_tokens() > self.max_tokens {
            self.compress_oldest();
        }
    }

    fn compress_oldest(&mut self) {
        // Summarize oldest N messages into single message
        // "Previous conversation summary: ..."
    }
}
```

**3. Low Memory Response**

```rust
// If available RAM drops below threshold, take action
struct MemoryWatchdog {
    min_free_ram: u64,
    critical_ram: u64,
}

impl MemoryWatchdog {
    fn check(&self) -> MemoryAction {
        let free = get_available_ram();
        if free < self.critical_ram {
            MemoryAction::RefuseGeneration
        } else if free < self.min_free_ram {
            MemoryAction::CompressContext
        } else {
            MemoryAction::None
        }
    }
}
```

### 6.6 The Generation Loop

The core autoregressive generation loop:

```rust
pub fn generate<F>(
    &self,
    messages: &[Message],
    params: GenerationParams,
    mut on_token: F,
) -> Result<GenerationResult, EngineError>
where
    F: FnMut(TokenEvent) -> ControlFlow<(), ()>,
{
    let start = Instant::now();

    // 1. Build prompt from messages
    let prompt = self.build_prompt(messages)?;

    // 2. Tokenize
    let mut tokens = self.tokenizer.encode(&prompt)?;

    // 3. Initialize KV cache
    let mut kv_cache = KVCache::new(self.config.max_context);

    // 4. Track stats
    let mut first_token_time = None;
    let mut generated_tokens = 0;
    let mut output_text = String::new();

    // 5. Generation loop
    loop {
        // Check for cancellation
        if self.cancelled.load(Ordering::Relaxed) {
            break;
        }

        // Prepare inputs (EP-specific, e.g., padding)
        let inputs = self.provider.prepare_inputs(&tokens)?;

        // Run inference
        let outputs = self.session.run(inputs)?;

        // Extract logits for last position
        let logits = outputs.get_last_logits()?;

        // Apply sampling
        let next_token = self.sampler.sample(&logits, &params)?;

        // Check for stop
        if next_token == self.tokenizer.eos_token() {
            on_token(TokenEvent::Done(self.compute_stats(start, generated_tokens)));
            break;
        }

        if generated_tokens >= params.max_tokens {
            on_token(TokenEvent::Done(self.compute_stats(start, generated_tokens)));
            break;
        }

        // Record first token time
        if first_token_time.is_none() {
            first_token_time = Some(start.elapsed());
        }

        // Decode token to text
        let text = self.tokenizer.decode(&[next_token])?;
        output_text.push_str(&text);

        // Emit token
        if on_token(TokenEvent::Token(text)) == ControlFlow::Break(()) {
            break;
        }

        // Update for next iteration
        tokens = vec![next_token];
        kv_cache.update(&outputs)?;
        generated_tokens += 1;
    }

    Ok(GenerationResult {
        text: output_text,
        tokens_generated: generated_tokens,
        time_to_first_token_ms: first_token_time.map(|d| d.as_millis() as u64).unwrap_or(0),
        total_time_ms: start.elapsed().as_millis() as u64,
        tokens_per_second: generated_tokens as f32 / start.elapsed().as_secs_f32(),
    })
}
```

### 6.7 Model Format & Quantization

#### 6.7.1 Shipped Models

| Model               | Quantization    | Size    | Target Hardware     |
| ------------------- | --------------- | ------- | ------------------- |
| Qwen 2.5 Coder 1.5B | INT4 (GPTQ/AWQ) | ~1.2 GB | 8GB RAM (default)   |
| Qwen 2.5 Coder 7B   | INT4 (GPTQ/AWQ) | ~4.5 GB | 16GB RAM (optional) |

#### 6.7.2 Model Artifacts

```
models/
├── qwen-2.5-coder-1.5b-int4/
│   ├── model.onnx              # Model graph + weights
│   ├── tokenizer.json          # HuggingFace tokenizer
│   ├── config.json             # Model configuration
│   └── special_tokens.json     # Special token mappings
│
└── qwen-2.5-coder-7b-int4/
    ├── model.onnx
    ├── tokenizer.json
    ├── config.json
    └── special_tokens.json
```

#### 6.7.3 EP-Specific Model Variants

For Qualcomm NPU (Phase 3):

- Requires QDQ (Quantize-Dequantize) format
- Separate download: `qwen-2.5-coder-1.5b-qdq.onnx`
- Installer detects Snapdragon and downloads appropriate variant

For OpenVINO:

- Can use same INT4 ONNX model
- No separate variant needed

---

## 7. User Experience

### 7.1 Installation Flow

#### 7.1.1 Online Installation

```
┌─────────────────────────────────────────────────────────────┐
│              SmolPC Code Helper Setup                       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Welcome to SmolPC Code Helper!                             │
│                                                             │
│  This AI coding assistant runs entirely on your computer.   │
│  No internet required after setup.                          │
│                                                             │
│                    [Next →]                                 │
│                                                             │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              Checking Your Computer...                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ✓ Processor: Intel Core i5-1235U                           │
│  ✓ Memory: 8 GB RAM                                         │
│  ✓ Graphics: Intel Iris Xe                                  │
│  ✓ AI Accelerator: Intel AI Boost (NPU) detected!           │
│  ✓ Storage: 120 GB available                                │
│                                                             │
│  ────────────────────────────────────────────────────────   │
│                                                             │
│  Based on your hardware, we recommend:                      │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ ● Standard AI Model (Recommended)                     │  │
│  │   Size: 1.2 GB | Works great with 8GB RAM             │  │
│  │   Good for: Learning to code, homework help           │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ ○ Enhanced AI Model                                   │  │
│  │   Size: 4.5 GB | Needs 16GB+ RAM                      │  │
│  │   ⚠ May be slow on your computer                      │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│               [← Back]  [Install →]                         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              Installing...                                  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Downloading AI Model...                                    │
│  ████████████████░░░░░░░░░░░░░░  45%                       │
│                                                             │
│  723 MB of 1.2 GB                                           │
│                                                             │
│                    [Cancel]                                 │
│                                                             │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              Ready to Go!                                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ✓ SmolPC Code Helper is installed                          │
│                                                             │
│  The app will use your Intel AI Boost for faster            │
│  responses. You can start coding!                           │
│                                                             │
│               [Launch SmolPC Code Helper]                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

#### 7.1.2 USB/Offline Installation

USB drive contents:

```
SmolPC-Installer/
├── Setup.exe
├── models/
│   ├── qwen-2.5-coder-1.5b-int4.onnx  (1.2 GB)
│   └── qwen-2.5-coder-7b-int4.onnx    (4.5 GB)
├── runtime/
│   ├── onnxruntime.dll
│   └── onnxruntime_providers_*.dll
├── config/
│   └── school-defaults.json           (optional pre-config)
└── README.txt
```

The installer:

1. Detects models on USB
2. Copies to local disk
3. No network requests
4. Uses pre-config if present

### 7.2 First Run Experience

```
┌─────────────────────────────────────────────────────────────┐
│                    SmolPC Code Helper                       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│              Loading AI Model...                            │
│                                                             │
│              ████████░░░░░░░░░░░░░░  35%                   │
│                                                             │
│              This may take a minute on first launch.        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

If NPU first-run optimization is needed (Qualcomm):

```
┌─────────────────────────────────────────────────────────────┐
│                    SmolPC Code Helper                       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│              Optimizing AI for Your Computer                │
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

### 7.3 Main Chat Interface

The chat interface remains similar to the current MVP with these additions:

````
┌─────────────────────────────────────────────────────────────┐
│  SmolPC Code Helper                               [─][□][×] │
├──────────────┬──────────────────────────────────────────────┤
│              │                                              │
│  Chats       │  ┌────────────────────────────────────────┐  │
│  ──────────  │  │ 🤖 Assistant                           │  │
│  > Python    │  │                                        │  │
│    homework  │  │ Here's how you can fix that loop:      │  │
│              │  │                                        │  │
│    JavaScript│  │ ```python                              │  │
│    basics    │  │ for i in range(5):                     │  │
│              │  │     print(i)                           │  │
│  + New Chat  │  │ ```                               [📋] │  │
│              │  │                                        │  │
│              │  │ The issue was that you were using      │  │
│              │  │ `range(1, 5)` which starts at 1...     │  │
│              │  └────────────────────────────────────────┘  │
│              │                                              │
│  ──────────  │  ┌────────────────────────────────────────┐  │
│  ⚙ Settings  │  │ 👤 You                                 │  │
│              │  │ Why doesn't my loop print 0?           │  │
│  ℹ About     │  └────────────────────────────────────────┘  │
│              │                                              │
│              │  ┌────────────────────────────────────────┐  │
│              │  │ Type your message...              [→]  │  │
│              │  └────────────────────────────────────────┘  │
│              │                                              │
│              │  CPU: Intel i5 | RAM: 2.1GB/8GB | 6.2 tok/s │
├──────────────┴──────────────────────────────────────────────┤
│  Model: Qwen 2.5 (1.5B) | Intel AI Boost Active ⚡          │
└─────────────────────────────────────────────────────────────┘
````

Key UI elements:

- **Status bar** showing active EP, memory usage, generation speed
- **Copy button** on code blocks
- **Syntax highlighting** in code blocks

### 7.4 Settings Panel

```
┌─────────────────────────────────────────────────────────────┐
│  Settings                                                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  AI Model                                                   │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ ● Standard (1.5B) - Currently active                  │  │
│  │ ○ Enhanced (7B) - Not installed [Download]            │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  Response Style                    (Future: Complexity dial)│
│  ┌───────────────────────────────────────────────────────┐  │
│  │ ○ Simple - Ages 11-13                                 │  │
│  │ ● Standard - Ages 14-16                               │  │
│  │ ○ Detailed - Ages 17-18                               │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  System Prompt                     (Teacher configurable)   │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ You are a helpful coding tutor for students...        │  │
│  │                                                  [✎]  │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  Data                                                       │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ [Export All Chats]  [Clear All Data]                  │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  About                                                      │
│  Version: 2.0.0                                             │
│  Engine: smolpc-engine 1.0.0                                │
│  Hardware: Intel Core i5-1235U, 8GB RAM                     │
│  Accelerator: Intel AI Boost (OpenVINO)                     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 7.5 Error States

#### 7.5.1 Model Loading Failed

```
┌───────────────────────────────────────┐
│  ⚠ Could not load AI model            │
│                                       │
│  The AI model file may be corrupted.  │
│                                       │
│  [Re-download Model]  [Try Anyway]    │
└───────────────────────────────────────┘
```

#### 7.5.2 Low Memory

```
┌───────────────────────────────────────┐
│  ⚠ Low memory                         │
│                                       │
│  Your computer is running low on      │
│  memory. Close other programs for     │
│  better performance.                  │
│                                       │
│  [Continue Anyway]  [Wait]            │
└───────────────────────────────────────┘
```

#### 7.5.3 Accelerator Failed (Fallback)

```
┌───────────────────────────────────────┐
│  ℹ Using standard mode                │
│                                       │
│  Could not enable Intel AI Boost.     │
│  Running on CPU (may be slower).      │
│                                       │
│  [OK]                                 │
└───────────────────────────────────────┘
```

---

## 8. Data Model

### 8.1 Local Storage Structure

```
~/.smolpc/                           (or %APPDATA%\SmolPC on Windows)
├── config.json                      # Application settings
├── profiles/
│   ├── default/
│   │   ├── profile.json             # Profile metadata
│   │   ├── chats/
│   │   │   ├── {uuid}.json          # Individual chat files
│   │   │   └── ...
│   │   └── settings.json            # Profile-specific settings
│   │
│   └── {username}/                  # Additional profiles
│       └── ...
│
├── models/
│   ├── qwen-2.5-coder-1.5b-int4/
│   │   └── ...
│   └── qwen-2.5-coder-7b-int4/
│       └── ...
│
├── cache/
│   ├── ep_selection.json            # Cached EP detection result
│   └── qnn_context/                 # QNN context binaries (if applicable)
│       └── ...
│
└── logs/
    └── app.log
```

### 8.2 Data Schemas

#### 8.2.1 Chat Schema

```typescript
interface Chat {
	id: string; // UUID
	title: string; // Auto-generated or user-set
	createdAt: string; // ISO 8601
	updatedAt: string; // ISO 8601
	messages: Message[];
	metadata: {
		model: string; // Model used for this chat
		tokensUsed: number; // Total tokens in conversation
	};
}

interface Message {
	id: string; // UUID
	role: 'user' | 'assistant' | 'system';
	content: string;
	timestamp: string; // ISO 8601
	metadata?: {
		tokensGenerated?: number;
		generationTimeMs?: number;
		tokensPerSecond?: number;
	};
}
```

#### 8.2.2 Profile Schema

```typescript
interface Profile {
	id: string; // UUID or username
	name: string; // Display name
	createdAt: string;
	settings: ProfileSettings;
}

interface ProfileSettings {
	responseStyle: 'simple' | 'standard' | 'detailed';
	systemPrompt?: string; // Custom system prompt override
	model: string; // Preferred model
}
```

#### 8.2.3 Application Config Schema

```typescript
interface AppConfig {
	version: string;
	activeProfile: string;
	installedModels: string[];
	hardware: {
		detectedAt: string;
		cpu: string;
		ram: number;
		gpu?: string;
		npu?: string;
	};
	engine: {
		cachedProvider: string;
		maxContext: number;
		unloadTimeoutSeconds: number;
	};
}
```

---

## 9. Educational Features

### 9.1 Current Scope (Phase 1-2)

For initial release, educational focus is achieved through:

1. **System Prompt Engineering**

   - Age-appropriate language
   - Explanatory responses (not just code)
   - Encouraging tone

2. **Response Style Toggle**
   - Simple (11-13): Very basic vocabulary, short explanations, analogies
   - Standard (14-16): Normal technical vocabulary, balanced explanations
   - Detailed (17-18): Technical language, comprehensive explanations

### 9.2 Future Educational Features (Phase 3+)

These features are planned but not part of initial scope:

#### 9.2.1 Teacher Administration Panel

```
┌─────────────────────────────────────────────────────────────┐
│  Teacher Dashboard                                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Current Lesson Context                                     │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Topic: Python Loops (Year 9 Curriculum, Week 5)       │  │
│  │ The AI will focus responses on this topic.            │  │
│  │                                               [Change]│  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  Student Activity (Last 7 days)                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Student       │ Questions │ Topics                    │  │
│  │ ──────────────┼───────────┼────────────────────────── │  │
│  │ Alex T.       │ 12        │ loops, functions          │  │
│  │ Sarah M.      │ 8         │ variables, loops          │  │
│  │ James K.      │ 3         │ syntax errors             │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  System Prompt Override                                     │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ [Default for Year 9 Python] ▼                         │  │
│  │                                                       │  │
│  │ You are helping Year 9 students learn Python.         │  │
│  │ Focus on: loops (for, while), basic functions.        │  │
│  │ Avoid: advanced topics like classes, decorators.      │  │
│  │                                               [Apply] │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

#### 9.2.2 Curriculum Integration (RAG)

Teachers can upload curriculum documents (PDF/text):

1. Documents are indexed locally (no cloud)
2. When students ask questions, relevant curriculum sections are retrieved
3. AI responses reference specific curriculum content

Implementation approach:

- Use a local embedding model (small, fast)
- Store embeddings in local vector DB (e.g., LanceDB, SQLite with vector extensions)
- Retrieve top-k relevant chunks for context

#### 9.2.3 Progress Reports

The system tracks:

- Topics the student asks about
- Common misconceptions (repeated similar questions)
- Time spent per topic

Teachers can export reports:

```json
{
	"student": "Alex T.",
	"period": "2024-01-15 to 2024-01-22",
	"summary": {
		"totalQuestions": 12,
		"topTopics": ["for loops", "range function", "indentation"],
		"commonErrors": ["off-by-one in range", "missing colon"],
		"timeSpent": "2h 15m"
	}
}
```

### 9.3 Language Support

The Qwen 2.5 Coder model supports multiple programming languages with varying capability:

| Language   | Capability | Notes                              |
| ---------- | ---------- | ---------------------------------- |
| Python     | Excellent  | Primary training focus             |
| JavaScript | Good       | Strong support                     |
| HTML/CSS   | Good       | Web development basics             |
| Java       | Moderate   | May need more prompting            |
| C/C++      | Moderate   | Less consistent than Python        |
| Scratch    | Limited    | Can explain concepts, not generate |

To improve capability in specific languages, the system prompt can include language-specific examples and guidance.

---

## 10. Implementation Phases

### Phase 1: CPU Engine MVP

**Goal:** Working inference on CPU, no hardware acceleration

**Deliverables:**

- [ ] `smolpc-engine` crate with CPU-only inference
- [ ] Tokenizer integration (HuggingFace tokenizers crate)
- [ ] Autoregressive generation loop
- [ ] Sampling (temperature, top-k, top-p)
- [ ] KV cache management
- [ ] Basic memory management (unload on inactivity)
- [ ] Context window limiting
- [ ] Streaming token output
- [ ] Tauri integration (commands, events)
- [ ] Model bundling (Qwen 2.5 Coder 1.5B INT4)
- [ ] Updated installer (online + offline)
- [ ] Remove all Ollama code

**Success Criteria:**

- Application launches and generates responses on 8GB system
- TTFT < 30 seconds (cold start), < 3 seconds (warm)
- Generation speed > 2 tokens/second on i3
- No crashes or OOM on 8GB system

**Research Required:**

- Validate `ort` crate for ONNX Runtime integration
- Confirm Qwen 2.5 ONNX model availability/export
- Benchmark memory usage with 1.5B model

### Phase 2: GPU & OpenVINO Acceleration

**Goal:** Hardware acceleration for Intel NPU and NVIDIA GPU

**Deliverables:**

- [ ] OpenVINO Execution Provider integration
- [ ] Intel NPU detection
- [ ] Input padding/bucketing for OpenVINO
- [ ] CUDA Execution Provider integration
- [ ] NVIDIA GPU detection
- [ ] EP fallback chain implementation
- [ ] EP caching (remember last successful EP)
- [ ] Updated status bar (show active accelerator)
- [ ] Enhanced model option (7B) with download

**Success Criteria:**

- OpenVINO acceleration works on Intel Core Ultra
- 5-10x speedup over CPU on NPU
- Graceful fallback if EP fails
- 7B model works on 16GB systems

**Research Required:**

- OpenVINO DLL bundling and licensing
- CUDA/cuDNN bundling and licensing
- OpenVINO static shape requirements

### Phase 3: Qualcomm & Platform Expansion

**Goal:** Qualcomm NPU support, cross-platform basics

**Deliverables:**

- [ ] QNN Execution Provider integration
- [ ] Snapdragon X detection
- [ ] QDQ model variant handling
- [ ] Context binary caching (first-run optimization)
- [ ] macOS support (CoreML EP)
- [ ] Linux support (CPU baseline)
- [ ] Context compression/summarization

**Success Criteria:**

- QNN acceleration works on Snapdragon X Elite
- First-run optimization completes in < 5 minutes
- Subsequent launches are fast (cached context binary)
- macOS and Linux builds functional

**Research Required:**

- QNN SDK integration and versioning
- Context binary caching implementation
- CoreML EP configuration

### Phase 4: Educational & Polish

**Goal:** Educational features, multi-user, teacher tools

**Deliverables:**

- [ ] Multi-user profiles
- [ ] Profile-isolated data
- [ ] Chat export (Markdown)
- [ ] Response style selector (complexity dial)
- [ ] Teacher administration panel (basic)
- [ ] System prompt customization UI
- [ ] Student activity logging
- [ ] Progress report export
- [ ] Curriculum RAG integration (basic)

**Success Criteria:**

- Multiple students can use shared computer with separate profiles
- Teachers can view student usage
- Teachers can customize AI behavior per class
- Chat export works correctly

### Phase 5: SmolPC Launcher & Ecosystem

**Goal:** Unified launcher for all SmolPC apps

**Deliverables:**

- [ ] SmolPC Launcher application
- [ ] Unified authentication
- [ ] Profile sync across apps
- [ ] Engine sharing across apps
- [ ] MCP integration for other apps
- [ ] GIMP Assistant integration
- [ ] LibreOffice Assistant integration

**Success Criteria:**

- Single login for all SmolPC apps
- Engine loaded once, shared by apps
- MCP communication works reliably

### Phase 6: VS Code Extension (CodeHelper Extension)

**Goal:** Integrate SmolPC Code Helper directly into VS Code as an extension, providing Copilot-like functionality

**Overview:**

The CodeHelper Extension brings the smolpc-engine into the IDE, offering:
- Sidebar chat panel (similar to current standalone app)
- Inline code completions (ghost text as you type)
- Context-aware assistance (understands current file, selection)
- Future: Full codebase understanding via RAG

**Architecture:**

```
┌─────────────────────────────────────────────────────────────┐
│                     VS Code                                 │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              CodeHelper Extension                    │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │   │
│  │  │ Chat Panel  │  │   Inline    │  │  Context    │  │   │
│  │  │  (Webview)  │  │ Completions │  │  Gatherer   │  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  │   │
│  └──────────────────────────┬──────────────────────────┘   │
│                             │ HTTP/SSE                      │
└─────────────────────────────┼───────────────────────────────┘
                              │
                    ┌─────────┴─────────┐
                    │  smolpc-engine    │
                    │  (daemon/server)  │
                    └───────────────────┘
```

**Deliverables:**

Phase 6A: Basic Chat Extension
- [ ] VS Code extension scaffolding (TypeScript)
- [ ] Engine daemon mode (HTTP server for IPC)
- [ ] Sidebar chat webview panel
- [ ] "Explain Code" command (right-click menu)
- [ ] "Fix Code" command (right-click menu)
- [ ] Current file/selection as context

Phase 6B: Inline Completions
- [ ] InlineCompletionItemProvider implementation
- [ ] Debounced trigger on typing pause
- [ ] Tab to accept, Escape to dismiss
- [ ] Surrounding code context (before/after cursor)
- [ ] Performance optimization for <500ms TTFT

Phase 6C: Codebase Awareness (Future)
- [ ] Local embedding model for code indexing
- [ ] Vector store for semantic search (LanceDB or similar)
- [ ] @file and @codebase commands
- [ ] Project-wide context retrieval
- [ ] Workspace indexing on open/change

**Success Criteria:**

Phase 6A:
- Extension installs from VSIX
- Chat panel works with current file context
- Commands appear in right-click menu

Phase 6B:
- Inline suggestions appear within 500ms
- Suggestions are contextually relevant
- Acceptance rate comparable to basic Copilot usage

Phase 6C:
- Can answer "where is X used?" queries
- Can explain how systems work across files
- Indexing completes in reasonable time (<5 min for medium projects)

**Technical Considerations:**

1. **Engine as Daemon:** The smolpc-engine runs as a background service with HTTP API. This allows both the Tauri app and VS Code extension to share the same running engine instance.

2. **Latency Requirements:** Inline completions require fast TTFT (<500ms). This strongly benefits from NPU acceleration. On CPU-only systems, inline completions may be disabled or degraded.

3. **Context Gathering:** VS Code APIs provide access to:
   - Current document content
   - Cursor position and selection
   - Open editors
   - Workspace symbols (via language server)
   - File system (for codebase indexing)

4. **Codebase RAG Architecture:**
   - Use small embedding model (e.g., all-MiniLM-L6-v2, ~80MB)
   - Index code files on workspace open
   - Store embeddings locally (per-workspace)
   - Retrieve top-k relevant chunks for context
   - No cloud, fully offline

**Research Required:**

- VS Code Extension API for inline completions
- Engine daemon mode implementation (HTTP server in Rust)
- Embedding model selection for code
- Vector storage options (LanceDB, SQLite with vector extensions)
- Latency optimization for inline completions

---

## 11. Risks & Mitigations

### 11.1 Technical Risks

| Risk                                        | Likelihood | Impact | Mitigation                                                          |
| ------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------- |
| `ort` crate doesn't support OpenVINO EP     | Medium     | High   | Validate early in Phase 1; if needed, use raw C API via unsafe Rust |
| 1.5B model insufficient for educational use | Medium     | High   | Test with target users early; have 7B as fallback                   |
| Memory pressure on 8GB systems              | High       | Medium | Aggressive memory management; 1.5B default; test extensively        |
| Qwen ONNX export not available              | Low        | Medium | Export ourselves using optimum/olive; document process              |
| Generation too slow on low-end CPUs         | High       | Medium | Set user expectations; optimize where possible; push NPU            |
| OpenVINO DLL bundling issues                | Medium     | Medium | Test on clean Windows installs; proper installer                    |

### 11.2 Product Risks

| Risk                                | Likelihood | Impact | Mitigation                                                |
| ----------------------------------- | ---------- | ------ | --------------------------------------------------------- |
| Students use AI to cheat, not learn | Medium     | High   | Teacher controls; explanatory mode; activity logging      |
| Teachers reject due to complexity   | Low        | High   | Simple installation; minimal configuration; good defaults |
| Performance not acceptable to users | Medium     | High   | NPU acceleration; manage expectations; 7B option          |
| Hardware diversity too high         | Medium     | Medium | Focus on Intel first; broad testing; fallback to CPU      |

### 11.3 Partnership Risks

| Risk                                      | Likelihood | Impact | Mitigation                                    |
| ----------------------------------------- | ---------- | ------ | --------------------------------------------- |
| OpenVINO API changes break integration    | Low        | High   | Pin to specific version; test before updates  |
| Intel partnership requirements change     | Low        | Medium | Maintain communication; flexible architecture |
| Microsoft partnership requirements change | Low        | Medium | Windows focus; maintain communication         |

---

## 12. Research Agenda

### 12.1 Pre-Implementation Research (Before Phase 1)

| Topic                      | Priority | Questions to Answer                                         |
| -------------------------- | -------- | ----------------------------------------------------------- |
| `ort` crate capabilities   | P0       | Does it support all required EPs? What version? How stable? |
| Qwen 2.5 ONNX availability | P0       | Are pre-exported models available? What quantization?       |
| Memory footprint           | P0       | Actual RAM usage of 1.5B model + KV cache on 8GB system?    |
| Generation speed           | P0       | Realistic tok/s on target hardware (i3, Celeron)?           |

### 12.2 Phase 1 Research (During Implementation)

| Topic                   | Priority | Questions to Answer                                   |
| ----------------------- | -------- | ----------------------------------------------------- |
| Tokenizer integration   | P0       | HuggingFace tokenizers crate compatibility with Qwen? |
| KV cache optimization   | P1       | Best approach for memory-efficient cache management?  |
| Sampling implementation | P1       | Reference implementations in Rust? Edge cases?        |
| Context compression     | P1       | Best approach for summarizing old messages?           |

### 12.3 Phase 2 Research

| Topic                 | Priority | Questions to Answer                             |
| --------------------- | -------- | ----------------------------------------------- |
| OpenVINO DLL bundling | P0       | Which DLLs? Size? Licensing terms?              |
| OpenVINO input shapes | P0       | How to implement bucketing? Performance impact? |
| CUDA bundling         | P1       | cuDNN licensing? Size? Version compatibility?   |
| Intel NPU detection   | P0       | How to reliably detect Core Ultra NPUs?         |

### 12.4 Phase 3 Research

| Topic                  | Priority | Questions to Answer                           |
| ---------------------- | -------- | --------------------------------------------- |
| QNN SDK integration    | P0       | Full implementation details, versioning, DLLs |
| Context binary caching | P0       | How to implement first-run optimization?      |
| QDQ model export       | P0       | Qualcomm AI Hub workflow? Manual export?      |
| CoreML EP              | P1       | macOS-specific considerations?                |

---

## 13. Success Metrics

### 13.1 Technical Metrics

| Metric                       | Target           | Measurement Method |
| ---------------------------- | ---------------- | ------------------ |
| TTFT (cold start, CPU, 8GB)  | < 30 seconds     | Benchmark suite    |
| TTFT (warm, CPU, 8GB)        | < 3 seconds      | Benchmark suite    |
| Tokens/second (CPU, i3, 8GB) | > 2 tok/s        | Benchmark suite    |
| Tokens/second (Intel NPU)    | > 15 tok/s       | Benchmark suite    |
| Peak RAM usage (1.5B)        | < 3 GB           | Memory profiling   |
| Peak RAM usage (7B)          | < 6 GB           | Memory profiling   |
| Crash rate                   | < 1% of sessions | Error logging      |
| EP fallback success rate     | > 99%            | Error logging      |

### 13.2 User Experience Metrics

| Metric                          | Target            | Measurement Method    |
| ------------------------------- | ----------------- | --------------------- |
| Installation success rate       | > 95%             | Deployment feedback   |
| First-run success rate          | > 95%             | Deployment feedback   |
| User satisfaction (students)    | > 4/5             | Surveys               |
| User satisfaction (teachers)    | > 4/5             | Surveys               |
| Daily active users (per school) | > 50% of students | Usage logging (local) |

### 13.3 Partnership Metrics

| Metric                | Target                      | Measurement Method |
| --------------------- | --------------------------- | ------------------ |
| Intel NPU utilization | Works on Core Ultra         | Demonstration      |
| OpenVINO integration  | Approved by Intel engineers | Review meeting     |
| Windows compatibility | Works on Windows 10/11      | Testing            |

---

## 14. Appendices

### Appendix A: Glossary

| Term         | Definition                                                                |
| ------------ | ------------------------------------------------------------------------- |
| **ONNX**     | Open Neural Network Exchange - open format for ML models                  |
| **ORT**      | ONNX Runtime - Microsoft's inference engine                               |
| **EP**       | Execution Provider - hardware backend for ORT (CPU, CUDA, OpenVINO, etc.) |
| **NPU**      | Neural Processing Unit - dedicated AI accelerator chip                    |
| **OpenVINO** | Intel's toolkit for optimizing and deploying AI inference                 |
| **QNN**      | Qualcomm Neural Network SDK                                               |
| **KV Cache** | Key-Value cache - stores intermediate attention states during generation  |
| **INT4**     | 4-bit integer quantization - reduces model size                           |
| **QDQ**      | Quantize-Dequantize - quantization format required by some NPUs           |
| **TTFT**     | Time to First Token - latency before first token appears                  |
| **tok/s**    | Tokens per second - generation speed                                      |
| **MCP**      | Model Context Protocol - protocol for AI-app communication                |
| **RAG**      | Retrieval-Augmented Generation - enhancing AI with retrieved documents    |

### Appendix B: Hardware Compatibility Matrix

| Hardware                       | EP Used        | Model | Expected Performance |
| ------------------------------ | -------------- | ----- | -------------------- |
| Intel Core Ultra (Meteor Lake) | OpenVINO (NPU) | INT4  | 15-25 tok/s          |
| Intel 12th-14th Gen (no NPU)   | CPU            | INT4  | 4-8 tok/s            |
| Intel 10th-11th Gen            | CPU            | INT4  | 3-5 tok/s            |
| Intel Celeron/Pentium          | CPU            | INT4  | 1-3 tok/s            |
| AMD Ryzen 5000-7000            | CPU            | INT4  | 4-8 tok/s            |
| AMD Ryzen 3000-4000            | CPU            | INT4  | 2-4 tok/s            |
| NVIDIA GTX 1060+               | CUDA           | INT4  | 10-30 tok/s          |
| NVIDIA RTX 3000+               | CUDA           | INT4  | 30-60 tok/s          |
| Qualcomm Snapdragon X Elite    | QNN (NPU)      | QDQ   | 20-40 tok/s          |
| Apple M1/M2/M3                 | CoreML         | INT4  | 15-40 tok/s          |

_Note: Performance estimates are approximate and require validation through benchmarking._

### Appendix C: Model Comparison

| Model               | Parameters | Quantized Size | RAM Required | Capability        |
| ------------------- | ---------- | -------------- | ------------ | ----------------- |
| Qwen 2.5 Coder 0.5B | 0.5B       | ~400 MB        | ~1.5 GB      | Basic completion  |
| Qwen 2.5 Coder 1.5B | 1.5B       | ~1.2 GB        | ~3 GB        | Good for learning |
| Qwen 2.5 Coder 3B   | 3B         | ~2.5 GB        | ~4 GB        | Better reasoning  |
| Qwen 2.5 Coder 7B   | 7B         | ~4.5 GB        | ~6 GB        | Strong capability |

_Recommendation: Ship 1.5B as default, 7B as optional download for 16GB+ systems._

### Appendix D: System Prompt Template

```
You are SmolPC Code Helper, an AI coding tutor for students ages {{age_range}}.

Your role:
- Help students understand programming concepts
- Explain errors and bugs clearly
- Guide students to solutions rather than just giving answers
- Use age-appropriate language and examples
- Be encouraging and patient

Guidelines:
- Keep explanations {{complexity_level}}
- Use analogies when helpful for younger students
- Always explain why code works, not just what it does
- If a student seems stuck, break down the problem into smaller steps
- Encourage good coding practices (comments, meaningful names, etc.)

{{#if curriculum_context}}
Current lesson context:
{{curriculum_context}}
Focus your responses on the current topic when relevant.
{{/if}}

{{#if teacher_notes}}
Teacher notes:
{{teacher_notes}}
{{/if}}
```

### Appendix E: File Size Budget

| Component           | Size    | Notes                 |
| ------------------- | ------- | --------------------- |
| Application binary  | ~30 MB  | Tauri app             |
| ONNX Runtime DLLs   | ~150 MB | Core + CPU EP         |
| OpenVINO DLLs       | ~200 MB | Additional for Intel  |
| CUDA DLLs           | ~400 MB | Additional for NVIDIA |
| Model (1.5B INT4)   | ~1.2 GB | Default               |
| Model (7B INT4)     | ~4.5 GB | Optional              |
| **Total (minimal)** | ~1.4 GB | CPU only              |
| **Total (full)**    | ~6.5 GB | All EPs + both models |

### Appendix F: References

1. ONNX Runtime documentation: https://onnxruntime.ai/docs/
2. `ort` Rust crate: https://docs.rs/ort/
3. OpenVINO documentation: https://docs.openvino.ai/
4. Qualcomm AI Hub: https://aihub.qualcomm.com/
5. Qwen 2.5 Coder: https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B
6. Tauri documentation: https://v2.tauri.app/
7. HuggingFace tokenizers: https://docs.rs/tokenizers/

---

## Document History

| Version | Date          | Author      | Changes     |
| ------- | ------------- | ----------- | ----------- |
| 1.0     | December 2025 | SmolPC Team | Initial PRD |

---

_This document is the single source of truth for SmolPC Code Helper development. All implementation decisions should reference this PRD. Updates to requirements must be reflected here._
