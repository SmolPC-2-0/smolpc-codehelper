# Code Review: Streaming Chat Flow

**Presentation Guide: Complete Flow from User Input → Streaming Response**

This document traces the complete code execution path when a user presses Enter on a prompt until they receive the streaming AI response.

---

## Flow Overview

```
┌──────────────────────────────────────────────────────────────────┐
│                         FRONTEND (Svelte 5)                      │
│                                                                  │
│  User Input → ChatInput → App.svelte → Tauri IPC invoke()        │
│                                ↓                                 │
│                          Store Updates                           │
│                                ↓                                 │
│                       Event Listeners Setup                      │
└───────────────────────────────┬──────────────────────────────────┘
                                │
                    ┌───────────▼────────────┐
                    │   Tauri IPC Bridge     │
                    │   (Type-Safe Channel)  │
                    └───────────┬────────────┘
                                │
┌───────────────────────────────▼──────────────────────────────────┐
│                      BACKEND (Rust/Tauri)                        │
│                                                                  │
│  lib.rs → ollama.rs → Security Validation → HTTP Request         │
│                                ↓                                 │
│                          Ollama API                              │
│                                ↓                                 │
│                      Stream Processing Loop                      │
│                                ↓                                 │
│                       Emit Events to Frontend                    │
└───────────────────────────────┬──────────────────────────────────┘
                                │
                    ┌───────────▼────────────┐
                    │    Tauri Events        │
                    │   (ollama_chunk)       │
                    └───────────┬────────────┘
                                │
┌───────────────────────────────▼──────────────────────────────────┐
│                         FRONTEND (Svelte 5)                      │
│                                                                  │
│  Event Handlers → Store Updates → ChatMessage Rendering          │
│                                ↓                                 │
│                      Markdown Display                            │
└──────────────────────────────────────────────────────────────────┘
```

**Duration**: ~10ms (local latency) + streaming time
**Async Operations**: 5 (Input → Store → IPC → HTTP → Stream → Events → Render)

---

## Phase 1: Frontend Input Handling

### 1.1 User Presses Enter

**File**: `src/lib/components/ChatInput.svelte`
**Lines**: 27-32

```typescript
function handleKeydown(e: KeyboardEvent) {
	if (e.key === 'Enter' && !e.shiftKey) {
		e.preventDefault();
		handleSubmit();
	}
}
```

**What Happens**:

- Detects Enter key (Shift+Enter adds newline)
- Prevents default form submission
- Calls `handleSubmit()`

---

### 1.2 Input Validation & Callback

**File**: `src/lib/components/ChatInput.svelte`
**Lines**: 16-25

```typescript
//src/lib/components/ChatInput.svelte
function handleSubmit() {
	const trimmed = inputValue.trim();
	if (trimmed && !disabled) {
		onSend(trimmed); // ← Calls parent's handler
		inputValue = '';
		if (textarea) {
			textarea.style.height = 'auto';
		}
	}
}
```

**What Happens**:

- Validates input is not empty
- Checks if generation is already in progress (`!disabled`)
- Calls `onSend` callback (provided by parent `App.svelte`)
- Clears input and resets textarea height

---

## Phase 2: Message Processing & IPC Invocation

### 2.1 Main Message Handler

**File**: `src/App.svelte`
**Lines**: 112-174

```typescript
async function handleSendMessage(content: string) {
	if (!ollamaStore.isConnected || isGenerating) return;

	// Create new chat if none exists
	if (!currentChat) {
		chatsStore.createChat(settingsStore.selectedModel);
	}

	if (!currentChat) return; // Safety check

	// Hide quick examples after first message
	showQuickExamples = false;

	// Reset scroll state for new message
	userHasScrolledUp = false;
	userInteractedWithScroll = false;

	// Add user message
	const userMessage: Message = {
		id: crypto.randomUUID(),
		role: 'user',
		content,
		timestamp: Date.now()
	};
	chatsStore.addMessage(currentChat.id, userMessage);
	scrollToBottom();

	// Create placeholder for assistant response
	const assistantMessage: Message = {
		id: crypto.randomUUID(),
		role: 'assistant',
		content: '',
		timestamp: Date.now(),
		isStreaming: true // ← Flag for streaming state
	};
	chatsStore.addMessage(currentChat.id, assistantMessage);
	scrollToBottom();

	isGenerating = true;
	cancelRequested = false;
	currentStreamingChatId = currentChat.id; // Track active chat
	currentStreamingMessageId = assistantMessage.id; // Track streaming message

	try {
		// Build context from previous messages
		const context = buildContext();

		// ★ Start streaming generation ★
		await invoke('generate_stream', {
			prompt: content,
			model: settingsStore.selectedModel,
			context: context.length > 0 ? context : null
		});
	} catch (error) {
		console.error('Generation error:', error);
		chatsStore.updateMessage(currentChat.id, assistantMessage.id, {
			content: `Error: ${error}`,
			isStreaming: false
		});
		isGenerating = false;
		currentStreamingChatId = null;
		currentStreamingMessageId = null;
	}
}
```

