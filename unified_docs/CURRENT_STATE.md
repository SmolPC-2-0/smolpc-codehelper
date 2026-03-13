# Current State

> **Purpose:** Living status document updated at the end of each session. Provides immediate context for any new session without reading code or git history.
>
> **Last Updated:** 2026-03-13

---

## Phase: Unified Assistant Specification

**Branch:** `docs/unified-assistant-spec` (from main at `9fb24f1`)
**Clone Location:** `C:\Users\Student\smolpc\CodeHelper\unified-assistant\`

---

## What's Done

### Specification Documents (unified_docs/)

| Document | Status | Description |
|----------|--------|-------------|
| README.md | ✅ Complete | Index, reading order, key decisions |
| ARCHITECTURE.md | ✅ Complete | System architecture, process model, diagrams |
| FRONTEND_SPEC.md | 🔄 In Progress | Svelte 5 frontend components, mode switching |
| VSCODE_EXTENSION_SPEC.md | 🔄 In Progress | VS Code extension for Code mode |
| MCP_INTEGRATION.md | ✅ Complete | MCP client, per-app server integration |
| MODEL_STRATEGY.md | ✅ Complete | Model selection, tiering, export pipeline |
| PACKAGING.md | ✅ Complete | Installer, DLLs, Python, code signing |
| GIT_WORKFLOW.md | ✅ Complete | Branch naming, parallel sessions, PRs |
| CODE_CONVENTIONS.md | ✅ Complete | Rust, Svelte 5, type sync, anti-patterns |
| LEARNINGS.md | ✅ Complete | Session-tracked corrections and discoveries |
| CURRENT_STATE.md | ✅ Complete | This document |
| RESOURCES.md | ✅ Complete | External links and references |

### Research Completed

All research findings are archived in the plan file (`C:\Users\Student\.claude\plans\ancient-wibbling-comet.md`) and have been incorporated into the spec documents:

- ✅ VS Code Extension Feasibility
- ✅ MCP Ecosystem (GIMP, Blender, LibreOffice)
- ✅ Engine Architecture (CPU migration planned)
- ✅ OpenVINO 2026 capabilities
- ✅ Blender-Assistant Evaluation
- ✅ Multi-Model Tier Management
- ✅ Python/MCP Packaging
- ✅ Model Selection (Qwen3 + Qwen3.5)
- ✅ Model Export Pipeline
- ✅ Full Packaging & Distribution

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

---

## What's In Progress

### Specification Documents
- FRONTEND_SPEC.md — Being written by background agent
- VSCODE_EXTENSION_SPEC.md — Being written by background agent

### Git
- Branch `docs/unified-assistant-spec` created but no commits yet
- Old `/docs/` directory deleted (contained outdated docs from main)
- All spec docs in `/unified_docs/` at workspace root

---

## What's Blocked

| Item | Blocker | Priority |
|------|---------|----------|
| Final model selection | Hands-on benchmarking needed | High |
| CPU runtime choice (Option A vs B) | Performance benchmarking | High |
| Model distribution strategy | School deployment testing | Medium |
| Qwen3.5 viability | OpenVINO NPU verification | Low (wait) |

---

## Next Steps

### Immediate (This Session)
1. Complete remaining spec docs (FRONTEND_SPEC, VSCODE_EXTENSION)
2. Update README.md to reflect `/unified_docs/` location
3. Commit all spec docs
4. Push branch

### Next Session
1. Update CLAUDE.md at repo root with unified assistant context
2. Begin implementation planning for first workstream
3. Decide on parallel AI session assignments (Claude vs Codex)

### Implementation Workstreams (Future)
1. **Unified Frontend** — `feature/unified-frontend`
2. **VS Code Extension** — `feature/vscode-extension`
3. **Generalized MCP Client** — `feature/mcp-client`
4. **CPU Runtime Migration** — `feature/cpu-runtime-migration`
5. **Model Export Pipeline** — `feature/model-export`

---

## Known Issues

1. Old `/docs/` directory has been deleted on this branch — all prior docs (ARCHITECTURE.md, ENGINE_API.md, etc.) are gone. They're still available on `main` but may conflict with unified_docs content.
2. CLAUDE.md at repo root still references old Phase 1.5/Phase 2 context — needs updating for unified assistant.
3. Model files are gitignored — any session needing inference must obtain model files separately.
4. ONNX Runtime DLLs are not in git — must be downloaded for builds.

---

## Session Log

### Session: 2026-03-13 (Unified Assistant Spec)

**Goal:** Crystallize all research findings into actionable specification documents.

**Completed:**
- Created fresh clone at `C:\Users\Student\smolpc\CodeHelper\unified-assistant\`
- Created branch `docs/unified-assistant-spec` from main (9fb24f1)
- Deleted old `/docs/` directory (conflicting/outdated)
- Created `/unified_docs/` at workspace root
- Wrote 10 of 12 spec documents (FRONTEND_SPEC and VSCODE_EXTENSION in progress)
- All research findings from previous sessions incorporated into docs

**Key Decisions Made:**
- Moved docs from `/docs/unified-assistant-spec/` to `/unified_docs/` for clarity
- Deleted all old docs to prevent conflation of conflicting ideas

**Blockers Identified:**
- Model selection requires hands-on benchmarking
- CPU runtime consolidation requires performance testing
