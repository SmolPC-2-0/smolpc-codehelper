# SmolPC Unified Assistant -- Specification Index

> **Read this document first.** It orients every AI session and human contributor
> working on the SmolPC Unified Assistant.

---

## Project Summary

SmolPC Code Helper is an **offline AI coding and creative assistant** for
secondary school students (ages 11--18) running budget Windows laptops.

**What it does:** A single Tauri 2 desktop app provides six assistant modes --
Code, GIMP, Blender, Writer, Calc, and Impress -- all powered by a shared local
inference server (`smolpc-engine-host` on port 19432). Code mode is delivered as
a VS Code extension that connects to the same engine.

**Who it is for:** Students and teachers in schools where cloud access, admin
rights, and high-end hardware cannot be assumed.

**Key constraints:**

| Constraint | Detail |
|---|---|
| Offline-first | No cloud calls, no telemetry. All inference runs locally. |
| Privacy-first | GDPR and FERPA compliant by design. |
| Budget hardware | Must run on 8 GB RAM; 16 GB is the comfort tier. |
| NPU support | Intel NPU acceleration via OpenVINO. |
| OS target | Windows primary (10/11). |
| No admin install | NSIS currentUser installer -- no elevated privileges. |

---

## Reading Order

Pick the path that matches your task. Every path starts with this README.

### New session (no prior context)

1. **This README**
2. [ARCHITECTURE.md](ARCHITECTURE.md) -- system-level understanding
3. [CURRENT_STATE.md](CURRENT_STATE.md) -- what is done, in progress, and blocked

### Frontend work

1. [ARCHITECTURE.md](ARCHITECTURE.md)
2. [FRONTEND_SPEC.md](FRONTEND_SPEC.md)
3. [CODE_CONVENTIONS.md](CODE_CONVENTIONS.md)

### VS Code extension work

1. [ARCHITECTURE.md](ARCHITECTURE.md)
2. [VSCODE_EXTENSION_SPEC.md](VSCODE_EXTENSION_SPEC.md)
3. [CODE_CONVENTIONS.md](CODE_CONVENTIONS.md)

### Engine / inference work

1. [ARCHITECTURE.md](ARCHITECTURE.md)
2. [MODEL_STRATEGY.md](MODEL_STRATEGY.md)
3. [CODE_CONVENTIONS.md](CODE_CONVENTIONS.md)

### MCP integration work

1. [ARCHITECTURE.md](ARCHITECTURE.md)
2. [MCP_INTEGRATION.md](MCP_INTEGRATION.md)
3. [CODE_CONVENTIONS.md](CODE_CONVENTIONS.md)

### Packaging / release work

1. [ARCHITECTURE.md](ARCHITECTURE.md)
2. [PACKAGING.md](PACKAGING.md)
3. [MODEL_STRATEGY.md](MODEL_STRATEGY.md)

### Starting a new workstream

1. **This README**
2. [GIT_WORKFLOW.md](GIT_WORKFLOW.md)
3. [ARCHITECTURE.md](ARCHITECTURE.md)
4. The relevant spec document for the workstream

---

## Document Index

| # | Document | Description |
|---|---|---|
| 1 | [ARCHITECTURE.md](ARCHITECTURE.md) | System architecture: process model, inference pipeline, mode switching, MCP integration, and how the VS Code extension fits in. Start here for any technical work. |
| 2 | [FRONTEND_SPEC.md](FRONTEND_SPEC.md) | Unified Tauri app frontend built with Svelte 5 runes and Tailwind 4. Covers the mode dropdown, chat UI, streaming via Tauri Channels, and the status bar. |
| 3 | [VSCODE_EXTENSION_SPEC.md](VSCODE_EXTENSION_SPEC.md) | VS Code extension for Code mode. Defines the InlineCompletionProvider, webview chat panel, LSP diagnostic integration, and education-oriented features. |
| 4 | [MCP_INTEGRATION.md](MCP_INTEGRATION.md) | MCP server integration for creative apps. Covers gimp-mcp (GIMP), the hybrid HTTP + MCP bridge for Blender, mcp-libre (LibreOffice), and the generalized MCP client. |
| 5 | [MODEL_STRATEGY.md](MODEL_STRATEGY.md) | Model selection and tiering. Defines Tier 1 (8 GB, 0.5--3B params) and Tier 2 (16 GB+, 4--8B params), INT4 quantization, the ONNX/OpenVINO export pipeline, and runtime consolidation. |
| 6 | [PACKAGING.md](PACKAGING.md) | Installer and distribution. Covers NSIS currentUser packaging, DLL bundling, Python/MCP deployment via bundled `uv.exe`, Azure Trusted Signing, directory structure, and update mechanism. |
| 7 | [GIT_WORKFLOW.md](GIT_WORKFLOW.md) | Branch naming conventions, rules for parallel AI sessions, PR workflow, commit discipline, and merge conflict prevention strategies. |
| 8 | [CODE_CONVENTIONS.md](CODE_CONVENTIONS.md) | Coding standards for the project. Rust patterns, Svelte 5 runes usage, type synchronization between Rust and TypeScript, error handling, logging, and testing. |
| 9 | [LEARNINGS.md](LEARNINGS.md) | Session-tracked corrections and discoveries, categorized by subsystem. A living record of mistakes made and lessons learned during development. |
| 10 | [CURRENT_STATE.md](CURRENT_STATE.md) | Living status document. Tracks what is done, in progress, and blocked for each workstream. Read after ARCHITECTURE.md to understand where the project stands. |
| 11 | [RESOURCES.md](RESOURCES.md) | External links: documentation, API references, GitHub repositories, HuggingFace model cards, and benchmark results. |

