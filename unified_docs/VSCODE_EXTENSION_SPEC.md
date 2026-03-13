# SmolPC Code Helper -- VS Code Extension Specification

**Version:** 1.0
**Last Updated:** 2026-03-13
**Status:** Implementation reference -- canonical source of truth for all AI-assisted development sessions building the VS Code extension

---

## How to Use This Document

This specification is the sole reference for building the SmolPC Code Helper VS Code extension. It assumes **zero prior context**. You do not need to read any other document in this repository to implement the extension. Every architectural decision, API contract, TypeScript interface, file path, protocol format, and design constraint is documented here.

The extension connects to an existing inference server (`smolpc-engine-host`) that is already built and running. You do NOT need to build or modify the engine. You only need to build the VS Code extension that communicates with it over HTTP.

---

## Table of Contents

1. [Product Context](#1-product-context)
2. [System Architecture](#2-system-architecture)
3. [Extension Manifest (package.json)](#3-extension-manifest-packagejson)
4. [Project Structure](#4-project-structure)
5. [Build System](#5-build-system)
6. [Engine Client](#6-engine-client)
7. [Bearer Token Authentication](#7-bearer-token-authentication)
8. [InlineCompletionProvider (Ghost Text Autocomplete)](#8-inlinecompletionprovider-ghost-text-autocomplete)
9. [Webview Chat Panel](#9-webview-chat-panel)
10. [Commands](#10-commands)
11. [LSP Diagnostics Integration](#11-lsp-diagnostics-integration)
12. [Education Philosophy and Prompt Engineering](#12-education-philosophy-and-prompt-engineering)
13. [Status Bar Integration](#13-status-bar-integration)
14. [Settings](#14-settings)
15. [Extension Lifecycle](#15-extension-lifecycle)
16. [Error Handling](#16-error-handling)
17. [Testing Strategy](#17-testing-strategy)
18. [Distribution and Installation](#18-distribution-and-installation)
19. [Constraints and Non-Goals](#19-constraints-and-non-goals)
20. [Critical Invariants](#20-critical-invariants)
21. [Reference Implementations](#21-reference-implementations)
22. [Implementation Checklist](#22-implementation-checklist)

---

## 1. Product Context

### What This Extension Is

SmolPC Code Helper is an **offline AI coding assistant** delivered as a VS Code extension for secondary school students (ages 11--18) on budget Windows laptops. It provides:

- **Ghost text autocomplete** (inline suggestions as you type, like GitHub Copilot)
- **Chat panel** (ask questions about code, get explanations, receive hints)
- **Error explanation** (reads LSP diagnostics, explains errors in plain language)
- **Hint mode** (gives educational hints instead of solutions)

### What This Extension Is NOT

- It is NOT an agentic coding assistant (no multi-file edits, no autonomous tool use)
- It is NOT a cloud service (all inference runs locally on the student's machine)
- It does NOT require GitHub Copilot to be installed
- It does NOT use the VS Code Chat Participant API (that requires Copilot)
- It does NOT run its own AI model (it connects to a shared engine process)

### Target Audience

- **Students:** Ages 11--18, secondary school, learning to code. Varied experience levels from absolute beginners to intermediate. Languages: primarily Python, JavaScript, HTML/CSS, with some Java, C#, and Scratch.
- **Teachers:** May configure settings, monitor usage patterns (future feature).
- **IT Admins:** Install the extension via `.vsix` file or Marketplace.

### Key Constraints

- **Model size:** 1.5--4B parameters. Good for short completions, explanations, and chat. NOT viable for complex multi-step reasoning or large code generation.
- **Speed:** 5--15 tokens/second depending on hardware. Users will see a brief delay for completions.
- **Offline-only:** No internet required. No telemetry. No accounts. All data stays local.
- **Privacy:** GDPR and FERPA compliant by design. No student data ever leaves the device.

### Estimated Scope

**3--6K lines of TypeScript** for the MVP. This is a focused extension, not a full IDE agent.

---

## 2. System Architecture

### How the Extension Fits in the Larger System

The SmolPC project has two user-facing products that share a single inference engine:

```
Student's PC (Windows, 8GB+ RAM)
|
+-- smolpc-engine-host.exe                   <-- Shared inference server (Axum, port 19432)
|   |                                             Already built. You do NOT modify this.
|   |-- Loads AI model once into memory
|   |-- Exposes OpenAI-compatible HTTP API
|   |-- Handles CPU / GPU / NPU inference
|   |-- SSE streaming for token-by-token responses
|
+-- Unified Tauri App (smolpc-codehelper.exe) <-- Desktop app for GIMP/Blender/LibreOffice modes
|   |                                             Connects to engine via HTTP. Separate product.
|   +-- HTTP + SSE --> smolpc-engine-host:19432
|
+-- VS Code Extension (THIS IS WHAT YOU BUILD)
    |
    +-- HTTP + SSE --> smolpc-engine-host:19432    <-- Same engine, same port, same API
    |
    +-- InlineCompletionProvider                   <-- Ghost text autocomplete
    +-- Webview Chat Panel                         <-- Chat UI in sidebar
    +-- Commands (explain, fix error, hint)        <-- VS Code command palette
    +-- LSP Diagnostics Reader                     <-- Read errors from language servers
```

### Key Architectural Facts

1. **The engine is already running.** The Tauri desktop app (or the system launcher) starts `smolpc-engine-host.exe` before the VS Code extension activates. The extension does NOT need to start the engine. If the engine is not running, the extension should show a helpful error message telling the student to start the SmolPC app.

2. **One model serves all clients.** The engine loads one model into RAM. Both the Tauri app and the VS Code extension share that single model. There is no model duplication.

3. **The API is OpenAI-compatible.** The engine exposes `POST /v1/chat/completions` with the same request/response format as the OpenAI API. Any OpenAI-compatible client library could be used, but for our scope a minimal custom HTTP client is simpler.

4. **Authentication is required.** Every HTTP request must include `Authorization: Bearer <token>`. The token is read from a file on disk (see [Section 7](#7-bearer-token-authentication)).

5. **Streaming uses SSE.** When `stream: true` is set in the request, the engine returns Server-Sent Events. Each event contains a single token. The extension must parse these events and display tokens incrementally.

---

## 3. Extension Manifest (package.json)

This is the complete `package.json` for the extension. Every field is intentional.

```json
{
  "name": "smolpc-codehelper",
  "displayName": "SmolPC Code Helper",
  "description": "Offline AI coding assistant for students. Provides autocomplete, code explanations, error hints, and guided learning -- all running locally on your machine.",
  "version": "0.1.0",
  "publisher": "smolpc",
  "license": "MIT",
  "engines": {
    "vscode": "^1.85.0"
  },
  "categories": [
    "Machine Learning",
    "Education",
    "Programming Languages"
  ],
  "keywords": [
    "ai",
    "autocomplete",
    "offline",
    "local",
    "education",
    "student",
    "coding assistant"
  ],
  "icon": "media/icon.png",
  "repository": {
    "type": "git",
    "url": "https://github.com/SmolPC-2-0/smolpc-codehelper"
  },
  "activationEvents": [
    "onStartupFinished"
  ],
  "main": "./dist/extension.js",
  "contributes": {
    "commands": [
      {
        "command": "smolpc.openChat",
        "title": "SmolPC: Open Chat Panel",
        "icon": "$(comment-discussion)"
      },
      {
        "command": "smolpc.explainCode",
        "title": "SmolPC: Explain Selected Code"
      },
      {
        "command": "smolpc.fixError",
        "title": "SmolPC: Explain Error"
      },
      {
        "command": "smolpc.askQuestion",
        "title": "SmolPC: Ask a Question"
      },
      {
        "command": "smolpc.hintMode",
        "title": "SmolPC: Get a Hint (Don't Solve)"
      },
      {
        "command": "smolpc.toggleAutocomplete",
        "title": "SmolPC: Toggle Autocomplete"
      }
    ],
    "menus": {
      "editor/context": [
        {
          "command": "smolpc.explainCode",
          "when": "editorHasSelection",
          "group": "smolpc@1"
        },
        {
          "command": "smolpc.fixError",
          "when": "editorHasSelection",
          "group": "smolpc@2"
        },
        {
          "command": "smolpc.hintMode",
          "when": "editorHasSelection",
          "group": "smolpc@3"
        }
      ]
    },
    "keybindings": [
      {
        "command": "smolpc.openChat",
        "key": "ctrl+shift+h",
        "mac": "cmd+shift+h"
      },
      {
        "command": "smolpc.explainCode",
        "key": "ctrl+shift+e",
        "mac": "cmd+shift+e",
        "when": "editorHasSelection"
      }
    ],
    "viewsContainers": {
      "activitybar": [
        {
          "id": "smolpc-sidebar",
          "title": "SmolPC Code Helper",
          "icon": "media/sidebar-icon.svg"
        }
      ]
    },
    "views": {
      "smolpc-sidebar": [
        {
          "type": "webview",
          "id": "smolpc.chatView",
          "name": "Chat",
          "visibility": "visible"
        }
      ]
    },
    "configuration": {
      "title": "SmolPC Code Helper",
      "properties": {
        "smolpc.engineUrl": {
          "type": "string",
          "default": "http://localhost:19432",
          "description": "URL of the SmolPC inference engine. Do not change unless instructed by your teacher or IT admin.",
          "order": 1
        },
        "smolpc.autocomplete.enabled": {
          "type": "boolean",
          "default": true,
          "description": "Enable inline code suggestions (ghost text) as you type.",
          "order": 2
        },
        "smolpc.autocomplete.debounceMs": {
          "type": "number",
          "default": 300,
          "minimum": 100,
          "maximum": 2000,
          "description": "How long to wait after you stop typing before requesting a suggestion (milliseconds).",
          "order": 3
        },
        "smolpc.autocomplete.maxTokens": {
          "type": "number",
          "default": 128,
          "minimum": 16,
          "maximum": 512,
          "description": "Maximum length of inline suggestions.",
          "order": 4
        },
        "smolpc.autocomplete.temperature": {
          "type": "number",
          "default": 0.2,
          "minimum": 0,
          "maximum": 2,
          "description": "Controls randomness of suggestions. Lower values give more predictable suggestions.",
          "order": 5
        },
        "smolpc.chat.hintMode": {
          "type": "boolean",
          "default": true,
          "description": "When enabled, the assistant gives hints and explanations instead of complete solutions.",
          "order": 6
        },
        "smolpc.chat.maxTokens": {
          "type": "number",
          "default": 512,
          "minimum": 64,
          "maximum": 2048,
          "description": "Maximum length of chat responses.",
          "order": 7
        },
        "smolpc.chat.temperature": {
          "type": "number",
          "default": 0.7,
          "minimum": 0,
          "maximum": 2,
          "description": "Controls randomness of chat responses. Higher values give more creative responses.",
          "order": 8
        }
      }
    }
  },
  "scripts": {
    "vscode:prepublish": "npm run build",
    "build": "esbuild ./src/extension.ts --bundle --outfile=dist/extension.js --external:vscode --format=cjs --platform=node --minify",
    "watch": "esbuild ./src/extension.ts --bundle --outfile=dist/extension.js --external:vscode --format=cjs --platform=node --sourcemap --watch",
    "lint": "eslint src/",
    "test": "vitest run",
    "package": "vsce package"
  },
  "devDependencies": {
    "@types/vscode": "^1.85.0",
    "@vscode/vsce": "^3.0.0",
    "esbuild": "^0.24.0",
    "eslint": "^9.0.0",
    "typescript": "^5.5.0",
    "vitest": "^2.0.0"
  }
}
```

### Activation Events Explained

`"onStartupFinished"` means the extension activates after VS Code has fully loaded. This is preferred over `"*"` (immediate activation) because:
- It does not slow down VS Code startup
- By the time the extension activates, the editor is ready for API calls
- The engine is more likely to be running by this point

### Why No Runtime Dependencies

The extension has ZERO runtime `dependencies` in package.json. Everything is bundled by esbuild into a single `dist/extension.js` file. The only external dependency is the `vscode` module, which is provided by VS Code itself and marked as `external` in the esbuild config. This keeps the extension small, fast to install, and free of supply-chain risk.

---

## 4. Project Structure

```
vscode-extension/
|-- package.json                 # Extension manifest (see Section 3)
|-- tsconfig.json                # TypeScript configuration
|-- .eslintrc.json               # ESLint configuration
|-- .vscodeignore                # Files to exclude from .vsix package
|
|-- src/
|   |-- extension.ts             # Entry point: activate() and deactivate()
|   |
|   |-- engine/
|   |   |-- client.ts            # HTTP client for smolpc-engine-host
|   |   |-- types.ts             # TypeScript interfaces for engine API
|   |   |-- token.ts             # Bearer token reader (filesystem)
|   |   +-- sse.ts               # SSE stream parser
|   |
|   |-- completion/
|   |   +-- provider.ts          # InlineCompletionItemProvider implementation
|   |
|   |-- chat/
|   |   |-- panel.ts             # WebviewViewProvider (sidebar chat panel)
|   |   +-- webview/
|   |       |-- index.html       # Chat UI HTML template
|   |       |-- chat.css         # Chat UI styles
|   |       +-- chat.js          # Chat UI logic (runs inside webview)
|   |
|   |-- commands/
|   |   |-- explain.ts           # smolpc.explainCode command
|   |   |-- fixError.ts          # smolpc.fixError command
|   |   |-- hint.ts              # smolpc.hintMode command
|   |   +-- ask.ts               # smolpc.askQuestion command
|   |
|   +-- utils/
|       |-- diagnostics.ts       # LSP diagnostic reader
|       |-- context.ts           # Code context extraction (cursor surroundings)
|       +-- prompts.ts           # System prompts and prompt templates
|
|-- media/
|   |-- icon.png                 # Extension icon (128x128)
|   +-- sidebar-icon.svg         # Activity bar icon (24x24, monochrome)
|
+-- test/
    |-- engine/
    |   +-- client.test.ts       # Engine client unit tests
    |-- completion/
    |   +-- provider.test.ts     # Completion provider tests
    +-- utils/
        +-- context.test.ts      # Context extraction tests
```

### File Responsibilities

| File | Purpose | Approximate LOC |
|------|---------|-----------------|
| `extension.ts` | Register providers, commands, and views. Connect to engine. | 80--120 |
| `engine/client.ts` | HTTP client with SSE streaming. All engine communication. | 200--300 |
| `engine/types.ts` | TypeScript interfaces matching engine API. | 80--100 |
| `engine/token.ts` | Read bearer token from `%LOCALAPPDATA%/SmolPC/engine-token.txt`. | 30--50 |
| `engine/sse.ts` | Parse SSE text/event-stream responses. | 60--80 |
| `completion/provider.ts` | InlineCompletionItemProvider with debouncing and cancellation. | 150--250 |
| `chat/panel.ts` | WebviewViewProvider for sidebar chat. Message passing to/from webview. | 200--300 |
| `chat/webview/*` | Chat UI (HTML/CSS/JS). Rendered inside VS Code webview. | 400--600 |
| `commands/*.ts` | Command handlers (explain, fix, hint, ask). | 50--80 each |
| `utils/diagnostics.ts` | Read and filter LSP diagnostics for current file. | 40--60 |
| `utils/context.ts` | Extract code around cursor for completion context. | 60--100 |
| `utils/prompts.ts` | System prompts for different modes (chat, hint, explain). | 80--120 |

---

## 5. Build System

### TypeScript Configuration (tsconfig.json)

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "commonjs",
    "lib": ["ES2022"],
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist", "test"]
}
```

### Build with esbuild

The extension is bundled into a single file using esbuild. This is the standard approach for VS Code extensions and provides:
- Fast builds (<1 second)
- Single output file (no node_modules shipped)
- Tree shaking (unused code removed)

```bash
# Development (watch mode with source maps)
npx esbuild ./src/extension.ts --bundle --outfile=dist/extension.js \
  --external:vscode --format=cjs --platform=node --sourcemap --watch

# Production (minified, no source maps)
npx esbuild ./src/extension.ts --bundle --outfile=dist/extension.js \
  --external:vscode --format=cjs --platform=node --minify
```

### .vscodeignore

Controls which files are excluded from the `.vsix` package:

```
.vscode/**
src/**
test/**
node_modules/**
.eslintrc.json
tsconfig.json
**/*.ts
**/*.map
.gitignore
```

Only `dist/extension.js`, `package.json`, `media/`, and `LICENSE` are included in the final package.

---

## 6. Engine Client

The engine client is the core communication layer. It handles all HTTP requests to `smolpc-engine-host` and parses SSE streaming responses.

### 6.1 TypeScript Interfaces (engine/types.ts)

```typescript
// ---- Request Types ----

export interface ChatMessage {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

export interface ChatCompletionRequest {
  model: string;
  messages: ChatMessage[];
  stream: boolean;
  max_tokens?: number;
  temperature?: number;
  top_k?: number;
  top_p?: number;
  repetition_penalty?: number;
  repetition_penalty_last_n?: number;
}

// ---- Response Types (non-streaming) ----

export interface ChatCompletionResponse {
  id: string;
  object: 'chat.completion';
  created: number;
  model: string;
  choices: Array<{
    index: number;
    message: ChatMessage;
    finish_reason: 'stop' | 'length';
  }>;
  usage: UsageStats;
}

// ---- Response Types (streaming) ----

export interface ChatCompletionChunk {
  id: string;
  object: 'chat.completion.chunk';
  created: number;
  model: string;
  choices: Array<{
    index: number;
    delta: {
      role?: string;
      content?: string;
    };
    finish_reason: 'stop' | 'length' | null;
  }>;
  usage?: UsageStats;
}

export interface UsageStats {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

// ---- Engine Status ----

export interface EngineStatus {
  ok: boolean;
  ready: boolean;
  state: 'idle' | 'starting' | 'resolving_assets' | 'probing' | 'loading_model' | 'ready' | 'failed';
  active_backend: string | null;
  active_model_id: string | null;
  generating: boolean;
  error_message: string | null;
}

// ---- Health Check ----

export interface HealthResponse {
  ok: boolean;
}
```

### 6.2 Engine Client Implementation (engine/client.ts)

```typescript
import * as vscode from 'vscode';
import { readBearerToken } from './token';
import { parseSSEStream } from './sse';
import {
  ChatCompletionRequest,
  ChatCompletionChunk,
  ChatMessage,
  EngineStatus,
  HealthResponse,
  UsageStats,
} from './types';

export class EngineClient {
  private baseUrl: string;
  private token: string | null = null;
  private abortController: AbortController | null = null;

  constructor(baseUrl: string) {
    this.baseUrl = baseUrl;
  }

  /**
   * Initialize the client by reading the bearer token from disk.
   * Must be called before any API requests.
   * Returns false if the token file cannot be read (engine not running).
   */
  async initialize(): Promise<boolean> {
    try {
      this.token = await readBearerToken();
      return true;
    } catch {
      return false;
    }
  }

  /**
   * Check if the engine is running and healthy.
   */
  async health(): Promise<boolean> {
    try {
      const response = await this.request<HealthResponse>('GET', '/engine/health');
      return response.ok === true;
    } catch {
      return false;
    }
  }

  /**
   * Get full engine status including backend info and model state.
   */
  async status(): Promise<EngineStatus> {
    return this.request<EngineStatus>('GET', '/engine/status');
  }

  /**
   * Non-streaming chat completion. Returns the full response at once.
   * Use for short requests where streaming is not needed.
   */
  async chatCompletion(
    messages: ChatMessage[],
    options?: {
      maxTokens?: number;
      temperature?: number;
    }
  ): Promise<string> {
    const body: ChatCompletionRequest = {
      model: 'current',
      messages,
      stream: false,
      max_tokens: options?.maxTokens,
      temperature: options?.temperature,
    };

    const response = await this.request<{ choices: Array<{ message: { content: string } }> }>(
      'POST',
      '/v1/chat/completions',
      body
    );

    return response.choices[0]?.message?.content ?? '';
  }

  /**
   * Streaming chat completion. Calls onToken for each generated token.
   * Returns usage stats when generation completes.
   *
   * @param messages - The conversation messages
   * @param onToken - Callback invoked for each token as it is generated
   * @param options - Generation parameters
   * @returns Usage statistics after generation completes
   */
  async chatCompletionStream(
    messages: ChatMessage[],
    onToken: (token: string) => void,
    options?: {
      maxTokens?: number;
      temperature?: number;
      topK?: number;
      topP?: number;
    }
  ): Promise<UsageStats | null> {
    // Cancel any in-flight generation before starting a new one
    this.cancelInFlight();

    this.abortController = new AbortController();

    const body: ChatCompletionRequest = {
      model: 'current',
      messages,
      stream: true,
      max_tokens: options?.maxTokens,
      temperature: options?.temperature,
      top_k: options?.topK,
      top_p: options?.topP,
    };

    const response = await fetch(`${this.baseUrl}/v1/chat/completions`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${this.token}`,
      },
      body: JSON.stringify(body),
      signal: this.abortController.signal,
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`Engine returned ${response.status}: ${errorText}`);
    }

    if (!response.body) {
      throw new Error('No response body for streaming request');
    }

    let usage: UsageStats | null = null;

    await parseSSEStream(response.body, (chunk: ChatCompletionChunk) => {
      const delta = chunk.choices[0]?.delta;
      if (delta?.content) {
        onToken(delta.content);
      }
      if (chunk.usage) {
        usage = chunk.usage;
      }
    });

    this.abortController = null;
    return usage;
  }

  /**
   * Cancel any in-flight streaming generation.
   */
  cancelInFlight(): void {
    if (this.abortController) {
      this.abortController.abort();
      this.abortController = null;
    }
  }

  /**
   * Tell the engine to cancel its current generation.
   * This is a server-side cancellation -- different from aborting the HTTP connection.
   */
  async cancelGeneration(): Promise<void> {
    try {
      await this.request('POST', '/engine/cancel');
    } catch {
      // Ignore errors -- generation may have already finished
    }
  }

  // ---- Private helpers ----

  private async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    if (!this.token) {
      throw new Error('Engine client not initialized. Call initialize() first.');
    }

    const response = await fetch(`${this.baseUrl}${path}`, {
      method,
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${this.token}`,
      },
      body: body ? JSON.stringify(body) : undefined,
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`Engine returned ${response.status}: ${errorText}`);
    }

    return response.json() as Promise<T>;
  }
}
```

### 6.3 SSE Stream Parser (engine/sse.ts)

The engine sends SSE events in the OpenAI format. Each event is a `data:` line containing JSON, terminated by `data: [DONE]`.

```typescript
import { ChatCompletionChunk } from './types';

/**
 * Parse an SSE (Server-Sent Events) stream from the engine.
 *
 * Expected format:
 *   data: {"id":"...","choices":[{"delta":{"content":"token"}}]}
 *   data: {"id":"...","choices":[{"delta":{},"finish_reason":"stop"}],"usage":{...}}
 *   data: [DONE]
 *
 * @param body - ReadableStream from a fetch response
 * @param onChunk - Callback for each parsed chunk
 */
export async function parseSSEStream(
  body: ReadableStream<Uint8Array>,
  onChunk: (chunk: ChatCompletionChunk) => void
): Promise<void> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });

      // SSE events are separated by double newlines
      const lines = buffer.split('\n');
      // Keep the last incomplete line in the buffer
      buffer = lines.pop() ?? '';

      for (const line of lines) {
        const trimmed = line.trim();

        if (trimmed === '') continue;          // Empty line (event separator)
        if (trimmed.startsWith(':')) continue;  // SSE comment

        if (!trimmed.startsWith('data: ')) continue;

        const data = trimmed.slice(6); // Remove 'data: ' prefix

        if (data === '[DONE]') {
          return; // Stream complete
        }

        try {
          const chunk = JSON.parse(data) as ChatCompletionChunk;
          onChunk(chunk);
        } catch {
          // Skip malformed JSON lines -- log in development
          console.warn('[SmolPC] Malformed SSE data:', data);
        }
      }
    }
  } finally {
    reader.releaseLock();
  }
}
```

### 6.4 Bearer Token Reader (engine/token.ts)

```typescript
import * as fs from 'fs/promises';
import * as path from 'path';
import * as os from 'os';

/**
 * Read the bearer token from the engine's token file.
 *
 * The engine writes its bearer token to:
 *   %LOCALAPPDATA%/SmolPC/engine-token.txt
 *
 * This file is created by smolpc-engine-host on first startup.
 * If the file does not exist, the engine has never been started.
 *
 * @returns The bearer token string
 * @throws If the token file cannot be read
 */
export async function readBearerToken(): Promise<string> {
  const tokenPath = getTokenPath();
  const content = await fs.readFile(tokenPath, 'utf-8');
  const token = content.trim();

  if (token.length === 0) {
    throw new Error('Token file is empty');
  }

  return token;
}

/**
 * Get the path to the engine token file.
 * On Windows: %LOCALAPPDATA%/SmolPC/engine-token.txt
 * On other platforms: ~/.local/share/SmolPC/engine-token.txt (unlikely but for dev)
 */
function getTokenPath(): string {
  const localAppData = process.env.LOCALAPPDATA;
  if (localAppData) {
    return path.join(localAppData, 'SmolPC', 'engine-token.txt');
  }

  // Fallback for non-Windows (development on macOS/Linux)
  return path.join(os.homedir(), '.local', 'share', 'SmolPC', 'engine-token.txt');
}
```

---

## 7. Bearer Token Authentication

### How Authentication Works

1. When `smolpc-engine-host.exe` starts for the first time, it generates a random alphanumeric bearer token.
2. The token is written to `%LOCALAPPDATA%/SmolPC/engine-token.txt`.
3. Every HTTP request to the engine must include the header: `Authorization: Bearer <token>`
4. Requests without this header are rejected with HTTP 401.

### Extension Flow

1. On activation, the extension reads the token file from disk.
2. If the file does not exist, the engine has never been started. Show an error message: "SmolPC engine is not running. Please start the SmolPC app first."
3. If the file exists but the engine is not responding (health check fails), the engine may have been stopped. Show: "SmolPC engine is not responding. Please restart the SmolPC app."
4. The token is cached in memory for the lifetime of the extension. It does not change while the engine is running.

### Security Considerations

- The token file is readable only by the current user (created by their process).
- The token provides a minimal authentication layer to prevent other processes on the same machine from using the engine unexpectedly.
- This is NOT meant to be strong security -- it is a localhost-only service.

---

## 8. InlineCompletionProvider (Ghost Text Autocomplete)

This is the most technically complex feature. It provides inline code suggestions that appear as ghost text in the editor, identical to how GitHub Copilot works.

### 8.1 How It Works

1. User types code in the editor
2. After a debounce period (default 300ms), the provider fires
3. The provider extracts context: lines before cursor (prefix) and lines after cursor (suffix)
4. A chat completion request is sent to the engine with a system prompt designed for code completion
5. The engine returns a completion
6. The completion is displayed as ghost text at the cursor position
7. User presses Tab to accept, or keeps typing to dismiss

### 8.2 Implementation (completion/provider.ts)

```typescript
import * as vscode from 'vscode';
import { EngineClient } from '../engine/client';
import { ChatMessage } from '../engine/types';
import { extractCompletionContext, CompletionContext } from '../utils/context';

export class SmolPCCompletionProvider implements vscode.InlineCompletionItemProvider {
  private client: EngineClient;
  private debounceTimer: ReturnType<typeof setTimeout> | null = null;
  private lastRequestId = 0;

  constructor(client: EngineClient) {
    this.client = client;
  }

  async provideInlineCompletionItems(
    document: vscode.TextDocument,
    position: vscode.Position,
    context: vscode.InlineCompletionContext,
    token: vscode.CancellationToken
  ): Promise<vscode.InlineCompletionItem[] | null> {
    // Check if autocomplete is enabled in settings
    const config = vscode.workspace.getConfiguration('smolpc');
    if (!config.get<boolean>('autocomplete.enabled', true)) {
      return null;
    }

    // Debounce: wait for the user to stop typing
    const debounceMs = config.get<number>('autocomplete.debounceMs', 300);
    await this.debounce(debounceMs, token);

    // If cancelled during debounce (user kept typing), return null
    if (token.isCancellationRequested) {
      return null;
    }

    // Extract code context around the cursor
    const completionContext = extractCompletionContext(document, position);

    // Build the completion prompt
    const messages = this.buildCompletionMessages(completionContext, document.languageId);

    // Track request ID for staleness checking
    const requestId = ++this.lastRequestId;

    try {
      const maxTokens = config.get<number>('autocomplete.maxTokens', 128);
      const temperature = config.get<number>('autocomplete.temperature', 0.2);

      // Non-streaming request for completions (we need the full text before showing)
      const completion = await this.client.chatCompletion(messages, {
        maxTokens,
        temperature,
      });

      // If a newer request has started, discard this result
      if (requestId !== this.lastRequestId || token.isCancellationRequested) {
        return null;
      }

      // Clean up the completion text
      const cleanedCompletion = this.cleanCompletion(completion, completionContext);
      if (!cleanedCompletion) {
        return null;
      }

      return [
        new vscode.InlineCompletionItem(
          cleanedCompletion,
          new vscode.Range(position, position)
        ),
      ];
    } catch (error) {
      // Silently fail -- do not interrupt the user with error popups for completions
      console.warn('[SmolPC] Completion request failed:', error);
      return null;
    }
  }

  /**
   * Build chat messages for code completion.
   * Uses a FIM-style prompt: provide the code before and after the cursor,
   * ask the model to fill in the middle.
   */
  private buildCompletionMessages(ctx: CompletionContext, languageId: string): ChatMessage[] {
    return [
      {
        role: 'system',
        content: `You are a code completion engine. Given the code before and after the cursor position, output ONLY the code that should be inserted at the cursor. Do not include explanations, markdown formatting, or code fences. Output only the raw code to insert. The programming language is ${languageId}.`,
      },
      {
        role: 'user',
        content: `Complete the code at the cursor position marked with <CURSOR>.

${ctx.prefix}<CURSOR>${ctx.suffix}`,
      },
    ];
  }

  /**
   * Clean up the model's completion output.
   * The model may include unwanted prefixes, suffixes, or formatting.
   */
  private cleanCompletion(
    rawCompletion: string,
    ctx: CompletionContext
  ): string | null {
    let cleaned = rawCompletion.trim();

    // Remove markdown code fences if the model included them
    if (cleaned.startsWith('```')) {
      const lines = cleaned.split('\n');
      lines.shift(); // Remove opening fence
      if (lines[lines.length - 1]?.trim() === '```') {
        lines.pop(); // Remove closing fence
      }
      cleaned = lines.join('\n');
    }

    // Remove any prefix that duplicates existing code
    // (model sometimes repeats the line before cursor)
    const lastPrefixLine = ctx.prefix.split('\n').pop() ?? '';
    if (cleaned.startsWith(lastPrefixLine) && lastPrefixLine.length > 0) {
      cleaned = cleaned.slice(lastPrefixLine.length);
    }

    // If the completion is empty or only whitespace, skip it
    if (cleaned.trim().length === 0) {
      return null;
    }

    return cleaned;
  }

  /**
   * Debounce helper that respects VS Code's CancellationToken.
   */
  private debounce(ms: number, token: vscode.CancellationToken): Promise<void> {
    return new Promise<void>((resolve) => {
      if (this.debounceTimer) {
        clearTimeout(this.debounceTimer);
      }

      this.debounceTimer = setTimeout(() => {
        this.debounceTimer = null;
        resolve();
      }, ms);

      // If cancelled, resolve immediately so provideInlineCompletionItems can return null
      token.onCancellationRequested(() => {
        if (this.debounceTimer) {
          clearTimeout(this.debounceTimer);
          this.debounceTimer = null;
        }
        resolve();
      });
    });
  }
}
```

### 8.3 Context Extraction (utils/context.ts)

```typescript
import * as vscode from 'vscode';

export interface CompletionContext {
  /** Code before the cursor (up to N lines) */
  prefix: string;
  /** Code after the cursor (up to N lines) */
  suffix: string;
  /** The full current line text */
  currentLine: string;
  /** Language identifier (e.g., 'python', 'javascript') */
  languageId: string;
  /** File name (for context about file type) */
  fileName: string;
}

/**
 * Extract code context around the cursor position.
 *
 * For the prefix: takes up to 50 lines before the cursor.
 * For the suffix: takes up to 20 lines after the cursor.
 *
 * These limits are chosen to fit within the model's context window
 * while providing enough surrounding code for meaningful completions.
 * The models used are 1.5-4B params with 2K-4K token context windows.
 * 50 lines of code is roughly 200-400 tokens.
 */
export function extractCompletionContext(
  document: vscode.TextDocument,
  position: vscode.Position
): CompletionContext {
  const prefixLineCount = 50;
  const suffixLineCount = 20;

  // Extract prefix (lines before cursor, including partial current line)
  const prefixStartLine = Math.max(0, position.line - prefixLineCount);
  const prefixRange = new vscode.Range(
    new vscode.Position(prefixStartLine, 0),
    position
  );
  const prefix = document.getText(prefixRange);

  // Extract suffix (remaining current line + lines after cursor)
  const suffixEndLine = Math.min(
    document.lineCount - 1,
    position.line + suffixLineCount
  );
  const suffixRange = new vscode.Range(
    position,
    new vscode.Position(suffixEndLine, document.lineAt(suffixEndLine).text.length)
  );
  const suffix = document.getText(suffixRange);

  // Current line (full text)
  const currentLine = document.lineAt(position.line).text;

  return {
    prefix,
    suffix,
    currentLine,
    languageId: document.languageId,
    fileName: document.fileName,
  };
}
```

### 8.4 Registration

In `extension.ts`, the provider is registered for all languages:

```typescript
const completionProvider = new SmolPCCompletionProvider(engineClient);

context.subscriptions.push(
  vscode.languages.registerInlineCompletionItemProvider(
    { pattern: '**' },  // All file types
    completionProvider
  )
);
```

### 8.5 Design Decisions

- **Non-streaming for completions:** Unlike chat, completions use non-streaming requests. The model must generate the full completion before it can be shown as ghost text -- partial ghost text would be confusing.
- **Low temperature (0.2):** Completions should be predictable and deterministic. High creativity is undesirable for code completion.
- **Short max_tokens (128):** The model is small (1.5--4B). Long completions degrade in quality. Short, accurate completions are better.
- **Debounce (300ms):** Prevents flooding the engine with requests on every keystroke. 300ms is long enough that most typing bursts complete before a request is sent.
- **Silent failure:** Completion errors are logged but never shown to the user. A failed completion should be invisible, not disruptive.

---

## 9. Webview Chat Panel

The chat panel provides a conversational interface in the VS Code sidebar. Students can ask questions, request explanations, and receive guided hints.

### 9.1 Why a Webview (Not Chat Participant API)

The VS Code **Chat Participant API** (`vscode.chat.createChatParticipant`) requires GitHub Copilot to be installed. SmolPC must work without Copilot. Therefore, the chat UI is built as a **Webview** -- a custom HTML/CSS/JS panel rendered inside VS Code.

### 9.2 WebviewViewProvider (chat/panel.ts)

```typescript
import * as vscode from 'vscode';
import { EngineClient } from '../engine/client';
import { ChatMessage } from '../engine/types';
import { getSystemPrompt, SystemPromptMode } from '../utils/prompts';

interface WebviewMessage {
  type: 'sendMessage' | 'cancelGeneration' | 'clearHistory' | 'ready';
  text?: string;
}

interface WebviewResponse {
  type: 'addUserMessage' | 'startAssistantMessage' | 'appendToken'
       | 'finishAssistantMessage' | 'setStatus' | 'showError' | 'clearMessages';
  text?: string;
  status?: string;
}

export class ChatPanelProvider implements vscode.WebviewViewProvider {
  public static readonly viewType = 'smolpc.chatView';

  private view?: vscode.WebviewView;
  private client: EngineClient;
  private conversationHistory: ChatMessage[] = [];
  private isGenerating = false;

  constructor(
    private readonly extensionUri: vscode.Uri,
    client: EngineClient
  ) {
    this.client = client;
  }

  resolveWebviewView(
    webviewView: vscode.WebviewView,
    _context: vscode.WebviewViewResolveContext,
    _token: vscode.CancellationToken
  ): void {
    this.view = webviewView;

    webviewView.webview.options = {
      enableScripts: true,
      localResourceRoots: [
        vscode.Uri.joinPath(this.extensionUri, 'src', 'chat', 'webview'),
        vscode.Uri.joinPath(this.extensionUri, 'media'),
      ],
    };

    webviewView.webview.html = this.getHtmlContent(webviewView.webview);

    // Handle messages from the webview
    webviewView.webview.onDidReceiveMessage(
      async (message: WebviewMessage) => {
        switch (message.type) {
          case 'sendMessage':
            if (message.text) {
              await this.handleUserMessage(message.text);
            }
            break;
          case 'cancelGeneration':
            await this.handleCancel();
            break;
          case 'clearHistory':
            this.handleClearHistory();
            break;
          case 'ready':
            this.postMessage({ type: 'setStatus', status: 'ready' });
            break;
        }
      }
    );
  }

  /**
   * Send a message to the chat (can be called from commands).
   * Opens the chat panel if it is not visible.
   */
  async sendMessage(text: string): Promise<void> {
    // Ensure the sidebar is visible
    if (this.view) {
      this.view.show(true);
    }

    // Wait briefly for the webview to be ready
    await new Promise(resolve => setTimeout(resolve, 100));

    await this.handleUserMessage(text);
  }

  private async handleUserMessage(text: string): Promise<void> {
    if (this.isGenerating) {
      this.postMessage({
        type: 'showError',
        text: 'Please wait for the current response to finish, or click Stop.',
      });
      return;
    }

    this.isGenerating = true;

    // Add user message to conversation history
    this.conversationHistory.push({ role: 'user', content: text });

    // Show user message in the webview
    this.postMessage({ type: 'addUserMessage', text });

    // Build messages array with system prompt
    const config = vscode.workspace.getConfiguration('smolpc');
    const hintMode = config.get<boolean>('chat.hintMode', true);
    const mode: SystemPromptMode = hintMode ? 'hint' : 'chat';

    const messages: ChatMessage[] = [
      { role: 'system', content: getSystemPrompt(mode) },
      ...this.conversationHistory,
    ];

    // Start assistant message in webview
    this.postMessage({ type: 'startAssistantMessage' });

    try {
      const maxTokens = config.get<number>('chat.maxTokens', 512);
      const temperature = config.get<number>('chat.temperature', 0.7);

      let fullResponse = '';

      await this.client.chatCompletionStream(
        messages,
        (token: string) => {
          fullResponse += token;
          this.postMessage({ type: 'appendToken', text: token });
        },
        { maxTokens, temperature }
      );

      // Add assistant response to conversation history
      this.conversationHistory.push({ role: 'assistant', content: fullResponse });

      this.postMessage({ type: 'finishAssistantMessage' });
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';

      // Check if it was a user cancellation
      if (errorMessage.includes('abort')) {
        this.postMessage({
          type: 'finishAssistantMessage',
          text: '(Generation stopped)',
        });
      } else {
        this.postMessage({
          type: 'showError',
          text: `Failed to get response: ${errorMessage}`,
        });
      }
    } finally {
      this.isGenerating = false;
    }
  }

  private async handleCancel(): Promise<void> {
    this.client.cancelInFlight();
    await this.client.cancelGeneration();
  }

  private handleClearHistory(): void {
    this.conversationHistory = [];
    this.postMessage({ type: 'clearMessages' });
  }

  private postMessage(message: WebviewResponse): void {
    this.view?.webview.postMessage(message);
  }

  /**
   * Generate the HTML content for the chat webview.
   * This is a self-contained HTML page with embedded CSS and JS.
   */
  private getHtmlContent(webview: vscode.Webview): string {
    // Use a nonce to restrict inline scripts (VS Code security requirement)
    const nonce = getNonce();

    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy"
    content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}';">
  <title>SmolPC Chat</title>
  <style>
    /* --- Base styles using VS Code theme variables --- */
    * {
      box-sizing: border-box;
      margin: 0;
      padding: 0;
    }
    body {
      font-family: var(--vscode-font-family);
      font-size: var(--vscode-font-size);
      color: var(--vscode-foreground);
      background: var(--vscode-sideBar-background);
      display: flex;
      flex-direction: column;
      height: 100vh;
      overflow: hidden;
    }

    /* --- Header --- */
    .header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 8px 12px;
      border-bottom: 1px solid var(--vscode-panel-border);
    }
    .header h3 {
      font-size: 13px;
      font-weight: 600;
    }
    .header-actions button {
      background: none;
      border: none;
      color: var(--vscode-foreground);
      cursor: pointer;
      padding: 4px;
      font-size: 12px;
      opacity: 0.7;
    }
    .header-actions button:hover {
      opacity: 1;
    }

    /* --- Messages area --- */
    .messages {
      flex: 1;
      overflow-y: auto;
      padding: 12px;
      display: flex;
      flex-direction: column;
      gap: 12px;
    }
    .message {
      padding: 8px 12px;
      border-radius: 6px;
      max-width: 95%;
      line-height: 1.5;
      white-space: pre-wrap;
      word-wrap: break-word;
    }
    .message-user {
      background: var(--vscode-button-background);
      color: var(--vscode-button-foreground);
      align-self: flex-end;
      border-radius: 12px 12px 2px 12px;
    }
    .message-assistant {
      background: var(--vscode-editor-background);
      border: 1px solid var(--vscode-panel-border);
      align-self: flex-start;
      border-radius: 12px 12px 12px 2px;
    }
    .message-error {
      background: var(--vscode-inputValidation-errorBackground);
      border: 1px solid var(--vscode-inputValidation-errorBorder);
      color: var(--vscode-errorForeground);
      align-self: center;
      font-size: 12px;
    }
    .message code {
      background: var(--vscode-textCodeBlock-background);
      padding: 1px 4px;
      border-radius: 3px;
      font-family: var(--vscode-editor-font-family);
      font-size: var(--vscode-editor-font-size);
    }
    .message pre {
      background: var(--vscode-textCodeBlock-background);
      padding: 8px;
      border-radius: 4px;
      overflow-x: auto;
      margin: 4px 0;
    }
    .message pre code {
      padding: 0;
      background: none;
    }

    /* --- Typing indicator --- */
    .typing-indicator {
      display: none;
      align-self: flex-start;
      padding: 8px 12px;
      color: var(--vscode-descriptionForeground);
      font-style: italic;
      font-size: 12px;
    }
    .typing-indicator.active {
      display: block;
    }

    /* --- Input area --- */
    .input-area {
      display: flex;
      gap: 6px;
      padding: 8px 12px;
      border-top: 1px solid var(--vscode-panel-border);
      background: var(--vscode-sideBar-background);
    }
    .input-area textarea {
      flex: 1;
      resize: none;
      border: 1px solid var(--vscode-input-border);
      background: var(--vscode-input-background);
      color: var(--vscode-input-foreground);
      padding: 6px 8px;
      border-radius: 4px;
      font-family: var(--vscode-font-family);
      font-size: var(--vscode-font-size);
      line-height: 1.4;
      min-height: 36px;
      max-height: 120px;
    }
    .input-area textarea:focus {
      outline: 1px solid var(--vscode-focusBorder);
    }
    .input-area textarea::placeholder {
      color: var(--vscode-input-placeholderForeground);
    }
    .input-area button {
      background: var(--vscode-button-background);
      color: var(--vscode-button-foreground);
      border: none;
      padding: 6px 12px;
      border-radius: 4px;
      cursor: pointer;
      font-size: 13px;
      align-self: flex-end;
    }
    .input-area button:hover {
      background: var(--vscode-button-hoverBackground);
    }
    .input-area button:disabled {
      opacity: 0.5;
      cursor: default;
    }
    .input-area button.stop-btn {
      background: var(--vscode-statusBarItem-errorBackground);
    }

    /* --- Welcome message --- */
    .welcome {
      text-align: center;
      padding: 20px;
      color: var(--vscode-descriptionForeground);
    }
    .welcome h2 {
      font-size: 16px;
      margin-bottom: 8px;
      color: var(--vscode-foreground);
    }
    .welcome p {
      font-size: 13px;
      margin-bottom: 12px;
    }
    .welcome .suggestions {
      display: flex;
      flex-direction: column;
      gap: 6px;
    }
    .welcome .suggestion {
      background: var(--vscode-editor-background);
      border: 1px solid var(--vscode-panel-border);
      border-radius: 6px;
      padding: 8px 12px;
      cursor: pointer;
      text-align: left;
      font-size: 12px;
      color: var(--vscode-foreground);
    }
    .welcome .suggestion:hover {
      border-color: var(--vscode-focusBorder);
    }
  </style>
</head>
<body>
  <div class="header">
    <h3>SmolPC Code Helper</h3>
    <div class="header-actions">
      <button id="clearBtn" title="Clear conversation">Clear</button>
    </div>
  </div>

  <div class="messages" id="messagesContainer">
    <div class="welcome" id="welcomeMessage">
      <h2>Hello!</h2>
      <p>I'm your coding helper. Ask me anything about your code.</p>
      <div class="suggestions">
        <div class="suggestion" data-text="Explain what my code does">Explain what my code does</div>
        <div class="suggestion" data-text="I have an error I don't understand">I have an error I don't understand</div>
        <div class="suggestion" data-text="Give me a hint for my homework">Give me a hint for my homework</div>
        <div class="suggestion" data-text="How do I use a for loop?">How do I use a for loop?</div>
      </div>
    </div>
  </div>

  <div class="input-area">
    <textarea id="messageInput" placeholder="Ask me anything..." rows="1"></textarea>
    <button id="sendBtn">Send</button>
  </div>

  <script nonce="${nonce}">
    (function() {
      const vscode = acquireVsCodeApi();
      const messagesContainer = document.getElementById('messagesContainer');
      const messageInput = document.getElementById('messageInput');
      const sendBtn = document.getElementById('sendBtn');
      const clearBtn = document.getElementById('clearBtn');
      const welcomeMessage = document.getElementById('welcomeMessage');

      let isGenerating = false;
      let currentAssistantMessage = null;

      // ---- Send message ----
      function sendMessage() {
        const text = messageInput.value.trim();
        if (!text || isGenerating) return;

        messageInput.value = '';
        messageInput.style.height = 'auto';
        vscode.postMessage({ type: 'sendMessage', text: text });
      }

      sendBtn.addEventListener('click', () => {
        if (isGenerating) {
          vscode.postMessage({ type: 'cancelGeneration' });
        } else {
          sendMessage();
        }
      });

      messageInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && !e.shiftKey) {
          e.preventDefault();
          sendMessage();
        }
      });

      // Auto-resize textarea
      messageInput.addEventListener('input', () => {
        messageInput.style.height = 'auto';
        messageInput.style.height = Math.min(messageInput.scrollHeight, 120) + 'px';
      });

      // Clear button
      clearBtn.addEventListener('click', () => {
        vscode.postMessage({ type: 'clearHistory' });
      });

      // Suggestion chips
      document.querySelectorAll('.suggestion').forEach(chip => {
        chip.addEventListener('click', () => {
          const text = chip.getAttribute('data-text');
          if (text) {
            messageInput.value = text;
            sendMessage();
          }
        });
      });

      // ---- Handle messages from extension ----
      window.addEventListener('message', (event) => {
        const message = event.data;

        switch (message.type) {
          case 'addUserMessage':
            hideWelcome();
            addMessage(message.text, 'user');
            break;

          case 'startAssistantMessage':
            isGenerating = true;
            sendBtn.textContent = 'Stop';
            sendBtn.classList.add('stop-btn');
            messageInput.disabled = true;
            currentAssistantMessage = addMessage('', 'assistant');
            break;

          case 'appendToken':
            if (currentAssistantMessage && message.text) {
              currentAssistantMessage.textContent += message.text;
              scrollToBottom();
            }
            break;

          case 'finishAssistantMessage':
            isGenerating = false;
            sendBtn.textContent = 'Send';
            sendBtn.classList.remove('stop-btn');
            messageInput.disabled = false;
            messageInput.focus();
            if (message.text && currentAssistantMessage) {
              currentAssistantMessage.textContent += message.text;
            }
            // Render markdown-like formatting in the final message
            if (currentAssistantMessage) {
              currentAssistantMessage.innerHTML = renderSimpleMarkdown(
                currentAssistantMessage.textContent
              );
            }
            currentAssistantMessage = null;
            scrollToBottom();
            break;

          case 'showError':
            addMessage(message.text, 'error');
            isGenerating = false;
            sendBtn.textContent = 'Send';
            sendBtn.classList.remove('stop-btn');
            messageInput.disabled = false;
            break;

          case 'clearMessages':
            messagesContainer.innerHTML = '';
            messagesContainer.appendChild(welcomeMessage);
            welcomeMessage.style.display = 'block';
            break;

          case 'setStatus':
            // Could update a status indicator
            break;
        }
      });

      // ---- Helper functions ----
      function addMessage(text, type) {
        const div = document.createElement('div');
        div.className = 'message message-' + type;
        div.textContent = text;
        messagesContainer.appendChild(div);
        scrollToBottom();
        return div;
      }

      function hideWelcome() {
        if (welcomeMessage) {
          welcomeMessage.style.display = 'none';
        }
      }

      function scrollToBottom() {
        messagesContainer.scrollTop = messagesContainer.scrollHeight;
      }

      /**
       * Simple markdown rendering for assistant messages.
       * Handles: inline code, code blocks, bold, italic.
       * This is intentionally simple -- not a full markdown parser.
       */
      function renderSimpleMarkdown(text) {
        if (!text) return '';
        let html = escapeHtml(text);

        // Code blocks: ```lang\ncode\n```
        html = html.replace(/```(\w*)\n([\s\S]*?)```/g,
          '<pre><code>$2</code></pre>');

        // Inline code: \`code\`
        html = html.replace(/\`([^\`]+)\`/g,
          '<code>$1</code>');

        // Bold: **text**
        html = html.replace(/\*\*([^\*]+)\*\*/g, '<strong>$1</strong>');

        // Italic: *text*
        html = html.replace(/\*([^\*]+)\*/g, '<em>$1</em>');

        // Line breaks
        html = html.replace(/\n/g, '<br>');

        return html;
      }

      function escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
      }

      // Notify extension that webview is ready
      vscode.postMessage({ type: 'ready' });
    })();
  </script>
</body>
</html>`;
  }
}

/** Generate a random nonce for Content Security Policy. */
function getNonce(): string {
  let text = '';
  const possible = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  for (let i = 0; i < 32; i++) {
    text += possible.charAt(Math.floor(Math.random() * possible.length));
  }
  return text;
}
```

### 9.3 Webview Security

VS Code webviews run in a sandboxed iframe. The Content Security Policy (CSP) must be set correctly:

- `default-src 'none'` -- Block everything by default
- `style-src ${webview.cspSource} 'unsafe-inline'` -- Allow VS Code theme styles and inline CSS
- `script-src 'nonce-${nonce}'` -- Only allow scripts with the correct nonce

The webview communicates with the extension host via `vscode.postMessage()` (webview to extension) and `webview.postMessage()` (extension to webview). These are the ONLY communication channels. The webview has no direct access to the filesystem, network, or VS Code API.

### 9.4 Conversation History

The chat panel maintains a `conversationHistory: ChatMessage[]` array in memory. This is sent with every request to the engine so the model has context from earlier in the conversation.

**Important:** The model is small (1.5--4B). Long conversation histories will overflow its context window (2K--4K tokens). Implement a sliding window:

```typescript
private getRecentHistory(maxMessages: number = 10): ChatMessage[] {
  // Keep the most recent N messages to fit within model context
  if (this.conversationHistory.length <= maxMessages) {
    return [...this.conversationHistory];
  }
  return this.conversationHistory.slice(-maxMessages);
}
```

When building the messages array for a request, use `getRecentHistory()` instead of the full `conversationHistory`.

### 9.5 Conversation History -- NOT Persisted to Disk

The MVP does NOT persist conversation history. When the user closes VS Code, the history is lost. This is intentional:
- Simplifies implementation
- Reduces privacy surface area
- Students are unlikely to need cross-session history for code questions

Persistence can be added later using VS Code's `globalState` or `workspaceState` storage.

---

## 10. Commands

Commands appear in the Command Palette (Ctrl+Shift+P) and in the right-click context menu (when code is selected).

### 10.1 smolpc.explainCode (commands/explain.ts)

Explains the currently selected code in plain language.

```typescript
import * as vscode from 'vscode';
import { ChatPanelProvider } from '../chat/panel';

export function registerExplainCommand(
  context: vscode.ExtensionContext,
  chatPanel: ChatPanelProvider
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand('smolpc.explainCode', async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        vscode.window.showInformationMessage('Open a file and select some code first.');
        return;
      }

      const selection = editor.selection;
      if (selection.isEmpty) {
        vscode.window.showInformationMessage('Select some code to explain.');
        return;
      }

      const selectedCode = editor.document.getText(selection);
      const languageId = editor.document.languageId;

      const prompt = `Explain this ${languageId} code in simple terms. What does it do, step by step?\n\n\`\`\`${languageId}\n${selectedCode}\n\`\`\``;

      await chatPanel.sendMessage(prompt);
    })
  );
}
```

### 10.2 smolpc.fixError (commands/fixError.ts)

Reads LSP diagnostics for the selected code and asks the model to explain the error.

```typescript
import * as vscode from 'vscode';
import { ChatPanelProvider } from '../chat/panel';
import { getDiagnosticsForSelection } from '../utils/diagnostics';

export function registerFixErrorCommand(
  context: vscode.ExtensionContext,
  chatPanel: ChatPanelProvider
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand('smolpc.fixError', async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        vscode.window.showInformationMessage('Open a file first.');
        return;
      }

      // Get diagnostics (errors/warnings) for the current selection or cursor line
      const diagnostics = getDiagnosticsForSelection(
        editor.document,
        editor.selection
      );

      if (diagnostics.length === 0) {
        vscode.window.showInformationMessage(
          'No errors found on the selected lines. The red/yellow squiggly lines indicate errors.'
        );
        return;
      }

      // Get the code around the error
      const errorContext = getErrorContext(editor, diagnostics[0]);

      // Build the prompt
      const languageId = editor.document.languageId;
      const errorMessages = diagnostics
        .map(d => `- Line ${d.range.start.line + 1}: ${d.message} [${severityToString(d.severity)}]`)
        .join('\n');

      const prompt = `I have ${diagnostics.length === 1 ? 'an error' : 'errors'} in my ${languageId} code. Please explain what is wrong and give me a hint on how to fix it. Do NOT give me the answer directly -- help me understand the problem so I can fix it myself.

**Errors:**
${errorMessages}

**Code around the error:**
\`\`\`${languageId}
${errorContext}
\`\`\``;

      await chatPanel.sendMessage(prompt);
    })
  );
}

function getErrorContext(
  editor: vscode.TextEditor,
  diagnostic: vscode.Diagnostic
): string {
  const startLine = Math.max(0, diagnostic.range.start.line - 3);
  const endLine = Math.min(
    editor.document.lineCount - 1,
    diagnostic.range.end.line + 3
  );
  const range = new vscode.Range(
    new vscode.Position(startLine, 0),
    new vscode.Position(endLine, editor.document.lineAt(endLine).text.length)
  );
  return editor.document.getText(range);
}

function severityToString(severity: vscode.DiagnosticSeverity): string {
  switch (severity) {
    case vscode.DiagnosticSeverity.Error: return 'Error';
    case vscode.DiagnosticSeverity.Warning: return 'Warning';
    case vscode.DiagnosticSeverity.Information: return 'Info';
    case vscode.DiagnosticSeverity.Hint: return 'Hint';
    default: return 'Unknown';
  }
}
```

### 10.3 smolpc.hintMode (commands/hint.ts)

Gives a hint for the selected code without solving the problem.

```typescript
import * as vscode from 'vscode';
import { ChatPanelProvider } from '../chat/panel';

export function registerHintCommand(
  context: vscode.ExtensionContext,
  chatPanel: ChatPanelProvider
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand('smolpc.hintMode', async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        vscode.window.showInformationMessage('Open a file and select some code first.');
        return;
      }

      const selection = editor.selection;
      const selectedCode = selection.isEmpty ? '' : editor.document.getText(selection);
      const languageId = editor.document.languageId;

      let prompt: string;
      if (selectedCode) {
        prompt = `I'm working on this ${languageId} code and I'm stuck. Give me a HINT to help me figure out what to do next. Do NOT write the solution for me -- just point me in the right direction.\n\n\`\`\`${languageId}\n${selectedCode}\n\`\`\``;
      } else {
        // No selection -- ask the user for their question
        const question = await vscode.window.showInputBox({
          prompt: 'What do you need a hint for?',
          placeHolder: 'e.g., How do I sort a list in Python?',
        });
        if (!question) return;
        prompt = `I'm learning ${languageId} and need a hint. ${question}\n\nGive me a HINT, not the answer. Help me think through it step by step.`;
      }

      await chatPanel.sendMessage(prompt);
    })
  );
}
```

### 10.4 smolpc.askQuestion (commands/ask.ts)

Opens the chat panel and optionally pre-fills a question.

```typescript
import * as vscode from 'vscode';
import { ChatPanelProvider } from '../chat/panel';

export function registerAskCommand(
  context: vscode.ExtensionContext,
  chatPanel: ChatPanelProvider
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand('smolpc.askQuestion', async () => {
      const question = await vscode.window.showInputBox({
        prompt: 'Ask SmolPC a question',
        placeHolder: 'e.g., What is a variable?',
      });

      if (!question) return;

      await chatPanel.sendMessage(question);
    })
  );
}
```

### 10.5 smolpc.toggleAutocomplete

Toggles inline completion on/off. This is a convenience command that updates the setting:

```typescript
context.subscriptions.push(
  vscode.commands.registerCommand('smolpc.toggleAutocomplete', () => {
    const config = vscode.workspace.getConfiguration('smolpc');
    const current = config.get<boolean>('autocomplete.enabled', true);
    config.update('autocomplete.enabled', !current, vscode.ConfigurationTarget.Global);
    vscode.window.showInformationMessage(
      `SmolPC autocomplete ${!current ? 'enabled' : 'disabled'}.`
    );
  })
);
```

---

## 11. LSP Diagnostics Integration

### 11.1 How to Read Diagnostics (utils/diagnostics.ts)

VS Code collects diagnostics from all installed language servers (Python, JavaScript, TypeScript, Java, etc.) and makes them available via the `vscode.languages.getDiagnostics()` API.

```typescript
import * as vscode from 'vscode';

/**
 * Get diagnostics (errors, warnings) that overlap with the given selection.
 * If the selection is empty (just a cursor), returns diagnostics for the cursor's line.
 */
export function getDiagnosticsForSelection(
  document: vscode.TextDocument,
  selection: vscode.Selection
): vscode.Diagnostic[] {
  const allDiagnostics = vscode.languages.getDiagnostics(document.uri);

  if (selection.isEmpty) {
    // Cursor position -- return diagnostics on the current line
    return allDiagnostics.filter(
      d => d.range.start.line <= selection.active.line
        && d.range.end.line >= selection.active.line
    );
  }

  // Selection range -- return diagnostics that overlap with the selection
  return allDiagnostics.filter(d =>
    selection.intersection(d.range) !== undefined
  );
}

/**
 * Get all errors (severity = Error) in the current document.
 */
export function getDocumentErrors(
  document: vscode.TextDocument
): vscode.Diagnostic[] {
  return vscode.languages.getDiagnostics(document.uri).filter(
    d => d.severity === vscode.DiagnosticSeverity.Error
  );
}

/**
 * Format diagnostics into a human-readable string for prompt engineering.
 */
export function formatDiagnosticsForPrompt(
  diagnostics: vscode.Diagnostic[]
): string {
  return diagnostics
    .map(d => {
      const severity = d.severity === vscode.DiagnosticSeverity.Error ? 'ERROR'
        : d.severity === vscode.DiagnosticSeverity.Warning ? 'WARNING'
        : 'INFO';
      const line = d.range.start.line + 1; // 1-indexed for humans
      const col = d.range.start.character + 1;
      const source = d.source ? ` (${d.source})` : '';
      return `[${severity}] Line ${line}, Col ${col}: ${d.message}${source}`;
    })
    .join('\n');
}
```

### 11.2 No Additional Language Servers Needed

The extension does NOT install any language servers. It reads diagnostics from whatever language servers the student already has installed (Python extension, ESLint, etc.). If no language servers are installed, `getDiagnostics()` returns an empty array and the "Fix Error" command reports "No errors found."

---

## 12. Education Philosophy and Prompt Engineering

### 12.1 Core Philosophy

SmolPC is for **learning**, not for doing homework. The assistant must:

1. **Hint, don't solve** -- Give guidance that leads the student to discover the answer
2. **Explain, don't just fix** -- When fixing an error, explain WHY it is wrong
3. **Build understanding** -- Use the Socratic method: ask guiding questions
4. **Age-appropriate language** -- Students are 11--18. Use simple language. Avoid jargon. Define technical terms when first used.
5. **Encourage** -- Learning to code is hard. Be encouraging and patient.

### 12.2 System Prompts (utils/prompts.ts)

```typescript
export type SystemPromptMode = 'chat' | 'hint' | 'explain' | 'completion';

/**
 * Get the system prompt for the specified mode.
 * These prompts are carefully crafted for small models (1.5-4B parameters).
 * They are short and direct because small models struggle with long system prompts.
 */
export function getSystemPrompt(mode: SystemPromptMode): string {
  switch (mode) {
    case 'chat':
      return CHAT_SYSTEM_PROMPT;
    case 'hint':
      return HINT_SYSTEM_PROMPT;
    case 'explain':
      return EXPLAIN_SYSTEM_PROMPT;
    case 'completion':
      return COMPLETION_SYSTEM_PROMPT;
  }
}

const CHAT_SYSTEM_PROMPT = `You are SmolPC, a friendly coding tutor for secondary school students (ages 11-18). You help students understand programming concepts and debug their code.

Rules:
- Use simple, clear language. Avoid jargon.
- When a student asks how to do something, explain the concept first, then show a short example.
- Keep your answers concise. Students have short attention spans.
- If you are unsure about something, say so honestly.
- Be encouraging and patient.`;

const HINT_SYSTEM_PROMPT = `You are SmolPC, a coding tutor for students. Your job is to give HINTS, not answers.

Rules:
- NEVER write the complete solution. Give hints that guide the student to discover the answer themselves.
- Ask guiding questions like "What do you think happens when...?" or "Have you tried...?"
- If the student is stuck, break the problem into smaller steps and hint at the first step.
- Use analogies to explain concepts.
- Be encouraging. Say things like "You're on the right track!" or "Good thinking!"
- Keep hints short (2-3 sentences).`;

const EXPLAIN_SYSTEM_PROMPT = `You are SmolPC, a coding tutor for students. Explain code clearly and simply.

Rules:
- Explain what the code does step by step.
- Use simple language. Define any technical terms.
- For each line or block, explain WHY it is there, not just what it does.
- If there is an error, explain what went wrong and WHY, then give a hint (not the fix).
- Use analogies when possible (e.g., "A variable is like a labelled box that stores a value").
- Keep explanations concise.`;

const COMPLETION_SYSTEM_PROMPT = `You are a code completion engine. Given the code before and after the cursor position, output ONLY the code that should be inserted at the cursor. Do not include explanations, markdown formatting, or code fences. Output only the raw code to insert.`;
```

### 12.3 Prompt Design for Small Models

Small models (1.5--4B parameters) behave differently from large models (70B+). Key guidelines:

1. **Short system prompts.** Long system prompts consume limited context window and confuse small models. Keep system prompts under 150 tokens.

2. **Explicit rules.** Small models need explicit instructions ("NEVER write the complete solution") rather than nuanced guidance ("try to guide the student").

3. **Simple formatting.** Do not ask the model to produce complex structured output (JSON, tables). Stick to natural language and code blocks.

4. **One task at a time.** Do not ask the model to do multiple things in one prompt (explain AND fix AND give alternatives). Ask for one thing per turn.

5. **Temperature matters.** Use low temperature (0.2) for completions (predictable). Use moderate temperature (0.7) for chat (varied, natural).

---

## 13. Status Bar Integration

The extension shows a status bar item in the bottom bar of VS Code to indicate engine connection status.

```typescript
// In extension.ts

function createStatusBarItem(context: vscode.ExtensionContext): vscode.StatusBarItem {
  const statusBarItem = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Right,
    100
  );
  statusBarItem.command = 'smolpc.openChat';
  context.subscriptions.push(statusBarItem);
  statusBarItem.show();
  return statusBarItem;
}

// Status states:
// Connected:    "$(check) SmolPC"
// Disconnected: "$(warning) SmolPC (offline)"
// Generating:   "$(loading~spin) SmolPC"

function updateStatusBar(
  statusBarItem: vscode.StatusBarItem,
  state: 'connected' | 'disconnected' | 'generating'
): void {
  switch (state) {
    case 'connected':
      statusBarItem.text = '$(check) SmolPC';
      statusBarItem.tooltip = 'SmolPC Code Helper -- Connected to engine';
      statusBarItem.backgroundColor = undefined;
      break;
    case 'disconnected':
      statusBarItem.text = '$(warning) SmolPC (offline)';
      statusBarItem.tooltip =
        'SmolPC engine is not running. Start the SmolPC app to enable AI features.';
      statusBarItem.backgroundColor = new vscode.ThemeColor(
        'statusBarItem.warningBackground'
      );
      break;
    case 'generating':
      statusBarItem.text = '$(loading~spin) SmolPC';
      statusBarItem.tooltip = 'SmolPC is thinking...';
      statusBarItem.backgroundColor = undefined;
      break;
  }
}
```

### Health Check Polling

The extension periodically checks if the engine is still running:

```typescript
// Poll engine health every 30 seconds
const healthCheckInterval = setInterval(async () => {
  const healthy = await engineClient.health();
  updateStatusBar(statusBarItem, healthy ? 'connected' : 'disconnected');
}, 30_000);

context.subscriptions.push({
  dispose: () => clearInterval(healthCheckInterval),
});
```

---

## 14. Settings

All settings are prefixed with `smolpc.` and registered in `package.json` (see [Section 3](#3-extension-manifest-packagejson)).

### Settings Summary

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `smolpc.engineUrl` | string | `http://localhost:19432` | Engine HTTP URL |
| `smolpc.autocomplete.enabled` | boolean | `true` | Enable ghost text completions |
| `smolpc.autocomplete.debounceMs` | number | `300` | Debounce delay in milliseconds |
| `smolpc.autocomplete.maxTokens` | number | `128` | Max completion length |
| `smolpc.autocomplete.temperature` | number | `0.2` | Completion temperature |
| `smolpc.chat.hintMode` | boolean | `true` | Enable hint-not-solve mode |
| `smolpc.chat.maxTokens` | number | `512` | Max chat response length |
| `smolpc.chat.temperature` | number | `0.7` | Chat temperature |

### Reading Settings

```typescript
const config = vscode.workspace.getConfiguration('smolpc');
const engineUrl = config.get<string>('engineUrl', 'http://localhost:19432');
const autocompleteEnabled = config.get<boolean>('autocomplete.enabled', true);
```

### Reacting to Setting Changes

The extension should react to setting changes without requiring a reload:

```typescript
context.subscriptions.push(
  vscode.workspace.onDidChangeConfiguration((e) => {
    if (e.affectsConfiguration('smolpc.engineUrl')) {
      // Reconnect to new engine URL
      const newUrl = vscode.workspace.getConfiguration('smolpc')
        .get<string>('engineUrl', 'http://localhost:19432');
      engineClient = new EngineClient(newUrl);
      engineClient.initialize();
    }

    if (e.affectsConfiguration('smolpc.autocomplete.enabled')) {
      // Toggle autocomplete (provider already checks this on each invocation)
      const enabled = vscode.workspace.getConfiguration('smolpc')
        .get<boolean>('autocomplete.enabled', true);
      vscode.window.showInformationMessage(
        `SmolPC autocomplete ${enabled ? 'enabled' : 'disabled'}.`
      );
    }
  })
);
```

---

## 15. Extension Lifecycle

### 15.1 Activation (extension.ts)

```typescript
import * as vscode from 'vscode';
import { EngineClient } from './engine/client';
import { SmolPCCompletionProvider } from './completion/provider';
import { ChatPanelProvider } from './chat/panel';
import { registerExplainCommand } from './commands/explain';
import { registerFixErrorCommand } from './commands/fixError';
import { registerHintCommand } from './commands/hint';
import { registerAskCommand } from './commands/ask';

let engineClient: EngineClient;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  // 1. Read engine URL from settings
  const config = vscode.workspace.getConfiguration('smolpc');
  const engineUrl = config.get<string>('engineUrl', 'http://localhost:19432');

  // 2. Create engine client
  engineClient = new EngineClient(engineUrl);

  // 3. Create status bar item
  const statusBarItem = createStatusBarItem(context);

  // 4. Try to initialize (read bearer token + health check)
  const initialized = await engineClient.initialize();
  if (!initialized) {
    updateStatusBar(statusBarItem, 'disconnected');
    vscode.window.showWarningMessage(
      'SmolPC engine is not running. Start the SmolPC app to enable AI features.',
      'Retry'
    ).then(action => {
      if (action === 'Retry') {
        vscode.commands.executeCommand('smolpc.openChat');
      }
    });
  } else {
    const healthy = await engineClient.health();
    updateStatusBar(statusBarItem, healthy ? 'connected' : 'disconnected');
  }

  // 5. Register InlineCompletionProvider
  const completionProvider = new SmolPCCompletionProvider(engineClient);
  context.subscriptions.push(
    vscode.languages.registerInlineCompletionItemProvider(
      { pattern: '**' },
      completionProvider
    )
  );

  // 6. Register Chat Panel (sidebar webview)
  const chatPanel = new ChatPanelProvider(context.extensionUri, engineClient);
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(
      ChatPanelProvider.viewType,
      chatPanel
    )
  );

  // 7. Register commands
  registerExplainCommand(context, chatPanel);
  registerFixErrorCommand(context, chatPanel);
  registerHintCommand(context, chatPanel);
  registerAskCommand(context, chatPanel);

  context.subscriptions.push(
    vscode.commands.registerCommand('smolpc.openChat', () => {
      vscode.commands.executeCommand('smolpc-sidebar.focus');
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('smolpc.toggleAutocomplete', () => {
      const current = config.get<boolean>('autocomplete.enabled', true);
      config.update('autocomplete.enabled', !current, vscode.ConfigurationTarget.Global);
      vscode.window.showInformationMessage(
        `SmolPC autocomplete ${!current ? 'enabled' : 'disabled'}.`
      );
    })
  );

  // 8. Start health check polling
  const healthCheckInterval = setInterval(async () => {
    const healthy = await engineClient.health();
    updateStatusBar(statusBarItem, healthy ? 'connected' : 'disconnected');
  }, 30_000);

  context.subscriptions.push({
    dispose: () => clearInterval(healthCheckInterval),
  });

  // 9. Listen for configuration changes
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration('smolpc.engineUrl')) {
        const newUrl = vscode.workspace.getConfiguration('smolpc')
          .get<string>('engineUrl', 'http://localhost:19432');
        engineClient = new EngineClient(newUrl);
        engineClient.initialize().then(ok => {
          updateStatusBar(statusBarItem, ok ? 'connected' : 'disconnected');
        });
        // Update references in providers
        completionProvider['client'] = engineClient;
        chatPanel['client'] = engineClient;
      }
    })
  );
}

export function deactivate(): void {
  // Cancel any in-flight generation
  engineClient?.cancelInFlight();
}

// --- Status bar helpers (see Section 13) ---

function createStatusBarItem(context: vscode.ExtensionContext): vscode.StatusBarItem {
  const item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  item.command = 'smolpc.openChat';
  item.show();
  context.subscriptions.push(item);
  return item;
}

function updateStatusBar(
  item: vscode.StatusBarItem,
  state: 'connected' | 'disconnected' | 'generating'
): void {
  switch (state) {
    case 'connected':
      item.text = '$(check) SmolPC';
      item.tooltip = 'SmolPC Code Helper -- Connected to engine';
      item.backgroundColor = undefined;
      break;
    case 'disconnected':
      item.text = '$(warning) SmolPC (offline)';
      item.tooltip = 'SmolPC engine is not running. Start the SmolPC app to enable AI features.';
      item.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
      break;
    case 'generating':
      item.text = '$(loading~spin) SmolPC';
      item.tooltip = 'SmolPC is thinking...';
      item.backgroundColor = undefined;
      break;
  }
}
```

### 15.2 Deactivation

On deactivation (`deactivate()` function), the extension cancels any in-flight HTTP requests. It does NOT stop the engine -- the engine is shared with the Tauri app and may still be in use.

### 15.3 Disposal

All VS Code disposables (providers, commands, status bar items, intervals) are pushed to `context.subscriptions`. VS Code automatically disposes them when the extension deactivates.

---

## 16. Error Handling

### 16.1 Error Categories

| Category | User Impact | Handling |
|----------|------------|----------|
| Engine not running | No AI features available | Status bar warning. Show retry button. |
| Token file missing | Cannot authenticate | Same as "engine not running" |
| Engine returns HTTP error | Request failed | Show error in chat panel. Log details. |
| Engine returns malformed SSE | Tokens lost | Skip malformed events. Log warning. |
| Request timeout | Response hangs | AbortController timeout. Show "request timed out" message. |
| User cancellation | Intentional | Cancel gracefully. Show "(stopped)" message. |
| Completion request fails | No ghost text | Silent failure. Log only. Never show popup for completions. |

### 16.2 Retry Logic

The extension does NOT implement retry logic for failed requests. The model is small and local -- if a request fails, it is usually because the engine is not running (network error) or the model produced an error. Retrying would not help.

The health check polling (every 30 seconds) handles reconnection: if the engine comes back after being stopped, the status bar updates and the extension resumes working.

### 16.3 Request Timeout

For chat requests, use a generous timeout (60 seconds) because the model is slow on weak hardware:

```typescript
// In the engine client, add timeout to fetch options
const controller = new AbortController();
const timeoutId = setTimeout(() => controller.abort(), 60_000);

try {
  const response = await fetch(url, { signal: controller.signal, ... });
} finally {
  clearTimeout(timeoutId);
}
```

For completion requests, use a shorter timeout (10 seconds) because stale completions are useless:

```typescript
const completionTimeout = 10_000;
```

---

## 17. Testing Strategy

### 17.1 Unit Tests (Vitest)

Use Vitest for testing pure logic that does not depend on the VS Code API:

```typescript
// test/engine/client.test.ts

import { describe, it, expect } from 'vitest';
import { parseSSEStream } from '../../src/engine/sse';

describe('SSE Parser', () => {
  it('should parse a single token event', async () => {
    const tokens: string[] = [];
    const stream = createMockStream([
      'data: {"id":"1","object":"chat.completion.chunk","created":1,"model":"test","choices":[{"index":0,"delta":{"content":"hello"},"finish_reason":null}]}\n\n',
      'data: [DONE]\n\n',
    ]);

    await parseSSEStream(stream, (chunk) => {
      if (chunk.choices[0]?.delta?.content) {
        tokens.push(chunk.choices[0].delta.content);
      }
    });

    expect(tokens).toEqual(['hello']);
  });

  it('should handle multiple tokens', async () => {
    const tokens: string[] = [];
    const stream = createMockStream([
      'data: {"id":"1","object":"chat.completion.chunk","created":1,"model":"test","choices":[{"index":0,"delta":{"content":"hello"},"finish_reason":null}]}\n\n',
      'data: {"id":"1","object":"chat.completion.chunk","created":1,"model":"test","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}\n\n',
      'data: [DONE]\n\n',
    ]);

    await parseSSEStream(stream, (chunk) => {
      if (chunk.choices[0]?.delta?.content) {
        tokens.push(chunk.choices[0].delta.content);
      }
    });

    expect(tokens).toEqual(['hello', ' world']);
  });

  it('should skip malformed lines', async () => {
    const tokens: string[] = [];
    const stream = createMockStream([
      'data: not json\n\n',
      'data: {"id":"1","object":"chat.completion.chunk","created":1,"model":"test","choices":[{"index":0,"delta":{"content":"ok"},"finish_reason":null}]}\n\n',
      'data: [DONE]\n\n',
    ]);

    await parseSSEStream(stream, (chunk) => {
      if (chunk.choices[0]?.delta?.content) {
        tokens.push(chunk.choices[0].delta.content);
      }
    });

    expect(tokens).toEqual(['ok']);
  });
});

// Helper to create a mock ReadableStream from string chunks
function createMockStream(chunks: string[]): ReadableStream<Uint8Array> {
  const encoder = new TextEncoder();
  let index = 0;

  return new ReadableStream({
    pull(controller) {
      if (index < chunks.length) {
        controller.enqueue(encoder.encode(chunks[index]));
        index++;
      } else {
        controller.close();
      }
    },
  });
}
```

### 17.2 Integration Testing

Integration tests against a real engine are NOT part of the automated test suite. They require the engine to be running with a loaded model. Document a manual test procedure:

**Manual Test Checklist:**
1. Start `smolpc-engine-host.exe` with a model loaded
2. Open VS Code with the extension installed
3. Verify status bar shows "SmolPC" with a checkmark
4. Type code in a `.py` file -- verify ghost text appears after 300ms pause
5. Press Tab to accept a completion
6. Select code, right-click, choose "SmolPC: Explain Selected Code" -- verify explanation appears in chat
7. Introduce a syntax error, right-click, choose "SmolPC: Explain Error" -- verify error explanation
8. Type a question in the chat panel -- verify streaming response
9. Click "Stop" during a response -- verify generation stops
10. Click "Clear" -- verify conversation is cleared
11. Stop the engine -- verify status bar shows "offline" within 30 seconds
12. Restart the engine -- verify status bar recovers within 30 seconds

### 17.3 Context Extraction Tests

```typescript
// test/utils/context.test.ts

import { describe, it, expect } from 'vitest';

describe('extractCompletionContext', () => {
  // These tests require mocking the VS Code TextDocument API.
  // Use a simple mock object that implements the required interface.

  it('should extract prefix and suffix around cursor', () => {
    const mockDocument = createMockDocument([
      'def hello():',
      '    print("Hello")',
      '    # cursor is here',
      '    return True',
    ]);

    // Cursor at line 2, character 4 (after "    ")
    const ctx = extractContext(mockDocument, { line: 2, character: 4 });

    expect(ctx.prefix).toContain('def hello():');
    expect(ctx.prefix).toContain('    print("Hello")');
    expect(ctx.suffix).toContain('return True');
  });
});
```

---

## 18. Distribution and Installation

### 18.1 Packaging as .vsix

```bash
# Install vsce (VS Code Extension CLI)
npm install -g @vscode/vsce

# Package the extension
cd vscode-extension/
npm run build
vsce package
# Output: smolpc-codehelper-0.1.0.vsix
```

### 18.2 Installation Methods

**Method 1: Command line (recommended for IT admins)**
```bash
code --install-extension smolpc-codehelper-0.1.0.vsix
```

**Method 2: VS Code UI**
1. Open VS Code
2. Go to Extensions view (Ctrl+Shift+X)
3. Click the "..." menu at the top
4. Choose "Install from VSIX..."
5. Select the `.vsix` file

**Method 3: VS Code Marketplace (future)**
```bash
# Requires a publisher account
vsce publish
```

**Method 4: Bundle with SmolPC installer**
The main SmolPC installer (`SmolPC-CodeHelper-Setup.exe`) can run `code --install-extension` during post-install to automatically install the extension:
```cmd
:: In NSIS installer post-install script
code --install-extension "%AppData%\SmolPC CodeHelper\extensions\smolpc-codehelper-0.1.0.vsix"
```

### 18.3 VS Code Version Requirement

The extension requires VS Code 1.85.0 or later (`engines.vscode: "^1.85.0"`). This version introduced stable support for:
- `InlineCompletionItemProvider` (no longer behind a flag)
- `WebviewView` for sidebar panels
- `languages.getDiagnostics()` API

VS Code 1.85 was released in November 2023. All currently maintained versions of VS Code satisfy this requirement.

### 18.4 .vsix Package Size

The packaged extension should be very small:
- `dist/extension.js` (bundled + minified): ~50--100 KB
- `package.json`: ~3 KB
- `media/`: ~10 KB (icons)
- **Total .vsix: ~100--200 KB**

This is tiny compared to extensions like GitHub Copilot (~10 MB) or Continue (~5 MB) because SmolPC has no runtime dependencies and no bundled model.

---

## 19. Constraints and Non-Goals

### 19.1 What the Extension Does NOT Do

| Feature | Why Not |
|---------|---------|
| Start/stop the engine | Engine lifecycle is managed by the Tauri app or system launcher |
| Load/switch models | Engine owns model management. Extension is a client only. |
| Rank or select inference backends | Engine owns backend selection (Critical Invariant 17.5) |
| Multi-file agentic edits | Model too small (1.5--4B). Too slow. Too error-prone. |
| Terminal command execution | Too risky for a student-facing tool. Out of scope for MVP. |
| File creation/deletion | Out of scope for MVP. May be added in v2. |
| Git operations | Out of scope |
| Use Chat Participant API | Requires GitHub Copilot installed |
| Persist conversation history | MVP does not persist. Can be added later. |
| Telemetry or analytics | Privacy-first. No data collection. |
| Language-specific features | Extension is language-agnostic. It reads diagnostics from existing language servers. |

### 19.2 Model Limitations

The models used (1.5--4B parameters) have real limitations:

- **Context window:** 2K--4K tokens. Cannot process an entire large file.
- **Code quality:** Good for short completions and explanations. Poor for complex algorithm generation.
- **Reasoning:** Limited multi-step reasoning. Cannot plan multi-file refactors.
- **Speed:** 5--15 tokens/second. Chat responses take a few seconds. Completions may feel sluggish on slow hardware.
- **Hallucination:** Small models hallucinate more than large models. Error explanations may occasionally be wrong.

The extension should NOT promise capabilities beyond what the model can deliver. The education-focused "hint" mode naturally limits expectations.

### 19.3 Platform Support

The VS Code extension itself is cross-platform TypeScript. However:
- The engine (`smolpc-engine-host.exe`) only runs on Windows
- The bearer token path uses `%LOCALAPPDATA%` (Windows-specific)
- NPU acceleration requires Intel Core Ultra on Windows

**Primary platform:** Windows 10/11
**Development:** Can develop the extension on macOS/Linux (pointing at a remote or mocked engine)

---

## 20. Critical Invariants

Rules that MUST NOT be violated. Breaking any of these will cause failures or architectural regressions.

### 20.1 Extension Does NOT Own Engine Lifecycle

The extension does NOT start, stop, or restart the engine. It connects to an already-running engine via HTTP. If the engine is not running, the extension shows a warning -- it does not attempt to spawn the engine process.

**Rationale:** The engine is shared between the Tauri app and the VS Code extension. The Tauri app (or system launcher) manages the engine lifecycle. Having multiple processes try to spawn the engine creates race conditions.

### 20.2 Extension Does NOT Select Backends

The extension sends requests to `/v1/chat/completions`. It does NOT call `/engine/load`, `/engine/ensure-started`, or any endpoint that changes the engine's backend or model selection. The engine makes all backend decisions internally.

### 20.3 Bearer Token is Read-Only

The extension reads the token from `%LOCALAPPDATA%/SmolPC/engine-token.txt`. It NEVER writes to this file. The engine creates the token.

### 20.4 Model Parameter "current"

When sending chat completion requests, the extension uses `"model": "current"`. The engine interprets this as "use whatever model is currently loaded." The extension does NOT specify a model ID.

### 20.5 No Telemetry

The extension must NOT send any data outside of `localhost`. No analytics, no crash reports, no usage tracking, no network calls except to the engine on port 19432.

### 20.6 Completion Failures are Silent

If an inline completion request fails (network error, timeout, malformed response), the failure is logged to the console and the user sees nothing. Completion errors NEVER produce popup notifications or error messages. A failed completion is invisible.

---

## 21. Reference Implementations

Study these open-source projects for patterns and architecture decisions.

### 21.1 Twinny (Recommended First Read)

- **URL:** https://github.com/rjmacarthy/twinny
- **Size:** ~3.6K LOC TypeScript
- **Status:** Archived (no longer maintained, but code is instructive)
- **Relevance:** Closest size and scope match to SmolPC. Local AI completion + chat.

**What to study:**
- `InlineCompletionItemProvider` implementation
- Debouncing and cancellation patterns
- Simple webview chat panel
- Engine client structure

### 21.2 Continue.dev

- **URL:** https://github.com/continuedev/continue
- **Size:** ~50K LOC
- **Relevance:** Full-featured AI coding assistant. Too complex for SmolPC's scope, but good reference for specific subsystems.

**What to study:**
- Webview chat panel architecture (under `gui/`)
- Provider registration patterns
- Multi-provider engine client design

### 21.3 Cline

- **URL:** https://github.com/cline/cline
- **Size:** ~30K LOC
- **Relevance:** Agentic coding assistant. Reference for tool use patterns.

**What to study:**
- Tool execution patterns (for future SmolPC features)
- Terminal integration (future consideration)
- File editing strategies (future consideration)

---

## 22. Implementation Checklist

Use this checklist to track progress. Each item is a discrete, testable milestone.

### Phase 1: Skeleton and Engine Connection

- [ ] Initialize project structure (`npm init`, `package.json`, `tsconfig.json`)
- [ ] Set up esbuild bundler (watch mode + production build)
- [ ] Implement `engine/token.ts` (read bearer token from disk)
- [ ] Implement `engine/sse.ts` (SSE stream parser)
- [ ] Implement `engine/types.ts` (TypeScript interfaces)
- [ ] Implement `engine/client.ts` (health check, non-streaming chat, streaming chat)
- [ ] Implement basic `extension.ts` (activate, create client, status bar)
- [ ] Verify: extension activates, status bar shows connected/disconnected

### Phase 2: Chat Panel

- [ ] Implement `chat/panel.ts` (WebviewViewProvider)
- [ ] Build webview HTML/CSS/JS (messages, input, styling with VS Code theme)
- [ ] Implement message passing (webview to extension, extension to webview)
- [ ] Implement streaming display (tokens appear one by one)
- [ ] Implement cancel generation (stop button)
- [ ] Implement clear history
- [ ] Implement conversation history (in-memory, sliding window)
- [ ] Implement `utils/prompts.ts` (system prompts)
- [ ] Verify: can chat with the model, responses stream in, cancel works

### Phase 3: Commands

- [ ] Implement `commands/explain.ts` (explain selected code)
- [ ] Implement `commands/fixError.ts` (explain error with diagnostics)
- [ ] Implement `commands/hint.ts` (give hints)
- [ ] Implement `commands/ask.ts` (open question)
- [ ] Implement `utils/diagnostics.ts` (read LSP diagnostics)
- [ ] Register all commands in `extension.ts`
- [ ] Add context menu entries
- [ ] Add keybindings
- [ ] Verify: right-click on selected code shows SmolPC options, commands work

### Phase 4: Inline Completion

- [ ] Implement `utils/context.ts` (extract code around cursor)
- [ ] Implement `completion/provider.ts` (InlineCompletionItemProvider)
- [ ] Implement debouncing (300ms default)
- [ ] Implement cancellation (CancellationToken)
- [ ] Implement request staleness detection
- [ ] Implement completion cleanup (remove code fences, duplicated prefix)
- [ ] Register provider in `extension.ts`
- [ ] Verify: ghost text appears after typing pause, Tab accepts, keeps-typing dismisses

### Phase 5: Polish

- [ ] Add health check polling (30-second interval)
- [ ] Handle settings changes without reload
- [ ] Add toggle autocomplete command
- [ ] Test with different VS Code themes (dark, light, high contrast)
- [ ] Test with different languages (Python, JavaScript, HTML, Java)
- [ ] Write unit tests (SSE parser, context extraction)
- [ ] Add error handling for all edge cases
- [ ] Package as .vsix
- [ ] Write manual test checklist

### Phase 6: Future Enhancements (Post-MVP)

- [ ] Persist conversation history across sessions
- [ ] Markdown rendering in chat (proper parser, syntax highlighting)
- [ ] Code block "Copy" and "Insert at Cursor" buttons in chat
- [ ] Diff view for suggested code changes
- [ ] Multi-file context (include imports/related files in completion context)
- [ ] Teacher configuration profile (restrict features, set hint mode permanently)
- [ ] Usage statistics dashboard (local only, for teacher review)

---

## Appendix A: Engine API Quick Reference

All endpoints are on `http://localhost:19432`. All require `Authorization: Bearer <token>`.

| Method | Path | Purpose | Used By Extension |
|--------|------|---------|-------------------|
| `GET` | `/engine/health` | Health check | Yes (polling) |
| `GET` | `/engine/status` | Full engine status | Yes (status bar tooltip) |
| `POST` | `/v1/chat/completions` | Chat completion (streaming + non-streaming) | Yes (chat + completions) |
| `POST` | `/engine/cancel` | Cancel in-flight generation | Yes (stop button) |
| `GET` | `/v1/models` | List available models | No |
| `POST` | `/engine/load` | Load/switch model | No (engine owns this) |
| `POST` | `/engine/ensure-started` | Trigger startup | No (Tauri app does this) |
| `POST` | `/engine/unload` | Unload model | No |
| `POST` | `/engine/shutdown` | Stop engine | No |

### Chat Completion Request

```
POST /v1/chat/completions
Authorization: Bearer <token>
Content-Type: application/json

{
  "model": "current",
  "messages": [
    {"role": "system", "content": "..."},
    {"role": "user", "content": "..."}
  ],
  "stream": true,
  "max_tokens": 512,
  "temperature": 0.7
}
```

### Chat Completion SSE Response

```
data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1710000000,"model":"qwen3-4b-instruct-2507","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1710000000,"model":"qwen3-4b-instruct-2507","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1710000000,"model":"qwen3-4b-instruct-2507","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":12,"completion_tokens":50,"total_tokens":62}}

data: [DONE]
```

---

## Appendix B: VS Code API Quick Reference

APIs used by the extension, with relevant documentation links.

| API | Purpose | Docs |
|-----|---------|------|
| `vscode.languages.registerInlineCompletionItemProvider` | Register ghost text provider | [InlineCompletionItemProvider](https://code.visualstudio.com/api/references/vscode-api#InlineCompletionItemProvider) |
| `vscode.window.registerWebviewViewProvider` | Register sidebar webview | [Webview API](https://code.visualstudio.com/api/extension-guides/webview) |
| `vscode.commands.registerCommand` | Register commands | [Extension API](https://code.visualstudio.com/api/references/vscode-api#commands) |
| `vscode.languages.getDiagnostics` | Read LSP diagnostics | [Languages API](https://code.visualstudio.com/api/references/vscode-api#languages) |
| `vscode.window.createStatusBarItem` | Status bar items | [Window API](https://code.visualstudio.com/api/references/vscode-api#window) |
| `vscode.workspace.getConfiguration` | Read settings | [Workspace API](https://code.visualstudio.com/api/references/vscode-api#workspace) |
| `vscode.workspace.onDidChangeConfiguration` | React to setting changes | [Events](https://code.visualstudio.com/api/references/vscode-api#workspace) |

---

## Appendix C: File Path Reference

| Path | Purpose |
|------|---------|
| `%LOCALAPPDATA%/SmolPC/engine-token.txt` | Bearer token (read by extension) |
| `%LOCALAPPDATA%/SmolPC/models/` | Model artifacts (NOT accessed by extension) |
| `%LOCALAPPDATA%/SmolPC/engine.db` | Engine state (NOT accessed by extension) |
| `%AppData%/SmolPC CodeHelper/` | Tauri app install directory (NOT relevant to extension) |

The VS Code extension itself is installed at:
- **Marketplace/vsix install:** `%USERPROFILE%/.vscode/extensions/smolpc.smolpc-codehelper-0.1.0/`
- **Development:** wherever the source code is cloned

The extension has NO local data storage in the MVP. If persistence is added later, use VS Code's `context.globalStorageUri` for data files.
