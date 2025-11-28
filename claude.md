# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## Project Overview

SmolPC Code Helper is an offline AI-powered coding assistant for secondary school students (ages 11-18). Built with **Tauri 2.6.2 + Svelte 5**, powered by local **Ollama** models. Runs 100% offline after initial setup.

**Key Principles:**

- **Offline-First**: No cloud, no telemetry, no API keys
- **Privacy-First**: Student data stays local (GDPR/FERPA compliant)
- **Educational**: Clear explanations, age-appropriate
- **Budget-Friendly**: Runs on older hardware
- **Cross-Platform**: Windows, macOS, Linux (x86/ARM)

---

## Development Commands

### Frontend (Svelte 5 + TypeScript)

```bash
# Development server with hot reload
npm run tauri dev

# Type checking
npm run check

# Format code
npm run format

# Lint
npm run lint

# Frontend-only dev server (for UI work without Tauri)
npm run dev
```

### Backend (Rust/Tauri)

```bash
# Check Rust compilation
cd src-tauri
cargo check

# Run clippy
cargo clippy

# Format Rust code
cargo fmt

# Clean build artifacts
cargo clean

# Build production bundle
npm run tauri build
# Output: src-tauri/target/release/bundle/
```

### Testing

**Note:** Test suite is minimal. Tests exist at:

- `src-tauri/src/benchmark/test_suite.rs`

```bash
# Run Rust tests (limited coverage)
cd src-tauri
cargo test
```

---

## Architecture & Key Concepts

### 1. Tauri IPC Communication Pattern

**Frontend → Backend Communication:**

```typescript
// Frontend (TypeScript)
import { invoke } from '@tauri-apps/api/core';

const result = await invoke<ReturnType>('command_name', {
	arg1: value1
});
```

```rust
// Backend (Rust)
#[tauri::command]
async fn command_name(arg1: String) -> Result<ReturnType, String> {
    // implementation
    Ok(result)
}

// Register in src-tauri/src/lib.rs
.invoke_handler(tauri::generate_handler![command_name])
```

**Streaming Responses (Ollama Chat):**

Backend emits events via `window.emit()`, frontend listens via `listen()`:

```rust
// Backend: src-tauri/src/commands/ollama.rs
window.emit("chat_chunk", chunk)?;
```

```typescript
// Frontend: src/lib/stores/chats.svelte.ts
const unlisten = await listen<string>('chat_chunk', (event) => {
	// Process chunk
});
```

### 2. State Management (Svelte 5 Runes)

**Frontend uses Svelte 5 runes** (`$state`, `$derived`, `$effect`), NOT Svelte 4 stores.

**Pattern:**

```typescript
// src/lib/stores/example.svelte.ts
let data = $state<T[]>([]);
let isLoading = $state(false);

export const exampleStore = {
	get data() {
		return data;
	},
	get isLoading() {
		return isLoading;
	},

	async load() {
		isLoading = true;
		data = await invoke<T[]>('load_data');
		isLoading = false;
	}
};
```

**Backend State (Tauri Managed State):**

```rust
// Thread-safe shared state with Arc<Mutex<T>>
pub struct AppState {
    data: Arc<Mutex<HashMap<String, String>>>,
}

// Initialize in lib.rs .setup()
.manage(AppState::default())

// Use in commands
#[tauri::command]
async fn use_state(state: State<'_, AppState>) -> Result<(), String> {
    let data = state.data.lock().await;
    // ...
}
```

### 3. Hardware Detection System (v2.2.0)

**Lazy initialization with OnceCell** (no startup race conditions):

```rust
// src-tauri/src/commands/hardware.rs
pub struct HardwareCache {
    info: OnceCell<Mutex<Option<HardwareInfo>>>,
}
```

Detection triggers on first call to `detect_hardware()` or `get_cached_hardware()`.

**Architecture:**

```
Frontend (hardware.svelte.ts)
    ├─► detect() ──────► Tauri IPC ──► detect_hardware()
    └─► getCached() ───► Tauri IPC ──► get_cached_hardware()
                                             │
                                    HardwareCache (OnceCell)
                                             │
                                    hardware::detector::detect_all()
                                             │
                                    hardware_query crate v0.2.1
                                             │
                                      Native OS APIs
```

**Uses hardware-query crate** for unified cross-platform detection:

- CPU: vendor, cores, frequency, cache, features (AVX2/AVX512/NEON/SVE)
- GPU: name, VRAM, backend (Metal/DirectX/Vulkan/CUDA), compute capability
- Memory: total/available GB
- Storage: capacity, type (SSD/HDD)
- NPU: detection with confidence (Apple Neural Engine, Intel AI Boost, AMD Ryzen AI)

**Type Synchronization Critical:**

- Rust: `src-tauri/src/hardware/types.rs`
- TypeScript: `src/lib/types/hardware.ts`
- These MUST match exactly for serialization

### 4. Ollama Integration

**Connection Pooling:**

```rust
// src-tauri/src/commands/ollama.rs
pub struct HttpClient {
    client: Arc<reqwest::Client>,
}
```

Reuses HTTP connections to `localhost:11434` for performance.

**Security:** OLLAMA_URL validation restricts to localhost only (prevents data exfiltration).

**Streaming Chat Flow:**

1. Frontend calls `generate_stream()`
2. Backend streams HTTP response from Ollama
3. Backend emits chunks via Tauri events
4. Frontend listens and updates UI reactively
5. Cancel via `StreamCancellation` shared state

### 5. Background Generation Pattern

Chats can generate in background while user switches to other chats:

```typescript
// src/lib/stores/chats.svelte.ts
let backgroundGenerations = $state<Map<string, boolean>>(new Map());
```

Only one active HTTP request at a time, but UI supports multiple chats.

---

## Critical File Locations

### Frontend Entry Points

- `src/main.ts` - App initialization
- `src/App.svelte` - Root component, routing
- `src/lib/stores/*.svelte.ts` - State management (Svelte 5 runes)
- `src/lib/components/` - UI components

### Backend Entry Points

- `src-tauri/src/main.rs` - Binary entry (calls `lib::run()`)
- `src-tauri/src/lib.rs` - Tauri setup, command registration, state initialization

### Command Modules

- `src-tauri/src/commands/ollama.rs` - Ollama API integration
- `src-tauri/src/commands/hardware.rs` - Hardware detection commands
- `src-tauri/src/commands/benchmark.rs` - Benchmarking system
- `src-tauri/src/commands/default.rs` - File I/O commands

### Hardware Detection

- `src-tauri/src/hardware/detector.rs` - Detection implementation
- `src-tauri/src/hardware/types.rs` - Rust type definitions
- `src/lib/types/hardware.ts` - TypeScript interfaces (MUST match Rust)
- `src/lib/stores/hardware.svelte.ts` - Frontend state

### Configuration

- `src-tauri/tauri.conf.json` - Tauri app configuration
- `src-tauri/Cargo.toml` - Rust dependencies
- `package.json` - Node.js dependencies, scripts
- `vite.config.ts` - Vite build configuration
- `tsconfig.json` - TypeScript configuration
- `tailwind.config.ts` - Tailwind CSS 4 configuration

---

## Code Conventions & Patterns

### TypeScript/Svelte

**File Naming:**

- Components: `PascalCase.svelte`
- Stores: `camelCase.svelte.ts`
- Types: `camelCase.ts`

**Component Structure:**

```svelte
<script lang="ts">
	// 1. Imports
	// 2. Props with $props()
	// 3. State with $state
	// 4. Derived with $derived
	// 5. Effects with $effect
	// 6. Functions
</script>

<div class="...">
	<!-- Template -->
</div>
```

**Store Pattern (Svelte 5):**

```typescript
let data = $state<T>(initialValue);

export const store = {
	get data() {
		return data;
	}, // Getter for reactivity

	method() {
		data = newValue; // Direct assignment
	}
};
```

**Naming:**

- Variables: `camelCase`
- Constants: `SCREAMING_SNAKE_CASE`
- Types/Interfaces: `PascalCase`
- Functions: `camelCase` with descriptive verbs
- Booleans: Prefix `is`, `has`, `should`

### Rust

**File Naming:** `snake_case.rs`

**Command Pattern:**

```rust
#[tauri::command]
pub async fn command_name(
    arg: String,
    state: State<'_, AppState>,
) -> Result<ReturnType, String> {
    // Return String for user-friendly errors
    Ok(result)
}
```

**Error Handling:**

- Use `Result<T, String>` for Tauri commands
- Use `thiserror` for internal error types
- Log with `log::error!()`, `log::warn!()`, `log::info!()`, `log::debug!()`
- Provide user-friendly error messages

**Naming:**

- Variables/functions: `snake_case`
- Types/structs/enums: `PascalCase`
- Constants: `SCREAMING_SNAKE_CASE`