---

## Quick Reference -- Key Decisions

| Decision | Choice | Rationale |
|---|---|---|
| App architecture | Unified Tauri 2 app (mode dropdown) + VS Code extension | Single binary for creative modes; extension for Code mode where VS Code is the natural host |
| Modes | Code, GIMP, Blender, Writer, Calc, Impress | Covers the core curriculum tools students use |
| Inference server | Shared `smolpc-engine-host` on `:19432` | One process manages models and memory; all modes connect to it |
| Inference runtimes | Migrating to 2: `onnxruntime-genai` (CPU/GPU) + `openvino_genai` (NPU) | Best coverage of Intel hardware without runtime bloat |
| Model tiering | Tier 1 (8 GB, 0.5--3B) / Tier 2 (16 GB+, 4--8B) | Tier 1 must work on the worst-case hardware |
| Frontend stack | Svelte 5 runes, Tailwind 4, Tauri Channels | Runes for fine-grained reactivity; Channels for streaming tokens |
| GIMP integration | `maorcc/gimp-mcp` | Community MCP server for GIMP, actively maintained |
| Blender integration | Hybrid: HTTP bridge + `blender-mcp` | Blender's Python API needs both REST and MCP access |
| LibreOffice integration | `patrup/mcp-libre` | MCP server supporting Writer, Calc, Impress |
| Python packaging | Bundle `uv.exe` as Tauri sidecar | Fast, no-admin Python env management |
| Installer | Tauri NSIS `currentUser` | No admin rights required -- critical for school deployments |
| Code signing | Azure Trusted Signing ($9.99/mo) | Affordable code signing to avoid SmartScreen warnings |
| Code mode | VS Code extension (webview chat + InlineCompletionProvider) | Students already use VS Code; inline completions feel native |

---

## Pending Decisions

These items are tracked but not yet resolved. Each will be finalized through
hands-on testing and documented in the relevant spec file.

| # | Decision | Blocker | Tracked in |
|---|---|---|---|
| 1 | **Final model selection** | Requires benchmarking on 8 GB target hardware | [MODEL_STRATEGY.md](MODEL_STRATEGY.md) |
| 2 | **CPU runtime consolidation** | Need to determine if `onnxruntime-genai` alone can cover CPU, or if native OpenVINO CPU path is needed | [MODEL_STRATEGY.md](MODEL_STRATEGY.md) |
| 3 | **Model distribution method** | Bundle models in installer vs download-on-first-run. Tradeoffs: installer size vs first-run UX on offline networks | [PACKAGING.md](PACKAGING.md) |

---

## How to Use This Specification

**For AI sessions (Claude, Codex, etc.):**
Read this README first, then follow the reading order for your task. Reference
`CURRENT_STATE.md` before starting work to avoid duplicating effort or
contradicting in-progress decisions. When you make a significant decision or
discover a correction, record it in `LEARNINGS.md`.

**For human contributors:**
Same reading order applies. The spec documents are the source of truth for
design decisions. If you change a decision, update the relevant spec document
and this README's quick-reference table.

**Living documents:** `CURRENT_STATE.md` and `LEARNINGS.md` are updated
frequently. The other documents are updated when decisions change. All documents
live in this directory (`/unified_docs/` at workspace root).

---

## Repository Cleanup Note

**The original `/docs/` directory has been DELETED on this branch.** It contained
outdated and potentially conflicting documentation from prior development phases
(Phase 1.5, ONNX plan, engine audits, etc.). Since this is a branch from `main`,
nothing is lost — all prior docs are preserved on `main` and can be referenced
if needed.

All authoritative documentation for the unified assistant lives exclusively in
`/unified_docs/`. Future sessions should NOT recreate `/docs/` or reference
documents from the `main` branch's `/docs/` unless explicitly checking historical
context. The spec docs in `/unified_docs/` supersede all prior documentation and
are based on the current codebase state + completed research + web research findings.
