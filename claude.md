# SmolPC Code Helper

## Project Overview

SmolPC Code Helper is an offline AI-powered coding assistant designed specifically for secondary school students (ages 11-18). Built with Svelte 5, Tauri 2, and Rust, it provides a modern, professional desktop application for learning programming with AI assistance that runs completely offline after initial setup.

**Current Version**: 2.1.0 (Production-Grade Benchmark System)

## Technology Stack

### Frontend
- **Framework**: Svelte 5 (with runes mode)
- **Build Tool**: Vite 6
- **Styling**: Tailwind CSS 4
- **UI Components**: shadcn-svelte
- **Icons**: Lucide Svelte
- **Language**: TypeScript

### Backend
- **Desktop Framework**: Tauri 2.6
- **Language**: Rust
- **HTTP Client**: reqwest (with streaming support)
- **Async Runtime**: tokio
- **System Monitoring**: sysinfo (process-specific resource tracking)
- **File Dialogs**: rfd

### AI Integration
- **Current Runtime**: Ollama (local, offline, HTTP API)
- **Primary Model**: Qwen 2.5 Coder (7B)
- **Secondary Model**: DeepSeek Coder (6.7B)
- **API**: HTTP streaming to localhost:11434
- **Future**: In-process llama.cpp integration (next optimization phase)

## Architecture

### Project Structure

```
smolpc-codehelper/
├── src/                                  # Frontend (Svelte 5 + TypeScript)
│   ├── App.svelte                       # Main application component
│   ├── main.ts                          # Entry point
│   ├── lib/
│   │   ├── components/                  # Reusable Svelte components
│   │   │   ├── Sidebar.svelte          # Chat list with time grouping
│   │   │   ├── ChatMessage.svelte      # Message display with markdown
│   │   │   ├── ChatInput.svelte        # Auto-resize input textarea
│   │   │   ├── StatusIndicator.svelte  # Ollama connection status
│   │   │   ├── ModelSelector.svelte    # AI model dropdown
│   │   │   ├── ContextToggle.svelte    # Conversation context toggle
│   │   │   ├── QuickExamples.svelte    # Preset prompt templates
│   │   │   └── ui/                     # shadcn-svelte base components
│   │   ├── stores/                      # Svelte 5 state management
│   │   │   ├── chats.svelte.ts         # Multi-chat state & persistence
│   │   │   ├── settings.svelte.ts      # App settings & preferences
│   │   │   └── ollama.svelte.ts        # Ollama connection state
│   │   ├── types/                       # TypeScript type definitions
│   │   │   ├── chat.ts                 # Chat & message types
│   │   │   ├── settings.ts             # Settings & model types
│   │   │   ├── ollama.ts               # Ollama API types
│   │   │   └── examples.ts             # Quick example types
│   │   └── utils/                       # Utility functions
│   │       ├── markdown.ts             # Custom markdown renderer
│   │       ├── storage.ts              # localStorage helpers
│   │       └── date.ts                 # Date formatting & grouping
│   └── styles.css                       # Global styles
│
├── src-tauri/                           # Rust Backend
│   ├── src/
│   │   ├── lib.rs                      # Main application setup
│   │   ├── main.rs                     # Entry point
│   │   ├── commands.rs                 # Command module aggregator
│   │   ├── commands/                   # Tauri command handlers
│   │   │   ├── mod.rs                  # Module exports
│   │   │   ├── errors.rs               # Error handling
│   │   │   ├── ollama.rs               # Ollama integration
│   │   │   ├── default.rs              # File I/O operations
│   │   │   └── benchmark.rs            # Benchmark Tauri commands
│   │   └── benchmark/                  # Benchmark System (v2.1.0)
│   │       ├── mod.rs                  # Module exports
│   │       ├── runner.rs               # Production-grade data collection
│   │       ├── metrics.rs              # Benchmark data structures
│   │       ├── test_suite.rs           # Standardized test prompts
│   │       └── export.rs               # CSV export functionality
│   ├── benchmarks/                     # Benchmark Results Storage
│   │   ├── README.md                   # Methodology documentation
│   │   └── *.csv                       # Benchmark data files
│   ├── Cargo.toml                      # Rust dependencies
│   ├── tauri.conf.json                 # Tauri configuration
│   └── capabilities/                   # Security permissions
│       └── default.json                # Permission definitions
│
├── .github/
│   └── workflows/                      # GitHub Actions
│       ├── claude-code-review.yml      # Automated PR review
│       ├── claude.yml                  # Claude agent workflow
│       ├── release.yml                 # Release automation
│       └── test-build.yml              # Build verification
│
├── package.json                         # Node dependencies
├── vite.config.ts                       # Vite configuration
├── svelte.config.js                     # Svelte preprocessor config
├── tailwind.config.js                   # Tailwind configuration
├── tsconfig.json                        # TypeScript configuration
├── CHANGES.md                           # Version changelog
├── PR_SUMMARY.md                        # Latest PR documentation
├── README.md                            # User documentation
└── claude.md                            # This file - workspace context
```

