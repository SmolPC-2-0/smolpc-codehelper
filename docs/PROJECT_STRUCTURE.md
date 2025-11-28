# SmolPC Code Helper - Project Structure

## Overview
```
smolpc-codehelper/
├── 📦 Frontend (Svelte 5 + TypeScript)
├── 🦀 Backend (Rust + Tauri 2.6.2)
├── 📚 Documentation
└── ⚙️ Configuration
```

---

## Detailed Structure

```
smolpc-codehelper/
│
├── src/                                    # 📦 FRONTEND (Svelte 5 + TypeScript)
│   ├── main.ts                            # Application entry point
│   ├── App.svelte                         # Root component & routing
│   │
│   └── lib/
│       ├── components/                    # UI Components
│       │   ├── ChatInput.svelte          # User message input
│       │   ├── ChatMessage.svelte        # Message display with markdown
│       │   ├── Sidebar.svelte            # Chat history navigation
│       │   ├── ModelSelector.svelte      # Ollama model picker
│       │   ├── ContextToggle.svelte      # Enable/disable context
│       │   ├── QuickExamples.svelte      # Predefined prompts
│       │   ├── HardwarePanel.svelte      # System specs display
│       │   ├── HardwareIndicator.svelte  # Status icon
│       │   ├── BenchmarkPanel.svelte     # Performance testing UI
│       │   ├── StatusIndicator.svelte    # Ollama connection status
│       │   │
│       │   └── ui/                       # Reusable UI primitives
│       │       ├── button/
│       │       ├── card/
│       │       └── input/
│       │
│       ├── stores/                       # State Management (Svelte 5 Runes)
│       │   ├── chats.svelte.ts          # Chat history & active chat
│       │   ├── ollama.svelte.ts         # Model list & connection
│       │   ├── hardware.svelte.ts       # Hardware detection state
│       │   ├── benchmark.svelte.ts      # Benchmark results
│       │   └── settings.svelte.ts       # User preferences
│       │
│       ├── types/                        # TypeScript Interfaces
│       │   ├── chat.ts                  # Chat, Message types
│       │   ├── ollama.ts                # Model, Response types
│       │   ├── hardware.ts              # HardwareInfo (MUST match Rust)
│       │   ├── settings.ts              # Settings type
│       │   └── examples.ts              # Quick example types
│       │
│       └── utils/                        # Utility Functions
│           ├── markdown.ts              # Markdown rendering (marked)
│           ├── storage.ts               # localStorage helpers
│           ├── date.ts                  # Timestamp formatting
│           └── utils.ts                 # General utilities
│
├── src-tauri/                            # 🦀 BACKEND (Rust + Tauri)
│   ├── src/
│   │   ├── main.rs                      # Binary entry (calls lib::run())
│   │   ├── lib.rs                       # Tauri setup, command registration
│   │   │
│   │   ├── commands/                    # Tauri IPC Commands
│   │   │   ├── mod.rs                  # Module exports
│   │   │   ├── ollama.rs               # Ollama API integration
│   │   │   │                           # - generate_stream()
│   │   │   │                           # - list_models()
│   │   │   │                           # - check_connection()
│   │   │   ├── hardware.rs             # Hardware detection
│   │   │   │                           # - detect_hardware()
│   │   │   │                           # - get_cached_hardware()
│   │   │   ├── benchmark.rs            # Benchmarking system
│   │   │   │                           # - run_benchmark()
│   │   │   │                           # - export_results()
│   │   │   ├── default.rs              # File I/O operations
│   │   │   │                           # - read_file()
│   │   │   │                           # - write_file()
│   │   │   └── errors.rs               # Error type definitions
│   │   │
│   │   ├── hardware/                    # Hardware Detection Module
│   │   │   ├── mod.rs                  # Module exports
│   │   │   ├── detector.rs             # Detection logic (hardware-query)
│   │   │   │                           # - detect_all()
│   │   │   │                           # - detect_cpu()
│   │   │   │                           # - detect_gpu()
│   │   │   │                           # - detect_memory()
│   │   │   │                           # - detect_npu()
│   │   │   ├── types.rs                # HardwareInfo structs (MUST match TS)
│   │   │   └── errors.rs               # Hardware-specific errors
│   │   │
│   │   ├── benchmark/                   # Benchmarking System
│   │   │   ├── mod.rs                  # Module exports
│   │   │   ├── runner.rs               # Benchmark orchestration
│   │   │   ├── metrics.rs              # Performance calculations
│   │   │   ├── sampling.rs             # Real-time metrics collection
│   │   │   ├── process.rs              # Benchmark task processing
│   │   │   ├── export.rs               # Results export (JSON/CSV)
│   │   │   └── test_suite.rs           # Unit tests
│   │   │
│   │   └── security/                    # Security Module (NEW)
│   │       ├── mod.rs                  # Security exports
│   │       └── tests.rs                # Security tests
│   │
│   ├── Cargo.toml                       # Rust dependencies
│   ├── build.rs                         # Build-time code generation
│   ├── tauri.conf.json                  # Tauri app configuration
│   │
│   ├── capabilities/
│   │   └── default.json                 # App permissions (IPC, fs, etc.)
│   │
│   ├── icons/                           # App icons (multi-platform)
│   │   ├── SmolPC.ico                  # Windows
│   │   ├── SmolPC.icns                 # macOS
│   │   └── *.png                       # Linux, various sizes
│   │
│   └── benchmarks/
│       └── README.md                    # Benchmark documentation
│
├── docs/                                 # 📚 DOCUMENTATION
│   ├── module_documentation/
│   │   ├── frontend_architecture.md    # Frontend design patterns
│   │   └── benchmark_comprehensive.md  # Benchmarking deep dive
│   │
│   └── .local_docs/                    # Development notes
│       ├── phase-2-llama-cpp-integration.md
│       ├── SECURITY_FIXES_IMPLEMENTATION.md
│       └── [various review/prep docs]
│
├── .github/                             # GitHub Configuration
│   └── workflows/
│       ├── release.yml                 # Automated builds
│       ├── claude-code-review.yml      # AI code review
│       └── claude.yml                  # Claude integration
│
├── .claude/                             # Claude Code Configuration
│   ├── agents/                         # Custom agents
│   │   ├── code-reviewer.md
│   │   ├── rust-pro.md
│   │   └── [other agents]
│   └── settings.local.json             # Local Claude settings
│
├── .vscode/                             # VS Code Configuration
│   ├── extensions.json                 # Recommended extensions
│   └── settings.json                   # Editor settings
│
├── package.json                         # ⚙️ CONFIGURATION
├── package-lock.json                    # Node dependencies lockfile
├── tsconfig.json                        # TypeScript config
├── vite.config.ts                       # Vite build config
├── tailwind.config.ts                   # Tailwind CSS 4 config
├── eslint.config.js                     # ESLint config
├── components.json                      # shadcn-svelte config
├── .prettierrc                          # Prettier formatting
│
├── CLAUDE.md                            # Project instructions for AI
├── README.md                            # User-facing documentation
├── CHANGES.md                           # Changelog
└── LICENSE                              # MIT License

```