### CSS/Styling (Tailwind 4)

**DO NOT use `@apply`** (Tailwind 4 doesn't support it).

Use utility classes directly:

```svelte
<div class="border-border bg-card flex items-center gap-2 rounded-lg border p-4">
	<!-- ... -->
</div>
```

**Conditional classes:**

```svelte
<button class:opacity-50={isDisabled} class="bg-primary ..."> Click </button>
```

**Color system (shadcn):**

- `bg-background`, `text-foreground`
- `bg-primary`, `text-primary-foreground`
- `bg-card`, `text-card-foreground`
- `bg-muted`, `text-muted-foreground`

---

## Important Implementation Details

### Security Considerations

**Implemented:**

- OLLAMA_URL validation (localhost only)
- HTTP connection pooling (prevents resource exhaustion)
- Event listener cleanup in `$effect` (prevents memory leaks)

**Needed (Known Issues):**

- Request timeouts
- Rate limiting
- Data size limits

### Performance Optimizations

**Frontend:**

- Svelte 5 fine-grained reactivity (minimal re-renders)
- Use `$derived` for computed values (not functions in template)
- Avoid `$effect` unless necessary
- Use `$state` objects for related data

**Backend:**

- Async/await for all I/O (non-blocking)
- Tokio runtime handles concurrency
- Hardware detection cached in memory
- HTTP connection pooling

### Git Workflow

**Branch naming:**

- Features: `feature/description`
- Fixes: `fix/description`
- AI work: `claude/description-sessionid`

**Commit messages (Conventional Commits):**

```
feat: add hardware detection system
fix: resolve startup race condition
docs: update README with v2.2.0 changes
refactor: migrate to hardware-query crate
chore: update dependencies
```

---

## Development Workflow

### Setup Development Environment

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

### Making Changes

**Frontend Changes (Hot Reload):**

- Edit Svelte/TypeScript files
- Save → Vite hot reloads automatically
- No restart needed

**Backend Changes (Requires Restart):**

- Edit Rust files
- Save → Press Ctrl+C → `npm run tauri dev`
- Rust recompiles on restart

### Before Committing

```bash
# Check types
npm run check

# Format code
npm run format

# Lint
npm run lint

# Compile Rust
cd src-tauri && cargo check && cargo clippy
```

---

## Common Tasks

### Adding a New Tauri Command

1. **Define command in appropriate module:**

```rust
// src-tauri/src/commands/mymodule.rs
#[tauri::command]
pub async fn my_command(arg: String) -> Result<String, String> {
    Ok(format!("Processed: {}", arg))
}
```

2. **Export from module:**

```rust
// src-tauri/src/commands/mod.rs
pub mod mymodule;
```

3. **Import in lib.rs:**

```rust
use commands::mymodule::my_command;
```

4. **Register in handler:**

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands
    my_command
])
```

5. **Call from frontend:**

```typescript
import { invoke } from '@tauri-apps/api/core';
const result = await invoke<string>('my_command', { arg: 'test' });
```

### Adding Managed State

1. **Define state struct:**

```rust
pub struct MyState {
    data: Arc<Mutex<HashMap<String, String>>>,
}

