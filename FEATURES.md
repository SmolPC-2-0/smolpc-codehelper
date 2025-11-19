# SmolPC Code Helper - Features

## Overview

SmolPC Code Helper is an offline AI-powered coding assistant designed specifically for secondary school students (ages 11-18). It provides intelligent programming help through a modern desktop application that runs completely offline after initial setup, ensuring privacy and accessibility.

---

## Core Features

### 🤖 Offline AI Assistant

**100% Local Processing**
- Uses Ollama to run AI models entirely on your computer
- No internet connection required after initial model download
- No cloud APIs, API keys, or external services
- Complete privacy - your code and questions never leave your device

**Supported AI Models**
- **Qwen 2.5 Coder (7B)**: Primary model, detailed educational explanations
- **DeepSeek Coder (6.7B)**: Faster alternative for quick lookups
- Easy model switching based on your needs

---

### 💬 Intelligent Chat Interface

**Natural Conversations**
- Ask coding questions in plain English
- Get explanations with well-commented code examples
- Optimized for student learning (ages 11-18)
- Clear, encouraging responses with step-by-step breakdowns

**Context-Aware Follow-ups**
- Optional conversation memory for related questions
- Build on previous explanations naturally
- Toggle context on/off per chat
- Ask follow-up questions without repeating information

**Real-Time Streaming**
- See AI responses as they're being generated
- Token-by-token display for immediate feedback
- Cancel generation if response goes off-track
- Background generation while switching between chats

---

### 📚 Multi-Chat Management

**Organize Your Work**
- Create multiple independent conversations
- Separate chats for different:
  - Programming languages (Python, JavaScript, etc.)
  - Topics (loops, functions, debugging)
  - Projects or homework assignments
  - Learning vs debugging sessions

**Smart Organization**
- Automatic chat titles from first message
- Time-based grouping: Today, Yesterday, Last 7 Days, Older
- Quick chat switching via sidebar
- Delete unwanted conversations

**Persistence**
- All chats automatically saved
- Resume conversations across sessions
- Data stored locally in browser storage
- No login or account required

---

### 🎯 Quick Example Prompts

**Pre-Built Templates**
- **Explain Functions**: Learn about functions and parameters
- **Debug Code**: Get help identifying and fixing errors
- **Write Calculator**: Build a basic calculator program
- **HTML Template**: Create HTML page structure
- **Sort Array**: Learn sorting algorithms
- **File Handling**: Read and write files

**Benefits**
- One-click usage for common tasks
- Category organization (Basics, Web, Algorithms, Debugging)
- Collapsible interface - hide when not needed
- Great starting points for beginners

---

### 💾 Data Persistence

**Auto-Save Everything**
- Chats saved automatically after every message
- Settings preserved across sessions
- Model preferences remembered
- No manual save required

**Local Storage**
- Fast load times (instant startup)
- No database setup needed
- Privacy-preserving (data never synced)
- Easy to clear if needed

---

## Advanced Features (v2.1.0)

### 📊 Production-Grade Benchmarking

**Performance Measurement**
- Measure AI model performance with academic research-quality accuracy
- Track tokens per second, latency, response time
- Process-specific CPU and memory monitoring
- 50ms sampling precision for detailed tracking

**Benchmark Methodology**
- **Token Metrics**: Native Ollama metadata (no estimation)
- **Timing**: Nanosecond-precision from Ollama API
- **Resource Monitoring**: Process-specific tracking with warmup
- **Statistical Rigor**: Median calculations, outlier resistance

**Data Export**
- Comprehensive CSV export
- Import into Excel, Google Sheets, or analysis tools
- Standardized test suite (12 tests across 4 categories)
- Compare performance across optimization phases

**Use Cases**
- Academic research and reports
- Performance regression detection
- Model comparison
- Optimization tracking

### 🔬 Resource Monitoring

**Accurate Measurements**
- Process-specific CPU usage tracking
- Memory usage before, during, and after inference
- Peak memory detection
- CPU baseline establishment (200ms required by sysinfo crate)

**Sampling Strategy**
- 50ms sampling intervals during inference
- Median calculations for outlier resistance
- Model warmup to eliminate first-call latency
- No fallback to inaccurate system-wide measurements

**Known Limitations** (v2.1.0 with HTTP Ollama)
- CPU measurements show ~4-16% instead of full system load
- Due to HTTP API architecture (separate processes)
- Will be resolved in next phase with in-process llama.cpp integration

---

## User Experience Features

### 🎨 Modern Interface

**Clean Design**
- Responsive layout adapts to window size
- Dark mode support (system preference)
- Minimal distractions for focus
- Professional appearance

**Intuitive Controls**
- Auto-resize text input (expands as you type)
- Keyboard shortcuts (Shift+Enter for new line)
- Clear status indicators (Ollama connection)
- Visual feedback for actions (copy confirmation)

### 🌐 Background Generation

**Multi-Tasking**
- Ask question in one chat
- Switch to another chat while it generates
- Responses continue in background
- Return anytime to see completed answer

**Benefits**
- Work on multiple questions simultaneously
- No waiting for long responses
- Efficient workflow
- Seamless chat switching

### 💻 Code Actions

**Copy to Clipboard**
- One-click copy for code blocks
- Visual confirmation (check icon)
- Multiple code blocks per response
- Preserves formatting