---

## Technology Stack

### Frontend
- **Framework**: Svelte 5 (with Runes: `$state`, `$derived`, `$effect`)
- **Language**: TypeScript
- **Build Tool**: Vite 6.3.5
- **Styling**: Tailwind CSS 4 + shadcn-svelte
- **Markdown**: marked.js (code rendering)

### Backend
- **Framework**: Tauri 2.6.2
- **Language**: Rust (Edition 2021)
- **Async Runtime**: Tokio
- **HTTP Client**: reqwest 0.12 (connection pooling)
- **Hardware Detection**: hardware-query 0.2.1
- **Serialization**: serde 1.0

### External Dependencies
- **AI Models**: Ollama (localhost:11434)
  - Qwen 2.5 Coder 7B
  - DeepSeek Coder 6.7B

---

## Data Flow Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      FRONTEND (Svelte 5)                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  Components  │→→│    Stores    │→→│   Tauri IPC  │      │
│  │  (.svelte)   │  │ (.svelte.ts) │  │   (invoke)   │      │
│  └──────────────┘  └──────────────┘  └───────┬──────┘      │
└────────────────────────────────────────────────│────────────┘
                                                 │
                                    ╔════════════▼═══════════╗
                                    ║   Tauri IPC Bridge     ║
                                    ║  (Type-Safe Commands)  ║
                                    ╚════════════╤═══════════╝
                                                 │
┌─────────────────────────────────────────────────────────────┐
│                     BACKEND (Rust/Tauri)                    │
│  ┌───────────────┐  ┌──────────────┐  ┌─────────────┐      │
│  │   Commands    │←─│    State     │  │  Ollama API │      │
│  │ (IPC Handlers)│  │ (Arc/Mutex)  │  │  (HTTP/SSE) │      │
│  └───────┬───────┘  └──────────────┘  └─────────────┘      │
│          │                                                   │
│  ┌───────▼────────────────────────────────────────┐         │
│  │         Core Modules                           │         │
│  │  - hardware::detector (HW detection)           │         │
│  │  - benchmark::runner (Performance tests)       │         │
│  │  - security (Input validation, rate limiting)  │         │
│  └────────────────────────────────────────────────┘         │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
              ┌──────────────────────┐
              │   System Resources   │
              │  - Ollama (AI)       │
              │  - Hardware APIs     │
              │  - File System       │
              └──────────────────────┘