impl Default for MyState {
    fn default() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
```

2. **Register in lib.rs:**

```rust
.manage(MyState::default())
```

3. **Use in commands:**

```rust
#[tauri::command]
async fn use_my_state(state: State<'_, MyState>) -> Result<(), String> {
    let mut data = state.data.lock().await;
    // ... use data
    Ok(())
}
```

### Adding a New Store (Svelte 5)

```typescript
// src/lib/stores/mystore.svelte.ts
import { invoke } from '@tauri-apps/api/core';
import type { MyType } from '$lib/types/mytype';

let data = $state<MyType[]>([]);
let isLoading = $state(false);
let error = $state<string | null>(null);

export const myStore = {
	get data() {
		return data;
	},
	get isLoading() {
		return isLoading;
	},
	get error() {
		return error;
	},

	async load() {
		isLoading = true;
		error = null;
		try {
			data = await invoke<MyType[]>('load_data');
		} catch (e) {
			error = e instanceof Error ? e.message : 'Unknown error';
		} finally {
			isLoading = false;
		}
	}
};
```

### Type Synchronization (Rust ↔ TypeScript)

When adding new types for IPC:

1. **Define in Rust:**

```rust
// src-tauri/src/hardware/types.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyInfo {
    pub field1: String,
    pub field2: Option<u32>,
}
```

2. **Mirror in TypeScript:**

```typescript
// src/lib/types/mytype.ts
export interface MyInfo {
	field1: string;
	field2: number | null;
}
```

**Important:** Option<T> in Rust = T | null in TypeScript

---

## Debugging

### Frontend

- Press `F12` in dev mode for Chrome DevTools
- Check Console for errors
- Network tab shows Tauri IPC calls
- Use `console.log()` for debugging

### Backend

- Logs written to:

  - macOS: `~/Library/Logs/com.smolpc.codehelper/`
  - Windows: `%APPDATA%\com.smolpc.codehelper\logs\`
  - Linux: `~/.local/share/com.smolpc.codehelper/logs/`

- Use log macros:

```rust
log::info!("Info message");
log::warn!("Warning message");
log::error!("Error: {}", error);
log::debug!("Debug message (dev only)");
```

- In dev mode, logs also appear in terminal running `npm run tauri dev`

### Hardware Detection Testing

```typescript
// In browser console (F12)
await window.__TAURI__.invoke('detect_hardware');
await window.__TAURI__.invoke('get_cached_hardware');
```

---

## External Dependencies

### Critical Dependencies

**Frontend:**

- `svelte@^5.28.1` - Framework (uses runes)
- `@tauri-apps/api@^2.5.0` - Tauri bindings
- `tailwindcss@^4.1.7` - Styling
- `vite@^6.3.5` - Build tool

**Backend:**

- `tauri@2.6.2` - Desktop framework
- `reqwest@0.12` - HTTP client
- `hardware-query@0.2.1` - Hardware detection
- `tokio@1` - Async runtime
- `serde@1.0` - Serialization

### Ollama Requirement

App requires **Ollama** running at `http://localhost:11434`:

- Not bundled with app
- User must install separately
- Models: Qwen 2.5 Coder (7B), DeepSeek Coder (6.7B)

---

## Known Limitations

1. **No tests** - Test suite needs to be added
2. **No XSS protection** - Markdown rendering needs DOMPurify
3. **No input validation** - Large prompts/contexts not limited
4. **No request timeouts** - Long requests can hang
5. **Single active request** - Only one Ollama request at a time
6. **localStorage only** - No SQLite/IndexedDB for large data

---

## Future Roadmap

**Phase 2 (Q1 2025) - Current Focus:**

- llama.cpp integration with hardware-optimized compilation
- Automatic model selection based on RAM
- GPU layer offloading configuration
- CPU optimization flags (AVX2/AVX512/NEON)
- Syntax highlighting in code blocks
- Export chat to markdown

**Phase 3 (Q2 2025):**

- Multiple simultaneous generations
- Code execution sandbox
- Image paste support
- Comprehensive test suite

---

## Important Notes for AI Assistants

### When Making Changes

**Always:**

- Read relevant files before modifying
- Maintain existing code style and patterns
- Update types when changing data structures (BOTH Rust and TypeScript)
- Test in dev mode before committing
- Use conventional commit messages
- Check for compilation errors (TypeScript + Rust)

**Never:**

- Use `any` type in TypeScript
- Use `@apply` in CSS (Tailwind 4 doesn't support it)
- Commit untested code
- Break existing functionality
- Add dependencies without justification
- Bypass security measures
- Use Svelte 4 stores (use Svelte 5 runes)

### Code Review Checklist

- [ ] TypeScript compiles (`npm run check`)
- [ ] Rust compiles (`cargo check`)
- [ ] No console errors in DevTools
- [ ] Changes work in dev mode
- [ ] Types synchronized (TS ↔ Rust)
- [ ] Error handling implemented
- [ ] Logging added for important operations
- [ ] Documentation updated if needed
- [ ] Commit message follows conventions

### When Uncertain

If unsure about:

- Project principles (offline, privacy, educational)
- Existing patterns to follow
- Potential breaking changes
- Simplest solution

**Stop and ask the user before proceeding.**

---

## Resources

- **Tauri 2:** https://v2.tauri.app/
- **Svelte 5:** https://svelte.dev/docs/svelte/overview
- **Svelte 5 Runes:** https://svelte.dev/docs/svelte/what-are-runes
- **Tailwind CSS 4:** https://tailwindcss.com/docs
- **hardware-query:** https://docs.rs/hardware-query/
- **Ollama:** https://ollama.com/docs

---

**Last Updated:** January 2025
**Project Version:** 2.2.0
**Target Audience:** AI assistants working on SmolPC Code Helper