```typescript
// src/App.svelte 112-174
async function handleSendMessage(content: string) {
	if (!ollamaStore.isConnected || isGenerating) return;

	// Create new chat if none exists

	// Hide quick examples after first message

	// Reset scroll state for new message

	// Add user message
	const userMessage: Message = {
		id: crypto.randomUUID(),
		role: 'user',
		content,
		timestamp: Date.now()
	};
	chatsStore.addMessage(currentChat.id, userMessage);
	scrollToBottom();

	// Create placeholder for assistant response
	const assistantMessage: Message = {
		id: crypto.randomUUID(),
		role: 'assistant',
		content: '',
		timestamp: Date.now(),
		isStreaming: true // ← Flag for streaming state
	};
	chatsStore.addMessage(currentChat.id, assistantMessage);
	scrollToBottom();

	isGenerating = true;
	cancelRequested = false;
	currentStreamingChatId = currentChat.id; // Track active chat
	currentStreamingMessageId = assistantMessage.id; // Track streaming message

	try {
		// Build context from previous messages
		const context = buildContext();

		// ★ Start streaming generation ★
		await invoke('generate_stream', {
			prompt: content,
			model: settingsStore.selectedModel,
			context: context.length > 0 ? context : null
		});
	} catch (error) {
		console.error('Generation error:', error);
		chatsStore.updateMessage(currentChat.id, assistantMessage.id, {
			content: `Error: ${error}`,
			isStreaming: false
		});
		isGenerating = false;
		currentStreamingChatId = null;
		currentStreamingMessageId = null;
	}
}
```

**Key Points**:

- **State Management**: Uses Svelte 5 `$state` runes for reactivity
- **Optimistic UI**: Creates placeholder message immediately (shows "Generating..." state)
- **Context Building**: Includes previous messages if context is enabled
- **Error Handling**: Updates UI even if backend fails
- **Tracking**: Stores IDs to route streaming chunks to correct message

---

### 2.2 Context Building (Conversation History)

**File**: `src/App.svelte`
**Lines**: 100-109

```typescript
function buildContext(): OllamaMessage[] {
	if (!settingsStore.contextEnabled || !currentChat) {
		return [];
	}

	return currentChat.messages.map((msg) => ({
		role: msg.role === 'user' ? 'user' : 'assistant',
		content: msg.content
	}));
}
```

**What Happens**:

- Checks if context is enabled in settings
- Converts all previous messages to Ollama format
- Returns empty array if disabled (single-turn conversation)

---

### 2.3 Store: Add Message

**File**: `src/lib/stores/chats.svelte.ts`
**Lines**: 63-76

```typescript
addMessage(chatId: string, message: Message) {
    const chat = chats.find((c) => c.id === chatId);
    if (chat) {
        chat.messages = [...chat.messages, message];  // Svelte 5 reactivity
        chat.updatedAt = Date.now();

        // Auto-generate title from first user message
        if (chat.messages.length === 1 && message.role === 'user') {
            chat.title = message.content.slice(0, 50) +
                (message.content.length > 50 ? '...' : '');
        }

        this.persist();  // Save to localStorage
    }
}
```

**What Happens**:

- Finds chat by ID
- Appends message (creates new array for Svelte 5 reactivity)
- Auto-generates chat title from first message
- Persists to localStorage

---

## Phase 3: Tauri IPC Bridge

### 3.1 Frontend IPC Call

**File**: `src/App.svelte`
**Line**: 160

```typescript
await invoke('generate_stream', {
	prompt: content,
	model: settingsStore.selectedModel,
	context: context.length > 0 ? context : null
});
```

**What Happens**:

- `@tauri-apps/api/core` serializes parameters to JSON
- Tauri validates command exists in registered handlers
- Sends IPC message to backend (cross-process communication)
- Returns promise that resolves when command completes (not when stream finishes!)

**Type Safety**:

- TypeScript → JSON → Rust deserialization
- Rust types must match TypeScript interfaces exactly

---

### 3.2 Backend Command Registration

**File**: `src-tauri/src/lib.rs`
**Lines**: 37-50

