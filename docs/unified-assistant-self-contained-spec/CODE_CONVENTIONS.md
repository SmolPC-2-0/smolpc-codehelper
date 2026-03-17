# Code Conventions

> **Purpose:** Defines coding standards, patterns, and anti-patterns for all code in the SmolPC Unified Assistant. Covers Rust backend, Svelte 5 frontend, TypeScript, type synchronization, error handling, logging, and testing.
>
> **Audience:** Every AI session writing code for this project. Read before implementing anything.
>
> **Last Updated:** 2026-03-13

---

## Table of Contents

1. [Rust Conventions](#rust-conventions)
2. [TypeScript / Svelte 5 Conventions](#typescript--svelte-5-conventions)
3. [Type Synchronization (Rust ↔ TypeScript)](#type-synchronization-rust--typescript)
4. [Tauri IPC Patterns](#tauri-ipc-patterns)
5. [Error Handling](#error-handling)
6. [Logging](#logging)
7. [Testing](#testing)
8. [Anti-Patterns](#anti-patterns)

---

## Rust Conventions

### DLL Loading Centralization

All DLL loading MUST go through `runtime_loading.rs`. No other file may call `libloading::Library::new()`.

```rust
// CORRECT — in runtime_loading.rs only
pub fn load_ort_genai(runtime_dir: &Path) -> Result<Library> {
    let dll_path = runtime_dir.join("onnxruntime").join("onnxruntime_genai.dll");
    unsafe { Library::new(&dll_path) }
}

// WRONG — in any other file
let lib = unsafe { Library::new("onnxruntime.dll") }; // NEVER: system PATH search, DLL hijacking risk
```

This is **enforced by a test** that scans the codebase for `Library::new` calls outside `runtime_loading.rs`.

### Thread-Safe Initialization

Use `OnceLock<Result<T>>` for fallible one-time initialization:

```rust
use std::sync::OnceLock;

static ENGINE: OnceLock<Result<Engine, String>> = OnceLock::new();

fn get_engine() -> Result<&'static Engine, String> {
    ENGINE.get_or_init(|| {
        Engine::new().map_err(|e| format!("Engine init failed: {e}"))
    }).as_ref().map_err(|e| e.clone())
}
```

**Why not `Once`?** `Once::call_once` doesn't return values. If you need to cache and return a result from one-time init, `OnceLock` is the correct primitive.

### State Tracking with AtomicBool

Use dedicated `AtomicBool` flags instead of `try_lock()`:

```rust
use std::sync::atomic::{AtomicBool, Ordering};

static IS_GENERATING: AtomicBool = AtomicBool::new(false);

async fn generate(prompt: &str) -> Result<String> {
    if IS_GENERATING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Err("Generation already in progress".into());
    }
    let result = do_generation(prompt).await;
    IS_GENERATING.store(false, Ordering::SeqCst);
    result
}
```

**Why not `try_lock()`?** `try_lock()` creates TOCTOU races. An explicit flag set/cleared in the function lifecycle is reliable.

### Fatal Init in Production, Non-Fatal in Dev

```rust
// In Tauri .setup()
if !cfg!(debug_assertions) {
    // Production: fail hard if model not found
    return Err("Model files missing — cannot start".into());
}
// Dev: warn and continue (model files may not be present)
log::warn!("Model files missing — running in dev mode without inference");
```

### Error Types

Use `thiserror` for library errors, `anyhow` for application errors:

```rust
// In engine-core (library crate)
#[derive(Debug, thiserror::Error)]
pub enum InferenceError {
    #[error("Model not loaded")]
    ModelNotLoaded,
    #[error("Backend error: {0}")]
    BackendError(String),
    #[error("Tokenization error: {0}")]
    TokenizationError(String),
}

// In engine-host (application crate)
use anyhow::Result;
async fn handle_request() -> Result<Response> { ... }
```

### Async Patterns

- All inference is async with Tokio (`tokio::spawn`, `tokio::task::spawn_blocking` for CPU-bound work)
- Never block the main thread
- Use `tokio::select!` for cancellation
- Use `tokio::sync::mpsc` for internal channels (not `std::sync::mpsc`)

---

## TypeScript / Svelte 5 Conventions

### Svelte 5 Runes — The Only Way

```typescript
// ✅ CORRECT — Svelte 5 runes
let count = $state(0);
let doubled = $derived(count * 2);
let items = $state<string[]>([]);

// Component props
let { name, onClose }: { name: string; onClose: () => void } = $props();

// Side effects
$effect(() => {
	console.log('count changed:', count);
});

// ❌ WRONG — Svelte 4 stores (DO NOT USE)
import { writable } from 'svelte/store'; // NEVER
const count = writable(0); // NEVER
$: doubled = $count * 2; // NEVER (Svelte 4 reactive declaration)
```

### Store Pattern (Module-Level State)

```typescript
// src/lib/stores/engine.svelte.ts

let status = $state<'unknown' | 'ready' | 'offline'>('unknown');
let currentModel = $state<string | null>(null);
let isGenerating = $state(false);

export const engineStore = {
	get status() {
		return status;
	},
	get currentModel() {
		return currentModel;
	},
	get isGenerating() {
		return isGenerating;
	},

	async checkHealth() {
		try {
			const healthy = await invoke<boolean>('engine_health');
			status = healthy ? 'ready' : 'offline';
		} catch {
			status = 'offline';
		}
	},

	async generate(prompt: string, onToken: (token: string) => void): Promise<string> {
		if (isGenerating) throw new Error('Already generating');
		isGenerating = true;
		try {
			// ... generation logic
			return result;
		} finally {
			isGenerating = false; // Always cleanup
		}
	}
};
```

**Key rules:**

- Single source of truth — component reads from store, doesn't maintain own copy
- Use `finally` for cleanup (never forget to reset `isGenerating`)
- Export as a plain object with getters, not a class

### Tailwind 4 — No @apply

```svelte
<!-- ✅ CORRECT — utility classes directly -->
<button class="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600">
    Send
</button>

<!-- ❌ WRONG — @apply (NOT supported in Tailwind 4) -->
<style>
    .btn {
        @apply px-4 py-2 bg-blue-500 text-white rounded-lg;  /* BREAKS */
    }
</style>
```

For reusable styles, use CSS variables or component extraction instead of `@apply`.

### Tauri Channel Streaming

```typescript
import { invoke, Channel } from '@tauri-apps/api/core';

async function streamChat(prompt: string): Promise<AssistantResponse> {
	const channel = new Channel<string>();
	let accumulated = '';

	channel.onmessage = (token: string) => {
		accumulated += token;
		// Update reactive state
		currentMessage = accumulated;
	};

	// invoke() resolves AFTER all channel messages are delivered
	const result = await invoke<AssistantResponse>('assistant_chat_stream', {
		prompt,
		onToken: channel
	});

	// No manual cleanup needed — channels are command-scoped and auto-cleanup
	return result;
}
```

**Why Channels, not Events?**

- Events are global broadcast — no lifecycle tie to `invoke()`
- Events can arrive before the listener is set up (race condition)
- Channels are command-scoped — tied to one `invoke()` call
- Channels are ordered — tokens arrive in sequence
- Channels auto-cleanup when `invoke()` completes

### Component File Naming

```
src/
├── routes/
│   ├── +page.svelte          # Main page
│   └── +layout.svelte        # Layout
├── lib/
│   ├── components/
│   │   ├── ChatMessage.svelte   # PascalCase for components
│   │   ├── StatusBar.svelte
│   │   ├── ModeDropdown.svelte
│   │   └── DevTools.svelte
│   ├── stores/
│   │   ├── engine.svelte.ts     # .svelte.ts for reactive stores
│   │   └── mode.svelte.ts
│   └── types/
│       ├── inference.ts         # Type definitions
│       └── mcp.ts
```

---

## Type Synchronization (Rust ↔ TypeScript)

### The Rule

**Types must match EXACTLY between Rust and TypeScript.** When you change a Rust struct that's returned via Tauri IPC, you MUST update the corresponding TypeScript type.

### Mapping

| Rust                | TypeScript                           |
| ------------------- | ------------------------------------ |
| `String`            | `string`                             |
| `bool`              | `boolean`                            |
| `i32`, `u32`, `f64` | `number`                             |
| `Vec<T>`            | `T[]`                                |
| `Option<T>`         | `T \| null`                          |
| `HashMap<K, V>`     | `Record<K, V>`                       |
| `()`                | `void`                               |
| `Result<T, String>` | `T` (error becomes rejected promise) |

### Example

```rust
// Rust: src-tauri/src/types.rs
#[derive(Debug, Serialize, Deserialize)]
pub struct AssistantResponse {
    pub reply: String,
    pub explain: Option<String>,
    pub undoable: Option<bool>,
    pub streamed: Option<bool>,
}
```

```typescript
// TypeScript: src/lib/types/inference.ts
type AssistantResponse = {
	reply: string;
	explain: string | null;
	undoable: boolean | null;
	streamed: boolean | null;
};
```

### Key Files for Sync

- **Rust types:** `src-tauri/src/types.rs` or inline in command files
- **TypeScript types:** `src/lib/types/inference.ts`, `src/lib/types/mcp.ts`
- **Tauri commands:** `src-tauri/src/commands/*.rs` → parameter and return types must match `invoke<T>()` generics

### Checklist When Changing Types

1. Change the Rust struct
2. Change the TypeScript type
3. Update all `invoke<T>()` call sites
4. Run `npm run check` (TypeScript compilation)
5. Run `cargo check` (Rust compilation)

---

## Tauri IPC Patterns

### Command Registration

```rust
// src-tauri/src/lib.rs
fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            assistant_chat_stream,
            engine_health,
            engine_cancel,
            mcp_list_tools,
            mcp_call_tool,
            macro_draw_line,
            macro_undo,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}
```

### Command with Channel (Streaming)

```rust
#[tauri::command]
async fn assistant_chat_stream(
    prompt: String,
    on_token: Channel<String>,       // Tauri Channel for streaming
    state: State<'_, AppState>,      // Managed state
) -> Result<AssistantResponse, String> {
    // Send tokens via channel during generation
    on_token.send("Hello ".to_string()).map_err(|e| e.to_string())?;
    on_token.send("world!".to_string()).map_err(|e| e.to_string())?;

    // Return final result (invoke() resolves with this)
    Ok(AssistantResponse {
        reply: "Hello world!".into(),
        explain: None,
        undoable: Some(false),
        streamed: Some(true),
    })
}
```

### Frontend Invoke Pattern

```typescript
// Always specify the return type generic
const result = await invoke<AssistantResponse>('assistant_chat_stream', {
	prompt: 'Hello',
	onToken: channel
});

// Error handling
try {
	const healthy = await invoke<boolean>('engine_health');
} catch (e) {
	// Rust Err(String) becomes a rejected promise with string message
	console.error('Engine error:', String(e));
}
```

---

## Error Handling

### Rust

- Library crates (`engine-core`): Use `thiserror` enums
- Application crates (`engine-host`): Use `anyhow::Result`
- Tauri commands: Return `Result<T, String>` (Tauri serializes to JS error)
- Never `unwrap()` in production code. Use `?` or explicit error handling.
- Use `map_err(|e| e.to_string())` to convert errors for Tauri IPC.

### TypeScript / Frontend

- `invoke()` errors are caught via try/catch (rejected promises)
- Display user-friendly messages, not raw error strings
- Log technical details to console for debugging

```typescript
try {
	await invoke('some_command');
} catch (e) {
	// User-facing message
	messages = [
		...messages,
		{
			role: 'assistant',
			text: 'Something went wrong. Please try again.'
		}
	];
	// Technical log
	console.error('Command failed:', e);
}
```

---

## Logging

### Rust

Use the `log` crate with `env_logger` or Tauri's built-in logging:

```rust
use log::{info, warn, error, debug, trace};

info!("Engine started on port 19432");
warn!("Model files missing — running in dev mode");
error!("Backend initialization failed: {}", err);
debug!("Loading model from {}", path.display());
trace!("Token generated: {}", token_id);
```

### Log Levels

| Level   | When to Use                                                        |
| ------- | ------------------------------------------------------------------ |
| `error` | Unrecoverable failures (engine crash, DLL load failure)            |
| `warn`  | Recoverable issues (model not found in dev, connection timeout)    |
| `info`  | Normal operations (startup, shutdown, model loaded, mode switched) |
| `debug` | Detailed flow (request handling, token generation)                 |
| `trace` | Very detailed (individual token IDs, tensor shapes)                |

### Frontend

Use `console.log/warn/error` sparingly. Don't log sensitive data (user prompts in production).

---

## Testing

### Rust Tests

```bash
# Unit tests (fast, no external deps)
cargo test

# Integration tests (need model files, ONNX Runtime)
cargo test -- --ignored --nocapture

# Clippy (lint)
cargo clippy -- -D warnings
```

### Frontend Tests

```bash
# TypeScript type checking
npm run check

# Linting
npm run lint
```

### What to Test

| Component      | Test Type   | What to Test                                      |
| -------------- | ----------- | ------------------------------------------------- |
| Model registry | Unit        | Model lookup, tier selection, artifact resolution |
| MCP client     | Unit        | JSON-RPC serialization, message parsing           |
| Engine client  | Integration | HTTP connection, SSE streaming                    |
| Type sync      | Build-time  | `npm run check` + `cargo check` both pass         |
| DLL loading    | Unit        | Only `runtime_loading.rs` calls `Library::new`    |

### Test Conventions

- Test files live next to source: `foo.rs` → `foo_test.rs` or `#[cfg(test)] mod tests` inline
- Use `#[ignore]` for tests requiring model files or DLLs (not available in CI)
- Test names describe the behavior: `test_model_registry_returns_smallest_model_for_8gb_ram`

---

## Anti-Patterns

### DO NOT

| Anti-Pattern                              | Why                                        | Instead                                                   |
| ----------------------------------------- | ------------------------------------------ | --------------------------------------------------------- |
| `import { writable } from 'svelte/store'` | Svelte 4 — not used                        | Use `$state`, `$derived`, `$props`                        |
| `@apply` in CSS                           | Not supported in Tailwind 4                | Use utility classes directly                              |
| `Library::new("dll_name.dll")`            | System PATH search, DLL hijacking          | Use absolute paths via `runtime_loading.rs`               |
| `unwrap()` in production code             | Panics on error                            | Use `?` or explicit error handling                        |
| Tauri Events for streaming                | Global, unordered, race conditions         | Use Tauri Channels                                        |
| Duplicating reactive state                | Components and stores diverge              | Single source of truth in store                           |
| `try_lock()` for state tracking           | TOCTOU race conditions                     | Use `AtomicBool`                                          |
| `Once::call_once` for fallible init       | Can't return errors                        | Use `OnceLock::get_or_init`                               |
| Amending commits after hook failure       | Modifies PREVIOUS commit                   | Create new commit                                         |
| Git worktrees                             | Has caused issues                          | Use separate clones                                       |
| `add_special_tokens: true` with ChatML    | Duplicates special tokens                  | `add_special_tokens: false` when prompt has ChatML tokens |
| Only checking one Qwen stop token         | Misses `<\|endoftext\|>` or `<\|im_end\|>` | Check both 151643 and 151645                              |
