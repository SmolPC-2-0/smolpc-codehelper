# AI Development Workflow

Dual-session workflow for Claude Code + Codex CLI on this repository. This is a local development reference, not a product document.

---

## Session Roles

### Claude Code (interactive session)

- Architecture decisions and multi-file refactors
- Complex debugging (OpenVINO NPU, DLL loading, backend selection policy)
- Cross-crate changes in `engine/`
- PR review before merge to main
- Self-improvement cycle (memory → rules promotion)
- Owns the merge: Claude reviews and merges Codex branches

### Codex CLI (autonomous session)

- Well-scoped, single-purpose tasks
- Isolated file edits, test fixes, config changes
- Build validation (`cargo check`, `cargo clippy`, `npm run check`)
- Documentation updates
- Tasks described upfront in a clear prompt

### The User

- Directs both sessions with task assignments
- Owns `codex/WORKING_ISSUES.md` updates at handoff boundaries
- Makes final merge/release decisions

---

## Branch Convention

```
main                        ← protected, only clean merges
fix/<topic>                 ← Claude session (bug fixes)
feat/<topic>                ← Claude session (features)
codex/<topic>               ← Codex session (scoped tasks)
```

**Rule:** Each session works on its own branch. Never have both sessions on the same branch simultaneously.

---

## Port Convention

The engine server binds to `127.0.0.1:19432` by default (`--port` flag to override).

| Session | Engine Port |
|---|---|
| Claude | 19432 (default) |
| Codex | 19442 |

If both sessions need a running engine simultaneously, Codex uses `--port 19442`.

---

## Coordination Protocol

### Shared state file

`codex/WORKING_ISSUES.md` is the coordination point. Format:

```markdown
### Issue title
- Status: Open | In Progress (Claude/Codex) | Resolved
- Severity: High | Medium | Low
- Context: what and why
- Mitigation: concrete next steps
```

### Handoff rules

1. **Before assigning a Codex task:** Update WORKING_ISSUES.md with the task status.
2. **When Codex completes:** Update status to Resolved with a commit reference.
3. **Before Claude merges a Codex branch:** Review the diff. Don't merge blind.

### File contention

High-contention files (avoid concurrent edits across sessions):
- `engine/crates/smolpc-engine-host/src/runtime_loading.rs`
- `engine/crates/smolpc-engine-host/src/main.rs`
- `Cargo.toml` (workspace root)
- `apps/codehelper/src-tauri/src/lib.rs`

---

## Self-Improvement Cycle

Based on [self-improving-agent](https://github.com/alirezarezvani/claude-skills/tree/main/engineering-team/self-improving-agent) methodology.

### Memory tiers

| Tier | File | Scope | Loaded |
|---|---|---|---|
| Project rules | `CLAUDE.md` | Project-wide | Every session, full |
| Scoped rules | `.claude/rules/*.md` | Path-matched | When matching files open |
| Auto-memory | `~/.claude/.../memory/MEMORY.md` | Session learnings | First 200 lines |
| Codex instructions | `AGENTS.md` | Codex conventions | Every Codex session |

### Promotion lifecycle

```
Error/pattern discovered → captured to memory (automatic or manual)
Pattern recurs 2-3x     → flagged during review
Pattern confirmed        → promoted to CLAUDE.md or .claude/rules/
Memory entry removed     → frees capacity
```

### Promotion scoring

| Dimension | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| Durability | One-time fix | Temp workaround | Stable pattern | Architectural truth |
| Impact | Nice-to-know | Saves 1 minute | Prevents mistakes | Prevents breakage |
| Scope | One file | One directory | Entire project | All projects |

**Promote when total >= 6.** Distill to prescriptive form: "Use X" / "Never Y".

### Review triggers

| Trigger | Action |
|---|---|
| Significant task completed | Review: what was learned? Score. Promote if >= 6. |
| CLAUDE.md approaching 150 lines | Extract scoped rules to `.claude/rules/` |
| Before starting a new phase | Check memory health: stale entries, consolidation |
| Codex branch merged | Check if new patterns worth capturing |

### Scoped rules directory

```
.claude/rules/
├── engine-openvino.md    ← OpenVINO/NPU-specific rules (engine/**/*.rs)
├── engine-models.md      ← Model-specific rules (engine/**/*.rs)
└── build-toolchain.md    ← Build script rules (scripts/**/*, *.ps1, *.sh)
```

Rules load only when path matchers match files being worked on. This keeps context lean.

### Error-capture hook

`.claude/scripts/error-capture.sh` fires on PostToolUse (Bash). Detects error patterns in command output and surfaces a reminder to save the fix. Zero overhead on success.

---

## Phase Plan

### Phase 1: NPU Finalization (current priority)

- Claude: OpenVINO NPU debugging, multi-file inference path work
- Codex: scoped tasks (test validation, config, doc updates)
- Self-improvement cycle active after each debugging session

### Phase 2: Engine Polish

- All backends working: CPU, DirectML, NPU
- PR review discipline for branch merges
- Dependency audit (Cargo + npm)
- Promote accumulated patterns

### Phase 3: Handoff Preparation

- Version bump and changelog (release-manager skill)
- Handoff documentation (codebase-onboarding skill)
- Final memory/rules cleanup
- Packaging (integrate feat/windows-dml-packaging approach)
- USB executable + clean GitHub repo

---

## Skills Loading Protocol

Skills from [alirezarezvani/claude-skills](https://github.com/alirezarezvani/claude-skills) are loaded ad-hoc, never pre-loaded. See `docs/SKILLS_REFERENCE.md` for the full lookup table.

**Rule:** One skill per focused session task. Load by fetching the SKILL.md when needed.