## Core Features

### 1. Multi-Chat Management
- **Multiple Conversations**: Create and manage multiple independent chat sessions
- **Automatic Titles**: First message automatically becomes chat title (truncated)
- **Time-based Organization**: Chats grouped into Today, Yesterday, Last 7 Days, Older
- **Persistent Storage**: All chats automatically saved to localStorage
- **Chat Switching**: Seamlessly switch between different conversations
- **Chat Deletion**: Remove unwanted conversations with confirmation

### 2. AI Code Generation
- **Streaming Responses**: Real-time token-by-token generation for better UX
- **Context Management**: Toggle conversation memory on/off per chat
- **Model Selection**: Choose between Qwen 2.5 Coder and DeepSeek Coder
- **Student-Friendly Prompts**: System prompt optimized for ages 11-18
- **Code Highlighting**: Custom syntax highlighting for common languages
- **Markdown Rendering**: Full markdown support with code blocks

### 3. Code Actions
- **Copy to Clipboard**: One-click copy for code blocks
- **Save to File**: Native file dialog with format filters
- **Multiple Code Blocks**: Handle multiple code snippets per response
- **Visual Feedback**: Copy confirmation with check icon

### 4. Quick Examples
- **Preset Prompts**: Calculator, Loops, Website, File I/O, Sorting, Debugging
- **Category Organization**: Basics, Web, Algorithms, Debugging
- **One-Click Usage**: Click to instantly send example prompt
- **Collapsible**: Hide/show on demand

### 5. Offline Operation
- **No Internet Required**: Runs completely offline after setup
- **Local AI Models**: Ollama runs on localhost
- **Bundled Dependencies**: All frontend libs bundled (no CDN)
- **Local Storage**: Persistent data in browser localStorage

### 6. Production-Grade Benchmarking (v2.1.0)
- **Accuracy-First Design**: Suitable for academic research reports
- **Process-Specific Monitoring**: Track Ollama inference process resources
- **Native Metrics**: Token counts and timing from Ollama metadata (no estimation)
- **Statistical Rigor**: Median calculations for outlier resistance
- **CSV Export**: Import into Excel, Google Sheets, or analysis tools
- **Standardized Test Suite**: Consistent prompts across short/medium/long/follow-up categories

## Benchmark System (v2.1.0)

### Purpose
Measure AI model performance with production-grade accuracy for:
- Academic research and reports
- Performance regression detection
- Model comparison across optimization phases
- Resource utilization analysis

### Architecture

#### Core Modules

**`benchmark/runner.rs`**: Production-grade data collection engine
- Model warmup to eliminate first-call latency
- Process-specific resource monitoring (CPU, memory)
- 50ms sampling intervals for rigorous measurements
- CPU baseline establishment (200ms required by sysinfo crate)
- Non-streaming API calls for complete Ollama metadata access
- Median calculations for statistical robustness

**`benchmark/metrics.rs`**: Data structures and calculations
- BenchmarkMetrics: Individual test results
- BenchmarkResults: Full suite aggregation
- Summary statistics: Per-category averages
- Timestamp generation for test identification

