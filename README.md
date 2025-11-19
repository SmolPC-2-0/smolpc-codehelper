# 🎓 SmolPC Code Helper

An offline AI-powered coding assistant for secondary school students (ages 11-18). Built with Tauri + Svelte 5 and powered by local Ollama models - works 100% offline after initial setup.

![Status](https://img.shields.io/badge/Status-Active-brightgreen)
![Version](https://img.shields.io/badge/Version-2.1.0-blue)
![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-blue)
![License](https://img.shields.io/badge/License-MIT-orange)

---

## ✨ Features

### Core Functionality
- 🤖 **100% Offline AI** - Uses local Ollama models (no cloud, no API keys)
- 💬 **Chat Interface** - Natural conversation-style coding help
- 📚 **Student-Friendly** - Clear explanations with well-commented code examples
- 🔄 **Multiple Chats** - Organize different projects/topics in separate conversations
- ⚡ **Streaming Responses** - See AI responses as they're generated
- 🎯 **Context-Aware** - Optional conversation history for follow-up questions
- 🌐 **Background Generation** - Switch chats while responses are generating
- 🔄 **Multiple Models** - Switch between different coding models
- 💾 **Auto-Save** - Chats persist across sessions

### Advanced Features (v2.1.0)
- 📊 **Production-Grade Benchmarking** - Measure AI model performance with academic research-quality accuracy
- 🔬 **Process-Specific Monitoring** - Track CPU and memory usage of AI inference with 50ms precision
- 📈 **Performance Analytics** - Export detailed CSV reports for optimization analysis
- 🎯 **Statistical Rigor** - Median calculations, outlier resistance, nanosecond-precision timing

---

## 📸 Screenshots

```
┌─────────────────────────────────────────────────────────────┐
│ [☰] SmolPC Code Helper                      ● Ollama Ready │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  Model: qwen2.5-coder:7b ▾    Context: [✓] Enabled          │
│                                                               │
│  ┌─── Welcome to SmolPC Code Helper! ──────────────────┐   │
│  │                                                        │   │
│  │  Your offline AI coding assistant for learning and   │   │
│  │  problem-solving                                      │   │
│  │                                                        │   │
│  │  Quick Examples:                                      │   │
│  │  [Explain Functions] [Debug Code] [Write Calculator] │   │
│  │  [HTML Template]     [Sort Array] [File Handling]    │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                               │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Ask a coding question (Shift+Enter for new line)...  │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                               │
└───────────────────────────────────────────────────────────────┘

Sidebar:
┌─────────────────┐
│ Chats           │
├─────────────────┤
│ ✓ New Chat      │
│   Python Loops  │
│   HTML Forms    │
│   Debug Help    │
└─────────────────┘
```

---

## 🚀 Quick Start

### Prerequisites

- **Node.js** v20 or higher
- **Rust** (latest stable)
- **Ollama** with coding models installed
- **10GB+ free disk space** (for AI models)
- **Internet connection** (for initial setup only)

---

## 🍎 macOS Setup

### Step 1: Install Prerequisites

```bash
# Install Homebrew (if not already installed)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install Node.js
brew install node

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install Xcode Command Line Tools
xcode-select --install

# Install Ollama
brew install ollama
```

### Step 2: Start Ollama and Download Models

```bash
# Start Ollama (keep this terminal open)
ollama serve
```

**In a NEW terminal window:**

```bash
# Download AI models (this takes 10-20 minutes)
ollama pull qwen2.5-coder:7b      # 4.7GB - Recommended
ollama pull deepseek-coder:6.7b   # 3.8GB - Faster alternative

# Verify installation
ollama list

# Quick test
ollama run qwen2.5-coder:7b "Write hello world in Python"
# Press Ctrl+D to exit
```

### Step 3: Clone and Run

```bash
# Clone the repository
git clone https://github.com/SmolPC-2-0/smolpc-codehelper.git
cd smolpc-codehelper

# Install dependencies
npm install

# Run the app (first time takes 3-5 minutes to compile)
npm run tauri dev
```

**The app window should open!** Try asking: "Explain what functions are in Python"

---

## 🪟 Windows Setup

### Step 1: Install Node.js

1. Download from **https://nodejs.org/**
2. Choose **LTS version** (v20+)
3. Run installer - **CHECK** "Automatically install necessary tools"
4. **Restart your computer** after installation

**Verify:**

```powershell
node --version
npm --version
```

### Step 2: Install Rust

1. Download from **https://rustup.rs/**
2. Click **"rustup-init.exe"**
3. Press **1** then Enter (default installation)
4. Wait 5-10 minutes
5. **Close and reopen PowerShell**

**Verify:**

```powershell
rustc --version
cargo --version
```

### Step 3: Install Ollama

1. Download from **https://ollama.com/download/windows**
2. Run **OllamaSetup.exe**
3. Ollama starts automatically in system tray (bottom-right)

**Download AI models:**

```powershell
# Download models (takes 10-20 minutes)
ollama pull qwen2.5-coder:7b      # 4.7GB - Recommended
ollama pull deepseek-coder:6.7b   # 3.8GB - Faster

# Verify
ollama list

# Test
ollama run qwen2.5-coder:7b "Write hello world in Python"
# Type /bye to exit
```

### Step 4: Clone and Run

```powershell
# Clone repository
git clone https://github.com/SmolPC-2-0/smolpc-codehelper.git
cd smolpc-codehelper

# Install dependencies
npm install

# Run the app (first time takes 3-5 minutes)
npm run tauri dev
```

**The app window should open!** Try asking: "Explain what functions are in Python"

---

## 📖 How to Use

### 1. Chat with the AI

Type your coding question in the input box at the bottom and press Enter:

- "What are variables in Python?"
- "How do I create a for loop in JavaScript?"
- "Explain how recursion works"
- "Show me how to read a file in Python"

The AI will respond with explanations and code examples in real-time.

### 2. Use Quick Examples

Click the example buttons to get started quickly:

- **Explain Functions** - Learn about functions and parameters
- **Debug Code** - Get help fixing errors
- **Write Calculator** - Build a basic calculator
- **HTML Template** - Create HTML structure
- **Sort Array** - Learn sorting algorithms
- **File Handling** - Read and write files

### 3. Have Follow-Up Conversations

**With Context Enabled** (default):
- The AI remembers previous messages in the current chat
- Ask follow-up questions naturally
- Build on previous examples

**Example conversation:**
```
You: "How do I create a function in Python?"
AI: [Explains functions with example]

You: "Can you show me one with multiple parameters?"
AI: [Builds on previous explanation]
```

**Toggle Context** using the switch in the top bar to disable history if needed.

### 4. Manage Multiple Chats

Click the **☰** button to open the sidebar:

- **New Chat** - Start a fresh conversation
- **Switch Chats** - Click any chat to switch to it
- **Delete** - Click 🗑️ to remove a chat
- **Auto-Titles** - Chats are automatically named from your first message

**Use cases:**
- One chat for each homework assignment
- Separate chats for different programming languages
- Keep debugging sessions separate from learning

### 5. Background Generation

Start typing a question in one chat, then switch to another chat while it's still generating:

- The response continues generating in the background
- Switch back anytime to see the completed answer
- Work on multiple questions simultaneously

### 6. Switch Models

Use the **Model** dropdown (top bar) to switch between:

- **qwen2.5-coder:7b** (Recommended) - More detailed, educational explanations
- **deepseek-coder:6.7b** (Faster) - Quicker responses, good for quick lookups

Different models available based on what you have installed in Ollama.

### 7. Cancel Generation

If a response is taking too long or going off-track:

- Click the **✖ Cancel Generation** button that appears while generating
- The response stops immediately
- Try rephrasing your question

---

## 🧪 Example Questions to Try

### Beginner Level

```
"What is a variable?"
"How do I print text in Python?"
"Show me a simple if statement"
"What's the difference between = and ==?"
```

### Intermediate Level

```
"Create a calculator with +, -, ×, ÷"
"Explain how lists work in Python"
"Show me a for loop that counts to 10"
"How do I create a class?"
```

### Advanced Level

```
"Explain recursion with the factorial example"
"How do I read a CSV file and find averages?"
"Show me a binary search algorithm"
"Explain generators vs list comprehensions"
```

### Debugging

Paste your code and ask:

```
"Why is this code not working?

def add(a, b)
    return a + b
"

"I'm getting 'IndexError: list index out of range' - help?"

"This loop runs forever, why?"
```

---

## 🏗️ Building an Executable

### Development Mode

```bash
npm run tauri dev
```

- Hot-reload enabled for Svelte changes
- Rust changes require restart
- Press Ctrl+C to stop

### Production Build

```bash
npm run tauri build
```

**Output locations:**

- **macOS:** `src-tauri/target/release/bundle/macos/SmolPC Code Helper.app`
- **Windows:** `src-tauri/target/release/bundle/msi/SmolPC Code Helper_x.x.x_x64_en-US.msi`
- **Linux:** `src-tauri/target/release/bundle/deb/smolpc-code-helper_x.x.x_amd64.deb`

Executable size: ~8-12MB (Ollama must still be installed separately)

---

## 📁 Project Structure

```
smolpc-codehelper/
├── src/                          # Frontend (Svelte 5)
│   ├── App.svelte               # Main application component
│   ├── lib/
│   │   ├── components/          # UI components
│   │   │   ├── Sidebar.svelte  # Chat list sidebar
│   │   │   ├── ChatMessage.svelte
│   │   │   ├── ChatInput.svelte
│   │   │   └── ...
│   │   ├── stores/              # State management
│   │   │   ├── chats.svelte.ts # Chat state (Svelte 5 runes)
│   │   │   ├── settings.svelte.ts
│   │   │   └── ollama.svelte.ts
│   │   └── types/               # TypeScript types
│   └── main.ts                  # Entry point
│
├── src-tauri/                   # Backend (Rust)
│   ├── src/
│   │   ├── commands/
│   │   │   ├── ollama.rs       # Ollama API integration
│   │   │   ├── benchmark.rs    # Benchmark Tauri commands
│   │   │   └── errors.rs       # Error handling
│   │   ├── benchmark/           # Benchmark System (v2.1.0)
│   │   │   ├── mod.rs          # Module exports
│   │   │   ├── runner.rs       # Production-grade data collection
│   │   │   ├── metrics.rs      # Benchmark data structures
│   │   │   ├── test_suite.rs   # Standardized test prompts
│   │   │   └── export.rs       # CSV export functionality
│   │   ├── lib.rs              # Main Rust library
│   │   └── main.rs             # Entry point
│   ├── benchmarks/              # Benchmark Results Storage
│   │   ├── README.md           # Methodology documentation
│   │   └── *.csv               # Benchmark data files
│   ├── Cargo.toml              # Rust dependencies
│   └── tauri.conf.json         # App configuration
│
├── package.json                # Node.js dependencies
├── vite.config.ts              # Vite configuration
├── tsconfig.json               # TypeScript configuration
├── FEATURES.md                 # Feature documentation
├── CHANGES.md                  # Version changelog
└── README.md                   # This file
```

---

## 🔧 Tech Stack

| Component         | Technology              | Why?                                   |
| ----------------- | ----------------------- | -------------------------------------- |
| Frontend          | Svelte 5                | Reactive, minimal boilerplate          |
| State Management  | Svelte 5 Runes          | Built-in reactivity                    |
| Styling           | Tailwind CSS 4          | Utility-first, fast development        |
| UI Components     | shadcn-svelte           | Pre-built accessible components        |
| Backend           | Rust                    | Performance, memory safety             |
| Desktop Framework | Tauri 2.0               | Small executables (~8MB vs 100MB+)     |
| Build Tool        | Vite 6                  | Fast HMR, optimized builds             |
| AI Engine         | Ollama                  | Best local LLM solution                |
| Primary Model     | Qwen 2.5 Coder (7B)     | Specialized for code, educational      |
| Secondary Model   | DeepSeek Coder (6.7B)   | Fast inference, good code quality      |
| Storage           | localStorage            | Persistent chats across sessions       |

---

## 🐛 Troubleshooting

### "Ollama not running" error

**Status indicator shows red dot** at top-right.

**macOS/Linux:**

```bash
# Start Ollama in terminal
ollama serve
```

**Windows:**

- Check system tray (bottom-right) for Ollama icon
- If not running: Start menu → search "Ollama" → launch it
- Or open Command Prompt and run: `ollama serve`

### "Failed to connect to Ollama"

Test if Ollama is responding:

```bash
# macOS/Linux/Windows (Command Prompt)
curl http://localhost:11434/api/tags

# Windows PowerShell
Invoke-WebRequest http://localhost:11434/api/tags
```

Should return JSON with model info. If not, restart Ollama.

### "No models available" in dropdown

```bash
# Check installed models
ollama list

# Should see models like:
# qwen2.5-coder:7b
# deepseek-coder:6.7b

# If empty, download models
ollama pull qwen2.5-coder:7b
```

Refresh the app after downloading models.

### Chat not saving/persisting

Chats are saved to browser's localStorage. If they're not persisting:

1. Check browser console (F12) for errors
2. Make sure you have storage permissions
3. Try clearing storage and restarting: Developer Tools → Application → Clear Storage

### Response generation stuck

If "Generating response..." never finishes:

1. Check Ollama is running (`ollama list` in terminal)
2. Click **Cancel Generation** button
3. Try a simpler question first
4. Check available RAM (models need 6-8GB free)

### App window is blank/white screen

1. Press **F12** to open Developer Tools
2. Check Console for errors
3. Common causes:
   - Missing `node_modules` - run `npm install`
   - Build issues - try `npm run tauri dev` again
   - Port conflicts - close other dev servers

### Rust compilation errors

```bash
# Update Rust
rustup update

# Clean and rebuild
cd src-tauri
cargo clean
cd ..
npm run tauri dev
```

### TypeScript errors

```bash
# Check TypeScript version
npx tsc --version

# Should be 5.0+
# If not, update dependencies
npm install
```

### Models downloading slowly

Models are large (4-5GB each). On slow internet:

- **Qwen 2.5 Coder:** ~4.7GB (10-30 minutes)
- **DeepSeek Coder:** ~3.8GB (10-25 minutes)

Download happens once. Use offline forever after.

### Custom Ollama URL

If running Ollama on a different port:

```bash
# Set environment variable before starting app
export OLLAMA_URL="http://localhost:8080"
npm run tauri dev

# Windows PowerShell
$env:OLLAMA_URL="http://localhost:8080"
npm run tauri dev
```

**Note:** Only localhost URLs are allowed for security.

---

## 💡 Tips & Tricks

### Keyboard Shortcuts

- **Shift+Enter** - New line in chat input (without sending)
- **Enter** - Send message
- **F12** - Open developer tools
- **Ctrl+C** - Stop dev server (terminal)

### For Best Results

1. **Be specific** - "Create a Python function that adds two numbers and returns the result" beats "make a function"
2. **Use examples** - "Show me like a calculator but for multiplication tables"
3. **Ask for explanations** - "Explain how this code works line by line"
4. **Break down complex tasks** - Ask step-by-step instead of all at once
5. **Use context wisely** - Enable context for follow-ups, disable for fresh topics

### Model Comparison

| Model          | Speed  | Detail        | Best For                        |
| -------------- | ------ | ------------- | ------------------------------- |
| Qwen Coder 7B  | Slower | More detailed | Learning, step-by-step teaching |
| DeepSeek 6.7B  | Faster | Concise       | Quick lookups, simple questions |

### Organizing Chats

Create separate chats for:
- Different programming languages (Python, JavaScript, etc.)
- Different topics (loops, functions, file I/O)
- Different projects (homework assignments)
- Debug sessions vs learning sessions

### Improving Response Quality

If responses aren't helpful:
1. Add more context to your question
2. Try rephrasing more specifically
3. Ask for examples or code snippets explicitly
4. Break complex questions into smaller parts
5. Enable context if asking follow-up questions

---

## 🎯 Use Cases

### For Students

- ✅ Get instant help with coding homework
- ✅ Learn new programming concepts with examples
- ✅ Debug code errors with explanations
- ✅ Practice coding problems offline
- ✅ Explore different programming languages
- ✅ Understand error messages
- ✅ Get study help 24/7

### For Teachers

- ✅ Demonstrate coding concepts in class
- ✅ Provide AI tutoring to all students
- ✅ Works without internet (after setup)
- ✅ Safe and private - data stays local
- ✅ Free - no per-student API costs
- ✅ Customize for curriculum needs
- ✅ Track common student questions

### For Schools

- ✅ Budget-friendly (runs on older hardware)
- ✅ Privacy-compliant (GDPR, FERPA friendly)
- ✅ No ongoing cloud costs
- ✅ Easy to deploy across computer labs
- ✅ Works during internet outages
- ✅ Scales to entire school
- ✅ Open source and auditable

---

## 🔐 Privacy & Security

**This app is 100% private:**

- ✅ No data sent to the internet (after model download)
- ✅ No cloud APIs or external services
- ✅ No tracking, analytics, or telemetry
- ✅ All AI processing happens on your computer
- ✅ No account or login required
- ✅ Chats stored locally in browser storage
- ✅ GDPR and FERPA compliant by design

**Security features:**

- ✅ OLLAMA_URL restricted to localhost only (prevents data exfiltration)
- ✅ HTTP client connection pooling (prevents resource exhaustion)
- ✅ Proper event listener cleanup (prevents memory leaks)
- ✅ Input sanitization (work in progress)

**Safe for students:**

- ✅ No inappropriate content (educational models)
- ✅ Age-appropriate explanations
- ✅ No ads or distractions
- ✅ No social features or chat with strangers
- ✅ Offline-first design

---

## 🚦 System Requirements

### Minimum

- **OS:** Windows 10, macOS 10.15+, or Linux (Ubuntu 20.04+)
- **RAM:** 8GB (6GB free for models)
- **Storage:** 12GB free
- **CPU:** Intel i3 / AMD Ryzen 3 or equivalent
- **GPU:** Not required (CPU inference)

### Recommended

- **RAM:** 16GB
- **Storage:** 20GB free (SSD preferred for faster model loading)
- **CPU:** Intel i5 / AMD Ryzen 5 or better
- **Display:** 1280×720 or higher

**Performance expectations:**

- **First response:** 5-15 seconds (model loading)
- **Subsequent responses:** 1-3 seconds to start streaming
- **Generation speed:** 10-30 tokens/second (varies by CPU)
- **SSD vs HDD:** SSD loads models 3-5× faster

---

## 🔄 Recent Updates

### Version 2.1.0 (Current - November 2025)

**Production-Grade Benchmarking System:**
- ✅ Process-specific resource monitoring (CPU, memory) with 50ms sampling precision
- ✅ Native Ollama metadata for token counts and timing (nanosecond precision)
- ✅ Statistical rigor: median calculations, outlier resistance, CPU baseline establishment
- ✅ Model warmup system to eliminate first-call latency from measurements
- ✅ Comprehensive CSV export for performance analysis
- ✅ Suitable for academic research reports

**Critical Bug Fixes:**
- ✅ Fixed memory peak detection (was mixing system-wide and process-specific measurements)
- ✅ Fixed CPU monitoring baseline (was showing 0% usage)
- ✅ Improved statistical accuracy with median instead of average

**Documentation:**
- ✅ Comprehensive benchmark methodology documentation
- ✅ Updated project structure with benchmark modules
- ✅ Added CHANGES.md with detailed version history

### Version 2.0 (December 2024)

**Major Features:**
- ✅ Migrated to Svelte 5 with runes for better reactivity
- ✅ Background generation - switch chats while AI responds
- ✅ HTTP client pooling for better resource management
- ✅ Configurable Ollama URL via environment variable
- ✅ Security: URL validation to prevent data exfiltration

---

## 📚 Resources

### Documentation

- **Tauri 2.0:** https://v2.tauri.app/
- **Svelte 5:** https://svelte.dev/docs/svelte/overview
- **Ollama:** https://ollama.com/docs
- **Tailwind CSS 4:** https://tailwindcss.com/docs
- **Vite:** https://vitejs.dev/

### AI Models

- **Qwen 2.5 Coder:** https://huggingface.co/Qwen/Qwen2.5-Coder-7B
- **DeepSeek Coder:** https://huggingface.co/deepseek-ai/deepseek-coder-6.7b-base

### Learning Resources

- **Rust Book:** https://doc.rust-lang.org/book/
- **Svelte Tutorial:** https://learn.svelte.dev/
- **TypeScript Handbook:** https://www.typescriptlang.org/docs/

---

## 🤝 Contributing

This is an open educational project. Contributions are welcome!

### How to Contribute

1. **Fork** the repository
2. **Clone** your fork: `git clone https://github.com/YOUR_USERNAME/smolpc-codehelper.git`
3. **Create** a feature branch: `git checkout -b feature/amazing-feature`
4. **Make** your changes
5. **Test** thoroughly on your platform
6. **Commit** with clear messages: `git commit -m 'feat: add amazing feature'`
7. **Push** to your fork: `git push origin feature/amazing-feature`
8. **Open** a Pull Request

### Contribution Ideas

**Features:**
- [ ] Multiple simultaneous generations (parallel chats)
- [ ] Syntax highlighting in messages
- [ ] Code block copy button
- [ ] Export chat to markdown/PDF
- [ ] Search across all chats
- [ ] Chat folders/categories
- [ ] Voice input support
- [ ] Image paste support (for debugging screenshots)

**UI/UX:**
- [ ] Customizable themes
- [ ] Font size adjustment
- [ ] Keyboard shortcuts overlay
- [ ] Better mobile/tablet support
- [ ] Accessibility improvements (screen reader)

**Educational:**
- [ ] More quick example prompts
- [ ] Tutorial mode for first-time users
- [ ] Curriculum-aligned examples
- [ ] Progress tracking
- [ ] Code execution sandbox

**Technical:**
- [ ] Add comprehensive tests
- [ ] Input validation and sanitization
- [ ] Better error handling
- [ ] Rate limiting
- [ ] Data size limits and cleanup
- [ ] Performance monitoring

### Code Style

- **TypeScript:** Use Prettier (already configured)
- **Rust:** Use `cargo fmt` and `cargo clippy`
- **Svelte:** Follow Svelte 5 runes patterns
- **Commits:** Use conventional commits (feat:, fix:, docs:, etc.)

---

## 🐛 Known Issues & Limitations

### Current Limitations

1. **Single generation at a time** - Can't ask questions in multiple chats simultaneously (background generation works, but only one active request)
2. **No XSS protection** - Markdown rendering needs DOMPurify integration
3. **No input validation** - Large prompts or contexts not limited yet
4. **No request timeouts** - Long-running requests can hang indefinitely
5. **Unbounded data growth** - No limits on chat/message count yet
6. **No tests** - Test suite needs to be added

### Planned Fixes

These will be addressed in future releases. See [Contributing](#contributing) for details.

---

## 📅 Roadmap

### ✅ Phase 1: MVP (Complete - Dec 2024)

- [x] Chat interface with Ollama integration
- [x] Multiple chat support
- [x] Streaming responses
- [x] Context-aware conversations
- [x] Model switching
- [x] Quick example prompts
- [x] Background generation
- [x] Svelte 5 migration

### 🚧 Phase 2: Polish (Q1 2025)

- [ ] Syntax highlighting in code blocks
- [ ] Copy code button on code blocks
- [ ] Export chat to markdown
- [ ] Search functionality
- [ ] Better error messages
- [ ] Input validation
- [ ] Request timeouts
- [ ] XSS protection

### 🔮 Phase 3: Advanced Features (Q2 2025)

- [ ] Multiple simultaneous generations
- [ ] Chat folders/organization
- [ ] Code execution sandbox
- [ ] Image paste for debugging
- [ ] Voice input
- [ ] Tutorial mode
- [ ] Progress tracking
- [ ] Comprehensive test suite

---

## 📝 License

MIT License - see [LICENSE](LICENSE) file for details.

Free for educational use. Attribution appreciated but not required.

---

## 🎓 About SmolPC

SmolPC Code Helper is part of the **SmolPC 2.0 initiative** - educational tools for secondary schools that:

- Run on budget hardware
- Work offline
- Respect student privacy
- Empower teachers and students
- Are open source and free

**Project Goals:**

- Make quality education accessible
- Reduce dependency on expensive cloud services
- Enable offline learning
- Give teachers powerful AI tools
- Support student privacy

**Other SmolPC Projects:**

- Educational utilities
- Offline learning resources
- Teacher productivity tools

---

## 👥 Credits

**Built by the SmolPC Team**

**Powered by:**

- [Tauri](https://tauri.app/) - Desktop application framework
- [Svelte](https://svelte.dev/) - Reactive UI framework
- [Ollama](https://ollama.com/) - Local LLM runtime
- [Qwen (Alibaba)](https://github.com/QwenLM/Qwen2.5-Coder) - Coding AI model
- [DeepSeek](https://github.com/deepseek-ai/DeepSeek-Coder) - Alternative coding model
- [Tailwind CSS](https://tailwindcss.com/) - Styling framework
- [shadcn-svelte](https://shadcn-svelte.com/) - UI components

**Special Thanks:**

- Secondary school teachers who provided feedback
- Students who tested early versions
- Open-source community
- Anthropic's Claude for development assistance

---

## 📧 Support

**For technical issues:**

1. Check [Troubleshooting](#troubleshooting) section
2. Search [GitHub Issues](https://github.com/SmolPC-2-0/smolpc-codehelper/issues)
3. Open a new issue with:
   - Your OS and version
   - Node, Rust, Ollama versions
   - Steps to reproduce
   - Screenshots if applicable

**For educational inquiries:**

- Contact your school's IT department
- See SmolPC project documentation

**For security issues:**

- Email: security@smolpc.org (if available)
- Or open a GitHub security advisory

---

## 🌟 Star This Project!

If this tool helps you or your students, please ⭐ **star the repository** on GitHub!

It helps others discover the project and motivates continued development.

---

## 📈 Stats

- **Project Start:** December 2024
- **Current Version:** 2.1.0
- **Lines of Code:** ~6,500+
- **Contributors:** SmolPC Team
- **License:** MIT
- **Platform Support:** Windows, macOS, Linux

---

**Made with ❤️ for students and teachers worldwide**

*Empowering education through open-source, privacy-first AI*
