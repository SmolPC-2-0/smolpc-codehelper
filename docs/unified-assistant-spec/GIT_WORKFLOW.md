# Git Workflow

> **Purpose:** Defines git practices for parallel AI sessions (Claude and Codex) working on the SmolPC Unified Assistant. Covers branch naming, commit discipline, PR workflow, parallel session rules, and merge conflict prevention.
>
> **Audience:** Every AI session working on this project. Read this before creating any branches or commits.
>
> **Last Updated:** 2026-03-13

---

## Table of Contents

1. [Branch Strategy](#branch-strategy)
2. [Branch Naming](#branch-naming)
3. [Commit Discipline](#commit-discipline)
4. [PR Workflow](#pr-workflow)
5. [Parallel Session Rules](#parallel-session-rules)
6. [Merge Conflict Prevention](#merge-conflict-prevention)
7. [Repo Cloning Strategy](#repo-cloning-strategy)

---

## Branch Strategy

### Branch Hierarchy

```
main (production-ready)
 └── docs/unified-assistant-spec (this spec branch)
      ├── feature/unified-frontend
      ├── feature/vscode-extension
      ├── feature/mcp-client
      ├── feature/model-export
      ├── feature/cpu-runtime-migration
      ├── fix/...
      └── refactor/...
```

### Rules

1. **`main` is protected.** Never push directly to main. All changes go through PRs.
2. **`docs/unified-assistant-spec` is the current spec branch.** Feature branches for implementation branch from this.
3. **Never force push** to any shared branch.
4. **One feature per branch.** Don't mix unrelated changes.
5. **Branch from the correct base.** Usually `docs/unified-assistant-spec` for new features, or `main` if the spec branch has been merged.

---

## Branch Naming

| Type | Pattern | Example |
|------|---------|---------|
| Feature | `feature/<description>` | `feature/unified-frontend` |
| Bug fix | `fix/<description>` | `fix/mcp-reconnect-timeout` |
| Refactor | `refactor/<description>` | `refactor/engine-client-api` |
| Documentation | `docs/<description>` | `docs/unified-assistant-spec` |
| Codex sessions | `codex/<description>` | `codex/libreoffice-mcp-client` |
| Claude sessions | `claude/<description>` | `claude/vscode-extension-scaffold` |

### Conventions

- Use lowercase with hyphens (kebab-case)
- Keep descriptions short but descriptive (2-4 words)
- Prefix with agent name for AI sessions: `codex/...` or `claude/...`
- Existing Codex branches on remote: `codex/libreoffice-*`, etc.

---

## Commit Discipline

### Conventional Commits

All commits follow the [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>[optional scope]: <description>

[optional body]

[optional footer]
```

### Types

| Type | When to Use | Example |
|------|------------|---------|
| `feat` | New feature | `feat: add mode dropdown component` |
| `fix` | Bug fix | `fix: resolve MCP reconnection timeout` |
| `docs` | Documentation only | `docs: add ARCHITECTURE.md spec` |
| `refactor` | Code change without new feature or fix | `refactor: extract MCP client trait` |
| `test` | Adding or fixing tests | `test: add engine client unit tests` |
| `style` | Formatting, missing semicolons, etc. | `style: format model registry assertion` |
| `chore` | Maintenance (deps, CI, etc.) | `chore: update ort crate to 2.0` |
| `perf` | Performance improvement | `perf: reduce KV cache allocations` |

### Scopes (Optional)

| Scope | What It Covers |
|-------|---------------|
| `engine` | Engine host, core, client |
| `frontend` | Svelte UI, components |
| `mcp` | MCP client/server integration |
| `inference` | Model loading, generation |
| `vscode` | VS Code extension |
| `packaging` | Build, installer, bundling |

### Rules

1. **Commit often.** After each logical step, not at the end of a session.
2. **Commit before risky changes.** Easy rollback point.
3. **NEVER amend commits.** Always create new commits. Amending can destroy previous work, especially when pre-commit hooks fail.
4. **Each commit should compile.** Don't commit broken code.
5. **Focus on "why", not "what."** The diff shows what changed. The message explains why.
6. **One logical change per commit.** Don't mix a feature with a refactor.

### Co-Author Tags

All AI-generated commits include a co-author tag:

```
feat(frontend): add mode dropdown component

Implements the mode selection dropdown in the unified app header.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
```

Or for Codex:
```
Co-Authored-By: Codex <noreply@openai.com>
```

---

## PR Workflow

### When to Open a PR

- At the end of a session, if work is ready for review
- Before merging any feature branch into `docs/unified-assistant-spec` or `main`
- Never merge without a PR (even for documentation)

### PR Structure

```markdown
## Summary
- What was done (1-3 bullet points)

## Changes
- List of files changed and why

## Test Plan
- [ ] How to verify the changes work
- [ ] What to test manually

## Notes
- Any caveats, known issues, or follow-up work needed
```

### PR Commands

```bash
# Push branch
git push -u origin feature/unified-frontend

# Create PR
gh pr create \
  --title "feat: add unified frontend with mode switching" \
  --body "$(cat <<'EOF'
## Summary
- Implemented mode dropdown component
- Added per-mode suggestion chips
- Integrated with engine client for streaming

## Test Plan
- [ ] Mode switching changes suggestion chips
- [ ] Chat streaming works in all modes
- [ ] Engine status bar updates correctly

🤖 Generated with Claude Code
EOF
)"
```

### Review Process

1. PR creator describes changes clearly
2. Reviewer (human or AI) checks:
   - Does it compile? (`npm run check`, `cargo check`)
   - Does it follow conventions? (Svelte 5 runes, no @apply, etc.)
   - Are types synchronized? (Rust ↔ TypeScript)
   - Are there tests? (for non-trivial logic)
3. Merge via GitHub (squash or merge commit, team preference)

---

## Parallel Session Rules

### The Problem

Multiple AI sessions (Claude + Codex) may work on the project simultaneously. Without rules:
- Both may edit the same file → merge conflicts
- Both may make conflicting architectural decisions
- Both may introduce duplicate code

### Rule 1: Non-Overlapping File Ownership

Each session/workstream owns specific directories. Do NOT edit files outside your ownership:

| Workstream | Owned Directories | Session |
|-----------|-------------------|---------|
| Unified Frontend | `apps/unified-assistant/src/`, `apps/unified-assistant/src-tauri/src/commands/` | Claude |
| VS Code Extension | `apps/vscode-extension/` | Codex |
| MCP Client (generalized) | `engine/crates/smolpc-mcp-client/` | Either |
| Model Export Pipeline | `scripts/model-export/`, CI configs | Codex |
| Engine Runtime Migration | `engine/crates/smolpc-engine-core/src/inference/` | Claude |

### Rule 2: Shared Files Require Coordination

These files are edited by multiple workstreams and require coordination:

| File | Who Edits | Coordination Method |
|------|-----------|-------------------|
| `Cargo.toml` (workspace) | Anyone adding crates | Check existing entries first |
| `CLAUDE.md` | Any session ending | Append, don't overwrite |
| `docs/unified-assistant-spec/CURRENT_STATE.md` | Any session ending | Append to session log section |
| `docs/unified-assistant-spec/LEARNINGS.md` | Any session with new learnings | Append to relevant category |

### Rule 3: Read Spec Docs Before Starting

Before any implementation work:
1. Read `docs/unified-assistant-spec/README.md`
2. Read `docs/unified-assistant-spec/ARCHITECTURE.md`
3. Read the spec doc for your workstream
4. Read `docs/unified-assistant-spec/CURRENT_STATE.md` for latest status

### Rule 4: Update State at Session End

At the end of every session:
1. Commit all changes
2. Update `docs/unified-assistant-spec/CURRENT_STATE.md` with session summary
3. Update `docs/unified-assistant-spec/LEARNINGS.md` if corrections were made
4. Push branch
5. Open PR if work is ready

---

## Merge Conflict Prevention

### Strategy 1: Directory-Level Isolation

Each workstream operates in its own directory subtree. If two sessions never edit the same directory, they can never conflict.

### Strategy 2: Small, Focused Commits

Large commits that touch many files are conflict magnets. Small commits that touch 1-3 files are easy to merge.

### Strategy 3: Rebase Before PR

Before opening a PR, rebase on the target branch:

```bash
git fetch origin
git rebase origin/docs/unified-assistant-spec
```

This ensures your branch is up-to-date and conflicts are resolved locally.

### Strategy 4: Lock Files

`Cargo.lock` and `package-lock.json` are common conflict sources. Rules:
- Don't manually edit lock files
- If conflict in lock file: accept theirs, then re-run `cargo build` / `npm install` to regenerate
- Always commit lock files (they're important for reproducibility)

### Resolving Conflicts

If you encounter a merge conflict:
1. **Investigate first** — understand what the other session changed and why
2. **Don't blindly accept** one side — merge intelligently
3. **Test after resolution** — ensure both changes work together
4. **If unsure** — ask the user before resolving

---

## Repo Cloning Strategy

### IMPORTANT: No Git Worktrees

**DO NOT use `git worktree`.** Use separate clones instead.

**Why:** Feedback from user — worktrees have caused issues in this project. Separate clones are safer and provide full isolation.

### Clone Layout

```
C:\Users\Student\smolpc\CodeHelper\
├── gimp-dev\           # Original development clone (GIMP assistant, feature/ort_setup)
├── smolpc-codehelper\  # Reference clone
├── working-codehelper\ # Working clone
├── final-edition\      # Final/clean clone
└── unified-assistant\  # Fresh clone for unified assistant spec (this one)
```

### Creating a New Clone

```bash
cd C:\Users\Student\smolpc\CodeHelper
git clone https://github.com/SmolPC-2-0/smolpc-codehelper.git <clone-name>
cd <clone-name>
git checkout -b <branch-name>
```

Each clone is fully independent. No shared state between clones.