**`benchmark/test_suite.rs`**: Standardized test prompts
- 12 tests across 4 categories: short/medium/long/follow-up
- Student-focused programming prompts
- Consistent workload for reliable comparisons
- Context-aware follow-up tests

**`benchmark/export.rs`**: CSV export functionality
- Three-section format: data, summary, metadata
- Excel/Google Sheets compatible
- Versioned output files (prefix + timestamp)

**`commands/benchmark.rs`**: Tauri command interface
- `run_benchmark_suite()`: Frontend trigger
- Progress events for UI updates
- Error propagation to frontend

### Measurement Methodology

#### Token Metrics
- **Source**: Ollama native metadata (`eval_count`)
- **Accuracy**: No estimation or approximation
- **Failure Mode**: Tests fail if metadata unavailable

#### Timing Metrics
- **Source**: Ollama nanosecond-precision timing
- `first_token_ms`: From `prompt_eval_duration`
- `total_time_ms`: From `total_duration`
- `tokens_per_sec`: Calculated from `eval_count` / `eval_duration`
- `avg_token_ms`: Calculated from `eval_duration` / `eval_count`

#### Resource Monitoring
- **Warmup Phase**: Load model, identify Ollama PID, eliminate cold start
- **Process-Specific**: Monitor Ollama inference process only
- **CPU Baseline**: 200ms delay between refresh cycles (sysinfo requirement)
- **Sampling**: 50ms intervals during inference
- **Memory Metrics**: Process-specific before/during(median)/peak/after
- **Statistical Method**: Median for memory_during (outlier-resistant)

### Known Limitations

#### CPU Measurement Undercounting
CPU shows ~4-16% instead of expected 50-100% due to HTTP API architecture:
- **Ollama process**: ~16% CPU, ~85% GPU (GPU-accelerated inference)
- **Code helper process**: ~40% CPU (HTTP client overhead, JSON parsing, async runtime)
- **What we measure**: Ollama process only

**Current Status**: Consistent measurements suitable for relative comparisons
**Resolution**: Planned in-process llama.cpp integration will eliminate HTTP overhead

#### GPU Metrics Not Captured
Currently no GPU utilization tracking. For GPU-accelerated inference, low CPU usage is expected and legitimate.

### CSV Output Format

```csv
# DATA SECTION
timestamp,iteration,category,model,first_token_ms,total_time_ms,tokens_per_sec,avg_token_ms,memory_before_mb,memory_peak_mb,cpu_percent,response_tokens,prompt
...

# SUMMARY SECTION
SUMMARY,avg_first_token_ms,avg_tokens_per_sec,avg_total_time_ms,avg_memory_mb,avg_cpu_percent,test_count
...

# METADATA SECTION
Total Duration,<seconds>
Benchmark Timestamp,<ISO8601>
Total Tests,<count>
```

### Usage

```bash
# Run from Tauri frontend (future UI integration)
# Or trigger via command handlers

# Output: src-tauri/benchmarks/baseline-<timestamp>.csv
```

## State Management (Svelte 5 Runes)

### Chats Store (`chats.svelte.ts`)
- **Purpose**: Manage multiple chat sessions and messages
- **State**:
  - `chats`: Array of all chat sessions
  - `currentChatId`: ID of active chat
  - `currentChat`: Derived current chat object
  - `sortedChats`: Chats sorted by last update
- **Actions**:
  - `createChat()`: Create new chat
  - `setCurrentChat()`: Switch active chat
  - `addMessage()`: Add message to chat
  - `updateMessage()`: Update message content/status
  - `deleteChat()`: Remove chat
  - `updateChatTitle()`: Rename chat
  - `persist()`: Save to localStorage

### Settings Store (`settings.svelte.ts`)
- **Purpose**: App preferences and configuration
- **State**:
  - `selectedModel`: Active AI model
  - `contextEnabled`: Conversation memory toggle
  - `temperature`: Generation randomness (0-1)
  - `theme`: UI theme (light/dark/system)