```rust
.invoke_handler(tauri::generate_handler![
    read,
    write,
    save_code,
    check_ollama,
    get_ollama_models,
    generate_stream,  // ← Our command registered here
    cancel_generation,
    run_benchmark,
    get_benchmarks_directory,
    open_benchmarks_folder,
    detect_hardware,
    get_cached_hardware
])
```

**What Happens**:

- `tauri::generate_handler!` macro creates type-safe routing
- Maps string command names to Rust functions
- Validates parameter types at compile time

---

## Phase 4: Backend Command Execution

### 4.1 Command Handler Entry Point

**File**: `src-tauri/src/commands/ollama.rs`
**Lines**: 188-230

```rust
// src-tauri/src/commands/ollama.rs 188-230
/// Generate streaming response from Ollama
#[tauri::command]
pub async fn generate_stream(
    app_handle: AppHandle,
    prompt: String,
    model: String,
    context: Option<Vec<OllamaMessage>>,
    client: State<'_, HttpClient>,
    config: State<'_, OllamaConfig>,
    cancellation: State<'_, StreamCancellation>,
) -> Result<(), Error> {
    // Create a new cancellation receiver for this stream
    let mut cancel_rx = cancellation.create_channel();

    // Build messages array with system prompt, context, and current prompt
    let mut messages = vec![OllamaMessage {
        role: "system".to_string(),
        content: SYSTEM_PROMPT.to_string(),  // ← Educational prompt
    }];

    // Add context messages if provided
    if let Some(ctx) = context {
        messages.extend(ctx);
    }

    // Add current user prompt
    messages.push(OllamaMessage {
        role: "user".to_string(),
        content: prompt,
    });

    let request = OllamaRequest {
        model,
        messages,
        stream: true,  // ← Enable streaming
    };

    let url = format!("{}/api/chat", config.base_url());
    let response = client.get()
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| Error::Other(format!("Failed to send request: {e}")))?;

    let mut stream = response.bytes_stream();
    // ... (continue to streaming loop)
```

**Key Points**:

- **Dependency Injection**: `State<'_, T>` parameters are managed by Tauri
- **System Prompt**: Adds educational context for student-appropriate responses
- **Context Chain**: System → History → Current prompt
- **Stream Flag**: `stream: true` enables Server-Sent Events (SSE)

---

### 4.2 Managed State: HTTP Client (Connection Pooling)

**File**: `src-tauri/src/commands/ollama.rs`
**Lines**: 22-39

```rust
/// Shared HTTP client for connection pooling
pub struct HttpClient {
    client: reqwest::Client,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl HttpClient {
    pub fn get(&self) -> &reqwest::Client {
        &self.client
    }
}
```

**What Happens**:

- Single `reqwest::Client` shared across all requests
- Reuses TCP connections to `localhost:11434`
- Reduces latency (no handshake per request)
- Registered in `lib.rs`: `.manage(HttpClient::default())`

---

### 4.3 Security: Ollama URL Validation

**File**: `src-tauri/src/commands/ollama.rs`
**Lines**: 42-62

```rust
pub struct OllamaConfig {
    base_url: String,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        // Read from environment variable or use default
        let base_url = env::var("OLLAMA_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());

        // ★ Validate URL is localhost only for security ★
        let validated_url = security::validate_ollama_url(&base_url)
            .unwrap_or_else(|err| {
                log::error!("{err}");
                log::warn!("Falling back to default: http://localhost:11434");
                "http://localhost:11434".to_string()
            });

        Self { base_url: validated_url }
    }
}
```

**File**: `src-tauri/src/security/mod.rs`
**Lines**: 125-155

```rust
/// Validates that an Ollama URL is localhost only
pub fn validate_ollama_url(url_str: &str) -> Result<String, String> {
    // Parse URL with proper parser (not naive string matching)
    let url = Url::parse(url_str)
        .map_err(|e| format!("Invalid Ollama URL format: {e}"))?;

    // Extract hostname for validation
    let host = url.host_str()
        .ok_or("Ollama URL must have a hostname")?;

    // Exact hostname matching (prevents bypass attacks)
    match host {
        "localhost" | "127.0.0.1" | "::1" => {
            log::info!("Ollama URL validated: {}", url_str);
            Ok(url_str.to_string())
        }
        _ => {
            log::error!(
                "SECURITY: Rejected non-localhost Ollama URL: '{}' (hostname: '{}')",
                url_str, host
            );
            Err(format!(
                "Security violation: Ollama must run on localhost. Found hostname: '{}'\n\
                 Allowed values: localhost, 127.0.0.1, ::1\n\
                 This restriction protects student privacy (GDPR/FERPA compliance).",
                host
            ))
        }
    }
}
```

**Security Guarantees**:

- Uses `url::Url` parser (RFC 3986 compliant)
- Validates hostname exactly (not prefix matching)
- Prevents data exfiltration to external servers
- GDPR/FERPA compliance (student data stays local)
- Falls back to safe default on failure

---

### 4.4 Student-Friendly System Prompt

**File**: `src-tauri/src/commands/ollama.rs`
**Lines**: 10-20

```rust
const SYSTEM_PROMPT: &str = r#"You are a helpful coding assistant designed for secondary school students (ages 11-18).
Your goal is to explain programming concepts clearly and provide well-commented code examples.

Guidelines:
- Use simple, encouraging language
- Break down complex concepts into steps
- Always include helpful comments in code
- Be patient and supportive
- Adapt explanations to the student's level
- Encourage learning and experimentation"#;
```

**Educational Design**:

- Tailored for ages 11-18
- Emphasizes clear explanations
- Encourages commented code
- Positive, supportive tone

---

## Phase 5: HTTP Streaming to Ollama

### 5.1 HTTP POST Request

**File**: `src-tauri/src/commands/ollama.rs`
**Lines**: 218-232

```rust
let request = OllamaRequest {
    model,
    messages,
    stream: true,
};

let url = format!("{}/api/chat", config.base_url());
let response = client.get()
    .post(&url)
    .json(&request)  // Serializes to JSON
    .send()
    .await
    .map_err(|e| Error::Other(format!("Failed to send request: {e}")))?;

let mut stream = response.bytes_stream();
```

**Request Format (JSON)**:

```json
{
	"model": "qwen2.5-coder:7b",
	"messages": [
		{
			"role": "system",
			"content": "You are a helpful coding assistant..."
		},
		{
			"role": "user",
			"content": "How do I write a for loop in Python?"
		}
	],
	"stream": true
}
```

**What Happens**:

- POST to `http://localhost:11434/api/chat`
- Serializes request with serde_json
- `stream: true` enables chunked transfer encoding
- `bytes_stream()` returns async stream of response chunks

---

## Phase 6: Stream Processing Loop

### 6.1 Main Streaming Loop with Cancellation

**File**: `src-tauri/src/commands/ollama.rs`
**Lines**: 234-299

```rust
// src-tauri/src/commands/ollama.rs 234-299
loop {
    tokio::select! {
        // ★ Branch 1: Check for cancellation ★
        _ = cancel_rx.recv() => {
            // Stream was cancelled
            cancellation.clear();
            if let Err(e) = app_handle.emit("ollama_cancelled", ()) {
                log::debug!("Failed to emit cancellation event (frontend may be closed): {e}");
            }
            return Ok(());
        }

        // ★ Branch 2: Process stream chunks ★
        chunk_result = stream.next() => {
            match chunk_result {
                Some(Ok(bytes)) => {
                    if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                        // Parse each line as JSON
                        for line in text.lines() {
                            if line.trim().is_empty() {
                                continue;
                            }

                            match serde_json::from_str::<OllamaResponse>(line) {
                                Ok(response) => {
                                    if let Some(message) = response.message {
                                        // ★ Emit chunk event with content ★
                                        if let Err(e) = app_handle.emit("ollama_chunk", message.content) {
                                            log::debug!("Frontend disconnected during stream, stopping: {e}");
                                            cancellation.clear();
                                            return Ok(());
                                        }
                                    }

                                    if response.done {
                                        // ★ Emit done event ★
                                        cancellation.clear();
                                        if let Err(e) = app_handle.emit("ollama_done", ()) {
                                            log::debug!("Failed to emit done event (frontend may be closed): {e}");
                                        }
                                        return Ok(());
                                    }
                                }
                                Err(e) => {
                                    log::warn!("Failed to parse Ollama response: {e} | Line: {line}");
                                    // Continue processing other lines - don't fail entire stream
                                }
                            }
                        }
                    }
                }
                Some(Err(e)) => {
                    cancellation.clear();
                    if let Err(emit_err) = app_handle.emit("ollama_error", format!("Stream error: {e}")) {
                        log::debug!("Failed to emit error event (frontend may be closed): {emit_err}");
                    }
                    return Err(Error::Other(format!("Stream error: {e}")));
                }
                None => {
                    // Stream ended
                    cancellation.clear();
                    return Ok(());
                }
            }
        }
    }
}
```

**Key Concepts**:

1. **`tokio::select!` Macro**:

   - Waits on multiple async operations simultaneously
   - Whichever completes first gets executed
   - Enables user cancellation during generation

2. **Chunk Processing**:

   - Each chunk is newline-delimited JSON
   - Multiple lines may arrive in one chunk
   - Parses each line independently

