# SmolPC Code Helper - Claude Development Guide

**Version:** 2.2.0
**Last Updated:** January 2025

This file provides comprehensive context for AI assistants (like Claude) working on the SmolPC Code Helper codebase. It documents architecture, conventions, patterns, and development workflows.

---

## Project Overview

**Purpose:** An offline AI-powered coding assistant for secondary school students (ages 11-18). Built with Tauri + Svelte 5, powered by local Ollama models.

**Core Principles:**

- 100% offline operation (after initial setup)
- Privacy-first (no telemetry, no cloud)
- Budget-friendly (runs on older hardware)
- Educational focus (clear explanations, student-appropriate)
- Cross-platform (Windows, macOS, Linux)

**Current Focus (v2.2.0):** Hardware detection system complete. Next phase is llama.cpp integration with hardware-optimized compilation.

---

## Tech Stack

### Frontend

- **Framework:** Svelte 5 (with runes for reactivity)
- **Language:** TypeScript
- **Styling:** Tailwind CSS 4 (utility-first, no `@apply`)
- **UI Components:** shadcn-svelte
- **Build Tool:** Vite 6
- **State Management:** Svelte 5 runes (`$state`, `$derived`, `$effect`)

### Backend

- **Framework:** Tauri 2.6.2
- **Language:** Rust (edition 2021, rust version 1.77.2+)
- **HTTP Client:** reqwest 0.12 (with connection pooling)
- **Async Runtime:** Tokio 1.x
- **Serialization:** serde + serde_json
- **Hardware Detection:** hardware-query v0.2.1
- **Logging:** log + tauri-plugin-log

### AI Integration

- **Engine:** Ollama (local LLM runtime)
- **Default Models:** Qwen 2.5 Coder (7B), DeepSeek Coder (6.7B)
- **API:** Ollama HTTP API at `http://localhost:11434`

---

## Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                  Frontend (Svelte 5)                    │
│  ┌───────────┐  ┌──────────┐  ┌────────────────────┐   │
│  │ Components│  │  Stores  │  │  Types/Interfaces  │   │
│  │  (.svelte)│◄─┤(.svelte.ts)├─►│    (.ts)          │   │
│  └───────────┘  └──────────┘  └────────────────────┘   │
└──────────────────────┬──────────────────────────────────┘
                       │
                 Tauri IPC (JSON)
                       │
┌──────────────────────┴──────────────────────────────────┐
│                  Backend (Rust/Tauri)                   │
│  ┌──────────┐  ┌──────────┐  ┌─────────┐  ┌──────────┐ │
│  │ Commands │  │ Hardware │  │Benchmark│  │  Utils   │ │
│  │   (.rs)  │  │  Module  │  │ Module  │  │   (.rs)  │ │
│  └──────────┘  └──────────┘  └─────────┘  └──────────┘ │
└──────────────────────┬──────────────────────────────────┘
                       │
          ┌────────────┴─────────────┐
          │                          │
    Ollama HTTP API          Native OS APIs
   (localhost:11434)      (hardware detection)
