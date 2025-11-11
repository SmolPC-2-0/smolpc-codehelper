# SmolPC Code Helper - Claude AI Context

## Project Overview

SmolPC Code Helper is a **desktop-based AI-powered coding assistant** designed specifically for secondary school students (ages 11-18). It's built as a 100% offline application that runs local LLM models through Ollama, ensuring privacy and independence from cloud services.

**Key Characteristics:**
- Desktop application using Tauri 2.0 framework
- Vanilla JavaScript frontend (no framework overhead)
- Rust backend for high-performance API communication
- Local Ollama integration (runs Qwen 2.5 Coder 7B or DeepSeek Coder 6.7B)
- Student-friendly interface with clear explanations
- Real-time streaming responses
- Cross-platform support (Windows, macOS)

## Architecture

### Tech Stack

**Frontend:**
- HTML5 + Vanilla JavaScript + CSS3
- marked.js for markdown rendering
- localStorage for chat persistence
- Tauri API for backend communication

**Backend:**
- Rust (Tokio async runtime)
- reqwest for HTTP client with streaming
- Tauri 2.0 for desktop framework
- serde/serde_json for serialization

**AI Engine:**
- Ollama (local server on http://localhost:11434)
- Primary: Qwen 2.5 Coder (7B) - detailed explanations
- Secondary: DeepSeek Coder (6.7B) - faster responses

### Project Structure

```
/
├── src/                          # Frontend files
│   ├── index.html               # Main UI (184 lines)
│   ├── main.js                  # Application logic (761 lines)
│   ├── styles.css               # Styling (~400 lines)
│   └── assets/                  # SVG images
│
├── src-tauri/                   # Backend (Rust)
│   ├── src/
│   │   ├── lib.rs              # Core logic (345 lines) - Ollama integration
│   │   └── main.rs             # Entry point (6 lines)
│   ├── Cargo.toml              # Rust dependencies
│   ├── tauri.conf.json         # App configuration
│   └── icons/                  # Platform-specific icons
│
├── .github/workflows/          # CI/CD
│   ├── claude.yml              # Claude Code integration
│   └── claude-code-review.yml  # Automated code reviews
│
└── README.md                   # Comprehensive documentation (665 lines)
```

## Key Files

### Frontend (src/)

**main.js (761 lines)** - Core application logic:
- State management: `chats`, `activeChat`, `contextEnabled`
- Chat lifecycle: `createNewChat()`, `loadChat()`, `deleteChat()`, `renameChat()`
- Message handling: `sendMessage()`, `addMessage()`, `getContextMessages()`
- Streaming: `generateResponseStream()` with event listeners
- Storage: `saveChatsToStorage()`, `loadChatsFromStorage()`
- UI rendering: `renderMessages()`, `renderChatList()`, `createMessageElement()`

**index.html (184 lines)** - UI structure:
- Sidebar with chat list
- Main content area with message display
- Fixed header with model selector
- Input area with send button
- Quick example buttons
- Status bar for feedback

**styles.css (~400 lines)** - Professional styling:
- Color scheme: Primary `#667eea` (blue), Secondary `#764ba2` (purple)
- Responsive layout with 260px collapsible sidebar
- Message bubbles (user/assistant distinction)
- Loading animations and transitions

### Backend (src-tauri/src/)

**lib.rs (345 lines)** - Core Rust logic:
- `AppState` - Shared HTTP client with connection pooling
- `check_ollama()` - Verify Ollama server status
- `generate_code()` - Non-streaming generation (legacy)
- `generate_code_stream()` - Real-time streaming with Tauri events
- `save_code()` - Native file dialog integration
- System prompt embedded for student-friendly responses

**Tauri Commands:**
1. `check_ollama` → GET /api/tags
2. `generate_code_stream(prompt, model, context)` → POST /api/generate (streaming)
3. `save_code(code, filename)` → Native file dialog

### Configuration

**tauri.conf.json:**
- App ID: `com.smolpc.codehelper`
- Window: 900x800px, resizable
- Frontend served from `/src`

**Cargo.toml dependencies:**
- tauri 2.0 (desktop framework)
- reqwest 0.12 (HTTP with streaming)
- tokio 1 (async runtime)
- serde/serde_json (serialization)
- tauri-plugin-dialog, tauri-plugin-opener

## Important Patterns

### Streaming Response Flow

1. **Frontend initiates:**
   ```javascript
   await window.__TAURI__.core.invoke('generate_code_stream', {
     prompt: userPrompt,
     model: selectedModel,
     context: contextMessages
   });
   ```

2. **Backend streams chunks:**
   ```rust
   while let Some(chunk) = stream.next().await {
       let json_line = parse_ollama_response(chunk);
       app_handle.emit("gen_chunk", json_line.response)?;
   }
   app_handle.emit("gen_done", ())?;
   ```

3. **Frontend listens for events:**
   ```javascript
   listen('gen_chunk', (event) => appendToMessage(event.payload));
   listen('gen_done', () => finalizeMessage());
   listen('gen_error', (event) => handleError(event.payload));
   ```

### Student-Friendly System Prompt

Embedded in lib.rs (lines ~100-120):
- Age-appropriate explanations
- Encouragement and positivity
- Well-commented code examples
- Clear step-by-step guidance
- Complexity awareness (Year 7 vs Year 13)

### Chat Persistence

- **Storage:** `localStorage.setItem('chats', JSON.stringify(chats))`
- **Load on startup:** `loadChatsFromStorage()`
- **Auto-save:** After every message send/receive
- **Structure:** Array of chat objects with `id`, `title`, `messages[]`, `timestamp`

### Context Management

- **Toggle:** Sidebar button to enable/disable conversation memory
- **Implementation:** `getContextMessages()` returns last N messages when enabled
- **Sent to backend:** As `context` parameter in `generate_code_stream()`

## Development Workflows

### Local Development

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

### Prerequisites

- Node.js 18+ (for Tauri CLI)
- Rust 1.70+ (for backend compilation)
- Ollama installed and running on port 11434
- Downloaded model: `ollama pull qwen2.5-coder:7b`

### Testing Ollama Connection

The app automatically checks Ollama status on startup via `checkOllamaStatus()`:
- Green status = Connected
- Red status = Disconnected (shows error message)

### Common Tasks

**Adding a new Tauri command:**
1. Define function in `src-tauri/src/lib.rs` with `#[tauri::command]`
2. Add to `invoke_handler![]` in `lib.rs`
3. Call from frontend: `window.__TAURI__.core.invoke('command_name', { args })`

**Modifying system prompt:**
- Edit the embedded string in `lib.rs` (~line 100)
- Rebuild: `npm run tauri build`

**Changing UI styling:**
- Edit `src/styles.css`
- Hot reload works in dev mode (`npm run tauri dev`)

**Adding new models:**
- Pull with Ollama: `ollama pull model-name`
- Add to model selector in `index.html` (line ~55)
- No backend changes needed

## Known Issues & Recent Fixes

### Recently Resolved (PR #6)

- ✅ Fixed early response cutoff in streaming
- ✅ Resolved icon display issues
- ✅ Optimized shared Ollama client (connection pooling)

### Current Status

- Branch: `claude/create-analysis-documentation-011CV2gdHDYQRzh82fwhykut`
- Last major feature: Streaming response implementation
- CI/CD: GitHub Actions with Claude Code integration

### Potential Improvements

1. **Performance:** Consider caching model responses for identical prompts
2. **UX:** Add dark mode toggle (currently light theme only)
3. **Features:** Export chat history to markdown/PDF
4. **Error handling:** Better offline detection and retry logic
5. **Accessibility:** ARIA labels and keyboard navigation

## Student-Focused Design Principles

When modifying this codebase, maintain these principles:

1. **Clarity over cleverness** - Code should be readable by beginners
2. **Helpful error messages** - Guide users to solutions
3. **Visual feedback** - Loading states, success/error indicators
4. **Encouragement** - Positive language in prompts and UI
5. **Privacy first** - All processing happens locally
6. **No dark patterns** - Transparent about what the app does

## Dependencies

### Frontend (package.json)

```json
{
  "devDependencies": {
    "@tauri-apps/cli": "^2"
  }
}
```

### Backend (Cargo.toml)

```toml
tauri = "2.0"
tauri-plugin-dialog = "2.0"
tauri-plugin-opener = "2.0"
serde = { version = "1", features = ["derive"] }
serde_json = "1.0"
reqwest = { version = "0.12", features = ["json", "stream"] }
futures-util = "0.3"
tokio = { version = "1", features = ["full"] }
rfd = "0.14"
```

## External Resources

- **Tauri Docs:** https://tauri.app/v2/
- **Ollama API:** https://github.com/ollama/ollama/blob/main/docs/api.md
- **Qwen 2.5 Coder:** https://ollama.com/library/qwen2.5-coder
- **Project README:** Comprehensive guide at `/README.md`

## Git Workflow

**Current Branch:** `claude/create-analysis-documentation-011CV2gdHDYQRzh82fwhykut`

**Branch Naming Convention:** `claude/<description>-<session-id>`

**Push Requirements:**
- Always use: `git push -u origin <branch-name>`
- Branch must start with `claude/` and end with session ID
- Retry logic: Up to 4 retries with exponential backoff (2s, 4s, 8s, 16s)

**Commit Message Style:**
- Be descriptive and concise
- Reference issue/PR numbers when applicable
- Use imperative mood ("Add feature" not "Added feature")

## Code Style

### JavaScript (Frontend)

- Use `const`/`let` (no `var`)
- Arrow functions for callbacks
- Template literals for strings
- Async/await for promises
- Descriptive variable names (`activeChat`, not `ac`)

### Rust (Backend)

- Follow Rust conventions (snake_case for functions)
- Use `?` operator for error handling
- Prefer `async/await` over manual futures
- Document public functions with `///`
- Keep functions focused and testable

### CSS

- BEM-like naming (`.chat-list__item`)
- Responsive design patterns
- CSS variables for colors (consider adding)
- Mobile-first approach

## Testing Strategy

Currently minimal automated testing. Opportunities:

1. **Unit tests:** Rust backend functions (parsing, API calls)
2. **Integration tests:** Full Ollama communication flow
3. **E2E tests:** Tauri WebDriver for UI automation
4. **Manual testing:** Student focus groups for UX validation

## Support & Contributing

- **Issues:** Report bugs via GitHub Issues
- **Discussions:** Use GitHub Discussions for feature requests
- **PRs:** Follow existing code style, include description
- **Claude Code:** Automated PR reviews enabled

---

**Last Updated:** 2025-11-11
**Documentation Version:** 1.0
**Target Audience:** AI assistants (Claude), developers, contributors