3. **Event Emission**:

   - `app_handle.emit("ollama_chunk", content)` sends to frontend
   - Non-blocking (fire-and-forget)
   - Detects frontend disconnection (return early)

4. **Error Resilience**:
   - Failed JSON parse → log warning, continue stream
   - Network error → emit error event, stop stream
   - Frontend disconnect → gracefully terminate

---

### 6.2 Ollama Response Format

**File**: `src-tauri/src/commands/ollama.rs`
**Lines**: 83-93

```rust
#[derive(Debug, Deserialize)]
pub struct OllamaResponse {
    pub message: Option<OllamaMessage>,
    pub done: bool,
    // Token count metadata (only present when done=true)
    pub eval_count: Option<usize>,
    // Timing metadata (only present when done=true)
    pub total_duration: Option<u64>,
    pub prompt_eval_duration: Option<u64>,
    pub eval_duration: Option<u64>,
}
```

**Example Streaming Response**:

```json
{"message": {"role": "assistant", "content": "Here's"}, "done": false}
{"message": {"role": "assistant", "content": " how"}, "done": false}
{"message": {"role": "assistant", "content": " you"}, "done": false}
{"message": {"role": "assistant", "content": " write"}, "done": false}
{"message": {"role": "assistant", "content": " a"}, "done": false}
{"message": {"role": "assistant", "content": " for"}, "done": false}
{"message": {"role": "assistant", "content": " loop"}, "done": false}
{"message": {"role": "assistant", "content": ":"}, "done": false}
{"message": {"role": "assistant", "content": ""}, "done": true, "eval_count": 127, "total_duration": 2489123456}
```

**Field Meanings**:

- `message.content`: Text chunk to append
- `done`: `true` on final chunk (includes metadata)
- `eval_count`: Total tokens generated
- `total_duration`: Total time in nanoseconds

---

### 6.3 Stream Cancellation Mechanism

**File**: `src-tauri/src/commands/ollama.rs`
**Lines**: 105-140

```rust
// src-tauri/src/commands/ollama.rs 105-140
/// Global state to manage stream cancellation
pub struct StreamCancellation {
    sender: Mutex<Option<broadcast::Sender<()>>>,
}

impl Default for StreamCancellation {
    fn default() -> Self {
        Self {
            sender: Mutex::new(None),
        }
    }
}

impl StreamCancellation {
    pub fn create_channel(&self) -> broadcast::Receiver<()> {
        let mut sender_lock = self.sender.lock()
            .expect("StreamCancellation mutex poisoned - indicates panic in stream handler");
        let (tx, rx) = broadcast::channel(1);
        *sender_lock = Some(tx);  // Store sender
        rx  // Return receiver to stream loop
    }

    pub fn cancel(&self) {
        let sender_lock = self.sender.lock()
            .expect("StreamCancellation mutex poisoned - indicates panic in stream handler");
        if let Some(sender) = sender_lock.as_ref() {
            let _ = sender.send(());  // Signal cancellation
        }
    }

    pub fn clear(&self) {
        let mut sender_lock = self.sender.lock()
            .expect("StreamCancellation mutex poisoned - indicates panic in stream handler");
        *sender_lock = None;  // Clear sender
    }
}
```

**Cancellation Flow**:

1. Stream starts → creates broadcast channel
2. User clicks "Cancel" → frontend calls `cancel_generation()`
3. `cancel_generation()` sends signal via broadcast
4. Stream loop receives signal in `tokio::select!`
5. Loop breaks, emits `ollama_cancelled` event

**File**: `src-tauri/src/commands/ollama.rs`
**Lines**: 182-185

```rust
#[tauri::command]
pub fn cancel_generation(cancellation: State<StreamCancellation>) {
    cancellation.cancel();
}
```

---

## Phase 7: Frontend Event Handling

### 7.1 Event Listener Setup

**File**: `src/App.svelte`
**Lines**: 229-340