```

### State Management Philosophy

**Svelte 5 Runes (Frontend):**

- Use `$state` for reactive state variables
- Use `$derived` for computed values
- Use `$effect` for side effects (sparingly)
- **Pattern:** Create stores in `src/lib/stores/*.svelte.ts` with exported objects containing state and methods
- **Example:**

  ```typescript
  export const hardwareStore = {
  	info: $state<HardwareInfo | null>(null),
  	isDetecting: $state(false),

  	async detect() {
  		this.isDetecting = true;
  		// ... detection logic
  		this.isDetecting = false;
  	}
  };
  ```

**Rust State (Backend):**

- Use Tauri's state management for shared resources
- Pattern: `Arc<Mutex<T>>` for thread-safe shared state
- Example: `HardwareCache`, connection pools
- Initialize in `.setup()` hook in `src-tauri/src/lib.rs`

### Data Flow

**User Action → Response:**

```
1. User clicks button (Component)
2. Component calls store method (Store)
3. Store invokes Tauri command via `@tauri-apps/api` (IPC)
4. Rust command handler processes request
5. Rust calls external API (Ollama) or OS API (hardware)
6. Response serialized to JSON
7. Store updates reactive state
8. Svelte reactivity triggers UI update
```

---

## Directory Structure

### Frontend (`src/`)

```
src/
├── App.svelte                 # Root component, routing logic
├── main.ts                    # Entry point, Tauri initialization
├── app.css                    # Global styles (Tailwind imports)
└── lib/
    ├── components/            # Svelte components
    │   ├── Sidebar.svelte     # Chat list sidebar
    │   ├── ChatMessage.svelte # Individual chat message
    │   ├── ChatInput.svelte   # Message input with streaming
    │   ├── HardwarePanel.svelte      # Full hardware info display
    │   ├── HardwareIndicator.svelte  # Status bar widget
    │   └── ui/                # shadcn-svelte components
    │       ├── button/
    │       ├── card/
    │       └── ...
    ├── stores/                # Svelte 5 state stores
    │   ├── chats.svelte.ts    # Chat state, message history
    │   ├── settings.svelte.ts # App settings (context, model)
    │   ├── ollama.svelte.ts   # Ollama connection status
    │   └── hardware.svelte.ts # Hardware detection state
    ├── types/                 # TypeScript type definitions
    │   ├── chat.ts            # Chat, Message interfaces
    │   ├── ollama.ts          # Ollama API types
    │   └── hardware.ts        # Hardware info types
    └── utils/                 # Utility functions
        └── cn.ts              # Class name merger (shadcn)
```

### Backend (`src-tauri/src/`)

```
src-tauri/src/
├── main.rs                    # Binary entry point (minimal)
├── lib.rs                     # Library entry point, Tauri setup
├── commands/                  # Tauri command handlers
│   ├── mod.rs                 # Module exports
│   ├── ollama.rs              # Ollama API integration
│   │   ├── stream_chat()      # Streaming chat endpoint
│   │   ├── list_models()      # Get available models
│   │   └── check_connection() # Health check
│   ├── hardware.rs            # Hardware detection commands
│   │   ├── detect_hardware()  # Trigger detection
│   │   └── get_cached_hardware() # Get cached results
│   └── errors.rs              # Error types and handling
├── hardware/                  # Hardware detection module
│   ├── mod.rs                 # Module exports
│   ├── types.rs               # HardwareInfo, CpuInfo, etc.
│   └── detector.rs            # Detection implementation
│       ├── detect_all()       # Main entry point
│       ├── convert_cpu_info() # CPU conversion
│       ├── convert_gpu_info() # GPU conversion
│       ├── convert_memory_info() # Memory conversion
│       ├── convert_storage_info() # Storage conversion
│       └── convert_npu_info() # NPU conversion
├── benchmark/                 # Benchmarking system
│   ├── mod.rs                 # Module exports
│   └── runner.rs              # llama.cpp benchmark execution
└── utils/                     # Utility functions
    └── mod.rs                 # Utilities (if needed)
```

### Configuration Files

```
.
├── package.json               # Node.js dependencies, scripts
├── tsconfig.json              # TypeScript configuration
├── vite.config.ts             # Vite build configuration
├── tailwind.config.ts         # Tailwind CSS configuration
├── src-tauri/
│   ├── Cargo.toml             # Rust dependencies
│   ├── tauri.conf.json        # Tauri app configuration
│   └── build.rs               # Rust build script
├── docs/
│   └── hardware-detection.md # Feature documentation
├── README.md                  # User-facing documentation
└── claude.md                  # This file
```

---

## Code Conventions

### TypeScript/Svelte Conventions

**File Naming:**

- Components: PascalCase (e.g., `ChatMessage.svelte`)
- Stores: camelCase with `.svelte.ts` (e.g., `chats.svelte.ts`)
- Types: camelCase with `.ts` (e.g., `hardware.ts`)
- Utilities: camelCase with `.ts` (e.g., `cn.ts`)

**Component Structure:**

```svelte
<script lang="ts">
	// 1. Imports
	import { onMount } from 'svelte';
	import { hardwareStore } from '$lib/stores/hardware.svelte';

	// 2. Props (with types)
	interface Props {
		title: string;
		optional?: boolean;
	}
	let { title, optional = false }: Props = $props();

	// 3. State
	let localState = $state(0);

	// 4. Derived state
	let computed = $derived(localState * 2);

	// 5. Effects
	$effect(() => {
		console.log('State changed:', localState);
	});

	// 6. Functions
	function handleClick() {
		localState++;
	}

	// 7. Lifecycle
	onMount(() => {
		// initialization
	});
</script>

<!-- 8. Template -->
<div class="container">
	<!-- markup -->
</div>

<!-- 9. Styles (if needed, prefer Tailwind) -->
<style>
	/* component-specific styles only */