**Save to File**
- Native file dialog with format filters
- Supported formats: Python, JavaScript, TypeScript, Rust, HTML, CSS, Text
- User chooses save location
- No auto-saves (privacy-preserving)

---

## Educational Features

### 👨‍🎓 Student-Optimized

**Age-Appropriate Content**
- Explanations tailored for ages 11-18
- Simple, encouraging language
- Step-by-step breakdowns
- Patient and supportive tone

**Learning Support**
- Helpful code comments in examples
- Level-appropriate complexity
- Multiple explanation approaches
- Error message interpretation

### 🏫 Teacher-Friendly

**Classroom Use**
- Works without internet (after setup)
- Safe and private (data stays local)
- Free (no per-student API costs)
- Demonstrate coding concepts in class

**Privacy-First**
- GDPR and FERPA compliant by design
- No tracking, analytics, or telemetry
- No account creation required
- Student data never leaves device

---

## Technical Features

### 🔐 Privacy & Security

**Data Protection**
- All processing happens locally
- No cloud APIs or external services
- No data sent to internet (after model download)
- Chats stored in browser localStorage only

**Security Measures**
- Ollama URL restricted to localhost only
- HTTP client connection pooling
- Proper event listener cleanup
- Input sanitization (ongoing improvement)

**Compliance**
- GDPR compliant by design
- FERPA friendly for schools
- No personal data collection
- Open source and auditable

### ⚡ Performance

**Efficient Operation**
- Small executable size (~8-12MB)
- Fast model loading with SSD
- Minimal memory footprint
- Responsive UI during inference

**System Requirements**
- **Minimum**: 8GB RAM, 12GB storage, Intel i3/AMD Ryzen 3
- **Recommended**: 16GB RAM, 20GB storage (SSD), Intel i5/AMD Ryzen 5
- **Performance**: 10-30 tokens/second (varies by CPU/GPU)

### 🔧 Developer Features

**Modern Tech Stack**
- **Frontend**: Svelte 5 (runes-based reactivity)
- **Backend**: Rust (Tauri 2.6)
- **Build**: Vite 6 with hot module replacement
- **Styling**: Tailwind CSS 4
- **AI Runtime**: Ollama (current), llama.cpp (planned)

**Code Quality**
- TypeScript throughout frontend
- Full type coverage
- Proper error handling
- Async/await non-blocking architecture

---

## Platform Support

### Cross-Platform Desktop

**Supported Operating Systems**
- **Windows**: Windows 10+
- **macOS**: macOS 10.15+
- **Linux**: Ubuntu 20.04+ (and derivatives)

**Build Targets**
- Native executables for each platform
- Small bundle sizes (Tauri advantage)
- Platform-specific installers (MSI, DMG, DEB)

---

## Roadmap

### Current: v2.1.0 (November 2025)
- ✅ Production-grade benchmarking system
- ✅ Process-specific resource monitoring
- ✅ Statistical rigor and accuracy

### Upcoming: v2.2.0 (Q1 2026)
- 🚧 Hardware detection (CPU, GPU, NPU)
- 🚧 llama.cpp integration (replace HTTP Ollama)
- 🚧 GGUF quantized models (Q4_0)
- 🚧 Improved CPU monitoring accuracy
- 🚧 In-process inference (faster, simpler)

### Future: v3.0+ (Q2 2026)
- 🔮 NPU acceleration support
- 🔮 Syntax highlighting in code blocks
- 🔮 Code execution sandbox
- 🔮 Export chat to markdown/PDF
- 🔮 Search across all chats
- 🔮 Voice input support
- 🔮 GPU utilization metrics

---

## Use Cases

### For Students
- ✅ Get instant help with coding homework
- ✅ Learn new programming concepts with examples
- ✅ Debug code errors with explanations
- ✅ Practice coding problems offline
- ✅ Explore different programming languages
- ✅ Understand error messages
- ✅ Study help available 24/7

### For Teachers
- ✅ Demonstrate coding concepts in class
- ✅ Provide AI tutoring to all students
- ✅ Works without internet (after setup)
- ✅ Safe and private - data stays local
- ✅ Free - no per-student API costs
- ✅ Customize for curriculum needs

### For Schools
- ✅ Budget-friendly (runs on older hardware)
- ✅ Privacy-compliant (GDPR, FERPA)
- ✅ No ongoing cloud costs
- ✅ Easy to deploy across computer labs
- ✅ Works during internet outages
- ✅ Open source and auditable

---

## Getting Started

**Quick Start Steps:**
1. Install Node.js, Rust, and Ollama
2. Download AI models (qwen2.5-coder:7b)
3. Clone repository and install dependencies
4. Run `npm run tauri dev`
5. Start asking coding questions!

**Full documentation available in README.md**

---

## Support & Community

**Documentation**
- README.md: Complete setup and usage guide
- claude.md: Technical workspace context
- CHANGES.md: Version history and updates
- src-tauri/benchmarks/README.md: Benchmark methodology

**Support**
- GitHub Issues for bug reports
- Comprehensive troubleshooting guide
- Active development and updates

---

**Made with ❤️ for students and teachers worldwide**

*Empowering education through open-source, privacy-first AI*