```typescript
// src/App.svelte 229-340
onMount(() => {
	let unlistenChunk: UnlistenFn;
	let unlistenDone: UnlistenFn;
	let unlistenError: UnlistenFn;
	let unlistenCancelled: UnlistenFn;

	async function setupListeners() {
		// ★ Listen for streaming chunks ★
		unlistenChunk = await listen<string>('ollama_chunk', (event) => {
			// Only process chunks if we're streaming
			if (!currentStreamingChatId || !currentStreamingMessageId || cancelRequested) {
				return;
			}
			// Find the streaming chat and message
			const streamingChat = chatsStore.chats.find((c) => c.id === currentStreamingChatId);
			if (!streamingChat) return;

			const streamingMessage = streamingChat.messages.find(
				(m) => m.id === currentStreamingMessageId
			);
			if (
				!streamingMessage ||
				streamingMessage.role !== 'assistant' ||
				!streamingMessage.isStreaming
			) {
				return;
			}
			// ★ Update the message content ★
			chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
				content: streamingMessage.content + event.payload // Append chunk
			});
			// Auto-scroll if this is the currently displayed chat
			if (currentChat?.id === currentStreamingChatId) {
				scrollToBottom();
			}
		});

		// ★ Listen for generation complete ★
		unlistenDone = await listen('ollama_done', () => {
			if (!currentStreamingChatId || !currentStreamingMessageId) return;

			// Mark the streaming message as complete
			chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
				isStreaming: false // Remove "Generating..." indicator
			});

			isGenerating = false;
			currentStreamingChatId = null;
			currentStreamingMessageId = null;
		});

		// ★ Listen for cancellation ★
		unlistenCancelled = await listen('ollama_cancelled', () => {
			isGenerating = false;
			currentStreamingChatId = null;
			currentStreamingMessageId = null;
		});

		// ★ Listen for errors ★
		unlistenError = await listen<string>('ollama_error', (event) => {
			if (!currentStreamingChatId || !currentStreamingMessageId) return;

			// Update the streaming message with error
			chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
				content: `Error: ${event.payload}`,
				isStreaming: false
			});

			isGenerating = false;
			currentStreamingChatId = null;
			currentStreamingMessageId = null;
		});
	}

	// Initial setup
	setupListeners();

	// Cleanup - unsubscribe from all events
	return async () => {
		if (unlistenChunk) unlistenChunk();
		if (unlistenDone) unlistenDone();
		if (unlistenError) unlistenError();
		if (unlistenCancelled) unlistenCancelled();
	};
});
```

**Key Points**:

- **Event Types**: `ollama_chunk`, `ollama_done`, `ollama_error`, `ollama_cancelled`
- **Defensive Checks**: Validates stream state before updating
- **Svelte Lifecycle**: `onMount()` for setup, return cleanup function
- **String Concatenation**: `content + event.payload` appends each chunk
- **Auto-scroll**: Only scrolls if viewing the active chat

---

### 7.2 Store: Update Message

**File**: `src/lib/stores/chats.svelte.ts`
**Lines**: 78-88

```typescript
updateMessage(chatId: string, messageId: string, updates: Partial<Message>) {
    const chat = chats.find((c) => c.id === chatId);
    if (chat) {
        const message = chat.messages.find((m) => m.id === messageId);
        if (message) {
            Object.assign(message, updates);  // Merge updates
            chat.updatedAt = Date.now();
            this.persist();  // Save to localStorage
        }
    }
}
```

**What Happens**:

- Finds chat and message by ID
- Merges partial updates (e.g., `{ content: "new text" }`)
- Triggers Svelte 5 reactivity (object mutation is tracked)
- Persists to localStorage

---

## Phase 8: UI Rendering

### 8.1 Message Rendering

**File**: `src/App.svelte`
**Lines**: 432-436

```svelte
<div class="space-y-4">
	{#each messages as message (message.id)}
		<ChatMessage {message} />
	{/each}
</div>
```

**Svelte Reactivity**:

- `{#each}` block automatically updates when `messages` changes
- `(message.id)` keyed iteration (efficient updates)
- Each chunk update triggers re-render of `<ChatMessage>`

---

### 8.2 ChatMessage Component

**File**: `src/lib/components/ChatMessage.svelte`
**Lines**: 1-82