</style>
```

**Store Pattern:**

```typescript
// src/lib/stores/example.svelte.ts
import { invoke } from '@tauri-apps/api/core';
import type { ExampleType } from '$lib/types/example';

let data = $state<ExampleType[]>([]);
let isLoading = $state(false);
let error = $state<string | null>(null);

export const exampleStore = {
	// Expose state as getters (for reactivity)
	get data() {
		return data;
	},
	get isLoading() {
		return isLoading;
	},
	get error() {
		return error;
	},

	// Methods
	async load() {
		isLoading = true;
		error = null;
		try {
			data = await invoke<ExampleType[]>('load_example');
		} catch (e) {
			error = e instanceof Error ? e.message : 'Unknown error';
		} finally {
			isLoading = false;
		}
	},

	clear() {
		data = [];
	}
};
```

**Naming Conventions:**

- Variables: camelCase
- Constants: SCREAMING_SNAKE_CASE
- Types/Interfaces: PascalCase
- Functions: camelCase (descriptive verbs: `handleClick`, `fetchData`)
- Boolean variables: Prefix with `is`, `has`, `should` (e.g., `isLoading`, `hasError`)

**Type Safety:**

- Always use TypeScript (no `any` unless absolutely necessary)
- Define interfaces for all data structures
- Use type imports: `import type { ... }`
- Prefer interfaces over types for objects

### Rust Conventions

**File Naming:**

- Modules: snake_case (e.g., `hardware_detection.rs`)
- Prefer `mod.rs` for module directories

**Code Structure:**

```rust
// 1. Module imports
use serde::{Deserialize, Serialize};
use tauri::State;

// 2. Type definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Example {
    pub field: String,
}

// 3. Implementation
impl Example {
    pub fn new(field: String) -> Self {
        Self { field }
    }
}

// 4. Tauri commands
#[tauri::command]
pub async fn example_command(state: State<'_, ExampleState>) -> Result<Example, String> {
    // implementation
    Ok(Example::new("value".to_string()))
}
```

**Naming Conventions:**

- Variables/functions: snake_case
- Types/structs/enums: PascalCase
- Constants: SCREAMING_SNAKE_CASE
- Modules: snake_case

**Error Handling:**

- Use `Result<T, String>` for Tauri commands (String for frontend display)
- Use `thiserror` for internal error types
- Log errors with `log::error!()`, `log::warn!()`
- Provide user-friendly error messages

**Tauri Commands:**

- Mark with `#[tauri::command]`
- Use `async` for I/O operations
- Return `Result<T, String>` for error handling
- Keep commands focused (single responsibility)

### CSS/Styling Conventions

**Tailwind CSS:**

