# SmolPC Unified Assistant -- Frontend Specification

**Version:** 1.0
**Last Updated:** 2026-03-13
**Status:** Canonical reference for all frontend implementation work

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Technology Stack and Constraints](#2-technology-stack-and-constraints)
3. [Critical Conventions](#3-critical-conventions)
4. [Architecture Overview](#4-architecture-overview)
5. [Type Definitions](#5-type-definitions)
6. [State Management](#6-state-management)
7. [Component Hierarchy](#7-component-hierarchy)
8. [Mode System](#8-mode-system)
9. [Chat UI](#9-chat-ui)
10. [Input Bar](#10-input-bar)
11. [Status Bar](#11-status-bar)
12. [Suggestion Chips](#12-suggestion-chips)
13. [DevTools Panel](#13-devtools-panel)
14. [Tauri IPC Integration](#14-tauri-ipc-integration)
15. [Streaming Pattern](#15-streaming-pattern)
16. [CSS and Design Tokens](#16-css-and-design-tokens)
17. [Responsive Layout](#17-responsive-layout)
18. [Markdown Rendering](#18-markdown-rendering)
19. [Keyboard Shortcuts](#19-keyboard-shortcuts)
20. [Persistence](#20-persistence)
21. [File Structure](#21-file-structure)
22. [Existing Reference Implementations](#22-existing-reference-implementations)

---

## 1. Project Overview

SmolPC Unified Assistant is a **Tauri 2 desktop application** that provides AI-powered assistance for five creative/productivity applications: GIMP (image editing), Blender (3D modeling), Writer (documents), Calc (spreadsheets), and Impress (presentations). A separate VS Code extension handles Code mode.

The Tauri app frontend is a single window with a **mode dropdown** that switches context between the five application modes. All modes share a single inference engine (`smolpc-engine-host`) running on `http://127.0.0.1:19432`. The engine handles model loading, backend selection (OpenVINO NPU, DirectML GPU, or CPU), and streaming text generation.

### What the Unified App Does

- Presents a chat interface where users type natural-language requests
- Routes those requests through the engine for LLM inference
- Dispatches tool calls to the appropriate MCP server for the active mode (e.g., GIMP MCP server for image editing commands)
- Displays streamed responses token-by-token
- Provides undo capability for undoable operations
- Shows connection status for both the engine and the active application's MCP server

### What the Unified App Does NOT Do

- It does not run inference locally in the frontend -- the engine host process handles all inference
- It does not directly communicate with GIMP/Blender/Writer/Calc/Impress -- MCP servers handle that
- It does not manage model downloads or engine lifecycle beyond startup handshake -- the engine manages itself

---

## 2. Technology Stack and Constraints

### Required Stack

| Layer | Technology | Version |
|-------|-----------|---------|
| Desktop framework | Tauri | 2.x |
| Frontend framework | Svelte | 5.x (runes) |
| Routing | SvelteKit | 2.x |
| CSS framework | Tailwind CSS | 4.x |
| Language | TypeScript | 5.6+ |
| Build tool | Vite | 6.x |
| Icons | Lucide Svelte (`@lucide/svelte`) | latest |
| Markdown sanitization | DOMPurify (`isomorphic-dompurify`) | latest |
| Tauri API | `@tauri-apps/api` | 2.x |

### Hard Constraints

1. **Svelte 5 runes only.** Do NOT use Svelte 4 stores (`writable`, `readable`, `derived` from `svelte/store`).
2. **Tailwind 4 only.** `@apply` is NOT supported. Use utility classes directly in templates or CSS variables for design tokens.
3. **Tauri Channels for streaming.** Do NOT use Tauri Events for token streaming. Channels are command-scoped and ordered; Events are global broadcast and can race.
4. **SPA mode.** SvelteKit runs with `ssr = false` and `adapter-static`. There is no server-side rendering.
5. **Offline-first.** No external network requests. No CDN fonts. All assets bundled locally.
6. **Target hardware.** Must perform acceptably on 8 GB RAM Intel laptops. Avoid heavy DOM operations.

---

## 3. Critical Conventions

### 3.1 Svelte 5 Runes (NOT Svelte 4 Stores)

This is the single most important convention. Every future session must follow this pattern.

**CORRECT -- Svelte 5 runes:**

```typescript
// In a .svelte.ts store file (module-level state)
let messages = $state<Message[]>([]);
let isGenerating = $state(false);

// Derived state
const messageCount = $derived(messages.length);
const hasMessages = $derived(messages.length > 0);

// Export as object with getters (reactive reads) and methods (mutations)
export const chatStore = {
  get messages() { return messages; },
  get isGenerating() { return isGenerating; },
  get messageCount() { return messageCount; },

  addMessage(msg: Message) {
    messages = [...messages, msg];
  },

  setGenerating(value: boolean) {
    isGenerating = value;
  },

  clear() {
    messages = [];
  }
};
```

```svelte
<!-- In a .svelte component file -->
<script lang="ts">
  import { chatStore } from '$lib/stores/chat.svelte';

  // Read reactive state via getters -- automatically tracked
  const count = $derived(chatStore.messageCount);

  // Local component state
  let inputValue = $state('');
</script>

<p>{chatStore.messages.length} messages</p>
<p>Generating: {chatStore.isGenerating}</p>
```

**WRONG -- Svelte 4 stores (DO NOT USE):**

```typescript
// NEVER DO THIS
import { writable, derived } from 'svelte/store'; // FORBIDDEN
const messages = writable<Message[]>([]); // FORBIDDEN
const count = derived(messages, $m => $m.length); // FORBIDDEN
```

```svelte
<!-- NEVER DO THIS -->
<script>
  import { messages } from '$lib/stores/chat';
  $: count = $messages.length; // FORBIDDEN -- $ prefix is Svelte 4
</script>
```

### 3.2 Component Props Pattern

Use `$props()` with a typed `Props` interface. Never use `export let`.

```svelte
<script lang="ts">
  interface Props {
    title: string;
    count?: number;
    onAction: (id: string) => void;
  }

  let { title, count = 0, onAction }: Props = $props();
</script>
```

**WRONG:**

```svelte
<script lang="ts">
  // NEVER use export let -- this is Svelte 4 syntax
  export let title: string;
  export let count: number = 0;
</script>
```

### 3.3 Effects

Use `$effect()` for side effects. Never use `$:` reactive statements.

```typescript
// CORRECT
$effect(() => {
  messages; // track dependency
  if (chatEl) chatEl.scrollTop = chatEl.scrollHeight;
});

// WRONG
$: if (chatEl) chatEl.scrollTop = chatEl.scrollHeight; // Svelte 4 syntax
```

### 3.4 Tailwind 4

```svelte
<!-- CORRECT: utility classes directly in markup -->
<div class="flex items-center gap-2 px-4 py-2 rounded-lg border border-gray-200">
  <span class="text-sm font-semibold text-gray-900">Title</span>
</div>

<!-- CORRECT: CSS variables for design tokens -->
<style>
  .my-component {
    background: var(--surface-widget);
    border: 1px solid var(--outline-soft);
    border-radius: var(--radius-lg);
  }
</style>
```

```css
/* WRONG: @apply is NOT supported in Tailwind 4 */
.my-class {
  @apply flex items-center gap-2; /* THIS WILL FAIL */
}
```

**Exception:** The existing codehelper `app.css` uses `@apply` inside a `@layer base` block. This works only because Tailwind 4 still processes `@layer base` directives in the CSS entry point. Do NOT use `@apply` in component `<style>` blocks or new CSS files.

### 3.5 Event Handlers

Use Svelte 5 event handler syntax: `onclick`, `onkeydown`, `oninput`. Not `on:click`, `on:keydown`, `on:input`.

```svelte
<!-- CORRECT: Svelte 5 -->
<button onclick={() => handleClick()}>Click</button>
<textarea onkeydown={handleKeydown} oninput={handleInput}></textarea>

<!-- WRONG: Svelte 4 -->
<button on:click={() => handleClick()}>Click</button>
```

### 3.6 Conditional Classes

Use template literals or ternary in `class` attribute. The `class:name={condition}` directive still works in Svelte 5 for simple boolean toggling.

```svelte
<!-- Both are acceptable in Svelte 5 -->
<div class={`status-pill ${isConnected ? 'connected' : ''}`}>
<div class="status-pill" class:connected={isConnected}>
```

---

## 4. Architecture Overview

### System Architecture

```
+---------------------------------------------------------------+
|                    Unified Assistant (Tauri 2)                 |
|  +----------------------------------------------------------+ |
|  |              Frontend (Svelte 5 + SvelteKit)              | |
|  |                                                            | |
|  |  +----------+  +----------+  +-----------+  +-----------+ | |
|  |  | ModeStore|  | ChatStore|  |InferStore  |  | UIStore   | | |
|  |  +----------+  +----------+  +-----------+  +-----------+ | |
|  |       |              |              |              |       | |
|  |  +----v--------------v--------------v--------------v----+ | |
|  |  |                  App Shell                           | | |
|  |  |  +--------+ +----------+ +-------+ +-------------+  | | |
|  |  |  | Header | | ChatArea | | Input | | DevTools     |  | | |
|  |  |  +--------+ +----------+ +-------+ +-------------+  | | |
|  |  +------------------------------------------------------+ | |
|  +----------------------------+-------------------------------+ |
|                               | Tauri IPC                       |
|  +----------------------------v-------------------------------+ |
|  |  Tauri Backend (Rust)                                      | |
|  |  commands/ --> smolpc-engine-client (HTTP to engine)        | |
|  |  commands/ --> MCP client (stdio to MCP servers)            | |
|  +----------------------------+-------------------------------+ |
+-------------------------------+--------------------------------+
                                | HTTP + SSE
+-------------------------------v--------------------------------+
|          smolpc-engine-host (axum, port 19432)                 |
|  POST /v1/chat/completions --> streaming inference             |
|  POST /engine/load         --> backend selection + model load  |
|  GET  /engine/status       --> readiness + backend diagnostics |
+----------------------------------------------------------------+
```

### Data Flow for a Chat Message

1. User types message in input bar, presses Enter
2. Component calls `chatStore.addMessage()` with user message
3. Component calls `chatStore.addMessage()` with empty assistant message (`isStreaming: true`)
4. Component creates a Tauri `Channel<string>` and sets `onmessage` callback
5. Component calls `invoke("assistant_chat_stream", { prompt, onToken: channel })`
6. Rust backend forwards prompt to engine host via HTTP, receives SSE stream
7. Each SSE token is sent through the Tauri Channel to the frontend
8. `onmessage` callback appends each token to the assistant message via `chatStore.updateMessage()`
9. `$effect` auto-scrolls chat to bottom
10. When `invoke` resolves, component sets `isStreaming: false` on the assistant message

---

## 5. Type Definitions

All TypeScript types must match their Rust counterparts exactly. The mapping rules are:

| Rust | TypeScript |
|------|-----------|
| `Option<T>` | `T \| null` |
| `String` | `string` |
| `bool` | `boolean` |
| `u32`, `i32`, `f64` | `number` |
| `Vec<T>` | `T[]` |
| `serde_json::Value` | `any` (avoid when possible) |
| `Result<T, String>` | Return type of `invoke<T>()` (error becomes rejected promise) |
| `#[serde(rename_all = "snake_case")]` | Property names use `snake_case` |

### 5.1 Core Chat Types

```typescript
// src/lib/types/chat.ts

export interface Message {
  id: string;                          // crypto.randomUUID()
  role: 'user' | 'assistant';
  text: string;                        // Message content (plain text or markdown)
  explain?: string | null;             // Optional explanation tip (shown below message)
  undoable?: boolean;                  // Whether this action can be undone
  isStreaming?: boolean;               // True while tokens are still arriving
  timestamp: number;                   // Date.now() at creation
}

export interface Chat {
  id: string;                          // crypto.randomUUID()
  title: string;                       // Auto-generated from first user message
  messages: Message[];
  createdAt: number;                   // Date.now()
  updatedAt: number;                   // Date.now(), updated on each message
  mode: AppMode;                       // Which mode this chat belongs to
  pinned?: boolean;
  archived?: boolean;
}
```

### 5.2 Mode Types

```typescript
// src/lib/types/mode.ts

export type AppMode = 'gimp' | 'blender' | 'writer' | 'calc' | 'impress';

export interface ModeConfig {
  id: AppMode;
  label: string;                       // Display name
  subtitle: string;                    // Short description
  icon: string;                        // Emoji or icon identifier
  systemPrompt: string;                // LLM system prompt for this mode
  suggestions: string[];               // Empty-state suggestion chips
  mcpServerName: string;               // MCP server identifier for connection
  accentColor?: string;                // Optional mode-specific accent
}
```

### 5.3 Assistant Response Type

This is the type returned by the Rust `assistant_request` command. It matches the Rust struct exactly.

```typescript
// src/lib/types/assistant.ts

export interface AssistantResponse {
  reply: string;                       // The text response to display
  explain?: string | null;             // Optional explanation for the user
  undoable?: boolean;                  // Whether the action can be undone
  plan: any;                           // Action plan (opaque to frontend)
  tool_results: any[];                 // Results from tool calls (opaque to frontend)
}
```

### 5.4 Engine Types

These types match the Rust DTOs in `engine_client_adapter.rs` and the engine host API.

```typescript
// src/lib/types/engine.ts

export type EngineReadinessState =
  | 'idle'
  | 'starting'
  | 'probing'
  | 'resolving_assets'
  | 'loading_model'
  | 'ready'
  | 'failed';

export interface EngineReadinessDto {
  attempt_id: string;
  state: EngineReadinessState;
  state_since: string;                 // ISO 8601 timestamp
  active_backend: string | null;       // "cpu" | "directml" | "openvino_npu" | null
  active_model_id: string | null;
  error_code: string | null;
  error_message: string | null;
  retryable: boolean;
}

export type InferenceBackend = 'cpu' | 'directml' | 'openvino_npu';

export interface GenerationMetrics {
  total_tokens: number;
  time_to_first_token_ms: number | null;
  tokens_per_second: number;
  total_time_ms: number;
}

export interface GenerationConfig {
  max_length: number;
  temperature: number;
  top_k: number | null;
  top_p: number | null;
  repetition_penalty: number;
  repetition_penalty_last_n: number;
}
```

### 5.5 MCP Types

```typescript
// src/lib/types/mcp.ts

export interface McpTool {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}

export interface McpToolCallResult {
  content: unknown;
  isError: boolean;
}
```

---

## 6. State Management

### 6.1 Store Architecture

All stores use the **module-level `$state` with exported object** pattern. There are no class-based stores and no Svelte 4 `writable()` stores.

Store files are named `*.svelte.ts` and live in `src/lib/stores/`. The `.svelte.ts` extension is required so that the Svelte compiler processes rune syntax (`$state`, `$derived`, `$effect`).

```
src/lib/stores/
  mode.svelte.ts      -- Active mode, mode configs
  chat.svelte.ts      -- Chat sessions, messages, per-mode histories
  engine.svelte.ts    -- Engine readiness, generation state, backend info
  mcp.svelte.ts       -- MCP connection status, available tools per mode
  ui.svelte.ts        -- Sidebar, overlays, scroll state
  settings.svelte.ts  -- User preferences (theme, temperature, etc.)
```

### 6.2 Store Pattern (Canonical Example)

Every store follows this exact structure:

```typescript
// src/lib/stores/example.svelte.ts

import type { SomeType } from '$lib/types/example';
import { saveToStorage, loadFromStorage } from '$lib/utils/storage';

// --- Private module-level state (only accessible via exported object) ---

const STORAGE_KEY = 'smolpc_example';
const initialData = loadFromStorage<SomeType[]>(STORAGE_KEY, []);

let items = $state<SomeType[]>(initialData);
let selectedId = $state<string | null>(null);
let isLoading = $state(false);

// --- Derived state ---

const selectedItem = $derived<SomeType | null>(
  items.find(item => item.id === selectedId) ?? null
);

const itemCount = $derived(items.length);

// --- Exported store object ---

export const exampleStore = {
  // Getters (reactive reads)
  get items() { return items; },
  get selectedId() { return selectedId; },
  get selectedItem() { return selectedItem; },
  get itemCount() { return itemCount; },
  get isLoading() { return isLoading; },

  // Mutations
  addItem(item: SomeType) {
    items = [...items, item];
    this.persist();
  },

  removeItem(id: string) {
    items = items.filter(i => i.id !== id);
    if (selectedId === id) {
      selectedId = items[0]?.id ?? null;
    }
    this.persist();
  },

  select(id: string) {
    if (items.some(i => i.id === id)) {
      selectedId = id;
    }
  },

  // Persistence
  persist() {
    saveToStorage(STORAGE_KEY, items);
  }
};
```

### 6.3 Single Source of Truth Rules

- If a store tracks `isGenerating`, components read it from the store. Components do NOT maintain their own `isGenerating` shadow variable.
- If a store tracks `messages`, components bind to `store.messages`. They do NOT copy messages into local state.
- Derived values computed from store state belong in the store as `$derived`, not recomputed in every component.
- The only exception is truly local UI state (e.g., `let showDropdown = $state(false)`) that no other component needs.

### 6.4 Mode Store

```typescript
// src/lib/stores/mode.svelte.ts

import type { AppMode, ModeConfig } from '$lib/types/mode';
import { saveToStorage, loadFromStorage } from '$lib/utils/storage';

const STORAGE_KEY = 'smolpc_active_mode';

const MODE_CONFIGS: Record<AppMode, ModeConfig> = {
  gimp: {
    id: 'gimp',
    label: 'GIMP',
    subtitle: 'AI-powered image editing',
    icon: 'palette',    // Lucide icon name, or use emoji fallback
    systemPrompt: `You are a GIMP image editing assistant. You help users edit images by calling GIMP tools through MCP. When a user asks you to perform an image operation, plan the steps and execute them using the available GIMP tools. After each operation, explain what you did and offer an undo option if the operation modified the image.`,
    suggestions: [
      'Draw a red circle',
      'Increase brightness',
      'Blur the image',
      'Draw a blue heart',
      'Blur the top half',
      'Brighten the bottom half',
    ],
    mcpServerName: 'gimp-mcp',
  },
  blender: {
    id: 'blender',
    label: 'Blender',
    subtitle: '3D modeling assistant',
    icon: 'box',
    systemPrompt: `You are a Blender 3D modeling assistant. You help users create and modify 3D scenes by calling Blender tools through MCP. Plan operations step by step and explain what each step does.`,
    suggestions: [
      'Create a cube',
      'Add a material',
      'Set up lighting',
      'Create a sphere',
      'Add a camera',
      'Render the scene',
    ],
    mcpServerName: 'blender-mcp',
  },
  writer: {
    id: 'writer',
    label: 'Writer',
    subtitle: 'Document assistant',
    icon: 'file-text',
    systemPrompt: `You are a LibreOffice Writer assistant. You help users create and format documents by calling Writer tools through MCP. When formatting text, explain the formatting options available.`,
    suggestions: [
      'Create a new document',
      'Make the title bold',
      'Insert a table',
      'Add page numbers',
      'Create a bulleted list',
      'Set margins to 1 inch',
    ],
    mcpServerName: 'writer-mcp',
  },
  calc: {
    id: 'calc',
    label: 'Calc',
    subtitle: 'Spreadsheet assistant',
    icon: 'table',
    systemPrompt: `You are a LibreOffice Calc spreadsheet assistant. You help users work with spreadsheets by calling Calc tools through MCP. When creating formulas, explain the formula syntax.`,
    suggestions: [
      'Create a formula',
      'Format as currency',
      'Create a chart',
      'Sum a column',
      'Sort by date',
      'Add conditional formatting',
    ],
    mcpServerName: 'calc-mcp',
  },
  impress: {
    id: 'impress',
    label: 'Impress',
    subtitle: 'Presentation assistant',
    icon: 'presentation',
    systemPrompt: `You are a LibreOffice Impress presentation assistant. You help users create and edit presentations by calling Impress tools through MCP. Suggest visual improvements and layout options.`,
    suggestions: [
      'Add a new slide',
      'Insert a title',
      'Add an image',
      'Change the layout',
      'Add speaker notes',
      'Apply a theme',
    ],
    mcpServerName: 'impress-mcp',
  },
};

let activeMode = $state<AppMode>(
  loadFromStorage<AppMode>(STORAGE_KEY, 'gimp')
);

const activeModeConfig = $derived(MODE_CONFIGS[activeMode]);

export const modeStore = {
  get activeMode() { return activeMode; },
  get config() { return activeModeConfig; },
  get allModes() { return Object.values(MODE_CONFIGS); },

  setMode(mode: AppMode) {
    activeMode = mode;
    saveToStorage(STORAGE_KEY, mode);
    // NOTE: Mode switching does NOT restart the engine or reload the model.
    // It changes: system prompt, MCP connection, suggestion chips, available tools.
    // Chat histories are per-mode (filtered by mode field on Chat objects).
  },
};
```

### 6.5 Chat Store (Per-Mode Histories)

The chat store manages all chat sessions across all modes. Chats are tagged with their `mode` field. When the active mode changes, the UI filters to show only chats for that mode.

```typescript
// Key difference from single-mode: chats have a `mode` field.
// The store exposes a derived `currentModeChats` that filters by modeStore.activeMode.

let chats = $state<Chat[]>(loadFromStorage<Chat[]>('smolpc_chats', []));
let currentChatId = $state<string | null>(null);

// Filter chats by active mode
const currentModeChats = $derived(
  chats.filter(c => c.mode === modeStore.activeMode)
);

// Current chat must belong to the active mode
const currentChat = $derived<Chat | null>(
  currentModeChats.find(c => c.id === currentChatId) ?? null
);
```

When mode switches:
- `currentChatId` may become null if the previously selected chat belongs to a different mode
- The UI shows the most recent chat for the new mode, or the empty state
- No chats are deleted -- they remain in storage and reappear when the mode switches back

### 6.6 Engine Store

The engine store wraps all Tauri IPC calls related to the inference engine. It owns `isGenerating` and ensures proper cleanup via `finally` blocks.

Key methods:
- `ensureStarted(request)` -- Blocking startup handshake
- `refreshReadiness()` -- Poll readiness from engine
- `generateStream(prompt, onToken, config)` -- Stream text generation via Channel
- `cancel()` -- Cancel current generation
- `syncStatus()` -- Refresh readiness + backend status

The `isGenerating` flag is set to `true` before generation starts and `false` in the `finally` block, guaranteeing cleanup even on error or cancellation.

---

## 7. Component Hierarchy

### 7.1 Unified App Shell

```
App.svelte (root)
|
+-- Header
|   +-- AppIdentity (icon + title + subtitle, driven by active mode)
|   +-- ModeDropdown (switches between GIMP/Blender/Writer/Calc/Impress)
|   +-- StatusBar (engine status + MCP connection + app info)
|   +-- DevToolsButton (toggle devtools panel)
|
+-- MainArea
|   +-- ChatArea
|   |   +-- EmptyState (shown when no messages)
|   |   |   +-- ModeWelcome (mode-specific welcome text)
|   |   |   +-- SuggestionChips (per-mode suggestions)
|   |   |
|   |   +-- MessageList
|   |   |   +-- ChatMessage (per message)
|   |   |       +-- Avatar
|   |   |       +-- MessageBubble
|   |   |       +-- ExplainTip (optional)
|   |   |       +-- UndoButton (optional)
|   |   |       +-- MessageActions (regenerate, continue, copy code)
|   |   |
|   |   +-- TypingIndicator (shown during generation)
|   |
|   +-- InputBar
|       +-- AutoGrowTextarea
|       +-- SendButton / StopButton
|
+-- DevToolsPanel (overlay, right side)
    +-- EngineSection (LLM health check)
    +-- McpSection (MCP tools list for active mode)
    +-- QuickActions (mode-specific test actions)
    +-- ActionLog
```

### 7.2 Root App Component

The root component manages:
- Engine startup handshake on mount
- Mode switching coordination
- Message sending and streaming
- Keyboard shortcut handling
- Auto-scroll behavior
- DevTools panel visibility

```svelte
<!-- src/routes/+page.svelte -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { Channel } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { modeStore } from "$lib/stores/mode.svelte";
  import { chatStore } from "$lib/stores/chat.svelte";
  import { engineStore } from "$lib/stores/engine.svelte";
  import { mcpStore } from "$lib/stores/mcp.svelte";
  import Header from "$lib/components/Header.svelte";
  import ChatArea from "$lib/components/ChatArea.svelte";
  import InputBar from "$lib/components/InputBar.svelte";
  import DevToolsPanel from "$lib/components/DevToolsPanel.svelte";
  import type { AssistantResponse, Message } from "$lib/types";

  let chatEl = $state<HTMLElement | undefined>(undefined);
  let showDevTools = $state(false);

  const currentChat = $derived(chatStore.currentChat);
  const messages = $derived(currentChat?.messages ?? []);

  // Auto-scroll when messages update
  $effect(() => {
    messages; // track dependency
    if (chatEl) chatEl.scrollTop = chatEl.scrollHeight;
  });

  // Reconnect MCP when mode changes
  $effect(() => {
    const mode = modeStore.activeMode;
    mcpStore.reconnect(mode);
  });

  onMount(async () => {
    await engineStore.ensureStarted();
    await mcpStore.connect(modeStore.activeMode);
  });

  async function handleSend(text: string) {
    if (!text.trim() || engineStore.isGenerating) return;

    const activeChat = currentChat
      ?? chatStore.createChat(modeStore.activeMode);

    // Add user message
    const userMsg: Message = {
      id: crypto.randomUUID(),
      role: 'user',
      text: text.trim(),
      timestamp: Date.now(),
    };
    chatStore.addMessage(activeChat.id, userMsg);

    // Add empty assistant message (streaming placeholder)
    const assistantMsg: Message = {
      id: crypto.randomUUID(),
      role: 'assistant',
      text: '',
      isStreaming: true,
      timestamp: Date.now(),
    };
    chatStore.addMessage(activeChat.id, assistantMsg);

    // Stream response
    const channel = new Channel<string>();
    channel.onmessage = (token: string) => {
      chatStore.appendToMessage(activeChat.id, assistantMsg.id, token);
    };

    try {
      const result = await invoke<AssistantResponse>("assistant_chat_stream", {
        prompt: text.trim(),
        mode: modeStore.activeMode,
        systemPrompt: modeStore.config.systemPrompt,
        onToken: channel,
      });

      chatStore.updateMessage(activeChat.id, assistantMsg.id, {
        text: result.reply || chatStore.getMessage(activeChat.id, assistantMsg.id)?.text || '',
        explain: result.explain,
        undoable: result.undoable ?? false,
        isStreaming: false,
      });
    } catch (e) {
      chatStore.updateMessage(activeChat.id, assistantMsg.id, {
        text: `Error: ${String(e)}`,
        isStreaming: false,
      });
    }
  }

  async function handleStop() {
    try {
      await invoke("engine_cancel");
    } catch (e) {
      console.error("Cancel failed:", e);
    }
  }

  async function handleUndo() {
    try {
      await invoke("macro_undo");
      // Optionally add a system message confirming undo
    } catch (e) {
      console.error("Undo failed:", e);
    }
  }
</script>

<div class="app">
  <Header
    bind:showDevTools
    mode={modeStore.config}
    engineReady={engineStore.isReady}
    mcpConnected={mcpStore.isConnected}
  />

  <div class="main">
    <ChatArea
      {messages}
      isGenerating={engineStore.isGenerating}
      suggestions={modeStore.config.suggestions}
      bind:chatEl
      onSuggestionClick={handleSend}
      onUndo={handleUndo}
    />

    <InputBar
      isGenerating={engineStore.isGenerating}
      disabled={!engineStore.isReady}
      onSend={handleSend}
      onStop={handleStop}
    />
  </div>

  {#if showDevTools}
    <DevToolsPanel
      mode={modeStore.activeMode}
      onClose={() => showDevTools = false}
    />
  {/if}
</div>
```

---

## 8. Mode System

### 8.1 Mode Dropdown Component

The mode dropdown is located in the header. It shows the current mode's icon and label, and opens a dropdown to switch modes.

```svelte
<!-- src/lib/components/ModeDropdown.svelte -->
<script lang="ts">
  import { modeStore } from '$lib/stores/mode.svelte';
  import type { AppMode } from '$lib/types/mode';

  let open = $state(false);

  function selectMode(mode: AppMode) {
    modeStore.setMode(mode);
    open = false;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') open = false;
  }
</script>

<div class="mode-dropdown" onkeydown={handleKeydown}>
  <button
    class="mode-dropdown-trigger"
    onclick={() => open = !open}
    aria-expanded={open}
    aria-haspopup="listbox"
  >
    <span class="mode-icon">{modeStore.config.icon}</span>
    <span class="mode-label">{modeStore.config.label}</span>
    <span class="mode-chevron">{open ? '\u25B2' : '\u25BC'}</span>
  </button>

  {#if open}
    <ul class="mode-dropdown-menu" role="listbox">
      {#each modeStore.allModes as mode}
        <li>
          <button
            class="mode-option"
            class:active={mode.id === modeStore.activeMode}
            onclick={() => selectMode(mode.id)}
            role="option"
            aria-selected={mode.id === modeStore.activeMode}
          >
            <span class="mode-option-icon">{mode.icon}</span>
            <div class="mode-option-text">
              <span class="mode-option-label">{mode.label}</span>
              <span class="mode-option-sub">{mode.subtitle}</span>
            </div>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
```

### 8.2 Mode Switching Behavior

**What changes when mode switches:**

| Aspect | Changes? | Details |
|--------|----------|---------|
| System prompt | Yes | Each mode has its own system prompt sent to the LLM |
| MCP server | Yes | Disconnects from current MCP, connects to new mode's MCP |
| Suggestion chips | Yes | Empty state shows mode-specific suggestions |
| Available tools | Yes | DevTools panel shows tools from the new mode's MCP |
| App icon/theme | Yes | Header icon and subtitle update |
| Chat history | Yes | UI filters to show chats tagged with the new mode |

**What does NOT change when mode switches:**

| Aspect | Changes? | Details |
|--------|----------|---------|
| Engine process | No | Engine keeps running, model stays loaded |
| Loaded model | No | Same model serves all modes |
| Other mode's chats | No | They persist in storage, hidden until mode switches back |
| Settings | No | Temperature, theme, etc. are global |

---

## 9. Chat UI

### 9.1 Message Display

Each message is rendered as a `ChatMessage` component. The layout differs between user and assistant messages.

**User messages:**
- Aligned to the right side
- Blue/primary-colored bubble
- No avatar (or user icon avatar)
- No action buttons

**Assistant messages:**
- Aligned to the left side
- Neutral/gray bubble
- Bot avatar icon
- Markdown rendering for content
- Optional "explain" tip below the bubble (yellow/amber callout)
- Optional "Undo" button for undoable operations
- Action buttons: Regenerate, Continue, Copy Code (if code blocks present)

### 9.2 ChatMessage Component

```svelte
<!-- src/lib/components/ChatMessage.svelte -->
<script lang="ts">
  import type { Message } from '$lib/types/chat';
  import { renderMarkdown } from '$lib/utils/markdown';

  interface Props {
    message: Message;
    onUndo?: () => void;
  }

  let { message, onUndo }: Props = $props();

  const renderedContent = $derived(renderMarkdown(message.text));
</script>

<div class="row {message.role}">
  {#if message.role === 'assistant'}
    <div class="avatar">
      <!-- Bot icon -->
    </div>
  {/if}

  <div class="col">
    <div class="bubble {message.role}">
      {#if message.role === 'assistant'}
        {@html renderedContent}
      {:else}
        {message.text}
      {/if}
    </div>

    {#if message.explain}
      <div class="tip">
        <span class="tip-text">{message.explain}</span>
      </div>
    {/if}

    {#if message.undoable && onUndo}
      <button class="undo-btn" onclick={onUndo}>Undo</button>
    {/if}

    {#if message.isStreaming}
      <div class="streaming-indicator">
        <span class="streaming-dot"></span>
        Generating...
      </div>
    {/if}
  </div>
</div>
```

### 9.3 Streaming Display

During token streaming, the assistant message bubble updates in real-time. The streaming indicator (pulsing dot + "Generating...") appears below the message content.

Token accumulation pattern:
```typescript
// In the store
appendToMessage(chatId: string, messageId: string, token: string) {
  const chat = chats.find(c => c.id === chatId);
  if (!chat) return;
  const msg = chat.messages.find(m => m.id === messageId);
  if (!msg || !msg.isStreaming) return;

  // Append token to existing text
  msg.text += token;
  chat.updatedAt = Date.now();
}
```

The Svelte reactivity system automatically re-renders the component when `msg.text` is mutated through the store.

### 9.4 Auto-Scroll Behavior

The chat area auto-scrolls to the bottom whenever new messages arrive or tokens stream in, UNLESS the user has manually scrolled up. This prevents the UI from fighting the user's scroll position.

```typescript
let userHasScrolledUp = $state(false);

function handleScroll() {
  if (!chatEl) return;
  const threshold = 5;
  const distanceFromBottom =
    chatEl.scrollHeight - chatEl.scrollTop - chatEl.clientHeight;
  if (distanceFromBottom <= threshold) {
    userHasScrolledUp = false;
  }
}

function handleWheel(event: WheelEvent) {
  if (event.deltaY < 0) {
    userHasScrolledUp = true;
  }
}

function scrollToBottom() {
  if (chatEl && !userHasScrolledUp) {
    chatEl.scrollTop = chatEl.scrollHeight;
  }
}

// Auto-scroll on new messages
$effect(() => {
  messages; // track
  scrollToBottom();
});
```

When the user has scrolled up, a "Jump to latest" button appears at the bottom of the chat area. Clicking it resets `userHasScrolledUp` and scrolls to bottom.

### 9.5 Typing Indicator

Shown when `isGenerating` is true and the latest assistant message has `isStreaming: true`. Uses three bouncing dots animation.

```svelte
{#if isGenerating}
  <div class="row assistant">
    <div class="avatar"><!-- Bot icon --></div>
    <div class="col">
      <div class="bubble assistant typing">
        <span></span><span></span><span></span>
      </div>
    </div>
  </div>
{/if}
```

CSS for the bouncing dots:

```css
.bubble.typing {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 12px 16px;
}

.bubble.typing span {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #999;
  display: inline-block;
  animation: bounce 1.2s infinite;
}

.bubble.typing span:nth-child(2) { animation-delay: 0.2s; }
.bubble.typing span:nth-child(3) { animation-delay: 0.4s; }

@keyframes bounce {
  0%, 60%, 100% { transform: translateY(0); opacity: 0.5; }
  30% { transform: translateY(-5px); opacity: 1; }
}
```

**Note:** The typing indicator and the streaming indicator on the message bubble serve different purposes. The typing indicator is a standalone element shown before any text arrives. The streaming indicator is attached to a message that already has partial text content.

---

## 10. Input Bar

### 10.1 Structure

The input bar is fixed at the bottom of the main area. It consists of:
- An auto-growing `<textarea>` for user input
- A send/stop button to the right of the textarea
- A hint line showing keyboard shortcuts

### 10.2 Auto-Growing Textarea

The textarea starts at 1 row and grows with content up to a maximum height (120px in the GIMP assistant, 200px in the codehelper).

```svelte
<script lang="ts">
  interface Props {
    isGenerating: boolean;
    disabled: boolean;
    onSend: (text: string) => void;
    onStop: () => void;
  }

  let { isGenerating, disabled, onSend, onStop }: Props = $props();

  let input = $state('');
  let textareaEl = $state<HTMLTextAreaElement | undefined>(undefined);

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  }

  function handleInput() {
    if (!textareaEl) return;
    textareaEl.style.height = 'auto';
    textareaEl.style.height = Math.min(textareaEl.scrollHeight, 120) + 'px';
  }

  function handleSubmit() {
    const trimmed = input.trim();
    if (!trimmed || disabled || isGenerating) return;
    onSend(trimmed);
    input = '';
    resetHeight();
  }

  function resetHeight() {
    if (textareaEl) textareaEl.style.height = 'auto';
  }
</script>

<div class="input-bar">
  <textarea
    bind:this={textareaEl}
    bind:value={input}
    onkeydown={handleKeydown}
    oninput={handleInput}
    placeholder={isGenerating ? 'Generating...' : 'Type a message... (Enter to send)'}
    disabled={disabled || isGenerating}
    rows="1"
  ></textarea>

  {#if isGenerating}
    <button class="stop-btn" onclick={onStop} title="Stop generation">
      <!-- Square icon -->
    </button>
  {:else}
    <button
      class="send-btn"
      onclick={handleSubmit}
      disabled={disabled || !input.trim()}
      title="Send"
    >
      <!-- Arrow up icon -->
    </button>
  {/if}
</div>
```

### 10.3 Keyboard Behavior

| Key Combination | Action |
|----------------|--------|
| Enter | Send message |
| Shift + Enter | Insert newline |
| (textarea disabled during generation) | -- |

### 10.4 Button States

| State | Button | Style |
|-------|--------|-------|
| Idle, empty input | Send (disabled) | Muted/faded appearance |
| Idle, has input | Send (enabled) | Blue/primary circle with arrow |
| Generating | Stop | Red circle with square icon |

---

## 11. Status Bar

### 11.1 Structure

The status bar sits in the header, to the right of the mode dropdown. It shows connection and readiness information.

### 11.2 Status Indicators

**Engine Status Indicator:**

| State | Visual | Text |
|-------|--------|------|
| `idle` / `starting` / `probing` / `resolving_assets` / `loading_model` | Gray dot, animated | "Starting engine..." |
| `ready` | Green dot | Model name (e.g., "qwen2.5-1.5b") |
| `failed` | Red dot | "Startup failed" + error code |
| Generating | Yellow/amber dot, pulsing | "Generating" |

**MCP Connection Status:**

| State | Visual | Text |
|-------|--------|------|
| Connected | Green pill | Mode-specific info (e.g., "photo.xcf" for GIMP) |
| Disconnected | Gray pill | "GIMP offline" / "Blender offline" / etc. |

### 11.3 StatusBar Component

```svelte
<!-- src/lib/components/StatusBar.svelte -->
<script lang="ts">
  interface Props {
    engineReady: boolean;
    engineState: string;        // EngineReadinessState
    mcpConnected: boolean;
    modelName: string | null;
    appInfo: string;            // e.g., image filename for GIMP
    modeName: string;
  }

  let { engineReady, engineState, mcpConnected, modelName, appInfo, modeName }: Props = $props();

  const engineLabel = $derived(
    engineState === 'ready'
      ? (modelName ?? 'Model loaded')
      : engineState === 'failed'
        ? 'Engine failed'
        : 'Starting engine...'
  );

  const mcpLabel = $derived(
    mcpConnected
      ? (appInfo || `${modeName} connected`)
      : `${modeName} offline`
  );
</script>

<div class="status-bar">
  <!-- Engine status -->
  <div class="status-pill" class:ready={engineReady} class:failed={engineState === 'failed'}>
    <span class="status-dot"></span>
    <span class="status-label">{engineLabel}</span>
  </div>

  <!-- MCP status -->
  <div class="status-pill" class:connected={mcpConnected}>
    <span class="status-dot"></span>
    <span class="status-label">{mcpLabel}</span>
  </div>
</div>
```

### 11.4 Status Pill CSS

```css
.status-pill {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 20px;
  background: #f5f5f5;
  border: 1px solid #e0e0e0;
  font-size: 12px;
  color: #888;
  transition: background 0.2s, color 0.2s;
}

.status-pill.connected,
.status-pill.ready {
  background: #f0faf3;
  border-color: #b8e6c6;
  color: #2a7a45;
}

.status-pill.failed {
  background: #fef2f2;
  border-color: #fecaca;
  color: #dc2626;
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #ccc;
  flex-shrink: 0;
  transition: background 0.2s;
}

.status-pill.connected .status-dot,
.status-pill.ready .status-dot {
  background: #34c759;
}

.status-pill.failed .status-dot {
  background: #dc2626;
}
```

---

## 12. Suggestion Chips

### 12.1 Behavior

Suggestion chips appear in the empty state (before any messages are sent). They are per-mode, defined in the `ModeConfig`. Clicking a chip sends that text as a user message.

### 12.2 Component

```svelte
<!-- src/lib/components/SuggestionChips.svelte -->
<script lang="ts">
  interface Props {
    suggestions: string[];
    onSelect: (text: string) => void;
  }

  let { suggestions, onSelect }: Props = $props();
</script>

<div class="empty-state">
  <div class="empty-icon"><!-- Mode-specific icon --></div>
  <p class="empty-title">What would you like to do?</p>
  <p class="empty-sub">Type a command or try one of these:</p>
  <div class="chips">
    {#each suggestions as suggestion}
      <button class="chip" onclick={() => onSelect(suggestion)}>
        {suggestion}
      </button>
    {/each}
  </div>
</div>
```

### 12.3 Per-Mode Suggestions

| Mode | Suggestions |
|------|------------|
| GIMP | "Draw a red circle", "Increase brightness", "Blur the image", "Draw a blue heart", "Blur the top half", "Brighten the bottom half" |
| Blender | "Create a cube", "Add a material", "Set up lighting", "Create a sphere", "Add a camera", "Render the scene" |
| Writer | "Create a new document", "Make the title bold", "Insert a table", "Add page numbers", "Create a bulleted list", "Set margins to 1 inch" |
| Calc | "Create a formula", "Format as currency", "Create a chart", "Sum a column", "Sort by date", "Add conditional formatting" |
| Impress | "Add a new slide", "Insert a title", "Add an image", "Change the layout", "Add speaker notes", "Apply a theme" |

### 12.4 Chip CSS

```css
.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  justify-content: center;
  max-width: 360px;
}

.chip {
  padding: 7px 14px;
  border-radius: 20px;
  border: 1px solid #d0d0d8;
  background: #fff;
  font-size: 13px;
  color: #444;
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s, color 0.15s;
  white-space: nowrap;
}

.chip:hover {
  background: #007aff;
  border-color: #007aff;
  color: #fff;
}
```

---

## 13. DevTools Panel

### 13.1 Structure

The DevTools panel is an overlay that slides in from the right side of the window. It sits on top of the chat area (position: absolute) and does not push content.

### 13.2 Sections

**Section 1: Engine / LLM Status**
- Shows current engine readiness state
- "Test Connection" button that calls `invoke("test_llm")`
- Displays connection result

**Section 2: MCP Tools (per mode)**
- Shows current MCP connection status for the active mode
- "Refresh Tools" button that calls `invoke("mcp_list_tools")`
- Displays available tools as JSON

**Section 3: Quick Actions (per mode)**
- Mode-specific test buttons
- GIMP mode: "Draw Line", "Crop Square", "Resize 1024w"
- Other modes: mode-appropriate quick test actions
- Action log showing recent results

### 13.3 Component

```svelte
<!-- src/lib/components/DevToolsPanel.svelte -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { AppMode } from '$lib/types/mode';

  interface Props {
    mode: AppMode;
    onClose: () => void;
  }

  let { mode, onClose }: Props = $props();

  let llmStatus = $state("Unknown");
  let mcpStatus = $state("Unknown");
  let llmTestResult = $state("");
  let toolsListResult = $state("");
  let actionLog = $state<string[]>([]);

  function logAction(msg: string) {
    actionLog = [msg, ...actionLog].slice(0, 20);
  }

  async function testLlm() {
    llmStatus = "Checking...";
    try {
      const result = await invoke<string>("test_llm");
      llmTestResult = result;
      llmStatus = "Connected";
    } catch (e) {
      llmTestResult = String(e);
      llmStatus = "Error";
    }
  }

  async function listTools() {
    mcpStatus = "Checking...";
    try {
      const result = await invoke<unknown>("mcp_list_tools");
      toolsListResult = JSON.stringify(result, null, 2);
      mcpStatus = "Connected";
    } catch (e) {
      toolsListResult = String(e);
      mcpStatus = "Disconnected";
    }
  }
</script>

<aside class="devtools">
  <div class="devtools-header">
    <span>Developer Tools</span>
    <button class="icon-btn" onclick={onClose}>X</button>
  </div>

  <details class="dev-section">
    <summary>LLM - <em>{llmStatus}</em></summary>
    <button class="dev-btn" onclick={testLlm}>Test Connection</button>
    {#if llmTestResult}<pre>{llmTestResult}</pre>{/if}
  </details>

  <details class="dev-section">
    <summary>{mode.toUpperCase()} MCP - <em>{mcpStatus}</em></summary>
    <button class="dev-btn" onclick={listTools}>Refresh Tools</button>
    {#if toolsListResult}<pre>{toolsListResult}</pre>{/if}
  </details>

  <details class="dev-section" open>
    <summary>Quick Actions</summary>
    <!-- Mode-specific actions rendered here -->
    {#if actionLog.length > 0}
      <pre class="action-log">{actionLog.join("\n")}</pre>
    {/if}
  </details>
</aside>
```

### 13.4 DevTools CSS

```css
.devtools {
  position: absolute;
  top: 57px;          /* Below header */
  right: 0;
  bottom: 0;
  width: 280px;
  background: #fff;
  border-left: 1px solid #e0e0e8;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  z-index: 10;
  box-shadow: -4px 0 16px rgba(0, 0, 0, 0.06);
}

.devtools-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 14px;
  border-bottom: 1px solid #eee;
  font-size: 13px;
  font-weight: 600;
  color: #333;
  flex-shrink: 0;
}

.dev-section {
  border-bottom: 1px solid #f0f0f0;
  font-size: 12px;
}

.dev-section > summary {
  padding: 10px 14px;
  cursor: pointer;
  user-select: none;
  color: #444;
  font-weight: 500;
  list-style: none;
}

.dev-section > summary::-webkit-details-marker { display: none; }
.dev-section > summary::before { content: ">> "; font-size: 9px; }
.dev-section[open] > summary::before { content: "v "; }
```

---

## 14. Tauri IPC Integration

### 14.1 Available Tauri Commands

These are the Rust `#[tauri::command]` functions available from the frontend via `invoke()`.

**Engine Commands (from engine-client adapter):**

| Command | Parameters | Return Type | Description |
|---------|-----------|-------------|-------------|
| `engine_ensure_started` | `{ request: EnsureStartedRequestDto }` | `EngineReadinessDto` | Blocking startup handshake |
| `engine_status` | none | `EngineReadinessDto` | Poll current readiness |
| `get_inference_backend_status` | none | `BackendStatus` | Full backend diagnostics |
| `list_models` | none | `AvailableModel[]` | List available models |
| `load_model` | `{ modelId: string }` | void | Load specific model |
| `unload_model` | none | void | Unload current model |
| `inference_generate` | `{ prompt, config?, onToken: Channel<string> }` | `GenerationMetrics` | Stream generation |
| `inference_generate_messages` | `{ messages, config?, onToken: Channel<string> }` | `GenerationMetrics` | Stream chat generation |
| `inference_cancel` | none | void | Cancel current generation |
| `check_model_readiness` | `{ modelId: string }` | `CheckModelResponse` | Check model readiness per lane |

**MCP Commands (from GIMP/mode backend):**

| Command | Parameters | Return Type | Description |
|---------|-----------|-------------|-------------|
| `mcp_list_tools` | none | `Value` (JSON) | List available MCP tools |
| `mcp_call_tool` | `{ name: string, arguments: Value }` | `Value` (JSON) | Call an MCP tool |

**Assistant Commands:**

| Command | Parameters | Return Type | Description |
|---------|-----------|-------------|-------------|
| `assistant_request` | `{ prompt: string }` | `AssistantResponse` | Non-streaming assistant request |
| `assistant_chat_stream` | `{ prompt, mode?, systemPrompt?, onToken: Channel<string> }` | `AssistantResponse` | Streaming assistant request |
| `test_llm` | none | `string` | Test LLM connection |
| `macro_undo` | none | void | Undo last operation |

**Note:** The exact command names and signatures depend on which Tauri backend is active. The GIMP assistant has its own set of commands. The unified app should consolidate these into a consistent command interface.

### 14.2 Invoke Pattern

```typescript
import { invoke } from "@tauri-apps/api/core";

// Simple command (no streaming)
try {
  const result = await invoke<AssistantResponse>("assistant_request", {
    prompt: "Draw a red circle"
  });
  console.log(result.reply);
} catch (e) {
  // Rust Result::Err(String) becomes a rejected promise with the error string
  console.error("Command failed:", e);
}

// Command with Channel streaming
import { Channel } from "@tauri-apps/api/core";

const channel = new Channel<string>();
channel.onmessage = (token: string) => {
  // Handle each token as it arrives
  accumulated += token;
};

const result = await invoke<AssistantResponse>("assistant_chat_stream", {
  prompt: "Draw a red circle",
  onToken: channel,
});
// invoke() resolves AFTER all channel messages have been delivered
// The Channel is automatically cleaned up -- it is scoped to this command invocation
```

---

## 15. Streaming Pattern

### 15.1 End-to-End Flow

```
Frontend                     Rust Backend                    Engine Host
   |                              |                              |
   |  invoke("assistant_chat_    |                              |
   |  stream", {prompt, onToken})|                              |
   |----------------------------->|                              |
   |                              |  POST /v1/chat/completions   |
   |                              |  (SSE stream request)        |
   |                              |----------------------------->|
   |                              |                              |
   |                              |  SSE: data: {"token":"Hello"}|
   |                              |<-----------------------------|
   |  Channel.onmessage("Hello") |                              |
   |<-----------------------------|                              |
   |                              |  SSE: data: {"token":" world"}|
   |                              |<-----------------------------|
   |  Channel.onmessage(" world")|                              |
   |<-----------------------------|                              |
   |                              |  SSE: data: [DONE]           |
   |                              |<-----------------------------|
   |                              |                              |
   |  invoke() resolves with     |                              |
   |  AssistantResponse          |                              |
   |<-----------------------------|                              |
```

### 15.2 Frontend Channel Setup

```typescript
async function streamGeneration(prompt: string): Promise<void> {
  // 1. Create a Channel (command-scoped, ordered, auto-cleaned)
  const channel = new Channel<string>();

  // 2. Set up the message handler BEFORE invoking
  channel.onmessage = (token: string) => {
    // Update reactive state -- this triggers re-render
    chatStore.appendToMessage(chatId, messageId, token);
  };

  // 3. Invoke the command, passing the channel
  try {
    const result = await invoke<AssistantResponse>("assistant_chat_stream", {
      prompt,
      onToken: channel,
    });
    // 4. invoke() resolves AFTER all tokens have been delivered via the channel
    // The `result` contains the final response metadata
  } catch (e) {
    // Handle error
  }

  // 5. No cleanup needed -- Channel is command-scoped
}
```

### 15.3 Why Channels, Not Events

| Aspect | Channel | Event |
|--------|---------|-------|
| Scope | Command-scoped (auto-cleanup) | Global (must manually unlisten) |
| Ordering | Guaranteed in-order delivery | No ordering guarantee |
| Race conditions | None (one consumer per channel) | Possible (multiple listeners) |
| Lifecycle | Tied to invoke() call | Must manage manually |

### 15.4 Cancellation

To cancel a streaming generation mid-flight:

```typescript
// Frontend
async function cancelGeneration() {
  try {
    await invoke("inference_cancel");
    // The streaming invoke() will reject or resolve shortly after
  } catch (e) {
    console.error("Cancel failed:", e);
  }
}
```

The Rust backend sets an `AtomicBool` cancel flag that the generation loop checks on each token. When cancelled, the `invoke()` promise may reject with an `INFERENCE_GENERATION_CANCELLED` error message -- handle this gracefully (do not show it as an error to the user).

```typescript
// In the catch block of the streaming invoke:
catch (e) {
  const message = String(e);
  if (message.includes('INFERENCE_GENERATION_CANCELLED') ||
      message.includes('Generation cancelled')) {
    // Not an error -- user cancelled intentionally
    return;
  }
  // Actual error
  error = message;
}
```

---

## 16. CSS and Design Tokens

### 16.1 Design Token Strategy

The unified app uses CSS custom properties (variables) for all design tokens. These are defined in `app.css` at the `:root` and `.dark` levels.

The existing codehelper app uses an OKLCH-based color system with semantic surface/outline/brand layers built on top. The unified app should follow the same approach.

### 16.2 Core Design Tokens (from existing codehelper app.css)

```css
:root {
  /* --- Base radius --- */
  --radius: 0.72rem;

  /* --- Semantic colors (OKLCH) --- */
  --primary: oklch(0.54 0.07 240);
  --primary-foreground: oklch(0.986 0.003 255);
  --background: oklch(0.986 0.003 255);
  --foreground: oklch(0.255 0.01 260);
  --muted: oklch(0.955 0.003 255);
  --muted-foreground: oklch(0.53 0.01 260);
  --border: oklch(0.89 0.006 255);
  --destructive: oklch(0.64 0.18 26);
  --card: oklch(0.992 0.003 255);
  --accent: oklch(0.948 0.004 255);

  /* --- Feedback colors --- */
  --color-success: oklch(0.71 0.11 153);
  --color-warning: oklch(0.83 0.12 86);

  /* --- Derived surface tokens --- */
  --surface-canvas: color-mix(in srgb, var(--color-background) 94%, black);
  --surface-subtle: color-mix(in srgb, var(--color-card) 95%, var(--color-accent));
  --surface-elevated: color-mix(in srgb, var(--color-card) 98%, white);
  --surface-widget: color-mix(in srgb, var(--color-card) 97%, white);
  --surface-floating: color-mix(in srgb, var(--color-card) 92%, white);
  --surface-hover: color-mix(in srgb, var(--color-accent) 58%, transparent);
  --surface-active: color-mix(in srgb, var(--color-primary) 10%, transparent);

  /* --- Brand tints --- */
  --brand-soft: color-mix(in srgb, var(--color-primary) 11%, transparent);
  --brand-soft-strong: color-mix(in srgb, var(--color-primary) 16%, transparent);

  /* --- Outline tokens --- */
  --outline-soft: color-mix(in srgb, var(--color-border) 88%, transparent);
  --outline-strong: color-mix(in srgb, var(--color-primary) 30%, var(--color-border));
  --focus-ring: color-mix(in srgb, var(--color-primary) 24%, transparent);

  /* --- Typography --- */
  --font-display: 'Inter', 'SF Pro Text', 'Segoe UI', sans-serif;
  --font-code: 'JetBrains Mono', 'IBM Plex Mono', monospace;

  /* --- Shadows --- */
  --shadow-soft: 0 10px 24px -18px rgb(13 18 34 / 30%);
  --shadow-strong: 0 24px 60px -30px rgb(13 18 34 / 46%);
  --glow-subtle: 0 0 0 1px rgb(255 255 255 / 2%),
                 0 12px 30px -24px rgb(15 22 42 / 50%);

  /* --- Motion --- */
  --motion-fast: 160ms cubic-bezier(0.16, 1, 0.3, 1);
  --motion-medium: 260ms cubic-bezier(0.22, 1, 0.36, 1);

  /* --- Radii (computed) --- */
  --radius-sm: calc(var(--radius) - 4px);
  --radius-md: calc(var(--radius) - 2px);
  --radius-lg: var(--radius);
  --radius-xl: calc(var(--radius) + 4px);
}
```

### 16.3 Dark Theme Tokens

```css
.dark {
  --background: oklch(0.17 0.006 262);
  --foreground: oklch(0.955 0.006 260);
  --card: oklch(0.205 0.008 262);
  --primary: oklch(0.72 0.08 240);
  --primary-foreground: oklch(0.16 0.006 262);
  --muted: oklch(0.254 0.008 262);
  --muted-foreground: oklch(0.785 0.007 260);
  --border: oklch(0.39 0.009 262 / 82%);
  --destructive: oklch(0.73 0.17 26);

  --color-success: oklch(0.76 0.1 152);
  --color-warning: oklch(0.86 0.11 86);

  --surface-canvas: oklch(0.165 0.006 262);
  --surface-subtle: color-mix(in srgb, var(--color-card) 97%, black);
  --surface-elevated: color-mix(in srgb, var(--color-card) 98%, black);
  --surface-widget: color-mix(in srgb, var(--color-card) 98%, black);

  --shadow-strong: 0 34px 70px -38px rgb(0 0 0 / 85%);
  --glow-subtle: 0 0 0 1px rgb(255 255 255 / 3%),
                 0 20px 48px -36px rgb(30 41 59 / 58%);
}
```

### 16.4 Theme Application

Theme is toggled by adding/removing the `dark` class on `<html>`:

```typescript
// src/lib/utils/theme.ts

export function applyTheme(theme: 'light' | 'dark' | 'system') {
  const resolved = theme === 'system'
    ? (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
    : theme;
  document.documentElement.classList.toggle('dark', resolved === 'dark');
}
```

### 16.5 Tailwind 4 Integration

The `app.css` file uses Tailwind 4's `@theme inline` block to register CSS variables as Tailwind theme values. This makes them available as Tailwind utility classes (e.g., `bg-background`, `text-foreground`, `border-border`).

```css
@theme inline {
  --color-background: var(--background);
  --color-foreground: var(--foreground);
  --color-primary: var(--primary);
  --color-primary-foreground: var(--primary-foreground);
  --color-border: var(--border);
  /* ... etc ... */
  --font-sans: var(--font-display);
  --font-mono: var(--font-code);
}
```

### 16.6 Component CSS Strategy

Components use a combination of:

1. **Tailwind utility classes** in templates for common patterns (`flex`, `items-center`, `gap-2`, `text-sm`, etc.)
2. **Scoped `<style>` blocks** for component-specific layout and visual treatment, referencing CSS variables
3. **`:global()` selectors** sparingly, for styling children of third-party components (e.g., shadcn button classes)

Example pattern from the codehelper:

```svelte
<article class="chat-message">
  <div class="chat-message__content prose prose-sm max-w-none break-words">
    {@html renderedContent}
  </div>
</article>

<style>
  .chat-message {
    display: flex;
    gap: 0.8rem;
    padding: 0.85rem;
    border: 1px solid var(--outline-soft);
    border-radius: calc(var(--radius-xl) + 1px);
    background: var(--surface-widget);
    box-shadow: var(--glow-subtle);
    animation: message-in var(--motion-medium);
  }

  /* For child elements inside rendered HTML */
  :global(.prose code) {
    font-family: var(--font-code);
    background: var(--surface-hover);
  }
</style>
```

---

## 17. Responsive Layout

### 17.1 Fixed Viewport Layout

The app uses a fixed `100vh` viewport with flex column layout. No scrolling on the body element.

```css
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
  position: relative;
}
```

### 17.2 Layout Zones

```
+------------------------------------------+
|  Header (flex-shrink: 0)                 |
|  [Icon] [Title] [ModeDropdown] [Status]  |
+------------------------------------------+
|                                          |
|  Chat Area (flex: 1, overflow-y: auto)   |
|                                          |
|  - Empty state or messages               |
|  - Scrollable                            |
|                                          |
+------------------------------------------+
|  Input Bar (flex-shrink: 0)              |
|  [Textarea] [Send/Stop]                 |
+------------------------------------------+
```

### 17.3 Breakpoints

The existing codehelper uses these breakpoints:

| Breakpoint | Behavior |
|-----------|----------|
| `> 900px` | Full layout with sidebar |
| `720px - 900px` | Reduced padding, sidebar collapsible |
| `< 768px` | Mobile-optimized padding, full-width controls |

### 17.4 DevTools Overlay Positioning

```css
.devtools {
  position: absolute;    /* Overlays chat, does not push content */
  top: 57px;             /* Below header */
  right: 0;
  bottom: 0;
  width: 280px;
  z-index: 10;
}
```

---

## 18. Markdown Rendering

### 18.1 Rendering Pipeline

Assistant messages support markdown formatting. The rendering pipeline:

1. Extract code blocks into placeholders (preserves them during processing)
2. Escape HTML in remaining text (XSS prevention)
3. Process inline formatting (code, bold, italic, links, lists)
4. Wrap text in paragraphs
5. Restore code blocks from placeholders
6. Sanitize final HTML with DOMPurify

### 18.2 Supported Markdown Features

| Feature | Syntax | Rendered |
|---------|--------|----------|
| Code blocks | ` ```lang\ncode\n``` ` | Syntax-highlighted block with copy button |
| Inline code | `` `code` `` | Styled inline element |
| Bold | `**text**` | `<strong>` |
| Italic | `*text*` | `<em>` |
| Headers | `# H1`, `## H2`, `### H3` | Sized headings |
| Links | `[text](url)` | Sanitized `<a>` with `target="_blank"` |
| Unordered lists | `- item` or `* item` | `<ul><li>` |
| Ordered lists | `1. item` | `<ol><li>` |

### 18.3 Code Block Rendering

Code blocks get a special visual treatment:
- Language label in a header bar
- Copy button (SVG clipboard icon) in the header bar
- Monospace font in a scrollable pre block
- Code content is base64-encoded in a `data-code` attribute for the copy button

Copy handling uses event delegation (CSP-compliant, no inline `onclick`):

```typescript
// Set up once via onMount
export function setupCodeCopyHandlers(container: HTMLElement): () => void {
  const handleClick = async (event: Event) => {
    const button = (event.target as HTMLElement).closest('.code-copy-btn');
    if (!button) return;
    const encodedCode = (button as HTMLElement).dataset.code;
    if (!encodedCode) return;
    const code = atob(encodedCode); // decode base64
    await navigator.clipboard.writeText(code);
    // Show success feedback
  };
  container.addEventListener('click', handleClick);
  return () => container.removeEventListener('click', handleClick);
}
```

### 18.4 Streaming Markdown

During streaming, the markdown renderer is called on every token update. This means:
- Incomplete code blocks (no closing ```) are still rendered as code blocks
- The renderer handles unclosed code blocks gracefully
- Bold/italic may flicker if a `*` arrives without its partner -- this is acceptable

### 18.5 Security

All rendered HTML is sanitized with DOMPurify:
- Only safe tags are allowed (`h1-h3`, `p`, `strong`, `em`, `code`, `pre`, `ul`, `ol`, `li`, `a`, `div`, `span`, `button`, `svg`, `path`)
- Only safe attributes are allowed (`href`, `class`, `data-code`, `title`, `aria-label`, `target`, `rel`, and SVG attributes)
- URLs are restricted to `https:`, `http:`, and `mailto:` protocols
- `javascript:`, `data:`, and other dangerous protocols are blocked

---

## 19. Keyboard Shortcuts

### 19.1 Global Shortcuts

| Shortcut | Action |
|----------|--------|
| `Enter` (in textarea) | Send message |
| `Shift + Enter` (in textarea) | Insert newline |
| `Escape` | Close any open overlay (devtools, shortcuts panel) |
| `Ctrl/Cmd + \` | Toggle sidebar |
| `Ctrl/Cmd + /` | Toggle keyboard shortcuts overlay |
| `?` (when not typing) | Show keyboard shortcuts |
| `Ctrl/Cmd + Shift + B` | Toggle benchmark panel |

### 19.2 Implementation

Keyboard shortcuts are handled via a global `keydown` listener attached in `onMount`:

```typescript
function handleKeyDown(event: KeyboardEvent) {
  const isMac = navigator.platform.toUpperCase().includes('MAC');
  const mod = isMac ? event.metaKey : event.ctrlKey;
  const typing = isTypingTarget(event.target);

  if (mod && event.key === '\\') {
    event.preventDefault();
    uiStore.toggleSidebar();
    return;
  }

  if (event.key === 'Escape') {
    if (uiStore.activeOverlay !== 'none') {
      uiStore.closeOverlay();
      event.preventDefault();
    }
    return;
  }

  // Don't intercept shortcuts when user is typing in an input
  if (!typing && event.key === '?') {
    event.preventDefault();
    showShortcutsOverlay = true;
  }
}

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  return ['TEXTAREA', 'INPUT', 'SELECT'].includes(target.tagName);
}

onMount(() => {
  window.addEventListener('keydown', handleKeyDown);
  return () => window.removeEventListener('keydown', handleKeyDown);
});
```

---

## 20. Persistence

### 20.1 Storage Utilities

All persistent state uses `localStorage` through type-safe wrapper functions:

```typescript
// src/lib/utils/storage.ts

export function saveToStorage<T>(key: string, data: T): void {
  try {
    localStorage.setItem(key, JSON.stringify(data));
  } catch (error) {
    console.error(`Failed to save to localStorage (${key}):`, error);
  }
}

export function loadFromStorage<T>(key: string, defaultValue: T): T {
  try {
    if (typeof window === 'undefined' || typeof localStorage === 'undefined') {
      return defaultValue;
    }
    const item = localStorage.getItem(key);
    if (item === null) return defaultValue;
    return JSON.parse(item) as T;
  } catch (error) {
    console.error(`Failed to load from localStorage (${key}):`, error);
    return defaultValue;
  }
}
```

### 20.2 Storage Keys

| Key | Type | Purpose |
|-----|------|---------|
| `smolpc_active_mode` | `AppMode` | Currently selected mode |
| `smolpc_chats` | `Chat[]` | All chat sessions (all modes) |
| `smolpc_current_chat` | `string \| null` | Currently selected chat ID |
| `smolpc_settings` | `AppSettings` | User preferences |

### 20.3 Persistence Rules

- Stores call `this.persist()` after every mutation that should survive page reloads
- Settings (theme, temperature, model selection) are persisted immediately
- Chat data is persisted after every message add/update
- Active mode is persisted on every mode switch
- Do NOT persist transient UI state (overlay open, sidebar open, scroll position)
- Do NOT persist engine/MCP connection status (these are always fetched fresh)

---

## 21. File Structure

### 21.1 Recommended Project Layout

```
apps/unified-assistant/
  src/
    app.css                          # Global styles, design tokens, Tailwind config
    app.html                         # HTML template
    routes/
      +layout.svelte                 # Root layout (imports app.css)
      +layout.ts                     # SPA mode config (ssr = false)
      +page.svelte                   # Main page (app shell)

    lib/
      types/
        chat.ts                      # Message, Chat interfaces
        mode.ts                      # AppMode, ModeConfig
        assistant.ts                 # AssistantResponse
        engine.ts                    # Engine DTOs (readiness, backend, metrics)
        mcp.ts                       # MCP tool types
        settings.ts                  # AppSettings

      stores/
        mode.svelte.ts               # Active mode, mode configs
        chat.svelte.ts               # Chat sessions, per-mode histories
        engine.svelte.ts             # Engine readiness, generation state
        mcp.svelte.ts                # MCP connection status per mode
        ui.svelte.ts                 # Sidebar, overlays, scroll state
        settings.svelte.ts           # User preferences

      components/
        Header.svelte                # App header with identity, mode, status
        ModeDropdown.svelte          # Mode selector dropdown
        StatusBar.svelte             # Engine + MCP status indicators
        ChatArea.svelte              # Scrollable message area
        ChatMessage.svelte           # Individual message bubble
        SuggestionChips.svelte       # Per-mode empty-state suggestions
        InputBar.svelte              # Textarea + send/stop button
        DevToolsPanel.svelte         # Developer tools overlay
        TypingIndicator.svelte       # Bouncing dots animation

      utils/
        storage.ts                   # localStorage wrappers
        markdown.ts                  # Markdown renderer + DOMPurify
        theme.ts                     # Theme application + system theme detection

  src-tauri/
    src/
      lib.rs                        # Tauri setup, command registration
      commands/
        assistant.rs                 # Chat + streaming commands
        mcp.rs                       # MCP tool commands per mode
        engine_client_adapter.rs     # Engine startup/readiness/inference
    Cargo.toml
    capabilities/
      default.json

  package.json
  vite.config.ts
  svelte.config.js
  tsconfig.json
  tailwind.config.ts                 # Minimal (Tailwind 4 needs less config)
```

### 21.2 File Naming Conventions

| Type | Convention | Example |
|------|-----------|---------|
| Svelte components | PascalCase | `ChatMessage.svelte` |
| Store files | camelCase + `.svelte.ts` | `chat.svelte.ts` |
| Type files | camelCase + `.ts` | `chat.ts` |
| Utility files | camelCase + `.ts` | `markdown.ts` |
| CSS files | camelCase + `.css` | `app.css` |
| Rust files | snake_case + `.rs` | `engine_client_adapter.rs` |

---

## 22. Existing Reference Implementations

### 22.1 GIMP Assistant (Simple, Single-File)

**Location:** `apps/gimp-assistant/src/routes/+page.svelte`

This is a self-contained single-file implementation with all logic in one component. It demonstrates:

- The minimal viable chat UI pattern
- Status pill component inline
- Suggestion chips inline
- DevTools panel inline
- Non-streaming `invoke("assistant_request")` call
- Undo via `invoke("macro_undo")`
- Auto-scroll with `$effect`
- Auto-growing textarea
- Typing indicator with bouncing dots

Key characteristics:
- All state is local (no stores)
- No markdown rendering (plain text bubbles)
- No streaming (full response at once)
- Scoped CSS only (no Tailwind, no design tokens)
- iOS-like visual style (blue bubbles, rounded pills)

This implementation should be used as the structural reference for the chat UI layout but upgraded with:
- Store-based state management
- Streaming via Channels
- Markdown rendering
- Design token CSS
- Mode dropdown

### 22.2 Code Helper (Complex, Component-Based)

**Location:** `apps/codehelper/src/`

This is a full-featured implementation with proper component architecture. It demonstrates:

- **Store pattern:** `chats.svelte.ts`, `inference.svelte.ts`, `settings.svelte.ts`, `ui.svelte.ts` -- all using Svelte 5 runes
- **Component hierarchy:** `App.svelte` -> `WorkspaceHeader` + `ConversationView` + `ComposerBar`
- **Streaming:** Full Channel-based streaming in `inference.svelte.ts` via `generateStreamMessages()`
- **Props pattern:** `interface Props` + `$props()` in every component
- **Design system:** Full OKLCH-based design tokens with light/dark themes
- **Markdown:** Custom renderer with DOMPurify sanitization
- **Sidebar:** Collapsible sidebar with chat history management
- **Overlays:** Benchmark, hardware, model info panels

Key files to study:
- `src/lib/stores/inference.svelte.ts` -- Canonical streaming pattern
- `src/lib/stores/chats.svelte.ts` -- Chat CRUD with persistence
- `src/lib/types/inference.ts` -- Full engine type definitions
- `src/lib/components/ChatMessage.svelte` -- Message rendering with actions
- `src/lib/components/ChatInput.svelte` -- Auto-growing textarea
- `src/lib/components/chat/ComposerBar.svelte` -- Send/stop button logic
- `src/App.svelte` -- Orchestration, streaming, keyboard shortcuts
- `src/app.css` -- Complete design token system

### 22.3 Migration Path

The unified app should:

1. Start from the codehelper's architecture (stores, components, design tokens)
2. Add the mode dropdown and mode store (new)
3. Replace the code-focused system prompt with per-mode system prompts
4. Replace QuickExamples with per-mode SuggestionChips
5. Add MCP connection management per mode (new)
6. Add undo capability from the GIMP assistant
7. Add the "explain" tip from the GIMP assistant's message format
8. Simplify the sidebar (remove code-specific features, add mode-based chat filtering)

---

## Appendix A: Rust-TypeScript Type Mapping Reference

This table shows the exact correspondence between Rust struct fields and TypeScript interface properties for the Tauri command boundary.

### AssistantResponse

| Rust (serde) | TypeScript | Notes |
|-------------|-----------|-------|
| `reply: String` | `reply: string` | |
| `explain: Option<String>` | `explain?: string \| null` | Svelte template checks with `{#if msg.explain}` |
| `undoable: Option<bool>` | `undoable?: boolean` | Defaults to `false` if absent |
| `plan: serde_json::Value` | `plan: any` | Opaque to frontend |
| `tool_results: Vec<serde_json::Value>` | `tool_results: any[]` | Opaque to frontend |

### EngineReadinessDto

| Rust (serde) | TypeScript |
|-------------|-----------|
| `attempt_id: String` | `attempt_id: string` |
| `state: String` | `state: EngineReadinessState` |
| `state_since: String` | `state_since: string` |
| `active_backend: Option<String>` | `active_backend: string \| null` |
| `active_model_id: Option<String>` | `active_model_id: string \| null` |
| `error_code: Option<String>` | `error_code: string \| null` |
| `error_message: Option<String>` | `error_message: string \| null` |
| `retryable: bool` | `retryable: boolean` |

---

## Appendix B: Tauri Channel API Reference

```typescript
import { Channel } from "@tauri-apps/api/core";

// Create a typed channel
const channel = new Channel<string>();

// Set the message handler (called for each message from Rust)
channel.onmessage = (message: string) => {
  // Process each message
};

// Pass channel to invoke -- Rust receives it as tauri::ipc::Channel<T>
const result = await invoke<ReturnType>("command_name", {
  onToken: channel,     // Parameter name must match Rust command parameter
});

// Key behaviors:
// - invoke() resolves AFTER all channel messages are delivered
// - Messages arrive in order
// - Channel is automatically cleaned up when invoke() resolves
// - No manual cleanup (unlisten, close, etc.) is needed
// - One channel per invoke() call -- do not reuse channels
```

Corresponding Rust side:

```rust
#[tauri::command]
async fn command_name(
    on_token: tauri::ipc::Channel<String>,
    // ... other parameters
) -> Result<ReturnType, String> {
    // Send messages through the channel
    on_token.send("Hello ".to_string()).map_err(|e| e.to_string())?;
    on_token.send("World".to_string()).map_err(|e| e.to_string())?;

    // Return value -- invoke() resolves with this AFTER all sends complete
    Ok(ReturnType { /* ... */ })
}
```

---

## Appendix C: SvelteKit SPA Configuration

The app runs as a SPA (Single Page Application) with no server-side rendering. This is required because Tauri does not provide a Node.js server.

```typescript
// src/routes/+layout.ts
export const ssr = false;
```

```svelte
<!-- src/routes/+layout.svelte -->
<script lang="ts">
  import "../app.css";
</script>

<slot />
```

The `adapter-static` is used in `svelte.config.js` with a fallback to `index.html`:

```javascript
// svelte.config.js
import adapter from '@sveltejs/adapter-static';

export default {
  kit: {
    adapter: adapter({
      fallback: 'index.html'
    })
  }
};
```

---

## Appendix D: Accessibility Baseline

- All interactive elements must have `aria-label` or visible text labels
- The mode dropdown uses `aria-expanded`, `aria-haspopup="listbox"`, `role="option"`, and `aria-selected`
- Chat messages area uses `role="region"` with `aria-label="Conversation messages"`
- Focus management: pressing Escape closes overlays and returns focus appropriately
- All buttons have `type="button"` to prevent accidental form submissions
- Keyboard navigation: Tab through interactive elements, Enter/Space to activate

---

## Appendix E: Reduced Motion Support

Respect the `prefers-reduced-motion` media query:

```css
@media (prefers-reduced-motion: reduce) {
  * {
    animation-duration: 0.001ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.001ms !important;
    scroll-behavior: auto !important;
  }
}
```

This is already present in the codehelper `app.css` and should be carried forward.