```svelte
<script lang="ts">
	import { onMount } from 'svelte';
	import type { Message } from '$lib/types/chat';
	import {
		renderMarkdown,
		copyToClipboard,
		extractCode,
		setupCodeCopyHandlers
	} from '$lib/utils/markdown';

	interface Props {
		message: Message;
	}

	let { message }: Props = $props();

	let copied = $state(false);
	let contentContainer: HTMLDivElement;

	// ★ Reactive markdown rendering ★
	const renderedContent = $derived(renderMarkdown(message.content));
	const codeBlocks = $derived(extractCode(message.content));

	// Combine all code blocks into one string
	const allCode = $derived(codeBlocks.join('\n\n'));

	// ... (copy/save handlers)

	// Setup event delegation for copy buttons (CSP-compliant)
	onMount(() => {
		if (contentContainer) {
			return setupCodeCopyHandlers(contentContainer);
		}
	});
</script>

<div class="flex gap-3 rounded-lg p-4 transition-colors ...">
	<!-- Avatar (User or Bot) -->
	<div class="flex-shrink-0">
		<div
			class="flex h-8 w-8 items-center justify-center rounded-full"
			class:bg-blue-600={message.role === 'user'}
			class:bg-green-600={message.role === 'assistant'}
		>
			{#if message.role === 'user'}
				<User class="h-5 w-5 text-white" />
			{:else}
				<Bot class="h-5 w-5 text-white" />
			{/if}
		</div>
	</div>

	<!-- Content -->
	<div class="min-w-0 flex-1" bind:this={contentContainer}>
		<div class="mb-1 text-sm font-semibold text-gray-700 dark:text-gray-300">
			{message.role === 'user' ? 'You' : 'AI Assistant'}
		</div>

		<!-- ★ Rendered Markdown ★ -->
		<div class="prose prose-sm dark:prose-invert max-w-none break-words ...">
			{@html renderedContent}
		</div>

		<!-- ★ Streaming Indicator ★ -->
		{#if message.isStreaming}
			<div class="mt-2 flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
				<span class="inline-block h-2 w-2 animate-pulse rounded-full bg-green-600"></span>
				Generating...
			</div>
		{/if}

		<!-- Code Actions (Copy/Save) -->
		{#if message.role === 'assistant' && codeBlocks.length > 0 && !message.isStreaming}
			<div class="mt-3 flex gap-2">
				<button onclick={handleCopyAllCode} ...> Copy All Code </button>
				<button onclick={handleSaveAllCode} ...> Save All Code </button>
			</div>
		{/if}
	</div>
</div>
```

**Key Features**:

1. **`$derived` Runes**: Auto-recompute when `message.content` changes
2. **Markdown Rendering**: `renderMarkdown()` converts to HTML (marked.js)
3. **Streaming Indicator**: Shows pulsing dot while `isStreaming === true`
4. **Code Extraction**: Parses ` ```language ``` ` blocks
5. **Copy/Save Actions**: Only shown after streaming completes

---

### 8.3 Markdown Rendering (marked.js)

**File**: `src/lib/utils/markdown.ts`
**Lines**: 1-50 (excerpt)

````typescript
import { marked } from 'marked';

export function renderMarkdown(content: string): string {
	if (!content) return '';

	try {
		// Configure marked for code highlighting
		marked.setOptions({
			breaks: true,
			gfm: true // GitHub Flavored Markdown
		});

		return marked.parse(content) as string;
	} catch (error) {
		console.error('Markdown rendering error:', error);
		return content; // Fallback to raw text
	}
}

export function extractCode(content: string): string[] {
	const codeBlockRegex = /```[\s\S]*?```/g;
	const matches = content.match(codeBlockRegex) || [];

	return matches.map((block) => {
		// Remove ``` delimiters and language identifier
		return block
			.replace(/```(\w+)?\n?/, '')
			.replace(/```$/, '')
			.trim();
	});
}
````

**Rendering Features**:

- Supports GitHub Flavored Markdown (tables, strikethrough)
- Preserves line breaks
- Syntax highlighting (CSS-based)
- Code block extraction for copy/save

---

## Complete Timeline (Typical Flow)

```
T+0ms    │ User presses Enter
         │ ├─ ChatInput.handleKeydown() detects Enter
         │ └─ ChatInput.handleSubmit() validates input
         │
T+2ms    │ App.handleSendMessage() called
         │ ├─ Creates user message
         │ ├─ Creates placeholder assistant message
         │ ├─ Builds context from history
         │ └─ Invokes Tauri IPC command
         │
T+5ms    │ Tauri IPC serialization
         │ ├─ TypeScript → JSON
         │ └─ Cross-process message
         │
T+7ms    │ Backend receives command
         │ ├─ ollama.rs generate_stream() starts
         │ ├─ Validates Ollama URL (localhost check)
         │ ├─ Adds system prompt
         │ ├─ Extends context
         │ └─ HTTP POST to Ollama
         │
T+15ms   │ Ollama processes request
         │ ├─ Loads model (if not cached)
         │ ├─ Generates tokens
         │ └─ Streams chunks via SSE
         │
T+150ms  │ First chunk arrives
         │ ├─ Backend parses JSON
         │ ├─ Emits "ollama_chunk" event
         │ └─ Frontend listener receives chunk
         │
T+152ms  │ Frontend updates UI
         │ ├─ chatsStore.updateMessage()
         │ ├─ Svelte reactivity triggers
         │ ├─ ChatMessage re-renders
         │ └─ Markdown updates
         │
T+160ms  │ Second chunk arrives
         │ └─ (repeat cycle)
         │
T+3000ms │ Final chunk arrives
         │ ├─ response.done === true
         │ ├─ Backend emits "ollama_done"
         │ ├─ Frontend clears streaming state
         │ └─ Removes "Generating..." indicator
```