- Use utility classes directly in components
- **DO NOT** use `@apply` (Tailwind 4 doesn't support it)
- Group utilities logically: layout → spacing → typography → colors → effects
- Use shadcn-svelte components for consistency
- Prefer `class:` directive for conditional classes in Svelte

**Example:**

```svelte
<div class="border-border bg-card flex items-center gap-2 rounded-lg border p-4 shadow-sm">
	<span class="text-foreground text-sm font-medium">Content</span>
</div>

<!-- Conditional classes -->
<button class:opacity-50={isDisabled} class="bg-primary text-primary-foreground rounded px-4 py-2">
	Click
</button>
```

**Color System (shadcn):**

- Use CSS variables: `bg-background`, `text-foreground`, `border-border`
- Primary: `bg-primary`, `text-primary-foreground`
- Muted: `bg-muted`, `text-muted-foreground`
- Card: `bg-card`, `text-card-foreground`

---

## Key Patterns and Practices

### 1. Tauri IPC Communication

**Frontend → Backend:**

```typescript
import { invoke } from '@tauri-apps/api/core';

// Simple invocation
const result = await invoke<ReturnType>('command_name', {
	arg1: value1,
	arg2: value2
});

// With error handling
try {
	const data = await invoke<HardwareInfo>('detect_hardware');
	hardwareStore.info = data;
} catch (error) {
	console.error('Detection failed:', error);
}
```

**Backend Command:**

```rust
#[tauri::command]
async fn command_name(arg1: String, arg2: u32) -> Result<ReturnType, String> {
    // implementation
    Ok(result)
}

// Register in lib.rs
.invoke_handler(tauri::generate_handler![
    commands::command_name,
])
```

### 2. State Management Patterns

**Frontend Store with Tauri:**

```typescript
let items = $state<Item[]>([]);
let isLoading = $state(false);

export const itemStore = {
	get items() {
		return items;
	},
	get isLoading() {
		return isLoading;
	},

	async load() {
		isLoading = true;
		try {
			items = await invoke<Item[]>('load_items');
		} finally {
			isLoading = false;
		}
	}
};
```

**Backend State:**

```rust
pub struct AppState {
    data: Arc<Mutex<HashMap<String, String>>>,
}

// In lib.rs setup
.manage(AppState {
    data: Arc::new(Mutex::new(HashMap::new())),
})

// In command
#[tauri::command]
async fn use_state(state: State<'_, AppState>) -> Result<(), String> {
    let mut data = state.data.lock().await;
    data.insert("key".to_string(), "value".to_string());
    Ok(())
}
```

### 3. Streaming Responses (Ollama)

**Backend:**

```rust
use futures_util::StreamExt;

#[tauri::command]
pub async fn stream_chat(
    window: tauri::Window,
    message: String,
) -> Result<(), String> {
    let stream = /* create HTTP stream */;

    tokio::pin!(stream);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;

        // Emit to frontend
        window.emit("chat_chunk", chunk)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}
```

**Frontend:**

```typescript
import { listen } from '@tauri-apps/api/event';

// Start streaming
await invoke('stream_chat', { message: 'Hello' });

// Listen for chunks
const unlisten = await listen<string>('chat_chunk', (event) => {
	console.log('Received:', event.payload);
});

// Clean up when done
unlisten();
```

### 4. Hardware Detection Pattern

**Always:**

- Detect on app startup (async, non-blocking)
- Cache results in memory (HardwareCache)
- Frontend requests cached data first
- If cache empty, trigger manual detection (race condition handling)

**Backend caching:**

```rust
pub struct HardwareCache {
    info: Arc<Mutex<Option<HardwareInfo>>>,
}

impl HardwareCache {
    pub async fn set(&self, info: HardwareInfo) {
        *self.info.lock().await = Some(info);
    }

    pub async fn get(&self) -> Option<HardwareInfo> {
        self.info.lock().await.clone()
    }
}
```

**Frontend auto-detection:**

```typescript
async getCached() {
    const cached = await invoke<HardwareInfo | null>('get_cached_hardware');
    if (cached) {
        this.info = cached;
    } else {
        // Backend still detecting, trigger manual detection
        await this.detect();
    }
}
```

---

## Hardware Detection System (v2.2.0)

### Overview

Comprehensive offline hardware profiling for intelligent AI optimization:

- **CPU**: Vendor, cores, frequency, cache, instruction sets (AVX2/AVX512/NEON/SVE)
- **GPU**: Name, VRAM, backend (Metal/DirectX/Vulkan/CUDA), CUDA compute capability
- **Memory**: Total/available RAM for model size selection
- **Storage**: Capacity, availability, type (SSD/HDD), device name
- **NPU**: Detection with confidence (Apple Neural Engine, Intel AI Boost, AMD Ryzen AI, Qualcomm Hexagon)

### Architecture

```
Frontend (hardware.svelte.ts)
    │
    ├─► detect() ──────► Tauri IPC ──► detect_hardware() command
    │                                        │
    └─► getCached() ───► Tauri IPC ──► get_cached_hardware()
                                             │
                                   HardwareCache (in-memory)
                                             │
                                   hardware::detect_all()
                                             │
                                   hardware_query crate
                                             │
                                      Native OS APIs
```

### Implementation Details

**Startup Detection:**

```rust
// src-tauri/src/lib.rs
.setup(|app| {
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        match hardware::detect_all().await {
            Ok(info) => {
                app_handle.state::<HardwareCache>().set(info).await;
            }
            Err(e) => {
                log::error!("Hardware detection failed: {}", e);
            }
        }
    });
    Ok(())
})
```

**Race Condition Handling:**

```typescript
// Frontend automatically triggers detection if cache is empty
async getCached() {
    const cached = await invoke<HardwareInfo | null>('get_cached_hardware');
    if (cached) {
        hardware = cached;
    } else {
        // Backend hasn't finished startup detection yet
        await this.detect();
    }
}
```

**Type Synchronization:**

- Rust structs in `src-tauri/src/hardware/types.rs`
- TypeScript interfaces in `src/lib/types/hardware.ts`
- Must match exactly for proper serialization

**Key Files:**

- `src-tauri/src/hardware/detector.rs` - Detection implementation
- `src-tauri/src/hardware/types.rs` - Rust type definitions
- `src-tauri/src/commands/hardware.rs` - Tauri commands and caching
- `src/lib/stores/hardware.svelte.ts` - Frontend state management
- `src/lib/types/hardware.ts` - TypeScript interfaces
- `src/lib/components/HardwarePanel.svelte` - Full hardware display
- `src/lib/components/HardwareIndicator.svelte` - Status bar widget

### Usage in Future Optimizations

```typescript
// Example: Model selection based on memory
function recommendModel(): string {
	const availableGB = hardwareStore.info?.memory.available_gb ?? 0;
	if (availableGB >= 20) return 'qwen2.5-coder:32b';
	if (availableGB >= 10) return 'qwen2.5-coder:14b';
	if (availableGB >= 6) return 'qwen2.5-coder:7b';
	return 'qwen2.5-coder:3b';
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

	return flags;
}
```

---

## Development Workflow

### Setting Up Development Environment

```bash
# 1. Clone repository
git clone https://github.com/SmolPC-2-0/smolpc-codehelper.git
cd smolpc-codehelper

# 2. Install dependencies
npm install

# 3. Start Ollama (separate terminal)
ollama serve

# 4. Download models
ollama pull qwen2.5-coder:7b

# 5. Run development server
npm run tauri dev
```

### Common Tasks

**Run Development Server:**

```bash
npm run tauri dev
# - Frontend: Hot reload on Svelte changes
# - Backend: Rebuild required for Rust changes
```

**Build Production:**

```bash
npm run tauri build
# Output: src-tauri/target/release/bundle/
```

**Format Code:**

```bash
# Frontend (TypeScript/Svelte)
npm run format

# Backend (Rust)
cd src-tauri
cargo fmt
```

**Lint Code:**

```bash
# Frontend
npm run lint

# Backend
cd src-tauri
cargo clippy
```

**Type Check:**

```bash
npm run check
```

**Clean Build:**

```bash
# Clean Rust artifacts
cd src-tauri
cargo clean
cd ..

# Clean node_modules (if needed)
rm -rf node_modules
npm install
```

### Git Workflow

**Branch Naming:**

- Features: `feature/description`
- Fixes: `fix/description`
- Claude work: `claude/description-sessionid`

**Commit Messages (Conventional Commits):**

```
feat: add hardware detection system
fix: resolve startup race condition
docs: update README with v2.2.0 changes
refactor: migrate to hardware-query crate
chore: update dependencies
```

---

## Important Files

### Configuration Files

**`src-tauri/tauri.conf.json`** - Tauri app configuration

- App identifier: `com.smolpc.codehelper`
- Window settings (dimensions, title, etc.)
- Security settings (allowlist)
- Bundle configuration (icons, version)

**`src-tauri/Cargo.toml`** - Rust dependencies

- Current version: 2.2.0
- Key dependencies: tauri, serde, reqwest, hardware-query, tokio

**`package.json`** - Node.js dependencies and scripts

- Scripts: `tauri dev`, `tauri build`
- Frontend dependencies: svelte, vite, tailwindcss

### Entry Points

**`src/main.ts`** - Frontend entry

- Initializes Svelte app
- Mounts to `#app` div

**`src-tauri/src/main.rs`** - Binary entry

- Calls `lib::run()`

**`src-tauri/src/lib.rs`** - Tauri setup

- Command registration
- State initialization
- Hardware detection on startup
- Window configuration

### Core Logic Files

**`src/lib/stores/chats.svelte.ts`** - Chat state management

- Manages all chats and messages
- Handles localStorage persistence
- Current chat selection

**`src-tauri/src/commands/ollama.rs`** - Ollama integration

- Streaming chat endpoint
- Model listing
- Connection checking
- HTTP client with connection pooling

**`src-tauri/src/hardware/detector.rs`** - Hardware detection

- Queries hardware-query crate
- Converts to internal types
- Handles platform differences

---

## Testing and Debugging

### Frontend Debugging

**DevTools:**

- Press `F12` in dev mode to open Chrome DevTools
- Check Console for errors
- Inspect Network tab for Tauri IPC calls

**Logging:**

```typescript
console.log('Debug message');
console.error('Error message');
console.warn('Warning message');
```

### Backend Debugging

**Logging:**

```rust
use log::{info, warn, error, debug};

info!("Info message");
warn!("Warning message");
error!("Error: {}", error);
debug!("Debug message (dev only)");
```

**Log Location:**

- macOS: `~/Library/Logs/com.smolpc.codehelper/`
- Windows: `%APPDATA%\com.smolpc.codehelper\logs\`
- Linux: `~/.local/share/com.smolpc.codehelper/logs/`

### Testing Hardware Detection

```typescript
// In browser console (DevTools)
await window.__TAURI__.invoke('detect_hardware');
await window.__TAURI__.invoke('get_cached_hardware');
```

---

## Security Considerations

### Implemented Security

**OLLAMA_URL Validation:**

- Only localhost URLs allowed
- Prevents data exfiltration to external servers

**HTTP Client Pooling:**

- Connection reuse prevents resource exhaustion

**Event Listener Cleanup:**

- Proper cleanup in `$effect` hooks
- Prevents memory leaks

### Future Security Improvements

**Needed:**

- [ ] Input sanitization (XSS prevention)
- [ ] Request timeouts
- [ ] Rate limiting
- [ ] Data size limits
- [ ] DOMPurify for markdown rendering

---

## Performance Considerations

### Frontend Performance

**Svelte 5 Optimizations:**

- Fine-grained reactivity (runes)
- Minimal re-renders
- Virtual DOM eliminated (compiled away)

**Best Practices:**

- Use `$derived` for computed values (not functions in template)
- Avoid `$effect` unless necessary (prefer reactive assignments)
- Use `$state` objects for related data

### Backend Performance

**Async Operations:**

- Use `async/await` for all I/O
- Tokio runtime handles concurrency
- Non-blocking startup detection

**Caching:**

- Hardware info cached in memory
- Prevents redundant expensive operations

**Connection Pooling:**

- HTTP client reuses connections
- Reduces overhead for Ollama API calls

---

## Future Development Roadmap

### Phase 2: Intelligent Optimization (Current)

**llama.cpp Integration:**

1. Download/build llama.cpp based on detected hardware
2. Generate CMake flags from CPU features (AVX2/AVX512/NEON)
3. Configure GPU layer offloading (Metal/CUDA/Vulkan)
4. Set optimal thread count based on CPU cores

**Model Management:**

1. Recommend models based on available RAM
2. Download manager with storage validation
3. Model format conversion (GGUF)
4. Quantization selection (Q4/Q5/Q8)

**Architecture Changes:**

- Add `model` module for model management
- Add `compiler` module for llama.cpp compilation
- Extend hardware detection with performance benchmarking
- Add persistent settings for user preferences

### Phase 3: Advanced Features

- Code execution sandbox
- Multi-model simultaneous generation
- Chat organization (folders, search)
- Export functionality (markdown, PDF)

---

## Key Decisions and Rationales

### Why Svelte 5 over React/Vue?

- Minimal boilerplate (less code = easier for students to understand)
- True reactivity (no virtual DOM overhead)
- Excellent TypeScript support
- Smaller bundle size
- Faster performance

### Why Tauri over Electron?

- Much smaller executables (~8MB vs 100MB+)
- Lower memory usage (no embedded Chromium)
- Better security model
- Rust backend (performance + safety)
- Modern architecture

### Why Ollama over cloud APIs?

- 100% offline operation (privacy + no API costs)
- Student data stays local (GDPR/FERPA compliant)
- Works without internet (schools, rural areas)
- No rate limits or usage caps
- Educational transparency

### Why hardware-query over multiple crates?

- Single unified API (simpler code)
- Cross-platform consistency
- Reduced dependency conflicts
- Actively maintained
- Comprehensive coverage (CPU, GPU, Memory, Storage, NPU)

### Why localStorage over IndexedDB/SQLite?

**Current (for chat persistence):**

- Simpler API for current needs
- No additional dependencies
- Sufficient for current data volume

**Future consideration:** May migrate to SQLite for:

- Better performance with large chat history
- Full-text search
- Data integrity
- Structured queries

---

## Contributing Guidelines for AI Assistants

### When Making Changes

**Always:**

1. Read relevant files before modifying
2. Maintain existing code style and patterns
3. Update types when changing data structures (both TS and Rust)
4. Test changes in dev mode before committing
5. Update documentation if adding features
6. Use conventional commit messages
7. Check for TypeScript/Rust compilation errors

**Never:**

1. Use `any` type in TypeScript
2. Use `@apply` in CSS (Tailwind 4 doesn't support it)
3. Commit untested code
4. Break existing functionality
5. Add dependencies without justification
6. Bypass security measures
7. Create features without user request

### Code Review Checklist

Before considering work complete:

- [ ] TypeScript compiles without errors (`npm run check`)
- [ ] Rust compiles without errors (`cargo build`)
- [ ] No console errors in browser DevTools
- [ ] Changes work in dev mode (`npm run tauri dev`)
- [ ] Types are synchronized (TS interfaces match Rust structs)
- [ ] Error handling is implemented
- [ ] Logging added for important operations
- [ ] Documentation updated if needed
- [ ] Git commit message follows conventions
- [ ] No unintentional files committed

### When Stuck or Uncertain

**Ask yourself:**

1. Does this align with project principles? (offline, privacy, educational)
2. Is there an existing pattern I should follow?
3. Will this break existing functionality?
4. Is this the simplest solution?
5. Does this need user approval?

**If yes to #5, stop and ask the user before proceeding.**

---

## Resources

### Documentation

- **Tauri:** https://v2.tauri.app/
- **Svelte 5:** https://svelte.dev/docs/svelte/overview
- **Svelte 5 Runes:** https://svelte.dev/docs/svelte/what-are-runes
- **Tailwind CSS 4:** https://tailwindcss.com/docs
- **shadcn-svelte:** https://shadcn-svelte.com/
- **hardware-query:** https://docs.rs/hardware-query/

### Project Documentation

- **README.md** - User-facing documentation
- **docs/hardware-detection.md** - Hardware detection feature guide
- **This file (claude.md)** - Developer/AI assistant guide

### Related Projects

- **Ollama:** https://ollama.com/
- **Qwen 2.5 Coder:** https://huggingface.co/Qwen/Qwen2.5-Coder-7B
- **DeepSeek Coder:** https://huggingface.co/deepseek-ai/deepseek-coder-6.7b-base

---

## Changelog

### v2.2.0 (January 2025)

- Added comprehensive hardware detection system
- CPU: vendor, cores, frequency, cache, instruction sets
- GPU: name, VRAM, backend, CUDA compute capability
- Memory: total/available RAM
- Storage: capacity, type (SSD/HDD)
- NPU: detection with confidence levels
- Fixed startup race condition
- Fixed NPU confidence badge display
- Migrated to hardware-query v0.2.1 crate

### v2.1.0 (December 2024)

- Added production-grade benchmarking system
- llama.cpp benchmark integration
- Result caching and persistence

### v2.0.0 (December 2024)

- Migrated to Svelte 5 with runes
- Background chat generation
- HTTP connection pooling
- Security improvements (URL validation)
- Memory leak fixes
- Tailwind 4 migration

---

**Last Updated:** January 2025
**Maintainer:** SmolPC Team
**AI Assistant Context:** This file provides comprehensive project context for AI-assisted development