- **Actions**:
  - `setModel()`: Change AI model
  - `toggleContext()`: Toggle conversation memory
  - `setTemperature()`: Adjust creativity
  - `resetToDefaults()`: Restore default settings

### Ollama Store (`ollama.svelte.ts`)
- **Purpose**: Ollama server connection management
- **State**:
  - `status`: Connection status object
  - `availableModels`: List of downloaded models
  - `isConnected`: Derived connection state
- **Actions**:
  - `checkConnection()`: Verify Ollama is running
  - `fetchModels()`: Get available models
  - `setError()`: Handle connection errors

## Backend (Rust/Tauri)

### Ollama Integration (`ollama.rs`)

#### System Prompt
Optimized for secondary school students with:
- Simple, encouraging language
- Step-by-step breakdowns
- Helpful code comments
- Patient and supportive tone
- Level-appropriate explanations

#### Commands

**`check_ollama() -> bool`**
- Verifies Ollama server is running
- Pings `http://localhost:11434/api/tags`
- Returns connection status

**`get_ollama_models() -> Vec<String>`**
- Fetches list of downloaded models
- Returns model names from Ollama

**`generate_stream(prompt, model, context) -> ()`**
- Streams AI response in real-time
- Builds message array with system prompt
- Includes conversation context if enabled
- Emits events: `ollama_chunk`, `ollama_done`, `ollama_error`

### File Operations (`default.rs`)

**`read(path) -> String`**
- Read file contents

**`write(path, contents) -> ()`**
- Write file contents

**`save_code(code) -> ()`**
- Opens native file save dialog
- Filters: Python, JavaScript, TypeScript, Rust, HTML, CSS, Text
- Saves code to selected file

### Benchmark Operations (`benchmark.rs`)

**`run_benchmark_suite(model, iterations) -> BenchmarkResults`**
- Executes full benchmark suite
- Emits progress events for UI updates
- Returns structured results
- Automatically exports to CSV

## Development

### Prerequisites
- Node.js 18+
- Rust 1.70+
- Ollama installed and running
- Qwen 2.5 Coder or DeepSeek Coder models pulled

### Setup
```bash
# Install frontend dependencies
npm install

# Run development mode
npm run dev

# Build for production
npm run build

# Run Tauri app
npm run tauri dev

# Build Tauri app
npm run tauri build
```

### Code Quality
```bash
# Type checking
npm run check

# Linting
npm run lint

# Formatting
npm run format

# Rust checks
cd src-tauri
cargo check
cargo clippy
cargo fmt
```

### Running Benchmarks
```bash
# Currently integrated into Tauri commands
# Future: UI panel for benchmark execution
# Manual: Trigger via frontend or command handlers
```

## Key Design Decisions

### 1. Svelte 5 Runes over Stores
- **Rationale**: Runes provide simpler, more performant reactivity
- **Implementation**: Custom store objects with runes for backward compatibility
- **Benefits**: Better TypeScript support, less boilerplate

### 2. Streaming for UI, Non-Streaming for Benchmarks
- **UI**: Streaming for real-time feedback and better UX
- **Benchmarks**: Non-streaming for complete Ollama metadata access
- **Trade-off**: Accuracy over real-time updates in benchmark context

### 3. LocalStorage Persistence
- **Rationale**: Simple, reliable, no backend required
- **Auto-save**: Every state change triggers persist
- **Benefits**: Instant load times, no database needed

### 4. Process-Specific Monitoring
- **Rationale**: Accurate measurements isolated from system noise
- **Implementation**: Warmup phase identifies Ollama PID
- **Benefits**: Reliable data regardless of background applications

### 5. TypeScript Throughout
- **Rationale**: Type safety and better DX
- **Coverage**: Full type coverage across frontend
- **Benefits**: Catch errors early, better autocomplete

## Performance Optimizations

### Frontend
- **Derived State**: Computed values cached automatically
- **Auto-scrolling**: Debounced for smooth experience
- **Markdown Rendering**: Only on message completion
- **Event Cleanup**: Proper listener disposal

