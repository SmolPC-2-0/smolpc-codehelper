# Current State

> **Purpose:** Living status document updated at the end of each session. Provides immediate context for any new session without reading code or git history.
>
> **Last Updated:** 2026-03-13

---

## Phase: Unified Assistant — Specification Complete

**Branch:** `docs/unified-assistant-spec` (from main at `9fb24f1`)
**Clone Location:** `C:\Users\Student\smolpc\CodeHelper\unified-assistant\`

---

## What's Done

### Specification Documents (docs/unified-assistant-spec/)

| Document | Status | Description |
|----------|--------|-------------|
| README.md | Complete | Index, reading order, key decisions |
| ARCHITECTURE.md | Complete | System architecture, process model, diagrams |
| FRONTEND_SPEC.md | Complete | Svelte 5 frontend components, mode switching |
| VSCODE_EXTENSION_SPEC.md | Complete | VS Code extension for Code mode |
| MCP_INTEGRATION.md | Complete | MCP client, per-app server integration |
| MODEL_STRATEGY.md | Complete | Model selection, tiering, export pipeline |
| PACKAGING.md | Complete | Installer, DLLs, Python, code signing |
| GIT_WORKFLOW.md | Complete | Branch naming, parallel sessions, PRs |
| CODE_CONVENTIONS.md | Complete | Rust, Svelte 5, type sync, anti-patterns |
| LEARNINGS.md | Complete | Session-tracked corrections and discoveries |
| CURRENT_STATE.md | Complete | This document |
| RESOURCES.md | Complete | External links and references |

### Research Completed

All research findings have been incorporated into the spec documents:

- VS Code Extension Feasibility
- MCP Ecosystem (GIMP, Blender, LibreOffice)
- Engine Architecture (CPU migration planned)
- OpenVINO 2026 capabilities
- Blender-Assistant Evaluation
- Multi-Model Tier Management
- Python/MCP Packaging
- Model Selection (Qwen3 + Qwen3.5)
- Model Export Pipeline
- Full Packaging & Distribution

### Decisions Finalized

| Decision | Choice |
|----------|--------|
| App architecture | Unified Tauri app (mode dropdown) + VS Code extension |
| Modes | All 6: Code, GIMP, Blender, Writer, Calc, Impress |
| Engine | Unchanged — shared smolpc-engine-host on :19432 |
| Inference backends | Migrating to 2 runtimes (onnxruntime-genai + openvino_genai) |
| Model strategy | Tiered: Tier 1 (8GB, 0.5-3B) + Tier 2 (16GB+, 4-8B) |
| GIMP MCP | maorcc/gimp-mcp |
| Blender | Hybrid: keep HTTP bridge + add blender-mcp |
| LibreOffice MCP | patrup/mcp-libre (extension mode) |
| Frontend | Svelte 5 runes, Tailwind 4, Tauri Channels |
| Code mode | VS Code extension (webview chat + InlineCompletionProvider) |
| Python packaging | Bundle uv.exe as Tauri sidecar |
| Installer | Tauri NSIS currentUser mode (no admin) |
| Code signing | Azure Trusted Signing ($9.99/month) |

### CLAUDE.md Updated

Root CLAUDE.md updated with:
- Current phase: Unified Assistant
- Unified Assistant Vision section with mode table
- All 12 spec doc references
- Workstream branch conventions
- Expanded resource links

---

## What's Blocked

| Item | Blocker | Priority |
|------|---------|----------|
| Final model selection | Hands-on benchmarking needed | High |
| CPU runtime choice (Option A vs B) | Performance benchmarking | High |
| Model distribution strategy | School deployment testing | Medium |
| Qwen3.5 viability | OpenVINO NPU verification | Low (wait) |

---

## Next Steps — Implementation Workstreams

All workstreams branch from `docs/unified-assistant-spec`:

| # | Workstream | Branch | Description |
|---|---|---|---|
| 1 | Unified Frontend | `feature/unified-frontend` | Mode dropdown, shared chat UI, per-mode config |
| 2 | VS Code Extension | `feature/vscode-extension` | InlineCompletionProvider + webview chat |
| 3 | MCP Client | `feature/mcp-client` | Generalized MCP client (stdio + TCP) |
| 4 | CPU Runtime Migration | `feature/cpu-runtime-migration` | Migrate off raw ort to onnxruntime-genai or OpenVINO |
| 5 | Model Export Pipeline | `feature/model-export` | ONNX + OpenVINO INT4 export automation |
| 6 | Installer/Packaging | `feature/packaging` | NSIS installer, DLL bundling, uv sidecar |

---

## Known Issues

1. Old `/docs/` directory was deleted on this branch — prior docs are preserved on `main` for historical reference.
2. Model files are gitignored — any session needing inference must obtain model files separately.
3. ONNX Runtime DLLs are not in git — must be downloaded for builds.

---

## Session Log

### Session: 2026-03-13 (Unified Assistant Spec)

**Goal:** Crystallize all research findings into actionable specification documents.

**Completed:**
- Created fresh clone at `C:\Users\Student\smolpc\CodeHelper\unified-assistant\`
- Created branch `docs/unified-assistant-spec` from main (9fb24f1)
- Deleted old `/docs/` directory (contained outdated phase 1-2 docs)
- Wrote all 12 specification documents in `docs/unified-assistant-spec/`
- All research findings from previous sessions incorporated into docs
- Updated root CLAUDE.md for unified assistant phase
- Fixed directory path from `unified_docs/` to `docs/unified-assistant-spec/`

**Key Decisions Made:**
- Spec docs live at `/docs/unified-assistant-spec/` (matches branch name)
- Deleted old docs to prevent conflation of conflicting ideas

**Blockers Identified:**
- Model selection requires hands-on benchmarking
- CPU runtime consolidation requires performance testing
