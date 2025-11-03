# 🎓 SmolPC Code Helper

An offline AI-powered coding assistant for secondary school students (ages 11-18). Built with Tauri and powered by local Ollama models - works 100% offline after initial setup.

![Status](https://img.shields.io/badge/Status-Working-brightgreen)
![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS-blue)
![License](https://img.shields.io/badge/License-Educational-orange)

---

## ✨ Features

- 🤖 **100% Offline AI** - Uses local Ollama models (no cloud, no API keys)
- 📚 **Student-Friendly** - Clear explanations with well-commented code
- 🐛 **Debug Helper** - Paste broken code and get fixes with explanations
- 💡 **Quick Examples** - Pre-built prompts for common coding tasks
- 💾 **Save & Copy** - Export generated code to files
- 🎨 **Clean Interface** - Simple, distraction-free UI
- 🔄 **Multiple Models** - Switch between Qwen Coder and DeepSeek Coder

---

## 📸 Screenshot

```
┌─────────────────────────────────────────────┐
│  🎓 SmolPC Code Helper           [Model ▾]  │
├─────────────────────────────────────────────┤
│  Ask a coding question:                     │
│  How do I create a function in Python?      │
│                                              │
│  Or paste your code for debugging:          │
│  [Code input area]                          │
│                                              │
│  Quick Examples:                             │
│  [Calculator] [Website] [Loops] [File I/O]  │
│                                              │
│            [🚀 Generate Code]                │
│                                              │
│  Generated Code:                             │
│  [AI-generated code with explanations]      │
│                                              │
│       [📋 Copy]  [💾 Save]  [🗑️ Clear]      │
└─────────────────────────────────────────────┘
```

---

## 🚀 Quick Start

### Prerequisites

Before you begin, you need:
- **Node.js** v18 or higher
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
git clone https://github.com/YOUR_USERNAME/smolpc-codehelper.git
cd smolpc-codehelper

# Install dependencies
npm install

# Run the app (first time takes 3-5 minutes to compile)
npm run tauri dev
```

**The app window should open!** Try asking: "How do I create a function in Python?"

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
git clone https://github.com/YOUR_USERNAME/smolpc-codehelper.git
cd smolpc-codehelper

# Install dependencies
npm install

# Run the app (first time takes 3-5 minutes)
npm run tauri dev
```

**The app window should open!** Try asking: "How do I create a function in Python?"

---

## 📖 How to Use

### 1. Ask Questions

Type any coding question in the question field:
- "How do I create a function in Python?"
- "Show me a for loop in JavaScript"
- "Explain what variables are"

Click **🚀 Generate Code** and wait 10-30 seconds.

### 2. Use Quick Examples

Click any example button to auto-fill common questions:
- **Calculator** - Build a basic calculator
- **Website** - Create HTML structure
- **Loops** - Learn loop syntax
- **File I/O** - Read/write files
- **Sorting** - Sort algorithms
- **Variables** - Data types

### 3. Debug Your Code

Paste your broken code in the "code input" area:

```python
def add_numbers(a, b)
    return a + b
print(add_numbers(5, 3)
```

Click **Generate** - the AI will explain what's wrong and how to fix it!

### 4. Save or Copy Results

- **📋 Copy** - Copy code to clipboard
- **💾 Save** - Save to a file (.py, .js, .html, etc.)
- **🗑️ Clear** - Reset everything

### 5. Switch Models

Use the dropdown (top-right) to switch between:
- **Qwen Coder** (Recommended) - More detailed explanations
- **DeepSeek** (Faster) - Quicker responses

---

## 🏗️ Building an Executable

### Development Mode
```bash
npm run tauri dev
```
- Hot-reload enabled for HTML/CSS/JS
- Rust changes require restart
- Press Ctrl+C to stop

### Production Build
```bash
npm run tauri build
```

**Output locations:**
- **macOS:** `src-tauri/target/release/bundle/macos/SmolPC Code Helper.app`
- **Windows:** `src-tauri/target/release/smolpc-codehelper.exe`

Executable size: ~5-10MB (Ollama must still be installed separately)

---

## 🧪 Example Queries to Try

### Beginner Level
```
"What is a variable in Python?"
"How do I print text in JavaScript?"
"Show me a simple if statement"
```

### Intermediate Level
```
"Create a calculator with +, -, ×, ÷ in Python"
"Build a simple HTML form with name and email"
"Write a function to find the largest number in a list"
```

### Advanced Level
```
"Explain how recursion works with examples"
"Create a class for a bank account in Python"
"Show me how to read a CSV file and calculate averages"
```

### Debugging
Paste broken code and ask:
```
"What's wrong with this code?"
"Why am I getting an error?"
"How can I fix this?"
```

---

## 📁 Project Structure

```
smolpc-codehelper/
├── src/                      # Frontend (HTML/CSS/JS)
│   ├── index.html           # Main UI structure
│   ├── styles.css           # Styling
│   └── main.js              # Frontend logic & Tauri API calls
│
├── src-tauri/               # Backend (Rust)
│   ├── src/
│   │   ├── lib.rs          # Main Rust code (Ollama API)
│   │   └── main.rs         # Entry point
│   ├── Cargo.toml          # Rust dependencies
│   └── tauri.conf.json     # App configuration
│
├── package.json            # Node.js dependencies
└── README.md               # This file
```

---

## 🔧 Tech Stack

| Component | Technology |
|-----------|------------|
| Frontend | Vanilla HTML/CSS/JavaScript |
| Backend | Rust |
| Desktop Framework | Tauri 2.0 |
| AI Engine | Ollama |
| Primary Model | Qwen 2.5 Coder (7B) |
| Secondary Model | DeepSeek Coder (6.7B) |

**Why these choices?**
- **Tauri** - Creates tiny executables (~5MB vs Electron's 100MB+)
- **Vanilla JS** - No framework bloat, easy for students to understand
- **Ollama** - Best local LLM solution, works offline
- **Qwen/DeepSeek** - Specialized coding models, student-friendly outputs

---

## 🐛 Troubleshooting

### "Ollama not running" error

**macOS:**
```bash
# Start Ollama in terminal
ollama serve
```

**Windows:**
- Check system tray for Ollama icon
- Or search "Ollama" in Start menu and launch it

### "Failed to connect to Ollama"

Test if Ollama is responding:
```bash
# macOS/Linux
curl http://localhost:11434/api/tags

# Windows PowerShell
Invoke-WebRequest http://localhost:11434/api/tags
```

Should return JSON with model info.

### Models not found

```bash
# Check installed models
ollama list

# Re-download if needed
ollama pull qwen2.5-coder:7b
```

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

### App window is blank

1. Press **F12** to open Developer Tools
2. Check Console tab for JavaScript errors
3. Verify all files are present in `src/` folder
4. Try restarting: Ctrl+C, then `npm run tauri dev`

### "Cannot access uninitialized variable" error

This means Tauri API isn't loading. Check:
- Make sure you're running `npm run tauri dev` (not opening HTML directly)
- Check browser console (F12) for errors

### Models downloading slowly

Models are 3-4GB each. On slow internet:
- **Qwen 2.5 Coder:** 4.7GB (~10-30 minutes)
- **DeepSeek Coder:** 3.8GB (~10-30 minutes)

Be patient! Download once, use forever offline.

### Windows: "Command not found"

After installing Rust/Node.js:
1. **Close PowerShell completely**
2. **Open a NEW PowerShell window**
3. Try commands again

---

## 💡 Tips & Tricks

### Keyboard Shortcuts
- **Ctrl+Enter** (Cmd+Enter on Mac) - Generate code from question field
- **F12** - Open developer tools
- **Ctrl+C** - Stop the dev server

### For Best Results
1. **Be specific** - "Create a Python function that adds two numbers" is better than "make a function"
2. **One task at a time** - Break complex requests into steps
3. **Use examples** - "Show me like the calculator example but for multiplication"
4. **Ask for explanations** - "Explain how this code works line by line"

### Model Comparison
| Model | Speed | Detail | Best For |
|-------|-------|--------|----------|
| Qwen Coder | Slower | More detailed | Learning, explanations |
| DeepSeek | Faster | Concise | Quick answers, debugging |

---

## 🎯 Use Cases

### For Students
- ✅ Get help with coding homework
- ✅ Learn new programming concepts
- ✅ Debug their own code
- ✅ Practice coding offline
- ✅ Explore different languages

### For Teachers
- ✅ Demonstrate coding concepts in class
- ✅ Provide 24/7 coding help
- ✅ Works without internet (after setup)
- ✅ Safe - no data leaves the computer
- ✅ Free - no API costs

### For Schools
- ✅ Budget-friendly (runs on older hardware)
- ✅ Privacy-compliant (fully offline)
- ✅ No ongoing costs
- ✅ Easy to deploy across labs

---

## 🔐 Privacy & Safety

**This app is 100% private:**
- ✅ No data sent to the internet
- ✅ No cloud APIs
- ✅ No tracking or analytics
- ✅ All processing happens on your computer
- ✅ No account required

**Safe for students:**
- ✅ No inappropriate content
- ✅ Educational focus
- ✅ Age-appropriate explanations
- ✅ No ads or distractions

---

## 🚦 System Requirements

### Minimum
- **OS:** Windows 10/11 or macOS 10.15+
- **RAM:** 8GB
- **Storage:** 10GB free
- **CPU:** Intel i3 or equivalent

### Recommended
- **RAM:** 16GB
- **Storage:** 20GB free (SSD preferred)
- **CPU:** Intel i5 or better

**Note:** First-time code generation may take 30-60 seconds on slower machines. Subsequent requests are faster.

---

## 📚 Resources

- **Tauri Documentation:** https://v2.tauri.app/
- **Ollama Documentation:** https://ollama.com/docs
- **Rust Book:** https://doc.rust-lang.org/book/
- **Qwen Model:** https://huggingface.co/Qwen
- **DeepSeek Model:** https://huggingface.co/deepseek-ai

---

## 🤝 Contributing

Part of the SmolPC 2.0 educational project. Contributions welcome!

### How to Contribute
1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Test thoroughly on both Mac and Windows
5. Commit your changes (`git commit -m 'Add amazing feature'`)
6. Push to the branch (`git push origin feature/amazing-feature`)
7. Open a Pull Request

### Ideas for Contributions
- [ ] Add more example prompts
- [ ] Support for more programming languages
- [ ] Syntax highlighting in output
- [ ] Dark mode
- [ ] History of past questions
- [ ] Export to PDF
- [ ] Multi-language support (Spanish, French, etc.)

---

## 📅 Roadmap

### ✅ Phase 1: MVP (COMPLETE)
- [x] Basic UI
- [x] Ollama integration
- [x] Code generation
- [x] Debugging help
- [x] Save/Copy functions
- [x] Example prompts
- [x] Model switching

### 🚧 Phase 2: Enhancements (Q1 2025)
- [ ] Syntax highlighting
- [ ] Dark mode
- [ ] Question history
- [ ] Better error messages
- [ ] Improved prompts

### 🔮 Phase 3: Advanced (Q2 2025)
- [ ] Multi-file projects
- [ ] Code execution sandbox
- [ ] Step-by-step tutorials
- [ ] Progress tracking

---

## 📝 License

Educational use as part of the SmolPC project.

---

## 🎓 About SmolPC

SmolPC Code Helper is part of the **SmolPC 2.0 initiative** - a suite of educational tools designed for secondary schools that:
- Run on budget hardware
- Work offline
- Respect student privacy
- Support teachers and students

**Other SmolPC Tools:**
- LibreOffice automation
- Educational utilities
- Offline learning resources

**Project Goals:**
- Make quality education accessible
- Reduce dependency on expensive hardware
- Enable offline learning
- Empower teachers with AI tools

---

## 👥 Credits

**Built by the SmolPC Team**

**Powered by:**
- Anthropic (Claude for development assistance)
- Ollama (Local LLM runtime)
- Alibaba (Qwen models)
- DeepSeek (DeepSeek Coder models)
- Tauri Team (Desktop framework)

**Special Thanks:**
- Secondary school teachers who provided feedback
- Students who tested the app
- Open-source community

---

## 📧 Support

**For setup issues:**
1. Check this README's troubleshooting section
2. Search existing GitHub issues
3. Open a new issue with details

**For educational use questions:**
- Contact your school's IT department
- Refer to SmolPC documentation

---

## 🌟 Star This Project!

If this tool helps you or your students, please ⭐ star the repository!

**Demo Ready:** January 2025 🎉

---

**Made with ❤️ for students and teachers**