### Backend
- **Streaming**: Reduces memory footprint for UI
- **Connection Pooling**: Reuse HTTP connections
- **Async/Await**: Non-blocking I/O operations
- **Resource Sampling**: Efficient 50ms intervals with minimal overhead

## Security Considerations

### XSS Prevention
- **HTML Escaping**: All user content escaped before rendering
- **Markdown Sanitization**: Code blocks safely rendered
- **No Eval**: No dynamic code execution

### File System
- **User Consent**: File dialogs for all file operations
- **No Auto-saves**: User explicitly chooses save locations
- **Benchmark Data**: Isolated to benchmarks/ directory

### Tauri Permissions
- **Minimal Scope**: Only required capabilities enabled
- **File Access**: Limited to user-selected files
- **Network**: Only localhost Ollama connection

## Current Development Phase: Optimization

### Completed (v2.1.0)
- ✅ Production-grade benchmark system
- ✅ Accurate process-specific monitoring
- ✅ Statistical rigor (median, CPU baseline)
- ✅ Comprehensive documentation

### Next Phase: llama.cpp Integration
**Goal**: Eliminate HTTP API overhead, improve performance

**Benefits**:
- Remove 40% CPU overhead from HTTP client
- Simplify resource monitoring (single process)
- Faster inference (no network layer)
- Better resource measurements (capture total CPU)
- Enable GPU metrics

**Implementation**:
- Replace Ollama HTTP calls with llama.cpp bindings
- Update benchmark system for in-process monitoring
- Maintain API compatibility for frontend

### Future Enhancements
1. **Export/Import**: Backup and restore chat history
2. **Search**: Find messages across all chats
3. **Code Execution**: Run code snippets safely
4. **File Context**: Analyze user's code files
5. **Settings Panel**: Advanced configuration UI
6. **Themes**: Light/dark mode with custom colors
7. **Keyboard Shortcuts**: Power user efficiency
8. **Benchmark UI**: Frontend panel for benchmark execution
9. **GPU Metrics**: Track GPU utilization during inference

## Troubleshooting

### Ollama Not Connected
- Ensure Ollama is installed and running
- Check `http://localhost:11434` is accessible
- Verify models are pulled: `ollama pull qwen2.5-coder:7b`

### Messages Not Persisting
- Check browser localStorage quota
- Verify no browser extension blocking storage
- Clear and reload if corrupted

### Slow Response Times
- Check Ollama model is fully loaded
- Reduce context window if using low-end hardware
- Consider smaller models (1.5B variants)

### Benchmark Issues
- Ensure Ollama process is running before benchmarks
- Check sufficient disk space in benchmarks/ directory
- Review benchmark README.md for methodology details
- CPU measurements ~4-16% are expected (HTTP architecture limitation)

## Contributing

### Code Style
- Use Prettier for formatting
- Follow TypeScript best practices
- Write descriptive commit messages
- Add comments for complex logic
- Document production-grade code with rustdoc

### Testing
- Test on multiple resolutions
- Verify offline functionality
- Check localStorage limits
- Run benchmarks before/after optimizations

### Benchmark Data
- Commit benchmark CSVs with meaningful prefixes (baseline, phase1, phase2, etc.)
- Document optimization changes in CHANGES.md
- Compare before/after results in PR descriptions

## Documentation

### Key Files
- **`README.md`**: User-facing documentation
- **`claude.md`**: This file - full workspace context for AI assistants
- **`CHANGES.md`**: Version changelog with detailed changes
- **`PR_SUMMARY.md`**: Latest PR documentation
- **`src-tauri/benchmarks/README.md`**: Benchmark methodology

### Keeping Updated
- Update CHANGES.md for each version
- Create PR_SUMMARY.md for significant changes
- Update this file when architecture changes
- Document known limitations and trade-offs

## License

MIT License - See LICENSE file for details

## Credits

- **UI Framework**: Svelte Team
- **Desktop Framework**: Tauri Team
- **AI Runtime**: Ollama Team (current), llama.cpp (planned)
- **Components**: shadcn-svelte
- **Icons**: Lucide Icons
- **System Monitoring**: sysinfo crate