**Total Latency**:

- **IPC Overhead**: ~5-10ms (local process communication)
- **First Chunk**: ~150ms (model inference + network)
- **Subsequent Chunks**: ~10-50ms each (token generation time)
- **Full Response**: 2-5 seconds (depends on prompt complexity)

---

## Presentation Code Snippets (Recommended Order)

### Slide 1: User Input

- **File**: `ChatInput.svelte:27-32` (Enter key handler)
- **File**: `ChatInput.svelte:16-25` (Submit handler)

### Slide 2: Frontend Message Processing

- **File**: `App.svelte:112-174` (handleSendMessage function)
- **File**: `App.svelte:100-109` (buildContext function)

### Slide 3: Tauri IPC Invocation

- **File**: `App.svelte:160-164` (invoke call)
- **File**: `lib.rs:37-50` (command registration)

### Slide 4: Backend Entry & Security

- **File**: `ollama.rs:188-230` (generate_stream function signature + setup)
- **File**: `security/mod.rs:125-155` (URL validation)

### Slide 5: HTTP Request to Ollama

- **File**: `ollama.rs:218-232` (HTTP POST + stream creation)
- **File**: `ollama.rs:10-20` (System prompt)

### Slide 6: Stream Processing Loop

- **File**: `ollama.rs:234-299` (tokio::select! loop)
- Highlight:
  - Cancellation handling
  - Chunk parsing
  - Event emission

### Slide 7: Frontend Event Listeners

- **File**: `App.svelte:246-271` (ollama_chunk listener)
- **File**: `App.svelte:274-285` (ollama_done listener)

### Slide 8: Store Updates & Rendering

- **File**: `chats.svelte.ts:78-88` (updateMessage)
- **File**: `ChatMessage.svelte:17-18` ($derived rendering)
- **File**: `ChatMessage.svelte:72-75` (HTML rendering + streaming indicator)

### Slide 9: Architecture Diagram

- Use the ASCII flow diagram from the top of this document

### Slide 10: Performance & Security Highlights

- Connection pooling (HttpClient)
- Localhost-only validation
- Stream cancellation mechanism
- Svelte 5 fine-grained reactivity

---

## Key Discussion Points for Presentation

### 1. **Type Safety**

- Rust types → JSON → TypeScript interfaces
- Compile-time validation prevents runtime errors
- Example: `OllamaMessage` must match on both sides

### 2. **Async Architecture**

- Everything is non-blocking (Tokio + async/await)
- Frontend doesn't freeze during generation
- User can switch chats while generating

### 3. **Error Handling**

- Network errors caught and displayed to user
- Malformed JSON chunks logged but don't break stream
- Frontend disconnection detected gracefully

### 4. **Security Measures**

- Localhost-only validation (GDPR/FERPA compliance)
- Proper URL parsing (prevents bypass attacks)
- Connection pooling (prevents resource exhaustion)

### 5. **User Experience**

- Optimistic UI (placeholder message immediately)
- Streaming feedback (no "loading..." spinner)
- Smooth auto-scroll (unless user scrolled up)
- Cancel button (user control)

### 6. **Code Quality**

- Separation of concerns (components, stores, commands)
- Dependency injection (Tauri managed state)
- Educational system prompt (age-appropriate)
- Comprehensive error logging

---

## Questions to Anticipate

**Q**: Why not use WebSockets instead of SSE (Server-Sent Events)?
**A**: Ollama API uses SSE (simpler, HTTP-based, unidirectional). WebSockets would require custom server.

**Q**: How do you prevent race conditions with multiple messages?
**A**: Track `currentStreamingChatId` and `currentStreamingMessageId`. Only one stream active at a time.

**Q**: What if the user switches chats while generating?
**A**: Stream continues in background. Event listeners check `currentChat?.id === currentStreamingChatId` before scrolling.

**Q**: How is the markdown sanitized (XSS prevention)?
**A**: **Not yet implemented** - planned Phase 2 feature with DOMPurify. Currently `{@html}` is used (trusted source: local Ollama).

**Q**: Why Svelte 5 runes instead of Svelte 4 stores?
**A**: Fine-grained reactivity, better TypeScript support, simpler state management.

**Q**: How do you test this flow?
**A**: **Manual testing only** - test suite needs to be added (known limitation).

---

**End of Code Review Document**
**Last Updated**: January 2025
**Total Lines of Code Traced**: ~500 lines across 6 files
**Estimated Presentation Time**: 15-20 minutes
