# Phase 6: VS Code Extension (CodeHelper Extension)

**Goal:** Integrate SmolPC Code Helper into VS Code as an extension with Copilot-like functionality.

**Prerequisites:**
- Phase 1-2 complete (engine with hardware acceleration)
- Phase 5 complete (engine daemon mode)

---

## Objectives

1. Create VS Code extension with chat panel
2. Implement inline code completions (ghost text)
3. Add context-aware commands (Explain, Fix)
4. Future: Full codebase understanding via RAG

---

## Architecture

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
                    │  (daemon)         │
                    └───────────────────┘
```

---

## Phased Deliverables

### Phase 6A: Basic Chat Extension

- [ ] VS Code extension scaffolding (TypeScript)
- [ ] Extension manifest (package.json)
- [ ] Sidebar webview chat panel
- [ ] Engine daemon communication
- [ ] "Explain Code" command (right-click)
- [ ] "Fix Code" command (right-click)
- [ ] Current file/selection as context

### Phase 6B: Inline Completions

- [ ] `InlineCompletionItemProvider` implementation
- [ ] Debounced trigger (500ms after typing stops)
- [ ] Tab to accept, Escape to dismiss
- [ ] Surrounding code context
- [ ] Performance optimization for <500ms TTFT

### Phase 6C: Codebase Awareness (Future)

- [ ] Local embedding model
- [ ] Vector store (LanceDB)
- [ ] Workspace indexing
- [ ] `@file` and `@codebase` commands
- [ ] Semantic code search

---

## Technical Details

### Extension Structure

```
vscode-codehelper/
├── package.json           # Extension manifest
├── src/
│   ├── extension.ts       # Entry point
│   ├── chat/
│   │   ├── panel.ts       # Webview panel
│   │   └── view.html      # Chat UI
│   ├── completion/
│   │   └── provider.ts    # Inline completions
│   ├── context/
│   │   └── gatherer.ts    # File/selection context
│   ├── engine/
│   │   └── client.ts      # HTTP client to daemon
│   └── commands/
│       ├── explain.ts
│       └── fix.ts
├── webview/               # Chat panel UI
└── test/
```

### Engine Client

```typescript
class EngineClient {
  private baseUrl = 'http://localhost:11435';

  async generate(messages: Message[], params: GenerationParams): AsyncGenerator<string> {
    const response = await fetch(`${this.baseUrl}/generate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ messages, params }),
    });

    const reader = response.body!.getReader();
    const decoder = new TextDecoder();

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      yield decoder.decode(value);
    }
  }

  async getStatus(): Promise<EngineStatus> {
    const response = await fetch(`${this.baseUrl}/status`);
    return response.json();
  }
}
```

### Inline Completion Provider

```typescript
class CodeHelperCompletionProvider implements vscode.InlineCompletionItemProvider {
  async provideInlineCompletionItems(
    document: vscode.TextDocument,
    position: vscode.Position,
    context: vscode.InlineCompletionContext,
    token: vscode.CancellationToken
  ): Promise<vscode.InlineCompletionItem[]> {
    // Get surrounding context
    const prefix = document.getText(new vscode.Range(
      new vscode.Position(Math.max(0, position.line - 50), 0),
      position
    ));
    const suffix = document.getText(new vscode.Range(
      position,
      new vscode.Position(position.line + 10, 0)
    ));

    // Build prompt
    const prompt = `<|fim_prefix|>${prefix}<|fim_suffix|>${suffix}<|fim_middle|>`;

    // Get completion (with timeout for latency)
    const completion = await this.engine.complete(prompt, { maxTokens: 100, timeout: 500 });

    return [{
      insertText: completion,
      range: new vscode.Range(position, position),
    }];
  }
}
```

---

## Latency Requirements

Inline completions must be fast to feel responsive:

| Metric | Target |
|--------|--------|
| TTFT | < 500ms |
| Debounce delay | 300-500ms |
| Max completion tokens | 50-100 |

On CPU-only systems, inline completions may need to be disabled or heavily throttled.

---

## Codebase RAG (Phase 6C)

### Components

1. **Embedding Model** - all-MiniLM-L6-v2 (~80MB)
2. **Vector Store** - LanceDB (embedded, no server)
3. **Indexer** - Scan workspace files, chunk, embed
4. **Retriever** - Semantic search for relevant context

### Workflow

```
User asks: "How does authentication work?"
    │
    ▼
Query embedding
    │
    ▼
Search vector store for similar chunks
    │
    ▼
Retrieve top-5 relevant code snippets
    │
    ▼
Include in prompt context
    │
    ▼
Generate response with codebase awareness
```

---

## Success Criteria

### Phase 6A
| Criteria | Target |
|----------|--------|
| Extension installs | Yes |
| Chat panel works | Yes |
| Explain/Fix commands work | Yes |

### Phase 6B
| Criteria | Target |
|----------|--------|
| Inline suggestions appear | Yes |
| TTFT < 500ms (on NPU) | Yes |
| Tab/Escape work | Yes |

### Phase 6C
| Criteria | Target |
|----------|--------|
| @codebase queries work | Yes |
| Indexing < 5 min (medium project) | Yes |
| Answers reference actual code | Yes |

---

## Research Required

| Topic | Questions |
|-------|-----------|
| VS Code InlineCompletion API | Best practices, limitations |
| Fill-in-middle prompting | Qwen FIM format |
| Embedding models | Best for code, size vs quality |
| LanceDB | Rust integration, performance |
| Webview communication | Message passing patterns |

---

*This phase is future work. Focus on Phases 1-5 first.*
