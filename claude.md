# SmolPC Code Helper

## Project Overview

SmolPC Code Helper is an offline AI-powered coding assistant designed specifically for secondary school students (ages 11-18). Built with Svelte 5, Tauri 2, and Rust, it provides a modern, professional desktop application for learning programming with AI assistance that runs completely offline after initial setup.

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
- **File Dialogs**: rfd

### AI Integration
- **Runtime**: Ollama (local, offline)
- **Primary Model**: Qwen 2.5 Coder (7B)
- **Secondary Model**: DeepSeek Coder (6.7B)
- **API**: HTTP streaming to localhost:11434

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
│   │   └── commands/                   # Tauri command handlers
│   │       ├── mod.rs                  # Module exports
│   │       ├── errors.rs               # Error handling
│   │       ├── ollama.rs               # Ollama integration
│   │       └── default.rs              # File I/O operations
│   ├── Cargo.toml                      # Rust dependencies
│   ├── tauri.conf.json                 # Tauri configuration
│   └── capabilities/                   # Security permissions
│
├── package.json                         # Node dependencies
├── vite.config.ts                       # Vite configuration
├── svelte.config.js                     # Svelte preprocessor config
├── tailwind.config.js                   # Tailwind configuration
├── tsconfig.json                        # TypeScript configuration
└── README.md                            # User documentation
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

## Markdown Rendering

### Custom Implementation
Due to environment constraints, a lightweight custom markdown parser is included:

**Supported Features:**
- Code blocks with language detection
- Inline code
- Headers (H1, H2, H3)
- Bold/Italic
- Links
- Unordered/Ordered lists
- Basic syntax highlighting (keywords, strings, numbers, comments)

**Upgrading to Production:**
When able to install packages:
```bash
npm install marked highlight.js
```

Then replace `/src/lib/utils/markdown.ts` with:
```typescript
import { marked } from 'marked';
import hljs from 'highlight.js';

export function renderMarkdown(text: string): string {
  return marked(text, {
    highlight: (code, lang) => {
      if (lang && hljs.getLanguage(lang)) {
        return hljs.highlight(code, { language: lang }).value;
      }
      return code;
    }
  });
}
```

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
```

## Key Design Decisions

### 1. Svelte 5 Runes over Stores
- **Rationale**: Runes provide simpler, more performant reactivity
- **Implementation**: Custom store objects with runes for backward compatibility
- **Benefits**: Better TypeScript support, less boilerplate

### 2. Streaming Only
- **Rationale**: Better UX with real-time feedback
- **Removed**: Non-streaming `generate_code` command
- **Benefits**: Faster perceived response time

### 3. LocalStorage Persistence
- **Rationale**: Simple, reliable, no backend required
- **Auto-save**: Every state change triggers persist
- **Benefits**: Instant load times, no database needed

### 4. Component Modularity
- **Rationale**: Reusability and maintainability
- **Pattern**: Small, focused components with clear props
- **Benefits**: Easy testing, scalable architecture

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
- **Streaming**: Reduces memory footprint
- **Connection Pooling**: Reuse HTTP connections
- **Async/Await**: Non-blocking I/O operations

## Security Considerations

### XSS Prevention
- **HTML Escaping**: All user content escaped before rendering
- **Markdown Sanitization**: Code blocks safely rendered
- **No Eval**: No dynamic code execution

### File System
- **User Consent**: File dialogs for all file operations
- **No Auto-saves**: User explicitly chooses save locations

### Tauri Permissions
- **Minimal Scope**: Only required capabilities enabled
- **File Access**: Limited to user-selected files
- **Network**: Only localhost Ollama connection

## Future Enhancements

### Planned Features
1. **Export/Import**: Backup and restore chat history
2. **Search**: Find messages across all chats
3. **Code Execution**: Run code snippets safely
4. **File Context**: Analyze user's code files
5. **Settings Panel**: Advanced configuration UI
6. **Themes**: Light/dark mode with custom colors
7. **Keyboard Shortcuts**: Power user efficiency
8. **Chat Renaming**: Custom chat titles

### Scalability Improvements
1. **Database**: SQLite for large chat histories
2. **Virtual Scrolling**: Handle thousands of messages
3. **Web Workers**: Offload markdown rendering
4. **Lazy Loading**: Load chats on demand

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

## Contributing

### Code Style
- Use Prettier for formatting
- Follow TypeScript best practices
- Write descriptive commit messages
- Add comments for complex logic

### Testing
- Test on multiple resolutions
- Verify offline functionality
- Check localStorage limits

## License

MIT License - See LICENSE file for details

## Credits

- **UI Framework**: Svelte Team
- **Desktop Framework**: Tauri Team
- **AI Runtime**: Ollama Team
- **Components**: shadcn-svelte
- **Icons**: Lucide Icons
