# SmolPC Code Helper - Frontend Architecture Documentation

## Overview
This document provides a comprehensive guide to the frontend architecture of SmolPC Code Helper, a Svelte 5 + Tauri 2.6.2 desktop application. It explains data flow, state management, component hierarchy, and integration patterns between the frontend and backend.

**Target Audience**: Developers, technical reviewers, and anyone who needs to understand how data flows through the application.

**Technology Stack**:
- **Framework**: Svelte 5 (runes-based reactivity)
- **Desktop Integration**: Tauri 2.6.2
- **Styling**: Tailwind CSS 4
- **Build Tool**: Vite 6.3.5
- **State Management**: Svelte 5 runes ($state, $derived, $effect)
- **Data Persistence**: localStorage

---

## Table of Contents
1. [Application Bootstrap & Entry Points](#section-1-application-bootstrap--entry-points)
2. [State Management Architecture](#section-2-state-management-architecture)
3. [Component Architecture & Hierarchy](#section-3-component-architecture--hierarchy)
4. [Tauri IPC Communication Patterns](#section-4-tauri-ipc-communication-patterns)
5. [Complete Data Flow Walkthroughs](#section-5-complete-data-flow-walkthroughs)
6. [Security & Best Practices](#section-6-security--best-practices)
7. [Utility Functions & Helpers](#section-7-utility-functions--helpers)
8. [Critical User-Facing Features](#section-8-critical-user-facing-features)
9. [Performance Optimizations](#section-9-performance-optimizations)
10. [Development Workflow & File Reference](#section-10-development-workflow--file-reference)

---

# Section 1: Application Bootstrap & Entry Points

## 1.1 Overview

The application bootstrap process is responsible for initializing the Svelte application, setting up global error handlers, and ensuring a graceful user experience even when errors occur. This section covers the complete initialization sequence from the browser loading the JavaScript to the application being fully interactive.

**Key Responsibilities**:
- Mount the Svelte application to the DOM
- Establish global error boundaries
- Initialize backend connections (Ollama)
- Load cached state (chats, settings, hardware)
- Set up event listeners for backend communication
- Create initial UI state

---

## 1.2 Main Entry Point: `main.ts`

**File**: `src/main.ts`

This is the first JavaScript file that executes when the application starts. It's responsible for bootstrapping the Svelte application and establishing error handling boundaries.

### Global Error Handler (Lines 5-16)

```typescript
window.addEventListener('error', (event) => {
	console.error('Global error caught:', event.error);
	document.body.innerHTML = `
		<div style="padding: 20px; font-family: sans-serif;">
			<h1>Application Error</h1>
			<p>An error occurred while loading the application:</p>
			<pre style="background: #f5f5f5; padding: 10px; border-radius: 4px; overflow: auto;">
				${event.error?.stack || event.error?.message || 'Unknown error'}
			</pre>
			<p>Please check the console for more details.</p>
		</div>
	`;
});
```

**What this does**: Catches any unhandled errors that occur anywhere in the application.

**How it works**:
- Listens for the browser's global `error` event
- Logs the error to the console for debugging
- Replaces the entire page with a user-friendly error message
- Displays the error stack trace to help identify the problem

**Why this matters**:
- **User Experience**: Instead of a blank screen or cryptic browser error, users see a clear message
- **Educational Context**: Students see the error stack, which can be a learning opportunity
- **Debugging**: Developers get immediate visibility into startup failures
- **Fail-Safe**: Prevents the application from silently failing

### Unhandled Promise Rejection Handler (Lines 18-21)

```typescript
window.addEventListener('unhandledrejection', (event) => {
	console.error('Unhandled promise rejection:', event.reason);
});
```

**What this does**: Catches promises that reject without a `.catch()` handler.

**How it works**: Listens for unhandled promise rejections and logs them to the console.

**Why this matters**:
- **Async Error Visibility**: Many Tauri IPC calls are async - this catches errors that might otherwise be silent
- **Development Aid**: Helps identify missing error handling in async code
- **Production Safety**: In production, this prevents silent failures in background operations

### Application Mount (Lines 23-38)

```typescript
try {
	mount(App, {
		target: document.getElementById('app')!
	});
} catch (error) {
	console.error('Failed to mount app:', error);
	document.body.innerHTML = `
		<div style="padding: 20px; font-family: sans-serif;">
			<h1>Failed to Start Application</h1>
			<p>The application failed to initialize:</p>
			<pre style="background: #f5f5f5; padding: 10px; border-radius: 4px; overflow: auto;">
				${error instanceof Error ? error.stack : String(error)}
			</pre>
			<p>Please check the console for more details.</p>
		</div>
	`;
	throw error;
}
```

**What this does**: Mounts the root Svelte component (`App.svelte`) to the DOM.

**How it works**:
1. Uses Svelte 5's `mount()` function (replaces Svelte 4's `new App()`)
2. Targets the `<div id="app">` element in `index.html`
3. If mounting fails, displays a user-friendly error screen
4. Re-throws the error to ensure it appears in the console

**Why this matters**:
- **Svelte 5 API**: Uses the new mounting API (not compatible with Svelte 4)
- **Critical Section**: If this fails, the entire application is unavailable
- **Error Recovery**: Even if mounting fails, users see a helpful message instead of a blank screen
- **Debugging**: Stack trace helps identify component initialization issues

**Connection to User Experience**: This is the first moment the user sees the application interface. If successful, the `App.svelte` component renders and the UI becomes interactive.

---

## 1.3 Root Component: `App.svelte`

**File**: `src/App.svelte`

The root component is the heart of the application. It orchestrates all major functionality including layout, event handling, state management, and communication between the frontend and backend.

### Component Imports (Lines 1-22)

```typescript
import { onMount } from 'svelte';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import Sidebar from '$lib/components/Sidebar.svelte';
import ChatMessage from '$lib/components/ChatMessage.svelte';
import ChatInput from '$lib/components/ChatInput.svelte';
import StatusIndicator from '$lib/components/StatusIndicator.svelte';
import HardwareIndicator from '$lib/components/HardwareIndicator.svelte';
import ModelSelector from '$lib/components/ModelSelector.svelte';
import ContextToggle from '$lib/components/ContextToggle.svelte';
import QuickExamples from '$lib/components/QuickExamples.svelte';
import BenchmarkPanel from '$lib/components/BenchmarkPanel.svelte';
import HardwarePanel from '$lib/components/HardwarePanel.svelte';
import { chatsStore } from '$lib/stores/chats.svelte';
import { settingsStore } from '$lib/stores/settings.svelte';
import { ollamaStore } from '$lib/stores/ollama.svelte';
import { hardwareStore } from '$lib/stores/hardware.svelte';
import type { Message } from '$lib/types/chat';
import type { OllamaMessage } from '$lib/types/ollama';
```

**What this does**: Imports all dependencies needed for the root component.

**How it's organized**:
1. **Svelte lifecycle**: `onMount` for initialization
2. **Tauri APIs**: `invoke` for commands, `listen` for events
3. **UI Components**: All child components that make up the interface
4. **Stores**: Reactive state containers for chats, settings, Ollama, and hardware
5. **Types**: TypeScript interfaces for type safety

**Why this matters**:
- **Dependency Visibility**: Shows all major systems the root component interacts with
- **Architecture Overview**: The imports reveal the component's responsibilities
- **Type Safety**: TypeScript imports ensure compile-time checks for data structures

### UI State Variables (Lines 24-38)

```typescript
// UI State
let isSidebarOpen = $state(true);
let isGenerating = $state(false);
let showQuickExamples = $state(true);
let messagesContainer: HTMLDivElement;
let inputAreaRef: HTMLDivElement;
let userHasScrolledUp = $state(false);
let cancelRequested = $state(false);
let currentStreamingChatId = $state<string | null>(null);
let currentStreamingMessageId = $state<string | null>(null);
let userInteractedWithScroll = $state(false);
let touchStartY = $state(0);
let showBenchmarkPanel = $state(false);
let showHardwarePanel = $state(false);
let bottomOffset = $state(0);
```

**What this does**: Declares reactive UI state using Svelte 5 runes.

**How it works**:
- **`$state()`**: Creates reactive variables that trigger UI updates when changed
- **Type annotations**: Some variables specify types (e.g., `$state<string | null>(null)`)
- **DOM references**: `messagesContainer` and `inputAreaRef` store references to HTML elements

**Key State Variables Explained**:

| Variable | Purpose | Impact on UX |
|----------|---------|--------------|
| `isSidebarOpen` | Controls sidebar visibility | Shows/hides chat history |
| `isGenerating` | Tracks if AI is generating response | Disables input, shows cancel button |
| `showQuickExamples` | Controls example prompts display | Helps new users get started |
| `userHasScrolledUp` | Tracks if user scrolled up manually | Disables auto-scroll during streaming |
| `currentStreamingChatId` | Which chat is receiving streamed response | Routes streaming chunks to correct chat |
| `currentStreamingMessageId` | Which message is being streamed | Updates specific message with new text |
| `showBenchmarkPanel` | Developer tool visibility | Hidden feature (Ctrl+Shift+B) |
| `showHardwarePanel` | Hardware details modal visibility | Shows detailed system information |

**Why this matters**:
- **Svelte 5 Runes**: Uses new reactivity API (not compatible with Svelte 4)
- **Fine-Grained Reactivity**: Only components using these variables re-render when they change
- **User Experience**: These variables directly control what users see and can interact with
- **Stream Management**: Tracking streaming state enables background generation (user can switch chats)

**Connection to Backend**:
- `currentStreamingChatId` and `currentStreamingMessageId` are set when calling `invoke('generate_stream')` (line 160)
- They're used when processing `ollama_chunk` events from the backend (line 247-271)

### Derived State (Lines 40-43)

```typescript
// Derived state
const currentChat = $derived(chatsStore.currentChat);
const messages = $derived(currentChat?.messages ?? []);
const hasNoChats = $derived(chatsStore.chats.length === 0);
```

**What this does**: Creates computed values that automatically update when dependencies change.

**How it works**:
- **`$derived()`**: Svelte 5 rune that creates a reactive computed value
- **Dependency tracking**: Svelte automatically tracks which reactive values are read
- **Auto-update**: When dependencies change, derived values recalculate

**Derived Values Explained**:
- **`currentChat`**: The currently active chat from the chat store
- **`messages`**: Array of messages in the current chat (empty array if no chat)
- **`hasNoChats`**: Boolean indicating if user has created any chats

**Why this matters**:
- **Performance**: Derived values are cached and only recalculate when dependencies change
- **DRY Principle**: Avoids repeating `chatsStore.currentChat?.messages ?? []` throughout the template
- **Reactivity**: Template automatically updates when derived values change
- **Type Safety**: `currentChat?.messages ?? []` prevents null reference errors

**Connection to Data Flow**:
1. User sends a message → `chatsStore.addMessage()` updates store
2. Store update triggers `currentChat` to recalculate
3. `messages` recalculates because `currentChat` changed
4. Template re-renders with new messages
5. User sees the new message appear

---

## 1.4 Initialization Sequence: `onMount()`

**File**: `src/App.svelte` (Lines 229-340)

The `onMount()` lifecycle hook is where the application truly comes to life. This is where we:
1. Check if the Ollama backend is running
2. Load cached hardware information
3. Set up event listeners for streaming chat
4. Create the initial chat if needed
5. Register keyboard shortcuts

### Event Listener Setup (Lines 245-308)

```typescript
async function setupListeners() {
	// Listen for streaming chunks
	unlistenChunk = await listen<string>('ollama_chunk', (event) => {
		// Only process chunks if we're streaming
		if (!currentStreamingChatId || !currentStreamingMessageId || cancelRequested) {
			return;
		}

		// Find the streaming chat and message
		const streamingChat = chatsStore.chats.find((c) => c.id === currentStreamingChatId);
		if (!streamingChat) return;

		const streamingMessage = streamingChat.messages.find((m) => m.id === currentStreamingMessageId);
		if (!streamingMessage || streamingMessage.role !== 'assistant' || !streamingMessage.isStreaming) {
			return;
		}

		// Update the message content
		chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
			content: streamingMessage.content + event.payload
		});

		// Only scroll if this is the currently displayed chat
		if (currentChat?.id === currentStreamingChatId) {
			scrollToBottom();
		}
	});

	// Listen for generation complete
	unlistenDone = await listen('ollama_done', () => {
		if (!currentStreamingChatId || !currentStreamingMessageId) return;

		// Mark the streaming message as complete
		chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
			isStreaming: false
		});

		isGenerating = false;
		currentStreamingChatId = null;
		currentStreamingMessageId = null;
	});

	// Listen for cancellation
	unlistenCancelled = await listen('ollama_cancelled', () => {
		isGenerating = false;
		currentStreamingChatId = null;
		currentStreamingMessageId = null;
	});

	// Listen for errors
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
```

**What this does**: Sets up event listeners for all Ollama backend events.

**How it works**:
- **`listen()`**: Tauri API function that registers event handlers
- **Returns unlisten function**: Each `listen()` call returns a function to cleanup the listener
- **Event types**: Four events handle the complete streaming lifecycle

**Event Types Explained**:

| Event | When It Fires | Handler Responsibility | User Impact |
|-------|---------------|----------------------|-------------|
| `ollama_chunk` | Backend streams new text | Append chunk to message content | User sees text appear character-by-character |
| `ollama_done` | Generation completes | Mark message as complete | Streaming indicator disappears, input re-enabled |
| `ollama_cancelled` | User cancels generation | Reset streaming state | Generation stops, input re-enabled |
| `ollama_error` | Backend error occurs | Display error in message | User sees error message instead of response |

**Critical Pattern - Background Streaming**:
```typescript
// Only scroll if this is the currently displayed chat
if (currentChat?.id === currentStreamingChatId) {
	scrollToBottom();
}
```

This allows the user to switch to a different chat while a response is generating in the background. The message still receives chunks, but the UI only scrolls if the user is viewing that chat.

**Why this matters**:
- **Event-Driven Architecture**: Frontend reacts to backend events instead of polling
- **Real-Time Updates**: Text streams to the UI as it's generated
- **Error Handling**: All error cases have dedicated handlers
- **Memory Safety**: Unlisten functions prevent memory leaks
- **Multi-Chat Support**: Background generation enables better UX

**Connection to Backend**:
1. Backend calls `window.emit("ollama_chunk", chunk)` (Rust code in `src-tauri/src/commands/ollama.rs`)
2. Frontend receives event via `listen()` callback
3. Frontend updates UI state
4. Svelte reactivity triggers component re-render
5. User sees updated text

### Initial State Loading (Lines 310-322)

```typescript
// Initial Ollama check
ollamaStore.checkConnection();

// Load cached hardware info
hardwareStore.getCached();

// Setup event listeners and track cleanup
const cleanupPromise = setupListeners();

// Create initial chat if none exists
if (hasNoChats) {
	chatsStore.createChat(settingsStore.selectedModel);
}

// Add keyboard event listener
window.addEventListener('keydown', handleKeyDown);
```

**What this does**: Initializes the application state in the correct order.

**How it works** (step-by-step):

1. **Check Ollama Connection** (`ollamaStore.checkConnection()`)
   - Calls Tauri command `check_ollama` to verify Ollama is running
   - Updates `ollamaStore.status.connected` (true/false)
   - If connected, fetches available models
   - **User sees**: Green/red status indicator in header

2. **Load Hardware Info** (`hardwareStore.getCached()`)
   - Calls Tauri command `get_cached_hardware`
   - Returns cached hardware data if available (fast)
   - If no cache, triggers full detection automatically
   - **User sees**: GPU name in hardware indicator (or "Detecting..." if not cached)

3. **Setup Event Listeners** (`setupListeners()`)
   - Registers handlers for `ollama_chunk`, `ollama_done`, `ollama_error`, `ollama_cancelled`
   - Returns promise for cleanup coordination
   - **User impact**: Streaming chat becomes functional

4. **Create Initial Chat** (if needed)
   - Only creates chat if `chatsStore.chats.length === 0`
   - Uses currently selected model from settings
   - **User sees**: "New Chat" appears in sidebar

5. **Register Keyboard Shortcuts**
   - Listens for `Ctrl+Shift+B` / `Cmd+Shift+B` to toggle benchmark panel
   - **User can**: Access hidden developer tools

**Order Matters**:
- Ollama check must complete before attempting to generate responses
- Hardware detection can happen async (doesn't block UI)
- Event listeners must be set up before any generation requests
- Initial chat created last (depends on settings being loaded)

**Why this matters**:
- **Lazy Loading**: Hardware detection only runs when needed (not on every startup)
- **Graceful Degradation**: App works even if Ollama isn't running (shows error state)
- **Fast Startup**: Cached data loaded first, full detection happens in background
- **User-Ready State**: By the time `onMount()` completes, the app is fully interactive

**Connection to User Experience**:
```
User opens app
    ↓
main.ts mounts App.svelte
    ↓
App.svelte renders initial UI (loading state)
    ↓
onMount() fires
    ↓
Ollama check completes (0.1-0.5s)
    ↓
Status indicator turns green ✓
    ↓
Hardware cache loaded (instant)
    ↓
Hardware indicator shows GPU name ✓
    ↓
Initial chat created (if needed)
    ↓
App fully interactive ✓
```

### Cleanup on Unmount (Lines 328-339)

```typescript
// Cleanup - wait for setup to complete before cleaning up
return async () => {
	await cleanupPromise;
	if (unlistenChunk) unlistenChunk();
	if (unlistenDone) unlistenDone();
	if (unlistenError) unlistenError();
	if (unlistenCancelled) unlistenCancelled();
	window.removeEventListener('keydown', handleKeyDown);
	window.removeEventListener('resize', handleResize);
	if (window.visualViewport) {
		window.visualViewport.removeEventListener('resize', handleResize);
	}
};
```

**What this does**: Cleans up resources when the component is destroyed.

**How it works**:
- **Return function**: `onMount()` can return a cleanup function
- **Async wait**: Waits for `setupListeners()` promise to ensure listeners exist
- **Unlisten calls**: Each Tauri event listener is removed
- **Window listeners**: Keyboard and resize handlers removed

**Why this matters**:
- **Memory Leak Prevention**: Event listeners must be removed to free memory
- **Resource Management**: Prevents handlers from firing after component destruction
- **Best Practice**: Critical for single-page applications (though less critical in Tauri)

**When this runs**:
- In a typical Tauri app, this rarely runs (root component persists)
- Would run during hot-reload in development
- Important for maintainability and correctness

---

## 1.5 Reactive Effects

Svelte 5 introduces `$effect()` for side effects that should run when reactive values change. `App.svelte` uses effects to handle chat switching and autoscroll.

### Chat Switch Effect (Lines 343-350)

```typescript
// Watch for chat changes
$effect(() => {
	// Track current chat ID to trigger effect
	currentChat?.id;

	// Reset scroll state when switching chats
	userHasScrolledUp = false;
	userInteractedWithScroll = false;
});
```

**What this does**: Resets scroll behavior when the user switches to a different chat.

**How it works**:
- **`$effect()`**: Runs whenever reactive dependencies change
- **Dependency**: `currentChat?.id` is read, so effect runs when it changes
- **State reset**: Scroll flags reset to allow autoscroll in new chat

**Why this matters**:
- **User Expectation**: When switching chats, users expect to see the most recent messages
- **Scroll Behavior**: If user scrolled up in Chat A, then switched to Chat B, Chat B should start at the bottom
- **Clean Slate**: Each chat switch resets autoscroll behavior

**Connection to User Experience**:
```
User clicks different chat in sidebar
    ↓
chatsStore.setCurrentChat(chatId)
    ↓
currentChat changes (derived value updates)
    ↓
$effect() detects currentChat.id changed
    ↓
userHasScrolledUp = false
    ↓
Messages effect triggers scrollToBottom()
    ↓
User sees latest messages in new chat ✓
```

### Autoscroll Effect (Lines 352-357)

```typescript
// Watch messages to auto-scroll
$effect(() => {
	if (messages.length > 0) {
		scrollToBottom();
	}
});
```

**What this does**: Automatically scrolls to the bottom when messages change.

**How it works**:
- **Dependency**: `messages.length` is read, so effect runs when it changes
- **Conditional**: Only scrolls if there are messages to display
- **Calls**: `scrollToBottom()` which respects `userHasScrolledUp` flag

**Why this matters**:
- **Streaming Chat**: As chunks arrive, messages update and viewport scrolls
- **New Messages**: When user or assistant sends a message, viewport scrolls to show it
- **User Override**: `scrollToBottom()` checks `userHasScrolledUp` before scrolling

**Connection to Data Flow**:
```
Backend emits ollama_chunk event
    ↓
Event listener updates message content
    ↓
chatsStore.updateMessage() modifies messages array
    ↓
messages derived value recalculates
    ↓
$effect() detects messages changed
    ↓
scrollToBottom() called
    ↓
Viewport scrolls (if user hasn't scrolled up)
    ↓
User sees new text appear at bottom ✓
```

---

## 1.6 Initialization Summary

### Complete Startup Sequence

Here's the full sequence from browser to interactive app:

```
1. Browser loads index.html
2. Browser executes src/main.ts
3. Global error handlers registered
4. Svelte mount() called
5. App.svelte component created
6. Svelte renders initial UI (loading state)
7. Browser displays UI to user
8. onMount() lifecycle hook fires
   a. ollamaStore.checkConnection() - backend check
   b. hardwareStore.getCached() - load hardware data
   c. setupListeners() - register event handlers
   d. Create initial chat (if needed)
   e. Register keyboard shortcuts
9. Ollama check completes
   → Status indicator updates (green/red)
10. Hardware cache loads
   → Hardware indicator shows GPU name
11. Event listeners active
   → App ready to receive streaming chunks
12. App fully interactive ✓
```

**Timeline** (typical startup on modern hardware):
- **0ms**: Browser loads HTML
- **50ms**: JavaScript executes, mount() called
- **100ms**: Initial UI renders
- **200ms**: onMount() fires
- **300ms**: Ollama check completes
- **301ms**: Hardware cache loads (nearly instant)
- **350ms**: App fully interactive

**Total time to interactive**: ~350ms

### Key Takeaways

1. **Graceful Error Handling**: Global error boundaries ensure users see helpful messages instead of blank screens

2. **Lazy Loading**: Hardware detection uses cached data for instant loading, full detection happens in background

3. **Event-Driven Architecture**: Backend communicates via Tauri events, frontend reacts with listeners

4. **Svelte 5 Reactivity**: Uses `$state`, `$derived`, and `$effect` for fine-grained updates

5. **Memory Safety**: All event listeners cleaned up on unmount

6. **User-Centric Design**: App is interactive within ~350ms, with clear loading states

### Files Involved in Bootstrap

| File | Lines | Responsibility |
|------|-------|----------------|
| `src/main.ts` | 1-39 | Error handlers, mount app |
| `src/App.svelte` | 1-22 | Imports and dependencies |
| `src/App.svelte` | 24-43 | State and derived values |
| `src/App.svelte` | 229-340 | Initialization sequence |
| `src/App.svelte` | 343-357 | Reactive effects |
| `src/lib/stores/ollama.svelte.ts` | - | Ollama connection check |
| `src/lib/stores/hardware.svelte.ts` | - | Hardware detection |
| `src/lib/stores/chats.svelte.ts` | - | Chat creation |

### Next Section Preview

Now that we understand how the application boots up and initializes, Section 2 will dive deep into the **State Management Architecture**, exploring how Svelte 5 stores manage reactive data, persist to localStorage, and communicate with the backend via Tauri IPC.

---

# Section 2: State Management Architecture

## 2.1 Overview

State management is the backbone of the application's data flow. SmolPC Code Helper uses **Svelte 5 runes** (`$state`, `$derived`, `$effect`) to create reactive stores that manage all application data. This section covers the architecture, patterns, and implementation details of five core stores.

**Key Principles**:
- **Single Source of Truth**: Each store owns specific data domains
- **Reactive by Default**: Changes automatically propagate to UI
- **Type-Safe**: Full TypeScript coverage for all state
- **Persistent**: Critical data saved to localStorage
- **Backend Integration**: Stores communicate with Tauri IPC

**Store Hierarchy**:
```
Frontend State
├── chatsStore (conversation history)
├── settingsStore (user preferences)
├── ollamaStore (backend connection status)
├── hardwareStore (system information)
└── benchmarkStore (developer tool state)
```

---

## 2.2 Svelte 5 Runes Pattern

Before diving into individual stores, let's understand the common pattern used across all stores.

### The Runes-Based Store Pattern

**File Example**: `src/lib/stores/chats.svelte.ts`

```typescript
// 1. Import utilities and types
import type { Chat } from '$lib/types/chat';
import { saveToStorage, loadFromStorage } from '$lib/utils/storage';

// 2. Define storage keys
const STORAGE_KEY = 'smolpc_chats';

// 3. Load initial state from localStorage
const initialChats = loadFromStorage<Chat[]>(STORAGE_KEY, []);

// 4. Create reactive state with $state()
let chats = $state<Chat[]>(initialChats);

// 5. Create derived state with $derived()
const sortedChats = $derived<Chat[]>(
    [...chats].sort((a, b) => b.updatedAt - a.updatedAt)
);

// 6. Export store object with getters and methods
export const chatsStore = {
    // Getters (expose reactive state)
    get chats() {
        return chats;
    },
    get sortedChats() {
        return sortedChats;
    },

    // Actions (mutate state)
    createChat(model: string) {
        chats = [...chats, newChat]; // Immutable update
        this.persist();
    },

    // Persistence
    persist() {
        saveToStorage(STORAGE_KEY, chats);
    }
};
```

**Why This Pattern**:

| Aspect | Benefit |
|--------|---------|
| **`$state()`** | Fine-grained reactivity - only changed values trigger updates |
| **Getters** | Components access state via `store.property` (reactive) |
| **Immutable Updates** | `chats = [...chats, newChat]` triggers reactivity correctly |
| **Type Safety** | TypeScript ensures compile-time correctness |
| **Encapsulation** | Internal state hidden, only exposed via public API |

**Key Differences from Svelte 4**:
- **No `writable()`**: Use `$state()` instead
- **No `subscribe()`**: Components access getters directly
- **No `update()`**: Directly assign to state variables
- **Simpler API**: Less boilerplate, more intuitive

---

## 2.3 Chat Store - Conversation Management

**File**: `src/lib/stores/chats.svelte.ts`

The chat store is the most complex store, managing unlimited conversation history with multi-chat support, message streaming, and auto-titling.

### Data Structures (Lines 1-2)

```typescript
import type { Chat, Message } from '$lib/types/chat';
```

**Message Interface** (`src/lib/types/chat.ts:1-7`):
```typescript
export interface Message {
    id: string;                    // UUID for unique identification
    role: 'user' | 'assistant';    // Who sent the message
    content: string;                // Message text (markdown)
    timestamp: number;              // Date.now() when created
    isStreaming?: boolean;          // True during live generation
}
```

**Chat Interface** (`src/lib/types/chat.ts:9-16`):
```typescript
export interface Chat {
    id: string;                    // UUID for chat
    title: string;                 // Display name (auto-generated or custom)
    messages: Message[];           // Array of messages in chronological order
    createdAt: number;             // Timestamp when chat created
    updatedAt: number;             // Timestamp of last activity
    model: string;                 // Which Ollama model used (e.g., 'qwen2.5-coder:7b')
}
```

**Why These Structures**:
- **UUIDs**: Enable safe deletion and updates without array index bugs
- **Timestamps**: Used for time-based grouping (Today, Yesterday, etc.)
- **`isStreaming` flag**: Controls UI indicators during live generation
- **`model` per chat**: Each chat can use different models

### State Variables (Lines 12-13)

```typescript
let chats = $state<Chat[]>(initialChats);
let currentChatId = $state<string | null>(initialCurrentId);
```

**What this does**: Creates two reactive state variables.

**How it works**:
- **`chats`**: Array of all conversation history
- **`currentChatId`**: ID of currently displayed chat (null if none)
- **`initialChats` / `initialCurrentId`**: Loaded from localStorage on startup (lines 8-9)

**Reactivity Flow**:
```
User creates new chat
    ↓
chatsStore.createChat() modifies chats
    ↓
chats array changes (new item added)
    ↓
Svelte detects change via $state()
    ↓
All components reading chatsStore.chats re-render
    ↓
Sidebar shows new chat in list ✓
```

### Derived State (Lines 16-22)

```typescript
const currentChat = $derived<Chat | null>(
    chats.find((chat) => chat.id === currentChatId) ?? null
);

const sortedChats = $derived<Chat[]>(
    [...chats].sort((a, b) => b.updatedAt - a.updatedAt)
);
```

**What this does**: Creates computed values that automatically recalculate when dependencies change.

**How it works**:
- **`currentChat`**: Finds the chat with matching `currentChatId`
- **`sortedChats`**: Sorts chats by most recently updated first
- **`$derived()`**: Svelte 5 rune that caches result until dependencies change

**Performance Benefits**:
- **Caching**: `sortedChats` only recalculates when `chats` array changes
- **No Re-computation**: Reading `chatsStore.sortedChats` multiple times uses cached value
- **Automatic Updates**: When `chats` changes, sorted list updates automatically

**Why `?? null`**:
- `find()` returns `undefined` if not found
- `?? null` converts `undefined` to `null` for cleaner type (`Chat | null` instead of `Chat | undefined`)

**Connection to UI**:
```
App.svelte:
    const currentChat = $derived(chatsStore.currentChat);
    const messages = $derived(currentChat?.messages ?? []);

Sidebar.svelte:
    {#each chatsStore.sortedChats as chat}
        <ChatListItem {chat} />
    {/each}
```

### Public API - Getters (Lines 26-38)

```typescript
export const chatsStore = {
    get chats() {
        return chats;
    },
    get currentChatId() {
        return currentChatId;
    },
    get currentChat() {
        return currentChat;
    },
    get sortedChats() {
        return sortedChats;
    },
    // ... methods
};
```

**What this does**: Exposes reactive state as read-only properties.

**How it works**:
- **Getters**: ES6 getters allow `chatsStore.chats` syntax (instead of `chatsStore.chats()`)
- **Reactivity**: Components reading these values automatically subscribe to changes
- **Read-Only**: External code cannot directly modify internal state

**Usage in Components**:
```typescript
// Component reads reactive value
const messages = $derived(chatsStore.currentChat?.messages ?? []);

// Svelte automatically tracks dependency
// When currentChat changes, messages recalculates
// Component re-renders with new messages
```

### Action: Create Chat (Lines 41-54)

```typescript
createChat(model: string): Chat {
    const newChat: Chat = {
        id: crypto.randomUUID(),
        title: 'New Chat',
        messages: [],
        createdAt: Date.now(),
        updatedAt: Date.now(),
        model
    };
    chats = [...chats, newChat];
    currentChatId = newChat.id;
    this.persist();
    return newChat;
}
```

**What this does**: Creates a new chat and sets it as current.

**How it works** (step-by-step):
1. **Generate UUID**: `crypto.randomUUID()` creates unique ID
2. **Create chat object**: Initializes with default values
3. **Add to array**: `[...chats, newChat]` creates new array (immutable update)
4. **Set as current**: Updates `currentChatId` to new chat's ID
5. **Persist**: Saves to localStorage
6. **Return**: Returns created chat (useful for caller)

**Critical Pattern - Immutable Update**:
```typescript
// WRONG - doesn't trigger reactivity
chats.push(newChat);

// RIGHT - triggers reactivity
chats = [...chats, newChat];
```

**Why Immutable**: Svelte 5 reactivity tracks assignments (`=`), not mutations (`.push()`). The spread operator creates a new array, triggering reactivity.

**Connection to User Experience**:
```
User clicks "New Chat" button in Sidebar
    ↓
Sidebar.svelte calls chatsStore.createChat(model)
    ↓
New chat added to chats array
    ↓
currentChatId set to new chat ID
    ↓
currentChat derived value updates
    ↓
App.svelte sees currentChat changed
    ↓
UI shows empty chat with "New Chat" title ✓
```

### Action: Add Message (Lines 63-76)

```typescript
addMessage(chatId: string, message: Message) {
    const chat = chats.find((c) => c.id === chatId);
    if (chat) {
        chat.messages = [...chat.messages, message];
        chat.updatedAt = Date.now();

        // Auto-generate title from first user message
        if (chat.messages.length === 1 && message.role === 'user') {
            chat.title = message.content.slice(0, 50) +
                (message.content.length > 50 ? '...' : '');
        }

        this.persist();
    }
}
```

**What this does**: Adds a message to a specific chat and auto-generates title if needed.

**How it works**:
1. **Find chat**: Searches `chats` array for matching ID
2. **Add message**: Immutable append to messages array
3. **Update timestamp**: Marks chat as recently active
4. **Auto-title**: If this is the first user message, use it as title (max 50 chars)
5. **Persist**: Save to localStorage

**Auto-Title Logic**:
```typescript
if (chat.messages.length === 1 && message.role === 'user') {
    chat.title = message.content.slice(0, 50) +
        (message.content.length > 50 ? '...' : '');
}
```

**Why This Condition**:
- **`messages.length === 1`**: First message in chat
- **`message.role === 'user'`**: User message (not assistant)
- **Result**: User sees their question as the chat title (better UX than "New Chat")

**Example**:
```
User types: "How do I implement a binary search in Python?"
    ↓
Title becomes: "How do I implement a binary search in Python?"

User types: "Explain the time complexity of merge sort and quicksort algorithms in detail"
    ↓
Title becomes: "Explain the time complexity of merge sort and qui..."
```

**Connection to Streaming**:
```
User sends message
    ↓
App.svelte calls addMessage() twice:
    1. addMessage(chatId, userMessage)
    2. addMessage(chatId, assistantPlaceholder) // isStreaming: true
    ↓
Messages appear in UI
    ↓
Backend starts streaming
    ↓
updateMessage() called repeatedly with chunks
    ↓
User sees text appear character-by-character
```

### Action: Update Message (Lines 78-88)

```typescript
updateMessage(chatId: string, messageId: string, updates: Partial<Message>) {
    const chat = chats.find((c) => c.id === chatId);
    if (chat) {
        const message = chat.messages.find((m) => m.id === messageId);
        if (message) {
            Object.assign(message, updates);
            chat.updatedAt = Date.now();
            this.persist();
        }
    }
}
```

**What this does**: Updates specific fields of a message (used for streaming).

**How it works**:
1. **Find chat**: Locate chat by ID
2. **Find message**: Locate message by ID within chat
3. **Apply updates**: Use `Object.assign()` to merge partial updates
4. **Update timestamp**: Mark chat as active
5. **Persist**: Save to localStorage

**Critical for Streaming**:
```typescript
// During streaming, called many times per second
chatsStore.updateMessage(chatId, messageId, {
    content: currentContent + newChunk  // Append new text
});

// When streaming completes
chatsStore.updateMessage(chatId, messageId, {
    isStreaming: false  // Remove streaming indicator
});
```

**`Partial<Message>` Type**:
```typescript
type Partial<Message> = {
    id?: string;
    role?: 'user' | 'assistant';
    content?: string;
    timestamp?: number;
    isStreaming?: boolean;
}
```

All fields optional - update only what you specify.

**Performance Consideration**:
- Called 50-100+ times during a typical response
- `Object.assign()` is fast (mutates in place)
- `persist()` debounced by browser (localStorage async)
- Svelte reactivity efficient (only re-renders changed message component)

### Action: Delete Chat (Lines 90-100)

```typescript
deleteChat(id: string) {
    const index = chats.findIndex((chat) => chat.id === id);
    if (index !== -1) {
        chats = chats.filter((chat) => chat.id !== id);
        if (currentChatId === id) {
            currentChatId = chats.length > 0 ? chats[0].id : null;
            saveToStorage(CURRENT_CHAT_KEY, currentChatId);
        }
        this.persist();
    }
}
```

**What this does**: Deletes a chat and handles current chat switching.

**How it works**:
1. **Find index**: Verify chat exists
2. **Filter out**: Create new array without deleted chat
3. **Handle current chat**: If deleting active chat, switch to another
4. **Persist both**: Save chats array and current chat ID

**Smart Switching Logic**:
```typescript
if (currentChatId === id) {
    currentChatId = chats.length > 0 ? chats[0].id : null;
    saveToStorage(CURRENT_CHAT_KEY, currentChatId);
}
```

**Why This Matters**:
- If user deletes the chat they're viewing, we need to show a different chat
- Falls back to first chat in array (most recently updated due to sorting)
- If no chats left, sets to `null` (triggers empty state in UI)

**Connection to User Experience**:
```
User clicks delete on "Python Tutorial" chat (currently viewing)
    ↓
Confirmation modal appears (in Sidebar component)
    ↓
User confirms deletion
    ↓
chatsStore.deleteChat(chatId)
    ↓
Chat removed from chats array
    ↓
currentChatId switches to next most recent chat
    ↓
currentChat derived value updates
    ↓
App.svelte shows different chat ✓
```

### Persistence Strategy (Lines 118-120)

```typescript
persist() {
    saveToStorage(STORAGE_KEY, chats);
}
```

**What this does**: Saves entire chat history to localStorage.

**How it works**:
- **`saveToStorage()`**: Utility wrapper around `localStorage.setItem()`
- **JSON serialization**: Converts `chats` array to JSON string
- **Automatic**: Called after every mutation

**Storage Keys** (Lines 4-5):
```typescript
const STORAGE_KEY = 'smolpc_chats';           // Stores chats array
const CURRENT_CHAT_KEY = 'smolpc_current_chat'; // Stores active chat ID
```

**Why Two Keys**:
- `smolpc_chats`: Large (entire conversation history)
- `smolpc_current_chat`: Small (just a UUID string)
- Separating allows loading current chat ID without parsing all chats

**Data Size Considerations**:
- localStorage typically limited to 5-10MB
- Each message ~200-1000 bytes (depending on content)
- ~5,000-25,000 messages before hitting limits
- **Future**: Migrate to IndexedDB for larger storage

**Connection to Startup** (from Section 1):
```
Browser loads app
    ↓
main.ts executes
    ↓
chats.svelte.ts module loads
    ↓
Lines 8-9 execute:
    const initialChats = loadFromStorage<Chat[]>(STORAGE_KEY, []);
    const initialCurrentId = loadFromStorage<string | null>(CURRENT_CHAT_KEY, null);
    ↓
State initialized with persisted data
    ↓
User sees previous chats immediately ✓
```

---

## 2.4 Settings Store - User Preferences

**File**: `src/lib/stores/settings.svelte.ts`

The settings store manages user preferences that affect how the application behaves.

### Data Structure (Lines 1-2)

```typescript
import type { AppSettings } from '$lib/types/settings';
import { DEFAULT_SETTINGS } from '$lib/types/settings';
```

**AppSettings Interface** (`src/lib/types/settings.ts:1-6`):
```typescript
export interface AppSettings {
    selectedModel: string;        // Which Ollama model to use
    contextEnabled: boolean;       // Include conversation history
    temperature: number;           // 0.0-1.0 (creativity/randomness)
    theme: 'light' | 'dark' | 'system';  // UI theme
}
```

**Default Settings** (`src/lib/types/settings.ts:19-24`):
```typescript
export const DEFAULT_SETTINGS: AppSettings = {
    selectedModel: 'qwen2.5-coder:7b',
    contextEnabled: true,
    temperature: 0.7,
    theme: 'system'
};
```

**Why These Defaults**:
- **Qwen 2.5 Coder**: Best code generation model at 7B size
- **Context enabled**: Better responses with conversation history
- **Temperature 0.7**: Balanced between deterministic and creative
- **System theme**: Respects user's OS preference

### State Variable (Lines 11)

```typescript
let settings = $state<AppSettings>(initialSettings);
```

**What this does**: Single reactive object containing all settings.

**Why Single Object**:
- **Atomic updates**: All settings saved together
- **Type safety**: TypeScript validates entire settings object
- **Simple persistence**: One `saveToStorage()` call saves everything

### Convenience Getters (Lines 19-30)

```typescript
get selectedModel() {
    return settings.selectedModel;
},
get contextEnabled() {
    return settings.contextEnabled;
},
get temperature() {
    return settings.temperature;
},
get theme() {
    return settings.theme;
}
```

**What this does**: Provides direct access to individual settings.

**Why Convenience Getters**:
```typescript
// Without getters
if (settingsStore.settings.contextEnabled) { }

// With getters
if (settingsStore.contextEnabled) { }
```

Cleaner syntax, less nesting.

### Action: Set Model (Lines 33-36)

```typescript
setModel(model: string) {
    settings.selectedModel = model;
    this.persist();
}
```

**What this does**: Changes the AI model used for new chats.

**How it works**:
1. **Mutate property**: Directly assign to `settings.selectedModel`
2. **Persist**: Save to localStorage
3. **Reactivity**: All components reading `selectedModel` update

**Connection to UI**:
```
User selects "DeepSeek Coder" from ModelSelector dropdown
    ↓
ModelSelector.svelte calls settingsStore.setModel('deepseek-coder:6.7b')
    ↓
settings.selectedModel updates
    ↓
Svelte reactivity triggers
    ↓
ModelSelector shows new selection ✓
    ↓
Next chat created uses new model
```

### Action: Toggle Context (Lines 38-41)

```typescript
toggleContext() {
    settings.contextEnabled = !settings.contextEnabled;
    this.persist();
}
```

**What this does**: Switches between single-turn and multi-turn conversations.

**Impact on Behavior**:
```typescript
// In App.svelte:100-109
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

**When `contextEnabled = false`**:
- Each user message treated independently
- AI doesn't remember previous conversation
- Faster responses (less tokens to process)
- Useful for unrelated quick questions

**When `contextEnabled = true`**:
- Full conversation history sent with each request
- AI remembers context ("it" refers to previous topic)
- Better for extended discussions
- Default mode

**Example**:
```
Context Enabled:
User: "How do I sort an array in Python?"
AI: "Use the sorted() function..."
User: "Show me an example"
AI: [Understands "example" means sorting example]

Context Disabled:
User: "How do I sort an array in Python?"
AI: "Use the sorted() function..."
User: "Show me an example"
AI: [Doesn't know what "example" refers to - treats as new question]
```

### Action: Set Temperature (Lines 48-51)

```typescript
setTemperature(temp: number) {
    settings.temperature = Math.max(0, Math.min(1, temp));
    this.persist();
}
```

**What this does**: Controls randomness/creativity of AI responses.

**How it works**:
- **Clamp value**: `Math.max(0, Math.min(1, temp))` ensures 0.0-1.0 range
- **Persist**: Save to localStorage

**Temperature Effects**:

| Value | Behavior | Use Case |
|-------|----------|----------|
| 0.0 | Deterministic | Same input → same output, code generation |
| 0.3 | Low randomness | Predictable, factual responses |
| 0.7 | Balanced (default) | Mix of creativity and consistency |
| 1.0 | High creativity | Varied responses, brainstorming |

**Implementation Note**: Temperature passed to Ollama backend but not currently used (would require modifying `generate_stream` command to accept it).

---

## 2.5 Ollama Store - Backend Connection

**File**: `src/lib/stores/ollama.svelte.ts`

The Ollama store manages the connection to the Ollama backend and tracks available models.

### Data Structure (Lines 2)

```typescript
import type { OllamaStatus } from '$lib/types/ollama';
```

**OllamaStatus Interface** (`src/lib/types/ollama.ts:6-10`):
```typescript
export interface OllamaStatus {
    connected: boolean;   // Is Ollama reachable?
    checking: boolean;    // Currently checking connection?
    error?: string;       // Error message if connection failed
}
```

### State Variables (Lines 5-11)

```typescript
let status = $state<OllamaStatus>({
    connected: false,
    checking: false,
    error: undefined
});

let availableModels = $state<string[]>([]);
```

**What this does**: Tracks connection state and downloaded models.

**State Machine**:
```
Initial: { connected: false, checking: false }
    ↓ checkConnection() called
Checking: { connected: false, checking: true }
    ↓ Tauri command returns
Connected: { connected: true, checking: false }
    OR
Error: { connected: false, checking: false, error: "..." }
```

### Action: Check Connection (Lines 27-47)

```typescript
async checkConnection(): Promise<boolean> {
    status.checking = true;
    status.error = undefined;

    try {
        const connected = await invoke<boolean>('check_ollama');
        status.connected = connected;
        status.checking = false;

        if (connected) {
            await this.fetchModels();
        }

        return connected;
    } catch (error) {
        status.connected = false;
        status.checking = false;
        status.error = error instanceof Error ? error.message : 'Failed to connect to Ollama';
        return false;
    }
}
```

**What this does**: Checks if Ollama is running and fetches available models.

**How it works**:
1. **Set checking state**: Shows loading indicator in UI
2. **Call Tauri command**: `invoke('check_ollama')` hits Ollama HTTP endpoint
3. **Update status**: Set `connected` based on response
4. **Fetch models**: If connected, load available models
5. **Handle errors**: Catch network/connection errors

**Backend Implementation** (`src-tauri/src/commands/ollama.rs`):
```rust
#[tauri::command]
pub async fn check_ollama(state: State<'_, HttpClient>) -> Result<bool, String> {
    let url = "http://localhost:11434/api/tags"; // Ollama API endpoint
    match state.client.get(url).send().await {
        Ok(response) => Ok(response.status().is_success()),
        Err(_) => Ok(false)
    }
}
```

**Connection to UI**:
```
App.svelte onMount:
    ollamaStore.checkConnection()
    ↓
StatusIndicator watches ollamaStore.status
    ↓
If connected: Green dot + "Connected"
If checking: Yellow dot + "Checking..."
If error: Red dot + error message
```

### Action: Fetch Models (Lines 49-57)

```typescript
async fetchModels(): Promise<void> {
    try {
        const models = await invoke<string[]>('get_ollama_models');
        availableModels = models;
    } catch (error) {
        console.error('Failed to fetch models:', error);
        availableModels = [];
    }
}
```

**What this does**: Loads list of models downloaded in Ollama.

**Backend Implementation**:
```rust
#[tauri::command]
pub async fn get_ollama_models(state: State<'_, HttpClient>) -> Result<Vec<String>, String> {
    let url = "http://localhost:11434/api/tags";
    let response = state.client.get(url).send().await?;
    let data: TagsResponse = response.json().await?;
    Ok(data.models.iter().map(|m| m.name.clone()).collect())
}
```

**Connection to UI**:
```
ModelSelector.svelte displays:
    - AVAILABLE_MODELS (hardcoded defaults: Qwen, DeepSeek)
    - ollamaStore.availableModels (actually downloaded)

If Ollama has additional models:
    - User can select them
    - Not shown if not downloaded
```

---

## 2.6 Hardware Store - System Information

**File**: `src/lib/stores/hardware.svelte.ts`

The hardware store manages system hardware detection with lazy loading and caching.

### State Variables (Lines 5-7)

```typescript
let hardware = $state<HardwareInfo | null>(null);
let loading = $state(false);
let error = $state<string | undefined>(undefined);
```

**What this does**: Tracks hardware data and detection status.

**State Flow**:
```
Initial: hardware = null, loading = false
    ↓ getCached() called
Loading: hardware = null, loading = true
    ↓ Backend returns cached data OR triggers detection
Loaded: hardware = {...}, loading = false
    OR
Error: hardware = null, loading = false, error = "..."
```

### Action: Detect (Lines 22-34)

```typescript
async detect(): Promise<void> {
    loading = true;
    error = undefined;

    try {
        hardware = await invoke<HardwareInfo>('detect_hardware');
    } catch (e) {
        error = String(e);
        console.error('Failed to detect hardware:', e);
    } finally {
        loading = false;
    }
}
```

**What this does**: Triggers full hardware detection.

**How it works**:
1. **Set loading**: Shows "Detecting..." indicator
2. **Call backend**: `invoke('detect_hardware')` runs detection via hardware-query crate
3. **Update hardware**: Stores result in state
4. **Handle errors**: Logs and displays error message

**Backend Detection** (`src-tauri/src/commands/hardware.rs`):
Uses `hardware_query` crate to detect:
- CPU (vendor, cores, frequency, features)
- GPU (name, VRAM, backend)
- NPU (Apple Neural Engine, Intel AI Boost, etc.)
- Memory (total/available)
- Storage (capacity, type)

**Performance**: Takes 50-200ms depending on system.

### Action: Get Cached (Lines 36-49)

```typescript
async getCached(): Promise<void> {
    try {
        const cached = await invoke<HardwareInfo | null>('get_cached_hardware');
        if (cached) {
            hardware = cached;
        } else {
            // If no cached data, trigger detection automatically
            await this.detect();
        }
    } catch (e) {
        console.error('Failed to get cached hardware:', e);
    }
}
```

**What this does**: Loads cached hardware data, or triggers detection if no cache.

**How it works**:
1. **Request cache**: Call `get_cached_hardware` backend command
2. **If cached**: Use cached data (instant)
3. **If no cache**: Automatically call `detect()` (first-time detection)

**Backend Caching** (`src-tauri/src/commands/hardware.rs`):
```rust
pub struct HardwareCache {
    info: OnceCell<Mutex<Option<HardwareInfo>>>,
}

// First call to detect_hardware() or get_cached_hardware() runs detection
// Subsequent calls return cached result
```

**Why Lazy Loading**:
- **Fast Startup**: App doesn't wait for hardware detection on startup
- **Cache Benefit**: Second launch instant (uses cache)
- **User-Triggered**: Only runs when needed (user clicks hardware indicator)

**Connection to Startup** (from Section 1):
```
App.svelte onMount():
    hardwareStore.getCached()
    ↓
Backend checks cache
    ↓
If cached (second+ launch): Returns immediately
If no cache (first launch): Triggers detection
    ↓
hardware state updates
    ↓
HardwareIndicator shows GPU name ✓
```

### Helper: Get Primary GPU (Lines 52-62)

```typescript
getPrimaryGpu() {
    if (!hardware || hardware.gpus.length === 0) {
        return null;
    }

    // Prefer discrete GPU
    const discrete = hardware.gpus.find((gpu) =>
        gpu.device_type.toLowerCase().includes('discrete')
    );
    return discrete || hardware.gpus[0];
}
```

**What this does**: Returns the most relevant GPU for display.

**How it works**:
1. **Check for GPUs**: Return null if no hardware data or no GPUs
2. **Prefer discrete**: Look for "discrete" in device_type (dedicated GPU)
3. **Fallback**: Use first GPU if no discrete found (integrated GPU)

**Why Prefer Discrete**:
- Systems may have both integrated + dedicated GPU
- Dedicated GPU more relevant for AI performance
- Example: Intel UHD Graphics (integrated) + NVIDIA RTX 3060 (discrete)
  → Show RTX 3060

**Connection to UI**:
```typescript
// HardwareIndicator.svelte
const gpu = hardwareStore.getPrimaryGpu();
const displayText = gpu?.name ?? 'Detecting...';
```

---

## 2.7 Benchmark Store - Developer Tool State

**File**: `src/lib/stores/benchmark.svelte.ts`

The benchmark store tracks the state of the hidden benchmarking tool (accessed via Ctrl+Shift+B).

### Data Structure (Lines 3-15)

```typescript
export interface BenchmarkProgress {
    current: number;       // Current test number (1-12)
    total: number;         // Total tests (12)
    current_test: string;  // Test name ("Code generation", etc.)
    iteration: number;     // Current iteration (1-3)
}

export interface BenchmarkState {
    isRunning: boolean;          // Is benchmark currently executing?
    progress: BenchmarkProgress | null;  // Current progress
    error: string | null;        // Error message if failed
    lastResultPath: string | null;  // Path to JSON results
}
```

### State Variable (Lines 25)

```typescript
let state = $state<BenchmarkState>({ ...initialState });
```

**What this does**: Single state object for all benchmark tracking.

### Actions (Lines 47-72)

```typescript
start() {
    state.isRunning = true;
    state.progress = null;
    state.error = null;
    state.lastResultPath = null;
}

updateProgress(progress: BenchmarkProgress) {
    state.progress = progress;
}

complete(resultPath: string) {
    state.isRunning = false;
    state.lastResultPath = resultPath;
    state.progress = null;
}

setError(error: string) {
    state.isRunning = false;
    state.error = error;
    state.progress = null;
}
```

**What these do**: Manage benchmark execution lifecycle.

**Lifecycle Flow**:
```
User clicks "Run Benchmark"
    ↓
BenchmarkPanel calls benchmarkStore.start()
    ↓
BenchmarkPanel calls invoke('run_comprehensive_benchmark')
    ↓
Backend emits 'benchmark_progress' events
    ↓
BenchmarkPanel listener calls updateProgress()
    ↓
Progress bar updates in UI
    ↓
Backend emits 'benchmark_complete' event
    ↓
BenchmarkPanel listener calls complete(path)
    ↓
"Open Results" button enabled ✓
```

**Why Separate Store**:
- Benchmark is developer tool (not user-facing)
- Keeps benchmark state isolated from main app state
- Can be easily removed or extended without affecting other stores

---

## 2.8 Store Communication & Data Flow

### How Stores Interact

Stores don't directly communicate with each other. Instead, they communicate through:
1. **Components** (coordination)
2. **Shared Tauri backend** (indirect)
3. **localStorage** (persistence)

**Example: Creating a Chat with Current Model**

```
User clicks "New Chat" in Sidebar
    ↓
Sidebar.svelte reads:
    const model = settingsStore.selectedModel
    ↓
Sidebar.svelte calls:
    chatsStore.createChat(model)
    ↓
New chat created with current model setting ✓
```

**Example: Checking Ollama Before Sending**

```
User types message in ChatInput
    ↓
App.svelte handleSendMessage() checks:
    if (!ollamaStore.isConnected) return;
    ↓
If not connected, message not sent
If connected, message sent ✓
```

### Persistence Strategy Summary

| Store | Storage Key | What's Saved | When |
|-------|-------------|--------------|------|
| chatsStore | `smolpc_chats` | All conversation history | After every message/chat mutation |
| chatsStore | `smolpc_current_chat` | Active chat ID | When current chat changes |
| settingsStore | `smolpc_settings` | User preferences | After every setting change |
| ollamaStore | N/A | Not persisted | Checked on every startup |
| hardwareStore | N/A | Not persisted | Backend caches in memory |
| benchmarkStore | N/A | Not persisted | Developer tool, ephemeral |

**Why Different Strategies**:
- **Chats**: Must persist (user data)
- **Settings**: Must persist (user preferences)
- **Ollama**: Dynamic (backend might restart, check fresh)
- **Hardware**: Static (cached in backend, rare changes)
- **Benchmark**: Temporary (results saved to files)

---

## 2.9 Type Safety & Synchronization

### TypeScript-Rust Type Mapping

Critical: Frontend types must match backend Rust types exactly for Tauri IPC serialization.

**Example: HardwareInfo**

**TypeScript** (`src/lib/types/hardware.ts`):
```typescript
export interface HardwareInfo {
    cpu: CpuInfo;
    gpus: GpuInfo[];
    npu?: NpuInfo;
    memory: MemoryInfo;
    storage: StorageInfo;
    detected_at: string;
}
```

**Rust** (`src-tauri/src/hardware/types.rs`):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub cpu: CpuInfo,
    pub gpus: Vec<GpuInfo>,
    pub npu: Option<NpuInfo>,
    pub memory: MemoryInfo,
    pub storage: StorageInfo,
    pub detected_at: String,
}
```

**Mapping Rules**:

| Rust | TypeScript |
|------|------------|
| `String` | `string` |
| `u32` / `u64` | `number` |
| `bool` | `boolean` |
| `Vec<T>` | `T[]` |
| `Option<T>` | `T \| undefined` or `T \| null` |
| `struct` | `interface` |

**What Happens if Types Mismatch**:
```
Backend returns: { cpu: {...}, gpus: [...], npu: null, ... }
Frontend expects: { cpu: CpuInfo, gpus: GpuInfo[], npu?: NpuInfo, ... }
    ↓
Serde serialization: ✓ (JSON conversion works)
TypeScript runtime: ✓ (types are compile-time only)
    ↓
BUT: If you try to access npu.confidence:
    TypeScript thinks: npu might be NpuInfo
    Runtime reality: npu is null
    ↓
Result: Runtime error if not handled with optional chaining
```

**Best Practice**:
```typescript
// WRONG - runtime error if npu is null
const confidence = hardwareStore.info.npu.confidence;

// RIGHT - safe optional chaining
const confidence = hardwareStore.info?.npu?.confidence ?? 0;
```

---

## 2.10 State Management Summary

### Key Takeaways

1. **Svelte 5 Runes**: All stores use `$state()`, `$derived()`, not Svelte 4 `writable()`

2. **Immutable Updates**: Always create new arrays/objects to trigger reactivity
   ```typescript
   // WRONG
   chats.push(newChat);

   // RIGHT
   chats = [...chats, newChat];
   ```

3. **Getters for Reactivity**: Components access state via getters, not direct property access

4. **Persistence**: Critical data (chats, settings) saved to localStorage after every mutation

5. **Type Safety**: TypeScript interfaces match Rust structs exactly for IPC

6. **Lazy Loading**: Hardware detection uses cache-first strategy for fast startup

7. **Error Handling**: All async operations wrapped in try/catch with user-friendly errors

### Store Responsibilities

| Store | Owns | Persists | Backend Calls |
|-------|------|----------|---------------|
| **chatsStore** | Conversation history | localStorage | None (events only) |
| **settingsStore** | User preferences | localStorage | None |
| **ollamaStore** | Connection status | No | `check_ollama`, `get_ollama_models` |
| **hardwareStore** | System info | No | `detect_hardware`, `get_cached_hardware` |
| **benchmarkStore** | Test progress | No | `run_comprehensive_benchmark` |

### Data Flow Pattern

```
User Action (UI Event)
    ↓
Component calls Store method
    ↓
Store updates internal $state
    ↓
Svelte reactivity triggers
    ↓
All dependent $derived values recalculate
    ↓
Components reading store values re-render
    ↓
UI updates ✓
    ↓
Store.persist() saves to localStorage (if applicable)
```

### Next Section Preview

Now that we understand state management, Section 3 will explore **Component Architecture & Hierarchy**, diving into how UI components consume store data, handle user interactions, and compose into a complete interface.

---

# Section 3: Component Architecture & Hierarchy

## 3.1 Overview

Components are the building blocks of the UI. SmolPC Code Helper uses Svelte 5 components that consume store data, handle user interactions, and render the interface. This section explores the component hierarchy, communication patterns, and key implementation details.

**Key Principles**:
- **Single Responsibility**: Each component has one clear purpose
- **Store Integration**: Components read from stores, never directly mutate
- **Event Communication**: Parent-child communication via props and events
- **Reactive Props**: Using Svelte 5 `$props()` rune
- **Type Safety**: Full TypeScript coverage for props and events

---

## 3.2 Component Hierarchy

The application follows a tree structure where `App.svelte` is the root and child components handle specific UI concerns.

### Visual Hierarchy

```
App.svelte (Root)
│
├── Sidebar.svelte
│   └── Uses: chatsStore, settingsStore
│   └── Renders: Chat list grouped by time
│   └── Confirmation modal (for delete)
│
├── Header (inline in App.svelte)
│   ├── HardwareIndicator.svelte
│   │   └── Uses: hardwareStore
│   └── StatusIndicator.svelte
│       └── Uses: ollamaStore
│
├── Controls Bar (inline in App.svelte)
│   ├── ModelSelector.svelte
│   │   └── Uses: settingsStore
│   └── ContextToggle.svelte
│       └── Uses: settingsStore
│
├── Messages Area
│   ├── QuickExamples.svelte (if no messages)
│   │   └── Emits: onSelectExample
│   └── ChatMessage.svelte (for each message)
│       └── Uses: markdown utils
│       └── Handles: copy/save code actions
│
├── Input Area
│   ├── Cancel Button (if generating)
│   └── ChatInput.svelte
│       └── Emits: onSend
│       └── Auto-resize textarea
│
├── BenchmarkPanel.svelte (hidden, Ctrl+Shift+B)
│   └── Uses: benchmarkStore
│   └── Event listeners for progress
│
└── HardwarePanel.svelte (modal)
    └── Uses: hardwareStore
    └── Detailed hardware info display
```

---

## 3.3 Sidebar Component

**File**: `src/lib/components/Sidebar.svelte`

The sidebar displays chat history grouped by time, handles chat selection, and provides new chat creation and deletion.

### Props Definition (Lines 8-13)

```typescript
interface Props {
    isOpen: boolean;
    onClose?: () => void;
}

let { isOpen = true, onClose }: Props = $props();
```

**What this does**: Defines component props using Svelte 5 `$props()` rune.

**How it works**:
- **`isOpen`**: Controls sidebar visibility (default true)
- **`onClose`**: Optional callback for closing sidebar (mobile responsiveness)
- **`$props()`**: Svelte 5 rune for reactive props (replaces Svelte 4 `export let`)

**Why This Pattern**:
```typescript
// Svelte 4 (old)
export let isOpen = true;
export let onClose = undefined;

// Svelte 5 (new)
let { isOpen = true, onClose }: Props = $props();
```

Benefits:
- **Type Safety**: Props interface enforces types
- **Destructuring**: Direct access to `isOpen` (not `$$props.isOpen`)
- **Defaults**: Inline default values

### Derived State - Time Grouping (Line 15)

```typescript
const chatGroups = $derived(groupChatsByTime(chatsStore.sortedChats));
```

**What this does**: Groups chats into time buckets (Today, Yesterday, Last 7 Days, Older).

**How it works**:
1. **Read store**: `chatsStore.sortedChats` (already sorted by `updatedAt`)
2. **Call utility**: `groupChatsByTime()` from `src/lib/utils/date.ts`
3. **Cache result**: `$derived()` only recalculates when `sortedChats` changes

**Result Structure**:
```typescript
[
  { label: "Today", chats: [...] },
  { label: "Yesterday", chats: [...] },
  { label: "Last 7 Days", chats: [...] },
  { label: "Older", chats: [...] }
]
```

**Connection to UI** (Lines 71-108):
```svelte
{#each chatGroups as group (group.label)}
    <h3>{group.label}</h3>
    {#each group.chats as chat (chat.id)}
        <!-- Chat item -->
    {/each}
{/each}
```

Nested loops: outer loop for time groups, inner loop for chats within group.

### New Chat Handler (Lines 17-19)

```typescript
function handleNewChat() {
    chatsStore.createChat(settingsStore.selectedModel);
}
```

**What this does**: Creates new chat with currently selected model.

**How it works**:
1. **Read setting**: `settingsStore.selectedModel` (e.g., "qwen2.5-coder:7b")
2. **Call store action**: `chatsStore.createChat(model)`
3. **Store handles**: Chat creation, UUID generation, setting as current, persistence

**Connection to User Experience**:
```
User clicks "New Chat" button (line 63)
    ↓
handleNewChat() called
    ↓
chatsStore.createChat() adds chat
    ↓
chatsStore.currentChatId set to new chat
    ↓
App.svelte detects currentChat changed
    ↓
UI shows empty chat ✓
```

### Chat Selection Handler (Lines 21-26)

```typescript
function handleSelectChat(chatId: string) {
    chatsStore.setCurrentChat(chatId);
    if (window.innerWidth < 768 && onClose) {
        onClose();
    }
}
```

**What this does**: Switches to selected chat and closes sidebar on mobile.

**How it works**:
1. **Update store**: `chatsStore.setCurrentChat(chatId)` changes `currentChatId`
2. **Mobile responsiveness**: If screen width < 768px, call `onClose()` callback
3. **Parent handles**: `App.svelte` receives callback and sets `isSidebarOpen = false`

**Why Mobile Closing**:
- On mobile, sidebar overlays content (full width)
- After selecting chat, user wants to see messages
- Auto-close provides better UX than requiring manual close

**Responsive Behavior**:
```
Desktop (>768px):
    User clicks chat → switches chat, sidebar stays open

Mobile (<768px):
    User clicks chat → switches chat, sidebar closes
```

### Delete Confirmation Pattern (Lines 28-48)

```typescript
// State for in-app confirmation modal
let showConfirm = $state(false);
let pendingDeleteId: string | null = $state(null);

function requestDelete(id: string) {
    pendingDeleteId = id;
    showConfirm = true;
}

function confirmDelete() {
    if (pendingDeleteId) {
        chatsStore.deleteChat(pendingDeleteId);
    }
    pendingDeleteId = null;
    showConfirm = false;
}

function cancelDelete() {
    pendingDeleteId = null;
    showConfirm = false;
}
```

**What this does**: Implements in-app confirmation modal (avoids `window.confirm()`).

**How it works**:
1. **User clicks delete** (line 97) → `requestDelete(chat.id)` called
2. **Store chat ID**: `pendingDeleteId` tracks which chat to delete
3. **Show modal**: `showConfirm = true` renders modal
4. **User confirms**: `confirmDelete()` deletes chat and closes modal
5. **User cancels**: `cancelDelete()` just closes modal

**Why Not `window.confirm()`**:
```typescript
// AVOID - platform-dependent, blocks event loop
if (window.confirm("Delete chat?")) {
    chatsStore.deleteChat(chatId);
}

// PREFER - custom modal, consistent styling, non-blocking
requestDelete(chatId); // Shows in-app modal
```

Benefits:
- **Consistent UX**: Matches app's dark mode, styling
- **Non-blocking**: Doesn't freeze browser
- **Accessible**: Keyboard support (ESC to cancel, lines 127-129)
- **Professional**: Better than browser's generic dialogs

**Modal Implementation** (Lines 119-142):
```svelte
{#if showConfirm}
    <div class="fixed inset-0 z-50 flex items-center justify-center">
        <!-- Dark overlay (backdrop) -->
        <div class="absolute inset-0 bg-black/40" onclick={cancelDelete}></div>

        <!-- Modal content -->
        <div class="relative z-10 ...">
            <h4>Delete chat</h4>
            <p>Are you sure? This action cannot be undone.</p>
            <div class="flex justify-end gap-2">
                <Button variant="outline" onclick={cancelDelete}>No</Button>
                <Button variant="destructive" onclick={confirmDelete}>Yes, delete</Button>
            </div>
        </div>
    </div>
{/if}
```

**Z-index layering**:
- Modal container: `z-50` (above all content)
- Backdrop: `absolute inset-0` (fills screen)
- Content: `z-10` (above backdrop within container)

### Delete Button Visibility (Lines 90-104)

```svelte
<button
    type="button"
    onclick={(e: MouseEvent) => {
        e.preventDefault();
        e.stopPropagation();
        requestDelete(chat.id);
    }}
    class="... opacity-0 group-hover:opacity-100 ..."
>
    <Trash2 class="h-3 w-3 text-red-600" />
</button>
```

**What this does**: Shows delete button only when hovering over chat item.

**How it works**:
- **Default state**: `opacity-0` (invisible)
- **Hover state**: `group-hover:opacity-100` (visible)
- **Event handling**: `e.stopPropagation()` prevents chat selection when clicking delete

**Why `stopPropagation()`**:
```
Without stopPropagation:
    User clicks delete button
        ↓
    Delete handler fires (requestDelete)
        ↓
    Event bubbles to parent button (handleSelectChat)
        ↓
    Chat switches AND delete modal shows (wrong!)

With stopPropagation:
    User clicks delete button
        ↓
    Delete handler fires (requestDelete)
        ↓
    Event stops (doesn't bubble)
        ↓
    Only delete modal shows ✓
```

---

## 3.4 ChatMessage Component

**File**: `src/lib/components/ChatMessage.svelte`

The ChatMessage component renders individual messages with markdown, streaming indicators, and code actions.

### Props Definition (Lines 8-12)

```typescript
interface Props {
    message: Message;
}

let { message }: Props = $props();
```

**What this does**: Receives a single message object to render.

**Message Type** (from `src/lib/types/chat.ts`):
```typescript
interface Message {
    id: string;
    role: 'user' | 'assistant';
    content: string;
    timestamp: number;
    isStreaming?: boolean;
}
```

### Derived State - Markdown Rendering (Lines 17-21)

```typescript
const renderedContent = $derived(renderMarkdown(message.content));
const codeBlocks = $derived(extractCode(message.content));
const allCode = $derived(codeBlocks.join('\n\n'));
```

**What this does**: Processes message content for display.

**How it works**:
1. **`renderedContent`**: Converts markdown to sanitized HTML
2. **`codeBlocks`**: Extracts all code blocks as array of strings
3. **`allCode`**: Combines all code blocks (for "Copy All" action)

**Why `$derived()`**:
- **During streaming**: `message.content` changes 50-100+ times
- **Recalculation**: Each change triggers markdown re-render
- **Performance**: `$derived()` ensures efficient updates
- **Caching**: Same content = cached result (no re-render)

**Markdown Processing** (from `src/lib/utils/markdown.ts`):
```typescript
export function renderMarkdown(text: string): string {
    // 1. Extract code blocks
    // 2. Escape HTML
    // 3. Process inline code, links, headers, lists
    // 4. DOMPurify sanitization
    return sanitizedHTML;
}
```

### Event Delegation for Code Copy (Lines 40-45)

```typescript
onMount(() => {
    if (contentContainer) {
        return setupCodeCopyHandlers(contentContainer);
    }
});
```

**What this does**: Sets up CSP-compliant code copy buttons.

**How it works**:
1. **Wait for mount**: `onMount()` ensures DOM exists
2. **Get container**: `contentContainer` bound to div (line 67)
3. **Setup delegation**: `setupCodeCopyHandlers()` adds single event listener
4. **Return cleanup**: Function returned by `setupCodeCopyHandlers()` removes listener

**Why Event Delegation**:
```
Without delegation (CSP violation):
    Each code block: <button onclick="copyCode()">...</button>
    Problem: Inline onclick violates Content Security Policy

With delegation (CSP-compliant):
    Container: <div> addEventListener('click', handler) </div>
    Each code block: <button class="code-copy-btn" data-code="...">
    Handler checks: if (target.matches('.code-copy-btn')) { copy() }
```

**Implementation** (from `src/lib/utils/markdown.ts:200-230`):
```typescript
export function setupCodeCopyHandlers(container: HTMLElement): () => void {
    const handleClick = async (event: Event) => {
        const button = target.closest('.code-copy-btn');
        if (!button) return;

        const encodedCode = button.dataset.code;
        const code = decodeBase64(encodedCode);
        await navigator.clipboard.writeText(code);

        // Show checkmark for 2s
    };

    container.addEventListener('click', handleClick);
    return () => container.removeEventListener('click', handleClick);
}
```

### Streaming Indicator (Lines 76-81)

```svelte
{#if message.isStreaming}
    <div class="mt-2 flex items-center gap-2 text-xs text-gray-500">
        <span class="inline-block h-2 w-2 animate-pulse rounded-full bg-green-600"></span>
        Generating...
    </div>
{/if}
```

**What this does**: Shows animated indicator during message generation.

**How it works**:
- **Condition**: Only renders if `message.isStreaming === true`
- **Animation**: `animate-pulse` CSS class (Tailwind) pulses opacity
- **Visual**: Green dot + "Generating..." text

**Lifecycle**:
```
1. Message created with isStreaming: true
    ↓
2. Indicator appears, pulses
    ↓
3. Backend streams chunks, content updates
    ↓
4. Backend emits 'ollama_done' event
    ↓
5. App.svelte sets isStreaming: false
    ↓
6. Indicator disappears ✓
```

### Code Actions (Lines 84-110)

```svelte
{#if message.role === 'assistant' && codeBlocks.length > 0 && !message.isStreaming}
    <div class="mt-3 flex gap-2">
        <button onclick={handleCopyAllCode}>
            <Copy class="h-3 w-3" />
            <span>Copy All Code</span>
        </button>
        <button onclick={handleSaveAllCode}>
            <Download class="h-3 w-3" />
            <span>Save All Code</span>
        </button>
    </div>
{/if}
```

**What this does**: Shows "Copy All Code" and "Save All Code" buttons.

**Conditions**:
1. **`message.role === 'assistant'`**: Only for AI responses (not user messages)
2. **`codeBlocks.length > 0`**: Only if message contains code
3. **`!message.isStreaming`**: Only after generation completes

**Why Wait for Streaming to Complete**:
- Prevents copying incomplete code
- Buttons appear after full response received
- Better UX (user knows generation is done)

**Copy All Handler** (Lines 23-29):
```typescript
async function handleCopyAllCode() {
    const success = await copyToClipboard(allCode);
    if (success) {
        copied = true;
        setTimeout(() => (copied = false), 2000);
    }
}
```

**Save All Handler** (Lines 31-38):
```typescript
async function handleSaveAllCode() {
    try {
        await invoke('save_code', { code: allCode });
    } catch (error) {
        console.error('Failed to save code:', error);
        alert('Failed to save file. Please try again.');
    }
}
```

**Backend Integration**:
```rust
// src-tauri/src/commands/default.rs
#[tauri::command]
pub async fn save_code(code: String) -> Result<(), String> {
    // Opens file save dialog
    // Writes code to selected file
    Ok(())
}
```

---

## 3.5 ChatInput Component

**File**: `src/lib/components/ChatInput.svelte`

A simple but powerful auto-resizing textarea with keyboard shortcuts.

### Props Definition (Lines 5-11)

```typescript
interface Props {
    onSend: (message: string) => void;
    disabled?: boolean;
    placeholder?: string;
}

let { onSend, disabled = false, placeholder = 'Ask a coding question...' }: Props = $props();
```

**What this does**: Defines component API.

**Props**:
- **`onSend`**: Callback function when user submits message
- **`disabled`**: Disables input during generation
- **`placeholder`**: Customizable placeholder text

### Auto-Resize Pattern (Lines 34-39)

```typescript
function handleInput() {
    if (textarea) {
        textarea.style.height = 'auto';
        textarea.style.height = textarea.scrollHeight + 'px';
    }
}
```

**What this does**: Grows textarea as user types multiple lines.

**How it works**:
1. **Reset height**: `height = 'auto'` collapses to content height
2. **Measure content**: `scrollHeight` is actual content height
3. **Set height**: Expand textarea to fit content

**Why Two Assignments**:
```typescript
// Wrong - doesn't shrink when user deletes text
textarea.style.height = textarea.scrollHeight + 'px';

// Right - resets then expands
textarea.style.height = 'auto';         // Collapse
textarea.style.height = textarea.scrollHeight + 'px';  // Expand to content
```

**Visual Effect**:
```
1 line:  [Type here...        ] (50px height)
         [                      ]

2 lines: [Line 1               ] (75px height)
         [Line 2               ]

3 lines: [Line 1               ] (100px height)
         [Line 2               ]
         [Line 3               ]
```

Max height capped at 200px (line 53), then scrolls.

### Keyboard Shortcuts (Lines 27-32)

```typescript
function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSubmit();
    }
}
```

**What this does**: Enter submits, Shift+Enter inserts newline.

**How it works**:
- **Enter alone**: Prevent default (no newline), call `handleSubmit()`
- **Shift+Enter**: No handler, default behavior (insert newline)

**User Experience**:
```
User types: "How do I" [presses Enter]
    ↓
Message sent ✓

User types: "Example:" [presses Shift+Enter]
    ↓
Newline inserted (multi-line message) ✓
```

### Submit Handler (Lines 16-25)

```typescript
function handleSubmit() {
    const trimmed = inputValue.trim();
    if (trimmed && !disabled) {
        onSend(trimmed);
        inputValue = '';
        if (textarea) {
            textarea.style.height = 'auto';
        }
    }
}
```

**What this does**: Validates and sends message.

**How it works**:
1. **Trim whitespace**: Remove leading/trailing spaces
2. **Validate**: Check not empty and not disabled
3. **Call callback**: `onSend(trimmed)` (parent handles)
4. **Clear input**: Reset `inputValue`
5. **Reset height**: Collapse textarea to single line

**Connection to Parent**:
```
User types "Hello" and presses Enter
    ↓
ChatInput handleSubmit() called
    ↓
onSend("Hello") called
    ↓
App.svelte handleSendMessage("Hello") called
    ↓
User message added to chat
    ↓
Backend invoked for AI response
```

---

## 3.6 ModelSelector Component

**File**: `src/lib/components/ModelSelector.svelte`

A simple dropdown for selecting AI models.

### Implementation (Lines 1-26)

```typescript
import { settingsStore } from '$lib/stores/settings.svelte';
import { AVAILABLE_MODELS } from '$lib/types/settings';

function handleModelChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    settingsStore.setModel(target.value);
}
```

```svelte
<select value={settingsStore.selectedModel} onchange={handleModelChange}>
    {#each AVAILABLE_MODELS as model}
        <option value={model.name}>
            {model.displayName} {#if model.size}({model.size}){/if}
        </option>
    {/each}
</select>
```

**What this does**: Displays dropdown of available models, updates setting on change.

**How it works**:
1. **Controlled input**: `value={settingsStore.selectedModel}` binds to store
2. **Render options**: Loop through `AVAILABLE_MODELS` constant
3. **Handle change**: `handleModelChange()` calls `settingsStore.setModel()`

**Data Source** (`src/lib/types/settings.ts:14-17`):
```typescript
export const AVAILABLE_MODELS: ModelInfo[] = [
    { name: 'qwen2.5-coder:7b', displayName: 'Qwen 2.5 Coder', size: '7B' },
    { name: 'deepseek-coder:6.7b', displayName: 'DeepSeek Coder', size: '6.7B' }
];
```

**Rendered HTML**:
```html
<select value="qwen2.5-coder:7b">
    <option value="qwen2.5-coder:7b">Qwen 2.5 Coder (7B)</option>
    <option value="deepseek-coder:6.7b">DeepSeek Coder (6.7B)</option>
</select>
```

---

## 3.7 HardwareIndicator Component

**File**: `src/lib/components/HardwareIndicator.svelte`

Displays GPU name (or CPU if no GPU) in header, clickable to open hardware panel.

### Derived State (Lines 11-12)

```typescript
const primaryGpu = $derived(hardwareStore.getPrimaryGpu());
const hasNpu = $derived(hardwareStore.info?.npu?.detected ?? false);
```

**What this does**: Reactively computes display data from hardware store.

**How it works**:
- **`primaryGpu`**: Gets best GPU (discrete preferred over integrated)
- **`hasNpu`**: Checks if NPU (Neural Processing Unit) detected

### Conditional Rendering (Lines 20-32)

```svelte
{#if primaryGpu}
    <Gpu class="h-3 w-3 text-primary" />
    <span class="max-w-[150px] truncate">{primaryGpu.name}</span>
    {#if hasNpu}
        <Zap class="h-3 w-3 text-yellow-500" />
    {/if}
{:else if hardwareStore.info}
    <Cpu class="h-3 w-3 text-primary" />
    <span>CPU Only</span>
{:else}
    <Cpu class="h-3 w-3 animate-pulse" />
    <span>Detecting...</span>
{/if}
```

**What this does**: Shows different states based on hardware detection.

**State Logic**:

| Condition | Icon | Text | Meaning |
|-----------|------|------|---------|
| `primaryGpu` exists | GPU icon | GPU name (e.g., "RTX 3060") | GPU detected |
| `hasNpu` also true | + Zap icon | (in addition) | NPU detected |
| `info` but no GPU | CPU icon | "CPU Only" | Detection complete, no GPU |
| No `info` | CPU icon (pulsing) | "Detecting..." | Detection in progress |

**Examples**:
```
Gaming PC: [GPU icon] RTX 3060
MacBook Pro: [GPU icon] Apple M1 Pro [Zap icon]
Old laptop: [CPU icon] CPU Only
First launch: [CPU icon (pulsing)] Detecting...
```

---

## 3.8 Component Communication Patterns

### Parent → Child (Props)

**Pattern**: Parent passes data to child via props.

**Example** (App.svelte → ChatInput):
```svelte
<!-- App.svelte -->
<ChatInput
    onSend={handleSendMessage}
    disabled={!ollamaStore.isConnected || isGenerating}
    placeholder={isGenerating
        ? 'Generating response...'
        : 'Ask a coding question...'}
/>
```

**Child receives**:
```typescript
// ChatInput.svelte
let { onSend, disabled, placeholder }: Props = $props();
```

### Child → Parent (Event Callbacks)

**Pattern**: Child calls parent function passed as prop.

**Example** (ChatInput → App.svelte):
```typescript
// ChatInput.svelte
function handleSubmit() {
    const trimmed = inputValue.trim();
    if (trimmed && !disabled) {
        onSend(trimmed);  // Call parent's handler
        inputValue = '';
    }
}
```

```typescript
// App.svelte
async function handleSendMessage(content: string) {
    // Handle message submission
    chatsStore.addMessage(currentChat.id, userMessage);
    await invoke('generate_stream', { prompt: content, ... });
}
```

**Data Flow**:
```
User types and submits in ChatInput
    ↓
ChatInput.handleSubmit() validates
    ↓
ChatInput calls onSend(message)
    ↓
App.handleSendMessage(message) executes
    ↓
Message added to store, generation started
```

### Component → Store (Direct Access)

**Pattern**: Components read from stores via getters, call store methods.

**Example** (Sidebar → chatsStore):
```typescript
// Sidebar.svelte
const chatGroups = $derived(groupChatsByTime(chatsStore.sortedChats));

function handleNewChat() {
    chatsStore.createChat(settingsStore.selectedModel);
}
```

**Reactivity**:
```
chatsStore.createChat() modifies store
    ↓
chatsStore.sortedChats derived value updates
    ↓
Sidebar's chatGroups recalculates
    ↓
Sidebar re-renders with new chat ✓
```

### Store → Component (Reactive Getters)

**Pattern**: Components automatically re-render when store values change.

**Example** (HardwareIndicator):
```typescript
const primaryGpu = $derived(hardwareStore.getPrimaryGpu());
```

**Update Flow**:
```
App.svelte onMount calls hardwareStore.getCached()
    ↓
Backend returns hardware data
    ↓
hardwareStore.info updates
    ↓
hardwareStore.getPrimaryGpu() returns new value
    ↓
HardwareIndicator's primaryGpu recalculates
    ↓
Component re-renders with GPU name ✓
```

---

## 3.9 Component Best Practices

### 1. Single Responsibility

Each component has one clear job:
- **Sidebar**: Chat list management
- **ChatMessage**: Render single message
- **ChatInput**: Accept user input
- **ModelSelector**: Model selection dropdown

### 2. Props Interface

Always define TypeScript interface for props:
```typescript
interface Props {
    message: Message;
    onSend?: (text: string) => void;
    disabled?: boolean;
}

let { message, onSend, disabled = false }: Props = $props();
```

### 3. Derived Over Functions in Templates

```svelte
<!-- WRONG - function called on every render -->
<div>{getMessage().content}</div>

<!-- RIGHT - cached until dependencies change -->
<script>
const messageContent = $derived(message.content);
</script>
<div>{messageContent}</div>
```

### 4. Event Delegation for Dynamic Content

Use event delegation for content generated at runtime (like code blocks):
```typescript
// Setup once on mount
onMount(() => {
    container.addEventListener('click', handleClick);
    return () => container.removeEventListener('click', handleClick);
});

// Handle all clicks in container
function handleClick(event: Event) {
    const button = event.target.closest('.code-copy-btn');
    if (button) {
        // Handle copy
    }
}
```

### 5. Cleanup on Unmount

Always cleanup resources:
```typescript
onMount(() => {
    const unlisten = await listen('event_name', handler);
    return () => unlisten();  // Cleanup
});
```

---

## 3.10 Component Summary

### Component Responsibilities

| Component | Responsibility | Stores Used | Emits Events |
|-----------|---------------|-------------|--------------|
| **Sidebar** | Chat list, creation, deletion | chatsStore, settingsStore | - |
| **ChatMessage** | Render message, code actions | - | - |
| **ChatInput** | Accept user input | - | onSend |
| **ModelSelector** | Model selection dropdown | settingsStore | - |
| **HardwareIndicator** | Show GPU/CPU summary | hardwareStore | onclick |
| **StatusIndicator** | Show Ollama connection | ollamaStore | - |
| **ContextToggle** | Toggle context setting | settingsStore | - |
| **QuickExamples** | Show example prompts | - | onSelectExample |
| **HardwarePanel** | Detailed hardware info | hardwareStore | - |
| **BenchmarkPanel** | Run benchmarks | benchmarkStore | - |

### Communication Patterns

1. **Props (Parent → Child)**
   ```svelte
   <ChildComponent prop={value} />
   ```

2. **Callbacks (Child → Parent)**
   ```typescript
   let { onEvent }: Props = $props();
   onEvent(data);
   ```

3. **Stores (Component ↔ Store)**
   ```typescript
   const data = $derived(store.value);
   store.action();
   ```

### Key Takeaways

1. **Svelte 5 Runes**: Use `$props()`, `$state()`, `$derived()` throughout
2. **Type Safety**: All props have TypeScript interfaces
3. **Single Responsibility**: Each component focuses on one concern
4. **Event Delegation**: CSP-compliant event handling for dynamic content
5. **Cleanup**: All event listeners and subscriptions cleaned up on unmount

### Next Section Preview

Now that we understand components, Section 4 will explore **Tauri IPC Communication Patterns**, detailing how the frontend communicates with the Rust backend through commands, events, and streaming.

---

# Section 4: Tauri IPC Communication Patterns

## 4.1 Introduction to Tauri IPC Architecture

### Overview

Tauri's Inter-Process Communication (IPC) is the bridge between the Svelte frontend (running in a WebView) and the Rust backend. This IPC system enables the application to leverage native OS capabilities, file system access, and local AI processing while maintaining a web-based UI.

**Architectural Significance**:
- **Process Isolation**: Frontend and backend run in separate processes for security and stability
- **Type-Safe Communication**: TypeScript and Rust types are serialized/deserialized via JSON
- **Bidirectional Flow**: Both request/response and event-based patterns supported
- **Security Layer**: IPC commands validate inputs and enforce localhost-only constraints

### Two Primary Communication Patterns

The application uses two distinct IPC patterns, each serving specific architectural needs:

#### Pattern A: Request/Response Commands
**Use Case**: Simple, synchronous-style operations with a single return value.

**Characteristics**:
- Frontend invokes a command with `invoke()`
- Backend processes and returns a `Result<T, Error>`
- Single response, then channel closes
- Suitable for: Connection checks, model fetching, file I/O, hardware detection

**Example Operations**:
- `check_ollama()` → `Result<bool, Error>`
- `get_ollama_models()` → `Result<Vec<String>, Error>`
- `detect_hardware()` → `Result<HardwareInfo, String>`

#### Pattern B: Event-Based Streaming
**Use Case**: Long-running operations requiring incremental updates.

**Characteristics**:
- Frontend invokes a command to start the stream
- Backend emits multiple events over time
- Frontend listens for events asynchronously
- Stream continues until completion, error, or cancellation
- Suitable for: AI response generation, long file processing

**Example Operations**:
- `generate_stream()` → emits `ollama_chunk`, `ollama_done`, `ollama_error`, `ollama_cancelled`

### Why Both Patterns?

The dual-pattern approach serves distinct performance and UX requirements:

**Request/Response Rationale**:
- Minimal overhead for simple queries
- Easier error handling (single try/catch)
- Immediate result for fast operations
- Prevents unnecessary event listener overhead

**Event-Based Streaming Rationale**:
- Enables real-time UI updates during long operations
- Non-blocking: User can interact with UI while streaming
- Supports cancellation mid-stream
- Optimal for AI generation (tokens arrive incrementally)
- Prevents frontend timeout on long operations

### IPC Security Model

**Localhost-Only Validation** (`src-tauri/src/security/mod.rs:112-155`):
```rust
pub fn validate_ollama_url(url_str: &str) -> Result<String, String> {
    let url = Url::parse(url_str)?;
    let host = url.host_str().ok_or("URL must have hostname")?;

    match host {
        "localhost" | "127.0.0.1" | "::1" => Ok(url_str.to_string()),
        _ => Err(format!("Security violation: Ollama must run on localhost"))
    }
}
```

**Why This Matters**:
- **GDPR/FERPA Compliance**: Student code/queries stay on local machine
- **No Data Exfiltration**: Prevents malicious configuration from sending data to external servers
- **Educational Context**: Schools require verifiable data privacy

---

## 4.2 IPC Communication Patterns

### 4.2.1 Pattern A: Request/Response Commands

#### Data Flow Sequence

**Frontend → Backend:**
1. TypeScript creates parameter object
2. `invoke<ReturnType>('command_name', { ...params })` called
3. Tauri API serializes params to JSON
4. IPC bridge transports JSON to backend
5. `serde_json` deserializes to Rust struct
6. Command handler executes
7. Returns `Result<T, Error>`

**Backend → Frontend:**
1. Rust `Result<T, Error>` created
2. `serde::Serialize` converts to JSON
3. IPC bridge transports JSON to frontend
4. Tauri API deserializes to JavaScript
5. Promise resolves with typed data or rejects with error

#### Example: Connection Check

**Frontend** (`src/lib/stores/ollama.svelte.ts:27-47`):
```typescript
async function checkConnection() {
    try {
        const connected = await invoke<boolean>('check_ollama');
        status.connected = connected;
        status.error = null;
    } catch (error) {
        status.connected = false;
        status.error = error instanceof Error ? error.message : 'Connection failed';
    }
}
```

**Backend** (`src-tauri/src/commands/ollama.rs:142-156`):
```rust
#[tauri::command]
pub async fn check_ollama(
    client: State<'_, HttpClient>,
    config: State<'_, OllamaConfig>,
) -> Result<bool, Error> {
    let url = format!("{}/api/tags", config.base_url());
    let response = client.get().get(&url).send().await;

    match response {
        Ok(resp) => Ok(resp.status().is_success()),
        Err(_) => Ok(false),
    }
}
```

**Key Points**:
- Async on both sides: `async fn` (Rust) and `await` (TypeScript)
- Managed state injected via `State<'_, T>` parameter
- Error handling: Rust `Result` → TypeScript try/catch
- Type safety: `invoke<boolean>` ensures frontend expects correct type

### 4.2.2 Pattern B: Event-Based Streaming

#### Bidirectional Communication Model

**Invocation Phase**:
- Frontend calls `invoke('generate_stream', { ...params })`
- Backend receives command and starts async task
- Command returns immediately (doesn't block)

**Streaming Phase**:
- Backend emits events: `app_handle.emit("event_name", payload)`
- Frontend listeners receive events: `listen<T>('event_name', callback)`
- Multiple events can be emitted over time
- Events continue until stream completes, errors, or cancels

**Cleanup Phase**:
- Backend emits final event (`ollama_done` or `ollama_error`)
- Backend clears cancellation state
- Frontend updates UI state and stops listening

#### Event Types in AI Streaming

**File**: `src-tauri/src/commands/ollama.rs:188-300`

| Event Name | Payload Type | Purpose | Frequency |
|------------|--------------|---------|-----------|
| `ollama_chunk` | `string` | Incremental text content | Multiple times (per token batch) |
| `ollama_done` | `()` | Stream completed successfully | Once (end) |
| `ollama_error` | `string` | Error message | Once (on error) |
| `ollama_cancelled` | `()` | User cancelled generation | Once (if cancelled) |

#### How Events Work Under the Hood

**Backend Emission** (`src-tauri/src/commands/ollama.rs:260-270`):
```rust
if let Some(message) = response.message {
    // Emit chunk event with text content
    if let Err(e) = app_handle.emit("ollama_chunk", message.content) {
        log::debug!("Frontend disconnected: {e}");
        return Ok(());
    }
}

if response.done {
    cancellation.clear();
    app_handle.emit("ollama_done", ())?;
    return Ok(());
}
```

**Frontend Listening** (`src/App.svelte:247-271`):
```typescript
unlistenChunk = await listen<string>('ollama_chunk', (event) => {
    if (!currentStreamingChatId || !currentStreamingMessageId || cancelRequested) {
        return;
    }

    const streamingChat = chatsStore.chats.find((c) => c.id === currentStreamingChatId);
    const streamingMessage = streamingChat.messages.find((m) => m.id === currentStreamingMessageId);

    // Append chunk to existing content
    chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
        content: streamingMessage.content + event.payload
    });

    // Auto-scroll if viewing active chat
    if (currentChat?.id === currentStreamingChatId) {
        scrollToBottom();
    }
});
```

**Critical Pattern**: Notice how the frontend validates state (`currentStreamingChatId`, `currentStreamingMessageId`) before processing chunks. This prevents race conditions when switching chats or cancelling.

#### Why Streaming Beats Polling

**Polling Approach (NOT used)**:
```typescript
// Hypothetical polling (inefficient)
while (!done) {
    const chunk = await invoke('get_next_chunk');
    updateUI(chunk);
    await sleep(100); // Waste CPU cycles
}
```

**Problems**:
- Busy-waiting wastes CPU
- Fixed polling interval (too fast = wasted requests, too slow = laggy UI)
- No way to know when stream completes without extra state

**Event-Based Approach (ACTUAL implementation)**:
```typescript
// Efficient event-driven
await listen('ollama_chunk', (event) => updateUI(event.payload));
await invoke('generate_stream', { ... }); // Start and forget
```

**Advantages**:
- Zero CPU usage while waiting
- Events arrive exactly when data is ready
- Natural completion signal (`ollama_done`)
- Supports cancellation without polling state

---

## 4.3 The Complete Message Flow (User Clicks "Send")

This section traces the complete journey of a chat message from user input to rendered AI response. Understanding this flow is critical for debugging, optimization, and extending functionality.

### Step 1: User Interaction

**File**: `src/lib/components/ChatInput.svelte:16-25`

```typescript
function handleSubmit() {
    const trimmed = inputValue.trim();
    if (trimmed && !disabled) {
        onSend(trimmed);  // Callback to parent (App.svelte)
        inputValue = '';  // Clear input
    }
}
```

**What Happens**:
- User types message and presses Enter or clicks Send button
- `handleSubmit()` validates input is non-empty
- Calls parent callback `onSend` with trimmed message
- Clears input field immediately (optimistic UI)

**State Changes**:
- `inputValue` reset to empty string
- No store updates yet

### Step 2: Message Handler Initialization

**File**: `src/App.svelte:112-174`

```typescript
async function handleSendMessage(content: string) {
    if (!ollamaStore.isConnected || isGenerating) return;

    // Create new chat if none exists
    if (!currentChat) {
        chatsStore.createChat(settingsStore.selectedModel);
    }

    // Add user message to store
    const userMessage: Message = {
        id: crypto.randomUUID(),
        role: 'user',
        content,
        timestamp: Date.now()
    };
    chatsStore.addMessage(currentChat.id, userMessage);

    // Create empty assistant message placeholder
    const assistantMessage: Message = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: '',  // Empty, will be filled by streaming
        timestamp: Date.now(),
        isStreaming: true  // Frontend-only flag
    };
    chatsStore.addMessage(currentChat.id, assistantMessage);

    isGenerating = true;
    currentStreamingChatId = currentChat.id;
    currentStreamingMessageId = assistantMessage.id;
    // ... (continues in Step 4)
}
```

**What Happens**:
- Validates Ollama is connected and not already generating
- Creates new chat if needed (first message)
- Creates user message object with UUID
- Adds user message to `chatsStore` → triggers Svelte reactivity → UI updates
- Creates **empty** assistant message with `isStreaming: true`
- Tracks which chat/message is currently streaming (for event handlers)

**State Changes**:
- `chatsStore.chats`: New user message appended
- `chatsStore.chats`: New assistant message appended (empty)
- `isGenerating`: `true`
- `currentStreamingChatId`: Current chat ID
- `currentStreamingMessageId`: Assistant message ID

**UI Impact**:
- User sees their message appear immediately
- Empty assistant message box appears with "streaming" indicator

### Step 3: Context Building

**File**: `src/App.svelte:100-109`

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
- Converts all previous messages in current chat to Ollama format
- Strips frontend-only fields like `id`, `timestamp`, `isStreaming`
- Returns array of `{role, content}` objects

**Why This Matters**:
- Context enables multi-turn conversations (AI remembers earlier messages)
- Respects user preference (context can be disabled to save tokens)
- Transforms frontend `Message` type to backend `OllamaMessage` type

**Example Context**:
```typescript
[
    { role: 'user', content: 'How do I use variables?' },
    { role: 'assistant', content: 'In Python, you create variables...' },
    { role: 'user', content: 'Can you show an example?' }  // Current message
]
```

### Step 4: IPC Invocation

**File**: `src/App.svelte:155-164`

```typescript
try {
    const context = buildContext();

    await invoke('generate_stream', {
        prompt: content,
        model: settingsStore.selectedModel,
        context: context.length > 0 ? context : null
    });
} catch (error) {
    // Error handling (shown in Step 10)
}
```

**What Happens**:
- Calls `buildContext()` to get conversation history
- Invokes `generate_stream` command with:
  - `prompt`: User's current message (string)
  - `model`: Selected model from settings (e.g., "qwen2.5-coder:7b")
  - `context`: Previous messages array or `null` if empty

**Data Serialization** (TypeScript → JSON):
```json
{
    "prompt": "Can you show an example?",
    "model": "qwen2.5-coder:7b",
    "context": [
        {"role": "user", "content": "How do I use variables?"},
        {"role": "assistant", "content": "In Python, you create variables..."}
    ]
}
```

**IPC Boundary Crossed**: JSON sent to Rust backend via Tauri IPC bridge.

### Step 5: Backend Command Reception

**File**: `src-tauri/src/commands/ollama.rs:188-216`

```rust
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
    // Create cancellation channel for this stream
    let mut cancel_rx = cancellation.create_channel();

    // Build messages array with system prompt
    let mut messages = vec![OllamaMessage {
        role: "system".to_string(),
        content: SYSTEM_PROMPT.to_string(),  // Student-friendly AI persona
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
    // ... (continues in Step 6)
}
```

**What Happens**:
- Command handler receives deserialized parameters
- Managed state automatically injected (`client`, `config`, `cancellation`)
- Creates broadcast channel for cancellation signaling
- Builds complete messages array:
  1. System prompt (defines AI behavior)
  2. Context messages (conversation history)
  3. Current user prompt

**Example Complete Messages Array**:
```rust
[
    { role: "system", content: "You are a helpful coding assistant..." },
    { role: "user", content: "How do I use variables?" },
    { role: "assistant", content: "In Python, you create variables..." },
    { role: "user", content: "Can you show an example?" }
]
```

**Why System Prompt First**: Ollama requires system prompt as first message to set AI behavior (student-friendly, clear explanations).

### Step 6: Ollama HTTP Request

**File**: `src-tauri/src/commands/ollama.rs:218-232`

```rust
let request = OllamaRequest {
    model,
    messages,
    stream: true,  // Enable streaming response
};

let url = format!("{}/api/chat", config.base_url());  // http://localhost:11434/api/chat
let response = client.get()
    .post(&url)
    .json(&request)
    .send()
    .await
    .map_err(|e| Error::Other(format!("Failed to send request: {e}")))?;

let mut stream = response.bytes_stream();
```

**What Happens**:
- Constructs `OllamaRequest` with `stream: true`
- Gets validated base URL from `OllamaConfig` (localhost-only)
- Uses shared `HttpClient` for connection pooling (reuses TCP connection)
- Sends HTTP POST to Ollama API
- Receives `bytes_stream()` instead of waiting for full response

**HTTP Request Details**:
```http
POST http://localhost:11434/api/chat HTTP/1.1
Content-Type: application/json

{
    "model": "qwen2.5-coder:7b",
    "messages": [
        {"role": "system", "content": "..."},
        {"role": "user", "content": "How do I use variables?"},
        ...
    ],
    "stream": true
}
```

**Why Connection Pooling Matters**: Reusing HTTP connections avoids TCP handshake overhead on repeated requests (significant for localhost, critical for remote servers).

### Step 7: Stream Processing Loop

**File**: `src-tauri/src/commands/ollama.rs:234-299`

```rust
loop {
    tokio::select! {
        // Check for cancellation signal
        _ = cancel_rx.recv() => {
            cancellation.clear();
            app_handle.emit("ollama_cancelled", ())?;
            return Ok(());
        }

        // Process incoming chunks
        chunk_result = stream.next() => {
            match chunk_result {
                Some(Ok(bytes)) => {
                    // Parse JSON lines
                    let text = String::from_utf8(bytes.to_vec())?;
                    for line in text.lines() {
                        let response: OllamaResponse = serde_json::from_str(line)?;

                        if let Some(message) = response.message {
                            // Emit chunk event to frontend
                            app_handle.emit("ollama_chunk", message.content)?;
                        }

                        if response.done {
                            cancellation.clear();
                            app_handle.emit("ollama_done", ())?;
                            return Ok(());
                        }
                    }
                }
                Some(Err(e)) => {
                    cancellation.clear();
                    app_handle.emit("ollama_error", format!("Stream error: {e}"))?;
                    return Err(Error::Other(format!("Stream error: {e}")));
                }
                None => {
                    cancellation.clear();
                    return Ok(());
                }
            }
        }
    }
}
```

**What Happens**:
- **`tokio::select!`**: Concurrently monitors two async operations:
  1. Cancellation signals from `cancel_rx`
  2. Incoming bytes from Ollama stream
- **Cancellation Path**: If user clicks cancel → `cancel_rx` receives signal → emits `ollama_cancelled` → exits loop
- **Chunk Processing Path**:
  - Receives bytes from Ollama
  - Converts to UTF-8 string
  - Parses each line as JSON (Ollama sends newline-delimited JSON)
  - Extracts message content
  - **Emits `ollama_chunk` event** with content string
  - Checks `done` flag → emits `ollama_done` → exits loop
- **Error Path**: Network error → emits `ollama_error` → exits loop

**Example Ollama Response Stream**:
```json
{"message":{"role":"assistant","content":"Sure"},"done":false}
{"message":{"role":"assistant","content":"!"},"done":false}
{"message":{"role":"assistant","content":" Here"},"done":false}
...
{"message":{"role":"assistant","content":""},"done":true,"eval_count":245}
```

**Performance Note**: `tokio::select!` is non-blocking. While waiting for next chunk, CPU is free to handle other tasks (other IPC commands, OS events).

### Step 8: Frontend Event Reception

**File**: `src/App.svelte:247-271`

```typescript
unlistenChunk = await listen<string>('ollama_chunk', (event) => {
    // Validate streaming state
    if (!currentStreamingChatId || !currentStreamingMessageId || cancelRequested) {
        return;
    }

    // Find the streaming chat (may not be currently displayed)
    const streamingChat = chatsStore.chats.find((c) => c.id === currentStreamingChatId);
    if (!streamingChat) return;

    const streamingMessage = streamingChat.messages.find((m) => m.id === currentStreamingMessageId);
    if (!streamingMessage?.isStreaming) return;

    // Append chunk to existing content
    chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
        content: streamingMessage.content + event.payload
    });

    // Only scroll if viewing active chat
    if (currentChat?.id === currentStreamingChatId) {
        scrollToBottom();
    }
});
```

**What Happens**:
- Event listener receives `ollama_chunk` event with payload (string)
- **Validation**: Checks if still in streaming state (prevents race conditions)
- **Finds Target**: Locates the specific chat and message being updated
- **Appends Content**: Concatenates new chunk to existing message content
- **Updates Store**: Calls `chatsStore.updateMessage()` → triggers Svelte reactivity
- **Conditional Scroll**: Only auto-scrolls if user is viewing the streaming chat

**State Changes**:
- `chatsStore.chats[i].messages[j].content`: Appended with new chunk

**UI Impact**:
- ChatMessage component re-renders
- User sees new text appear incrementally

**Background Generation Support**: Notice the `if (currentChat?.id === currentStreamingChatId)` check. This enables users to switch to other chats while generation continues in background.

### Step 9: Stream Completion

**File**: `src/App.svelte:274-285`

```typescript
unlistenDone = await listen('ollama_done', () => {
    if (!currentStreamingChatId || !currentStreamingMessageId) return;

    // Mark message as complete
    chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
        isStreaming: false
    });

    isGenerating = false;
    currentStreamingChatId = null;
    currentStreamingMessageId = null;
});
```

**What Happens**:
- Backend emits `ollama_done` event
- Frontend sets `isStreaming: false` on the message
- Clears global streaming state variables
- Enables Send button (user can send next message)

**State Changes**:
- `chatsStore.chats[i].messages[j].isStreaming`: `false`
- `isGenerating`: `false`
- `currentStreamingChatId`: `null`
- `currentStreamingMessageId`: `null`

**UI Impact**:
- Streaming indicator disappears
- Send button re-enabled
- Message is now "complete" and persisted to localStorage

### Step 10: Error Handling

**File**: `src/App.svelte:165-174`, `src/App.svelte:295-307`

**Invocation Error**:
```typescript
try {
    await invoke('generate_stream', { ... });
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
```

**Stream Error Event**:
```typescript
unlistenError = await listen<string>('ollama_error', (event) => {
    if (!currentStreamingChatId || !currentStreamingMessageId) return;

    chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
        content: `Error: ${event.payload}`,
        isStreaming: false
    });

    isGenerating = false;
    currentStreamingChatId = null;
    currentStreamingMessageId = null;
});
```

**What Happens**:
- Errors can occur at two points:
  1. **Invocation**: Command fails to start (Ollama not running, invalid params)
  2. **Streaming**: Mid-stream failure (network error, Ollama crash)
- Both paths update the assistant message with error text
- Cleanup state to allow retry

**UI Impact**:
- User sees error message in chat
- Can retry by sending another message

---

## 4.4 Registered IPC Commands Reference

All available IPC commands are registered in `src-tauri/src/lib.rs:37-50`. This table provides a complete reference for frontend-backend communication.

### Command Registration

```rust
.invoke_handler(tauri::generate_handler![
    read,
    write,
    save_code,
    check_ollama,
    get_ollama_models,
    generate_stream,
    cancel_generation,
    run_benchmark,
    get_benchmarks_directory,
    open_benchmarks_folder,
    detect_hardware,
    get_cached_hardware
])
```

### Complete Command Reference

| Command | File | Pattern | Parameters | Return Type | Purpose |
|---------|------|---------|------------|-------------|---------|
| `read` | `default.rs:5-18` | Request/Response | `path: String` | `Result<String, Error>` | Read file contents from disk |
| `write` | `default.rs:20-30` | Request/Response | `path: String, contents: String` | `Result<(), Error>` | Write file contents to disk |
| `save_code` | `default.rs:32-62` | Request/Response | `filename: String, code: String` | `Result<String, Error>` | Save code snippet to Documents folder |
| `check_ollama` | `ollama.rs:142-156` | Request/Response | None | `Result<bool, Error>` | Check if Ollama server is running |
| `get_ollama_models` | `ollama.rs:158-178` | Request/Response | None | `Result<Vec<String>, Error>` | Fetch list of available models |
| `generate_stream` | `ollama.rs:188-300` | Event-Based | `prompt: String, model: String, context: Option<Vec<OllamaMessage>>` | `Result<(), Error>` | Stream AI response (emits events) |
| `cancel_generation` | `ollama.rs:182-185` | State Mutation | None | `()` | Cancel active stream |
| `run_benchmark` | `benchmark.rs` | Request/Response | `model: String, test_type: String` | `Result<BenchmarkResult, String>` | Run performance benchmark |
| `get_benchmarks_directory` | `benchmark.rs` | Request/Response | None | `Result<String, String>` | Get path to benchmark results directory |
| `open_benchmarks_folder` | `benchmark.rs` | Request/Response | None | `Result<(), String>` | Open benchmarks folder in file explorer |
| `detect_hardware` | `hardware.rs` | Request/Response | None | `Result<HardwareInfo, String>` | Detect system hardware (triggers detection if not cached) |
| `get_cached_hardware` | `hardware.rs` | Request/Response | None | `Result<Option<HardwareInfo>, String>` | Get cached hardware info (returns None if not detected yet) |

### Command Categories

#### File I/O Commands (`src-tauri/src/commands/default.rs`)

**Purpose**: Enable frontend to read/write files with OS-level permissions.

**Security Consideration**: No sandboxing implemented. Future enhancement: Path validation to restrict access to specific directories.

**Usage Pattern**:
```typescript
// Read file
const content = await invoke<string>('read', { path: '/path/to/file.txt' });

// Write file
await invoke('write', { path: '/path/to/file.txt', contents: 'Hello world' });

// Save code snippet (auto-generates filename)
const savedPath = await invoke<string>('save_code', {
    filename: 'example.py',
    code: 'print("Hello")'
});
```

#### Ollama Integration Commands (`src-tauri/src/commands/ollama.rs`)

**Purpose**: Interface with local Ollama server for AI generation.

**Key Pattern**: `generate_stream` uses event-based streaming while `check_ollama` and `get_ollama_models` use request/response.

**Usage Pattern**:
```typescript
// Check connection
const connected = await invoke<boolean>('check_ollama');

// Get models
const models = await invoke<string[]>('get_ollama_models');

// Start streaming (listen for events separately)
await invoke('generate_stream', {
    prompt: 'Explain variables',
    model: 'qwen2.5-coder:7b',
    context: null
});

// Cancel streaming
await invoke('cancel_generation');
```

#### Hardware Detection Commands (`src-tauri/src/commands/hardware.rs`)

**Purpose**: Detect system capabilities for model recommendations.

**Lazy Detection Pattern**: `detect_hardware()` triggers detection on first call via `OnceCell`. Subsequent calls return cached value.

**Usage Pattern**:
```typescript
// Trigger detection (first call)
const hardware = await invoke<HardwareInfo>('detect_hardware');

// Get cached (fast, subsequent calls)
const cached = await invoke<HardwareInfo | null>('get_cached_hardware');
```

#### Benchmarking Commands (`src-tauri/src/commands/benchmark.rs`)

**Purpose**: Performance testing for models on user's hardware.

**Usage Pattern**:
```typescript
// Run benchmark
const result = await invoke<BenchmarkResult>('run_benchmark', {
    model: 'qwen2.5-coder:7b',
    testType: 'generation_speed'
});

// Get results directory path
const dir = await invoke<string>('get_benchmarks_directory');

// Open in file explorer
await invoke('open_benchmarks_folder');
```

---

## 4.5 Managed State Architecture

Tauri's managed state provides globally shared, thread-safe state accessible to all command handlers. The application uses four managed states to coordinate complex operations.

### State Registration

**File**: `src-tauri/src/lib.rs:33-36`

```rust
.manage(StreamCancellation::default())  // Cancellation coordination
.manage(HttpClient::default())          // Connection pooling
.manage(OllamaConfig::default())        // Ollama URL configuration
.manage(HardwareCache::default())       // Hardware detection cache
```

### 4.5.1 StreamCancellation

**File**: `src-tauri/src/commands/ollama.rs:105-140`

**Purpose**: Coordinate stream cancellation between the `cancel_generation()` command and the active `generate_stream()` task.

#### Implementation

```rust
pub struct StreamCancellation {
    sender: Mutex<Option<broadcast::Sender<()>>>,
}

impl StreamCancellation {
    pub fn create_channel(&self) -> broadcast::Receiver<()> {
        let mut sender_lock = self.sender.lock().unwrap();
        let (tx, rx) = broadcast::channel(1);
        *sender_lock = Some(tx);
        rx
    }

    pub fn cancel(&self) {
        let sender_lock = self.sender.lock().unwrap();
        if let Some(sender) = sender_lock.as_ref() {
            let _ = sender.send(());  // Broadcast cancellation signal
        }
    }

    pub fn clear(&self) {
        let mut sender_lock = self.sender.lock().unwrap();
        *sender_lock = None;  // Remove sender
    }
}
```

#### How It Works

1. **Stream Start**: `generate_stream()` calls `cancellation.create_channel()` → creates broadcast channel → stores sender in `Mutex`
2. **Cancellation**: User clicks cancel → `cancel_generation()` called → `cancellation.cancel()` → broadcasts `()` signal via sender
3. **Reception**: `tokio::select!` in `generate_stream()` receives signal → emits `ollama_cancelled` → exits loop
4. **Cleanup**: Stream end → `cancellation.clear()` → removes sender → ready for next stream

#### Why Tokio Broadcast Channel?

**Alternatives Considered**:
- **AtomicBool**: Can't await changes, requires polling
- **oneshot channel**: Only works once, need new channel per stream
- **mpsc channel**: Requires receiver ownership transfer

**Broadcast Channel Benefits**:
- Multiple receivers can listen (future extensibility)
- Awaitable: Integrates with `tokio::select!`
- Single sender, multiple receivers pattern
- Cheap to create (lightweight)

#### Thread Safety

- `Mutex` protects concurrent access to sender
- `broadcast::Sender` is `Clone` + `Send` + `Sync`
- Commands can run concurrently without data races

### 4.5.2 HttpClient

**File**: `src-tauri/src/commands/ollama.rs:22-39`

**Purpose**: HTTP connection pooling for requests to Ollama server.

#### Implementation

```rust
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

#### Why Connection Pooling Matters

**Without Pooling** (new client per request):
- Each request creates new TCP connection
- TCP handshake: SYN → SYN-ACK → ACK (3 round-trips)
- TLS handshake: ClientHello → ServerHello → ... (multiple round-trips)
- Socket creation overhead

**With Pooling** (shared client):
- Reuses existing TCP connections
- Avoids handshake overhead
- Significant performance improvement even for localhost (avoids syscall overhead)
- Critical for repeated requests (model fetching, multiple generations)

**Performance Impact**:
- **Cold start**: ~10-15ms (TCP + socket setup)
- **Pooled**: ~1-2ms (reuse existing connection)
- **Savings**: ~8-13ms per request (13x faster)

#### Thread Safety

- `reqwest::Client` is `Clone` + `Send` + `Sync`
- Internal Arc ensures sharing across threads
- No mutex needed (client handles internal synchronization)

### 4.5.3 OllamaConfig

**File**: `src-tauri/src/commands/ollama.rs:41-68`

**Purpose**: Store and validate Ollama server URL with security enforcement.

#### Implementation

```rust
pub struct OllamaConfig {
    base_url: String,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        let base_url = env::var("OLLAMA_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());

        // Security validation - MUST be localhost
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

#### Security Validation

**File**: `src-tauri/src/security/mod.rs:112-155`

```rust
pub fn validate_ollama_url(url_str: &str) -> Result<String, String> {
    let url = Url::parse(url_str)?;
    let host = url.host_str().ok_or("URL must have hostname")?;

    match host {
        "localhost" | "127.0.0.1" | "::1" => Ok(url_str.to_string()),
        _ => Err(format!(
            "Security violation: Ollama must run on localhost. Found: '{}'\n\
             This protects student privacy (GDPR/FERPA compliance).",
            host
        ))
    }
}
```

**What It Prevents**:
- External server connections (e.g., `https://evil.com:11434`)
- Data exfiltration attacks
- Accidental cloud API usage
- Privacy violations

**Why It Matters**:
- **Educational Context**: Schools require verifiable local processing
- **GDPR Compliance**: Student data cannot leave device without consent
- **FERPA Compliance**: US education privacy law
- **No Cloud Vendor Lock-In**: App works without internet

#### Initialization Timing

- Validation happens **once** at app startup (`Default::default()`)
- Invalid URLs fall back to localhost (app continues running)
- Logged error provides diagnostic info

### 4.5.4 HardwareCache

**File**: `src-tauri/src/commands/hardware.rs`

**Purpose**: Lazy hardware detection with `OnceCell` to avoid startup race conditions.

#### Implementation Pattern

```rust
pub struct HardwareCache {
    info: OnceCell<Mutex<Option<HardwareInfo>>>,
}

impl Default for HardwareCache {
    fn default() -> Self {
        Self {
            info: OnceCell::new(),
        }
    }
}
```

#### Lazy Detection Strategy

**Why Not Detect at Startup?**
- Hardware detection takes 100-500ms
- Frontend might not need hardware info immediately
- Avoids blocking app startup
- Prevents race conditions with frontend initialization

**Detection Trigger**:
- First call to `detect_hardware()` or `get_cached_hardware()`
- `OnceCell::get_or_init()` ensures single execution
- Subsequent calls return cached value

#### Detailed Coverage

Hardware detection is covered extensively in Section 5. This section focuses on the managed state pattern, not the detection implementation.

---

## 4.6 Type Synchronization (Rust ↔ TypeScript)

Type synchronization is critical for IPC communication. Mismatched types cause runtime serialization errors, silent data corruption, or frontend crashes.

### 4.6.1 Serialization Mechanics

#### TypeScript → Rust

1. **TypeScript Object Created**:
   ```typescript
   const params = { prompt: "Hello", model: "qwen2.5-coder:7b", context: null };
   ```

2. **Tauri API Serializes to JSON**:
   ```json
   {"prompt":"Hello","model":"qwen2.5-coder:7b","context":null}
   ```

3. **IPC Bridge Transports**: JSON string sent via IPC channel

4. **`serde_json` Deserializes**:
   ```rust
   let prompt: String = "Hello";
   let model: String = "qwen2.5-coder:7b";
   let context: Option<Vec<OllamaMessage>> = None;
   ```

5. **Command Handler Receives Typed Data**

#### Rust → TypeScript

1. **Rust Struct/Enum Created**:
   ```rust
   let hardware = HardwareInfo { cpu: CpuInfo { ... }, ... };
   ```

2. **`serde::Serialize` Converts to JSON**:
   ```json
   {"cpu":{"vendor":"AMD","cores":8,...},"gpu":{...}}
   ```

3. **IPC Bridge Transports**: JSON string sent via IPC channel

4. **Tauri API Deserializes to JavaScript**:
   ```javascript
   const hardware = {cpu: {vendor: "AMD", cores: 8, ...}, gpu: {...}};
   ```

5. **TypeScript Receives Typed Object**:
   ```typescript
   const hardware: HardwareInfo = await invoke('detect_hardware');
   ```

### 4.6.2 Type Mapping Rules

| Rust Type | TypeScript Type | Notes |
|-----------|-----------------|-------|
| `String` | `string` | UTF-8 string |
| `bool` | `boolean` | true/false |
| `i32`, `u32`, `i64`, `u64` | `number` | JavaScript numbers are f64 (may lose precision for large i64/u64) |
| `f32`, `f64` | `number` | Floating point |
| `Vec<T>` | `T[]` | Array of T |
| `Option<T>` | `T \| null` | **Critical**: null, not undefined |
| `HashMap<K, V>` | `{ [key: string]: V }` | Object/Record |
| `()` (unit) | `undefined` | Used for commands with no return value |
| Struct with `#[derive(Serialize)]` | `interface` | Object with matching fields |
| Enum with `#[derive(Serialize)]` | `type` union or discriminated union | Depends on enum representation |

**Critical Rule**: `Option<T>` in Rust **always** maps to `T | null` in TypeScript, **never** `T | undefined`. Tauri serializes `None` as `null`, not `undefined`.

### 4.6.3 Key Type Pairs

#### OllamaMessage

**Rust** (`src-tauri/src/commands/ollama.rs:70-74`):
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
}
```

**TypeScript** (`src/lib/types/ollama.ts`):
```typescript
export interface OllamaMessage {
    role: 'system' | 'user' | 'assistant';
    content: string;
}
```

**Difference**: TypeScript uses union type for `role` (more type-safe), Rust uses `String` (Ollama API accepts any string, but we only send 3 values).

**Why Not Enum in Rust?**
- Ollama API expects lowercase strings, not enum variants
- `serde` would serialize enum as `{"role":"User"}` (capitalized)
- Simpler to use `String` and validate at API boundary

#### Message (Frontend-Only)

**TypeScript** (`src/lib/types/chat.ts`):
```typescript
export interface Message {
    id: string;
    role: 'user' | 'assistant';
    content: string;
    timestamp: number;
    isStreaming?: boolean;
}
```

**Important**: This type is **never** sent to Rust. It's frontend-only state.

- `id`: UUID for React-style key prop
- `timestamp`: For display ("2 minutes ago")
- `isStreaming`: UI flag to show loading indicator

**Transformation for IPC**:
```typescript
// Frontend Message → Rust OllamaMessage
const ollamaMsg: OllamaMessage = {
    role: message.role,
    content: message.content
    // Strips id, timestamp, isStreaming
};
```

#### HardwareInfo

**Rust** (`src-tauri/src/hardware/types.rs`):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub cpu: CpuInfo,
    pub gpu: Option<GpuInfo>,
    pub memory: MemoryInfo,
    pub storage: StorageInfo,
    pub npu: Option<NpuInfo>,
}
```

**TypeScript** (`src/lib/types/hardware.ts`):
```typescript
export interface HardwareInfo {
    cpu: CpuInfo;
    gpu: GpuInfo | null;
    memory: MemoryInfo;
    storage: StorageInfo;
    npu: NpuInfo | null;
}
```

**Critical**: `Option<GpuInfo>` → `GpuInfo | null` (not `| undefined`).

### 4.6.4 Custom Error Serialization

**File**: `src-tauri/src/commands/errors.rs:1-34`

#### Custom Implementation

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("{0}")]
    Other(String),
}

#[derive(serde::Serialize)]
#[serde(tag = "name", content = "message")]
#[serde(rename_all = "camelCase")]
enum ErrorName {
    Io(String),
    FromUtf8Error(String),
    Other(String),
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let message = self.to_string();  // thiserror provides Display impl
        let name = match self {
            Self::Io(_) => ErrorName::Io(message),
            Self::Utf8(_) => ErrorName::FromUtf8Error(message),
            Self::Other(_) => ErrorName::Other(message),
        };
        name.serialize(serializer)
    }
}
```

#### Serialized Format

**Rust Error**:
```rust
Error::Io(std::io::Error::new(ErrorKind::NotFound, "file not found"))
```

**Serialized JSON**:
```json
{
    "name": "io",
    "message": "file not found"
}
```

**TypeScript Reception**:
```typescript
try {
    await invoke('read', { path: '/nonexistent' });
} catch (error) {
    // error is { name: 'io', message: 'file not found' }
    console.error(`Error [${error.name}]: ${error.message}`);
}
```

#### Why Custom Serialization?

**Default `thiserror` Serialization** (not suitable):
```json
{
    "Io": {
        "kind": "NotFound",
        "message": "file not found",
        "os_error": null
    }
}
```

**Problems**:
- Complex nested structure
- Frontend doesn't care about `os_error`, `kind`
- Difficult to extract user-friendly message

**Custom Serialization** (user-friendly):
```json
{
    "name": "io",
    "message": "file not found"
}
```

**Benefits**:
- Flat structure
- Human-readable message
- Error type discriminator (`name`)
- Easy to display in UI

---

## 4.7 Error Handling Across IPC Boundary

Robust error handling ensures users see actionable error messages instead of cryptic stack traces or silent failures.

### 4.7.1 Error Flow

#### Backend Error Generation

**File**: `src-tauri/src/commands/ollama.rs:225-230`

```rust
let response = client.get()
    .post(&url)
    .json(&request)
    .send()
    .await
    .map_err(|e| Error::Other(format!("Failed to send request: {e}")))?;
```

**Breakdown**:
- `send()` returns `Result<Response, reqwest::Error>`
- `.map_err()` transforms `reqwest::Error` → `Error::Other(String)`
- `?` operator propagates error up call stack
- Command returns `Result<(), Error>`

#### Serialization

**Custom `Serialize` impl** (`src-tauri/src/commands/errors.rs:20-33`):
```rust
impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let message = self.to_string();  // "Failed to send request: connection refused"
        let name = match self {
            Self::Other(_) => ErrorName::Other(message),
            // ... other variants
        };
        name.serialize(serializer)
    }
}
```

**Output JSON**:
```json
{
    "name": "other",
    "message": "Failed to send request: connection refused"
}
```

#### Frontend Reception

**File**: `src/lib/stores/ollama.svelte.ts:31-47`

```typescript
async function checkConnection() {
    try {
        const connected = await invoke<boolean>('check_ollama');
        status.connected = connected;
        status.error = null;
    } catch (error) {
        status.connected = false;
        status.error = error instanceof Error ? error.message : 'Connection failed';
    }
}
```

**Error Handling**:
- `invoke()` throws if command returns `Err(...)`
- `catch` block receives error object
- Type guard: `error instanceof Error` (defensive programming)
- Fallback message if error is unexpected type
- Store updated → UI shows error

### 4.7.2 Error Propagation Example

**Scenario**: User tries to generate with Ollama not running.

#### Stack Trace

1. **Frontend**: `invoke('generate_stream', { ... })` called
2. **Backend**: `generate_stream()` command executes
3. **HTTP Request**: `client.post(...).send().await` fails (connection refused)
4. **Error Transform**: `.map_err(|e| Error::Other(...))` creates `Error::Other("Failed to send request: connection refused")`
5. **Propagation**: `?` operator returns error
6. **Serialization**: Custom `Serialize` impl converts to JSON
7. **IPC Transport**: JSON error sent to frontend
8. **Frontend Exception**: `invoke()` promise rejects
9. **Catch Block**: Error displayed in UI

#### Code Path

**Backend** (`src-tauri/src/commands/ollama.rs:225-230`):
```rust
let response = client.get()
    .post(&url)
    .json(&request)
    .send()
    .await
    .map_err(|e| Error::Other(format!("Failed to send request: {e}")))?;
    // ↑ This line fails, error propagates up
```

**Frontend** (`src/App.svelte:165-174`):
```typescript
try {
    await invoke('generate_stream', { ... });
} catch (error) {
    console.error('Generation error:', error);  // Log for debugging
    chatsStore.updateMessage(currentChat.id, assistantMessage.id, {
        content: `Error: ${error}`,  // Show in chat
        isStreaming: false
    });
    // Cleanup state...
}
```

**UI Display**:
```
User: Hello
Assistant: Error: Failed to send request: connection refused
```

### 4.7.3 User-Friendly Error Messages

**Strategy**: Human-readable strings, not debug output.

#### Bad Error Messages

```rust
// ❌ Debug output
.map_err(|e| Error::Other(format!("{:?}", e)))?;
// Frontend sees: "Error { kind: NotFound, message: \"file not found\", ... }"
```

```rust
// ❌ No context
.map_err(|e| Error::Other(e.to_string()))?;
// Frontend sees: "connection refused" (what connection?)
```

#### Good Error Messages

```rust
// ✅ Contextual + actionable
.map_err(|e| Error::Other(format!("Failed to send request to Ollama: {e}")))?;
// Frontend sees: "Failed to send request to Ollama: connection refused"
```

```rust
// ✅ Explains cause + solution
if !Path::new(&path).exists() {
    return Err(Error::Other(format!(
        "File not found: {}\nCheck that the file exists and you have permission to read it.",
        path
    )));
}
```

#### Logging vs UI Display

**Backend Logging**:
```rust
log::error!("Failed to connect to Ollama at {}: {}", url, e);
log::debug!("Request details: {:?}", request);  // Verbose, debugging only
```

**User-Facing Error**:
```rust
Error::Other("Failed to connect to Ollama. Is Ollama running?".to_string())
```

**Separation of Concerns**:
- **Logs**: Detailed, for developers, includes stack traces
- **UI Errors**: Concise, for users, actionable

---

## 4.8 Special IPC Patterns

### 4.8.1 Stream Cancellation Pattern

Stream cancellation is a sophisticated pattern enabling users to stop long-running AI generation mid-stream.

#### Complete Flow

1. **User Clicks "Cancel Generation" Button**

   **File**: `src/App.svelte:183-204`

   ```typescript
   async function handleCancelGeneration() {
       cancelRequested = true;

       try {
           await invoke('cancel_generation');
       } catch (error) {
           console.error('Failed to cancel:', error);
       }

       isGenerating = false;

       if (currentStreamingChatId && currentStreamingMessageId) {
           chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
               isStreaming: false
           });
       }

       currentStreamingChatId = null;
       currentStreamingMessageId = null;
   }
   ```

2. **Frontend Invokes `cancel_generation` Command**

   IPC call to backend (non-blocking, returns immediately).

3. **Backend Command Executes**

   **File**: `src-tauri/src/commands/ollama.rs:182-185`

   ```rust
   #[tauri::command]
   pub fn cancel_generation(cancellation: State<StreamCancellation>) {
       cancellation.cancel();
   }
   ```

4. **Cancellation Signal Broadcast**

   **File**: `src-tauri/src/commands/ollama.rs:127-133`

   ```rust
   pub fn cancel(&self) {
       let sender_lock = self.sender.lock().unwrap();
       if let Some(sender) = sender_lock.as_ref() {
           let _ = sender.send(());  // Broadcast () to all receivers
       }
   }
   ```

5. **Active Stream Receives Signal**

   **File**: `src-tauri/src/commands/ollama.rs:235-243`

   ```rust
   loop {
       tokio::select! {
           _ = cancel_rx.recv() => {
               // Cancellation signal received
               cancellation.clear();
               app_handle.emit("ollama_cancelled", ())?;
               return Ok(());
           }
           // ... chunk processing
       }
   }
   ```

6. **Backend Emits `ollama_cancelled` Event**

   Event sent to frontend via IPC.

7. **Frontend Event Listener Receives Cancellation**

   **File**: `src/App.svelte:288-292`

   ```typescript
   unlistenCancelled = await listen('ollama_cancelled', () => {
       isGenerating = false;
       currentStreamingChatId = null;
       currentStreamingMessageId = null;
   });
   ```

8. **State Cleanup on Both Sides**

   - **Backend**: `cancellation.clear()` removes sender
   - **Frontend**: Resets streaming state variables

#### Why `tokio::select!`?

**Purpose**: Concurrently wait for multiple async operations.

**Alternative Approaches**:

**Polling** (inefficient):
```rust
loop {
    if should_cancel() {  // Check flag
        return Ok(());
    }
    process_chunk();
    tokio::time::sleep(Duration::from_millis(10)).await;  // Waste CPU
}
```

**`tokio::select!`** (efficient):
```rust
loop {
    tokio::select! {
        _ = cancel_rx.recv() => return Ok(()),  // React immediately
        chunk = stream.next() => process(chunk),  // React immediately
    }
}
```

**Benefits**:
- Zero CPU usage while waiting
- Immediate reaction to cancellation (no polling delay)
- Idiomatic Tokio async code

### 4.8.2 Background Generation Pattern

Background generation enables users to switch between chats while AI generation continues in non-visible chats.

#### State Tracking

**File**: `src/App.svelte:150-153`

```typescript
isGenerating = true;
currentStreamingChatId = currentChat.id;  // Track which chat is streaming
currentStreamingMessageId = assistantMessage.id;  // Track which message
```

**Key Variables**:
- `currentStreamingChatId`: ID of chat currently receiving streamed content
- `currentStreamingMessageId`: ID of specific message being updated
- `currentChat`: Currently **displayed** chat (user's view)

**Distinction**: `currentStreamingChatId` may **differ** from `currentChat.id` if user switches chats during generation.

#### Event Listener Logic

**File**: `src/App.svelte:247-271`

```typescript
unlistenChunk = await listen<string>('ollama_chunk', (event) => {
    if (!currentStreamingChatId || !currentStreamingMessageId || cancelRequested) {
        return;
    }

    // Find streaming chat (may not be currently displayed)
    const streamingChat = chatsStore.chats.find((c) => c.id === currentStreamingChatId);
    if (!streamingChat) return;

    const streamingMessage = streamingChat.messages.find((m) => m.id === currentStreamingMessageId);
    if (!streamingMessage?.isStreaming) return;

    // Update message in store (works for background chats)
    chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
        content: streamingMessage.content + event.payload
    });

    // Only scroll if viewing active chat
    if (currentChat?.id === currentStreamingChatId) {
        scrollToBottom();
    }
});
```

**Key Logic**:
1. **Validation**: Checks if still in streaming state
2. **Find Target**: Locates chat/message by ID (not by current view)
3. **Update Store**: Appends chunk to correct message
4. **Conditional Scroll**: Only scrolls if user is viewing the streaming chat

#### User Experience Flow

**Scenario**: User starts generation in Chat A, switches to Chat B.

1. User sends message in Chat A
2. `currentStreamingChatId = chatA.id`, `currentChat = chatA`
3. Streaming starts, chunks arrive
4. User clicks Chat B in sidebar
5. `currentChat = chatB` (displayed chat changes)
6. `currentStreamingChatId` **still** `chatA.id` (streaming continues)
7. Event listener receives chunks → updates Chat A messages
8. No auto-scroll (user viewing Chat B)
9. User switches back to Chat A → sees complete response

**Why This Works**:
- Messages stored in `chatsStore`, not component state
- Event listener uses IDs, not references to current view
- Svelte reactivity updates all chat views when store changes

### 4.8.3 Security Validation Pattern

Security validation enforces localhost-only Ollama connections, preventing data exfiltration.

#### Validation Point

**File**: `src-tauri/src/commands/ollama.rs:46-62`

```rust
impl Default for OllamaConfig {
    fn default() -> Self {
        let base_url = env::var("OLLAMA_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());

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

**Timing**: Validation happens **once** at app startup during `OllamaConfig` initialization.

#### Validation Implementation

**File**: `src-tauri/src/security/mod.rs:125-155`

```rust
pub fn validate_ollama_url(url_str: &str) -> Result<String, String> {
    // Parse URL with proper parser
    let url = Url::parse(url_str)
        .map_err(|e| format!("Invalid Ollama URL format: {e}"))?;

    // Extract hostname
    let host = url.host_str()
        .ok_or("Ollama URL must have a hostname")?;

    // Exact hostname matching
    match host {
        "localhost" | "127.0.0.1" | "::1" => {
            log::info!("Ollama URL validated: {}", url_str);
            Ok(url_str.to_string())
        }
        _ => {
            log::error!(
                "SECURITY: Rejected non-localhost Ollama URL: '{}' (hostname: '{}')",
                url_str,
                host
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

#### Attack Scenarios Prevented

**Scenario 1: Malicious Environment Variable**
```bash
OLLAMA_URL=https://evil.com:11434 ./smolpc-codehelper
```

**Result**:
- `validate_ollama_url("https://evil.com:11434")` rejects
- Logs: `SECURITY: Rejected non-localhost Ollama URL: 'https://evil.com:11434'`
- Falls back to `http://localhost:11434`
- App continues with safe default

**Scenario 2: Naive String Bypass Attempt**
```bash
OLLAMA_URL=http://localhost.evil.com:11434
```

**Result**:
- `Url::parse()` extracts hostname: `localhost.evil.com`
- Match fails (not `localhost`, `127.0.0.1`, or `::1`)
- Rejected

**Scenario 3: IP Address Spoofing**
```bash
OLLAMA_URL=http://192.168.1.100:11434
```

**Result**:
- Hostname: `192.168.1.100`
- Not in allowed list → rejected

#### Why This Matters

**Educational Context**:
- Schools deploy apps for students
- Student code/queries contain personal info
- GDPR (EU) and FERPA (US) require data stays local
- External API usage requires explicit consent

**Technical Context**:
- Prevents accidental cloud API usage (student thinks local, uses cloud)
- Prevents malicious configuration (attacker replaces Ollama with logging proxy)
- Enforces offline-first architecture

---

## 4.9 Critical Files Reference

### Frontend Files

| File | Purpose | Key Lines | What to Study |
|------|---------|-----------|---------------|
| **src/App.svelte** | Main orchestration, message handling, event listeners | 100-109 (buildContext), 112-174 (handleSendMessage), 247-307 (event listeners) | Complete message flow, streaming logic |
| **src/lib/stores/chats.svelte.ts** | Chat and message state management | 63-88 (addMessage, updateMessage, deleteMessage) | Message CRUD operations, Svelte 5 runes |
| **src/lib/stores/ollama.svelte.ts** | Ollama connection state | 27-47 (checkConnection, fetchModels) | Request/response IPC pattern |
| **src/lib/components/ChatInput.svelte** | User input component | 16-25 (handleSubmit) | Event delegation to parent |
| **src/lib/types/chat.ts** | TypeScript type definitions | All | Frontend Message interface |
| **src/lib/types/ollama.ts** | Ollama type definitions | All | OllamaMessage interface (matches Rust) |

### Backend Files

| File | Purpose | Key Lines | What to Study |
|------|---------|-----------|---------------|
| **src-tauri/src/lib.rs** | App setup, command registration, managed state | 16-53 | Tauri initialization, `.manage()`, `.invoke_handler()` |
| **src-tauri/src/commands/ollama.rs** | Ollama integration, streaming, cancellation | 22-39 (HttpClient), 105-140 (StreamCancellation), 188-300 (generate_stream) | Event-based streaming, tokio::select!, error handling |
| **src-tauri/src/commands/default.rs** | File I/O commands | All | Request/response pattern, file operations |
| **src-tauri/src/commands/errors.rs** | Error type serialization | 1-34 | Custom Serialize impl, error formatting |
| **src-tauri/src/security/mod.rs** | Security validation | 112-155 (validate_ollama_url) | URL parsing, localhost validation |
| **src-tauri/src/hardware/types.rs** | Hardware type definitions (Rust) | All | Type synchronization with TypeScript |

---

## 4.10 Data Flow Sequences (Bullet-Point Format)

### 4.10.1 Complete Event Flow Sequence

Use this sequence to generate flow diagrams:

1. User clicks Send button in ChatInput component
2. `ChatInput.handleSubmit()` fires
3. `onSend` callback invoked → `App.handleSendMessage()` receives message
4. Store updates (chatsStore):
   - Add user message object (`id`, `role: 'user'`, `content`, `timestamp`)
   - Add empty assistant message (`id`, `role: 'assistant'`, `content: ''`, `isStreaming: true`)
5. State updates:
   - `isGenerating = true`
   - `currentStreamingChatId = currentChat.id`
   - `currentStreamingMessageId = assistantMessage.id`
6. `buildContext()` called → extracts previous messages → transforms to Ollama format
7. `invoke('generate_stream', { prompt, model, context })` called
8. **[IPC BOUNDARY CROSSED - TypeScript → Rust]**
9. Tauri serializes params to JSON → IPC bridge transports
10. Backend: `generate_stream()` command handler activated
11. serde_json deserializes JSON → Rust types
12. Managed state injected: `HttpClient`, `OllamaConfig`, `StreamCancellation`
13. `cancellation.create_channel()` → broadcast channel created
14. Build complete messages array:
    - System prompt (student-friendly AI persona)
    - Context messages (conversation history)
    - Current user prompt
15. HTTP POST sent to Ollama (`localhost:11434/api/chat`) with `stream: true`
16. `response.bytes_stream()` → async stream obtained
17. Stream processing loop starts (`tokio::select!`):
    - Branch 1: `cancel_rx.recv()` (waits for cancellation)
    - Branch 2: `stream.next()` (waits for chunks)
18. For each chunk received from Ollama:
    - Parse bytes to UTF-8 string
    - Parse newline-delimited JSON lines
    - Extract `message.content` from `OllamaResponse`
    - `app_handle.emit("ollama_chunk", content)` → event emitted
19. **[IPC BOUNDARY CROSSED - Rust → TypeScript]**
20. Tauri serializes event payload to JSON → IPC bridge transports
21. Frontend event listener (`listen<string>('ollama_chunk')`) receives event
22. Event handler validates:
    - `currentStreamingChatId` exists
    - `currentStreamingMessageId` exists
    - `cancelRequested` is false
23. Find streaming chat and message by ID in `chatsStore`
24. `chatsStore.updateMessage()` → append `event.payload` to `message.content`
25. Svelte reactivity triggers re-render of ChatMessage component
26. UI updates → user sees incremental text appear
27. If `currentChat.id === currentStreamingChatId` → `scrollToBottom()`
28. Steps 18-27 repeat for each chunk (hundreds of times)
29. When Ollama sends `done: true` in response:
    - Backend emits `ollama_done` event
    - **[IPC BOUNDARY CROSSED - Rust → TypeScript]**
    - Frontend listener receives `ollama_done`
    - `chatsStore.updateMessage()` → set `isStreaming: false`
    - State cleanup: `isGenerating = false`, `currentStreamingChatId = null`, `currentStreamingMessageId = null`
30. Final UI update → streaming indicator removed, Send button enabled

### 4.10.2 Serialization Flow Sequence

**TypeScript → Rust:**

1. TypeScript object created with typed properties
2. Tauri API (`invoke()`) serializes object to JSON using `JSON.stringify()`
3. IPC Bridge transports JSON string across process boundary
4. Tauri backend receives JSON string
5. `serde_json::from_str()` deserializes JSON to Rust struct
6. Command handler receives typed Rust parameters

**Rust → TypeScript:**

1. Rust struct/enum created with typed fields
2. `serde::Serialize` trait serializes to JSON via custom or derived implementation
3. IPC Bridge transports JSON string across process boundary
4. Tauri frontend receives JSON string
5. Tauri API deserializes JSON to JavaScript object using `JSON.parse()`
6. TypeScript receives typed object (type assertion via `invoke<T>()`)

---

## 4.11 Key Takeaways

### Critical Concepts

1. **Two IPC Patterns Serve Different Purposes**
   - Request/Response: Simple queries, single result
   - Event-Based Streaming: Long operations, incremental updates
   - Choice depends on operation duration and UX requirements

2. **Event-Based Streaming Enables Real-Time AI Responses**
   - Non-blocking: UI remains interactive during generation
   - Incremental updates: User sees tokens arrive in real-time
   - Cancellable: Mid-stream termination via broadcast channels
   - Superior to polling: Zero CPU waste, immediate reaction

3. **Managed State Coordinates Complex Operations**
   - `StreamCancellation`: Tokio broadcast channel for async cancellation
   - `HttpClient`: Connection pooling for performance (13x faster)
   - `OllamaConfig`: Security validation (localhost-only)
   - `HardwareCache`: Lazy detection with OnceCell

4. **Type Synchronization is Critical**
   - Both Rust and TypeScript types must match exactly
   - `Option<T>` → `T | null` (not `undefined`)
   - Custom `Serialize` impl for user-friendly errors
   - Frontend-only fields (like `isStreaming`) never sent to backend

5. **Security Validation Happens at IPC Boundary**
   - Localhost-only enforcement prevents data exfiltration
   - GDPR/FERPA compliance for educational context
   - URL parsing (not naive string matching) prevents bypass
   - Validation at app startup (not per-request)

6. **Error Handling Provides User-Friendly Messages**
   - Backend: Contextual error messages, not debug output
   - Serialization: Flat JSON structure (`{name, message}`)
   - Frontend: try/catch with fallback messages
   - Logging: Detailed errors for developers, concise for users

7. **Background Generation and Cancellation Show Sophisticated State Management**
   - Streaming continues when user switches chats
   - Event listeners use IDs, not current view references
   - `tokio::select!` for concurrent cancellation monitoring
   - Cleanup on both frontend and backend sides

8. **Svelte 5 Reactivity Automatically Updates UI**
   - Store changes trigger component re-renders
   - No manual DOM manipulation needed
   - `$derived` for computed values
   - `$effect` for side effects (event listeners)

### Architectural Patterns

- **Process Isolation**: Frontend (WebView) and backend (Rust) in separate processes
- **Type Safety**: TypeScript and Rust enforce types at compile time
- **Async Throughout**: All I/O is non-blocking (async/await, tokio)
- **Shared State**: Managed state with Arc/Mutex for thread safety
- **Event-Driven**: Listeners react to events, not polling
- **Security by Default**: Validation enforced, not optional

---

## 4.12 Common Pitfalls and Best Practices

### Common Pitfalls

1. **Forgetting to Update Both TypeScript and Rust Types**
   - **Problem**: Add field to Rust struct, forget TypeScript interface → deserialization succeeds but TypeScript doesn't know field exists
   - **Solution**: Update both files simultaneously, use same PR/commit
   - **Detection**: Runtime error when accessing new field in TypeScript

2. **Not Cleaning Up Event Listeners**
   - **Problem**: Event listeners created in `onMount()` but not removed → memory leak
   - **Solution**: Store `UnlistenFn` and call in `onDestroy()` or cleanup function
   - **Pattern**:
     ```typescript
     onMount(async () => {
         const unlisten = await listen('event', handler);
         return () => unlisten();  // Cleanup on unmount
     });
     ```

3. **Using `any` Type (Loses Type Safety)**
   - **Problem**: `const result: any = await invoke('command')` → no type checking
   - **Solution**: `const result = await invoke<ReturnType>('command')`
   - **Benefit**: TypeScript catches type mismatches at compile time

4. **Not Handling Error Cases in try/catch**
   - **Problem**: `await invoke('command')` without try/catch → unhandled promise rejection
   - **Solution**: Always wrap IPC calls in try/catch
   - **Pattern**:
     ```typescript
     try {
         await invoke('command');
     } catch (error) {
         console.error('Command failed:', error);
         // Update UI with error state
     }
     ```

5. **Forgetting Managed State in lib.rs**
   - **Problem**: Create new state struct but forget `.manage()` → command fails with "state not found"
   - **Solution**: Add `.manage(MyState::default())` in `lib.rs` setup
   - **Error**: "failed to get state for type MyState"

6. **Not Validating External Inputs at IPC Boundary**
   - **Problem**: Trust user input without validation → path traversal, injection attacks
   - **Solution**: Validate all inputs in command handlers
   - **Example**:
     ```rust
     if path.contains("..") {
         return Err(Error::Other("Path traversal not allowed".to_string()));
     }
     ```

7. **Using `Option<T>` with `undefined` in TypeScript**
   - **Problem**: Check `if (value === undefined)` for Rust `Option<T>` → fails
   - **Solution**: Check `if (value === null)` or `if (!value)`
   - **Rust `None`**: Serializes to JSON `null`, not `undefined`

8. **Assuming Invoke Order Matches Completion Order**
   - **Problem**: Call `invoke('slow')` then `invoke('fast')`, assume slow completes first
   - **Solution**: Use `await` or track completion via IDs
   - **Concurrency**: Commands execute concurrently, may complete out of order

### Best Practices

1. **Type All IPC Calls**
   ```typescript
   const result = await invoke<HardwareInfo>('detect_hardware');
   // NOT: const result = await invoke('detect_hardware');
   ```

2. **Use Custom Error Messages**
   ```rust
   .map_err(|e| Error::Other(format!("Failed to read file '{}': {}", path, e)))?
   // NOT: .map_err(|e| e.into())?
   ```

3. **Validate Inputs Early**
   ```rust
   #[tauri::command]
   pub fn read(path: String) -> Result<String, Error> {
       if path.is_empty() {
           return Err(Error::Other("Path cannot be empty".to_string()));
       }
       // ...
   }
   ```

4. **Log Errors, Display User-Friendly Messages**
   ```rust
   log::error!("Database query failed: {:?}", err);
   Err(Error::Other("Failed to load data. Please try again.".to_string()))
   ```

5. **Use Managed State for Shared Resources**
   - HTTP clients, database connections, caches → managed state
   - Per-request data → command parameters

6. **Clean Up Resources in Both Directions**
   - Frontend: Remove event listeners on unmount
   - Backend: Clear channels/streams on completion

7. **Test Type Synchronization**
   - Send example data from backend → verify frontend receives correctly
   - Use TypeScript's strict mode to catch `null`/`undefined` issues

8. **Document IPC Commands**
   - Add doc comments to Rust command functions
   - Specify parameter types, return types, errors
   - Example:
     ```rust
     /// Check if Ollama server is running
     ///
     /// # Returns
     /// - `Ok(true)` if server responds to health check
     /// - `Ok(false)` if server is unreachable
     ///
     /// # Errors
     /// Returns `Err` if HTTP client fails to initialize
     #[tauri::command]
     pub async fn check_ollama(...) -> Result<bool, Error> { ... }
     ```

---

## See Also

- **Section 2: State Management Architecture** - Svelte 5 runes and store patterns
- **Section 3: Component Architecture & Hierarchy** - How components trigger IPC calls
- **Section 5: Complete Data Flow Walkthroughs** - Hardware detection, benchmarking
- **Section 6: Security & Best Practices** - Security validation details

---