```

---

## Key Communication Patterns

### 1. Standard Request-Response
```
Frontend              Backend
   │                     │
   │── invoke('cmd') ───→│
   │                     │ (async processing)
   │←──── Result ────────│
   │                     │
```

### 2. Streaming Events (Ollama Chat)
```
Frontend              Backend              Ollama
   │                     │                   │
   │── generate_stream() │                   │
   │                     │─── HTTP POST ────→│
   │                     │                   │
   │                     │←── SSE chunk 1 ───│
   │←─ emit("chunk") ────│                   │
   │                     │                   │
   │                     │←── SSE chunk 2 ───│
   │←─ emit("chunk") ────│                   │
   │                     │                   │
```

### 3. Cached Hardware Detection
```
Frontend              Backend              Hardware
   │                     │                   │
   │── detect() ─────────→│                   │
   │                     │ (check cache)     │
   │                     │   ├─ MISS ────────→│
   │                     │   │               │
   │                     │   │←─ CPU info ───│
   │                     │   │←─ GPU info ───│
   │                     │   │←─ RAM info ───│
   │                     │   │               │
   │                     │   └─ (cache it)   │
   │←──── HardwareInfo ──│                   │
   │                     │                   │
   │── getCached() ──────→│                   │
   │                     │ (check cache)     │
   │                     │   └─ HIT          │
   │←──── HardwareInfo ──│                   │
   │                     │                   │
```

---

## Type Synchronization (Critical!)

**Rust ↔ TypeScript types MUST match exactly for serialization:**

| Rust Type | Location | TypeScript Type | Location |
|-----------|----------|-----------------|----------|
| `HardwareInfo` | `src-tauri/src/hardware/types.rs` | `HardwareInfo` | `src/lib/types/hardware.ts` |
| `CPUInfo` | `src-tauri/src/hardware/types.rs` | `CPUInfo` | `src/lib/types/hardware.ts` |
| `GPUInfo` | `src-tauri/src/hardware/types.rs` | `GPUInfo` | `src/lib/types/hardware.ts` |
| `OllamaModel` | `src-tauri/src/commands/ollama.rs` | `OllamaModel` | `src/lib/types/ollama.ts` |
| `BenchmarkResult` | `src-tauri/src/benchmark/types.rs` | `BenchmarkResult` | `src/lib/types/benchmark.ts` |

**Mapping Rules:**
- Rust `Option<T>` = TypeScript `T | null`
- Rust `Vec<T>` = TypeScript `T[]`
- Rust `String` = TypeScript `string`
- Rust `u32/u64` = TypeScript `number`
- Rust `bool` = TypeScript `boolean`

---

## Build Artifacts (Ignored in Git)

```
src-tauri/target/          # Rust compilation output
  ├── debug/               # Development builds
  ├── release/             # Production builds
  └── rust-analyzer/       # IDE analysis cache

node_modules/              # Node.js dependencies

dist/                      # Frontend build output
```

---

## Entry Points

### Development
```bash
npm run tauri dev
# Starts: Vite dev server (frontend) + Tauri (backend)
# Entry: src/main.ts → src-tauri/src/main.rs
```

### Production Build
```bash
npm run tauri build
# Output: src-tauri/target/release/bundle/
# Platforms: Windows (.msi), macOS (.dmg), Linux (.deb/.AppImage)
```

---

## Security Architecture

### Implemented
- Localhost-only Ollama URL validation
- HTTP connection pooling (resource exhaustion prevention)
- Event listener cleanup (memory leak prevention)
- CORS restrictions

### Planned (Phase 2)
- Request timeouts
- Rate limiting
- Input size validation
- DOMPurify for markdown XSS prevention

---

## Performance Optimizations

### Frontend
- Svelte 5 fine-grained reactivity (minimal re-renders)
- Lazy component loading
- Virtual scrolling for long chat histories (planned)
- Debounced user input

### Backend
- Tokio async runtime (non-blocking I/O)
- HTTP connection pooling (reuse connections)
- Hardware detection caching (OnceCell)
- Zero-copy streaming (Ollama responses)

---

## Development Workflow

1. **Frontend Changes**: Edit `.svelte`/`.ts` → Vite hot reloads
2. **Backend Changes**: Edit `.rs` → Restart `npm run tauri dev`
3. **Type Changes**: Update **BOTH** Rust + TypeScript interfaces
4. **Testing**: Manual testing in dev mode (limited test coverage)

---

**Version**: 2.2.0
**Last Updated**: January 2025
**Maintainer**: SmolPC Team
