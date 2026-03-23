# Ralph Agent Instructions

You are an autonomous coding agent working on the SmolPC Code Helper engine supervisor redesign.

## Your Task

1. Read the PRD at `scripts/ralph/prd.json`
2. Read the progress log at `scripts/ralph/progress.txt` (check Codebase Patterns section first)
3. Check you're on the correct branch `ralph/engine-supervisor-redesign`. If not, check it out.
4. Pick the **highest priority** user story where `passes: false`
5. Read the full design spec at `docs/superpowers/specs/2026-03-23-engine-supervisor-redesign.md`
6. Read `PROMPT.md` (project root) for detailed per-phase implementation guidance
7. Read `CLAUDE.md` (project root) for project conventions and learnings
8. Implement that single user story
9. Run quality checks (see Project Quality Checks below)
10. Update CLAUDE.md files if you discover reusable patterns (see below)
11. If checks pass, commit ALL changes with message: `feat(engine): US-XXX — [Story Title]`
12. Update `scripts/ralph/prd.json` to set `passes: true` for the completed story
13. Also update `tasks/prd.json` to keep it in sync
14. Commit prd.json updates: `chore: mark US-XXX complete in prd.json`
15. Push: `git push`
16. Append your progress to `scripts/ralph/progress.txt`

## Project Quality Checks

Run these before committing:

```bash
# Rust
cargo check --workspace
cargo clippy --workspace
cargo test -p smolpc-engine-core
cargo test -p smolpc-engine-host

# Frontend (from apps/codehelper/)
cd apps/codehelper && npm run check && npm run lint
```

## Project-Specific Rules

- **Svelte 5 runes only** — `$state`, `$derived`, `$effect`. No `writable`/`readable`.
- **Tailwind 4** — utility classes only, no `@apply`.
- **Tauri Channels for streaming** — `tauri::ipc::Channel<T>`, not global Events (except the supervisor's lifecycle events which use `app_handle.emit()`).
- **EngineClient is Clone** — Arc-based `reqwest::Client`. `get_client()` returns cheap clones.
- **Tauri managed state already wraps in Arc** — do NOT add redundant `Arc` inside the handle.
- **Windows DETACHED_PROCESS** — `child.wait()` doesn't work. Use HTTP health + PID checks.
- **Token regeneration on restart is CRITICAL** — stale tokens cause "unhealthy after idle".
- **Use LSP** (findReferences, goToDefinition) when tracing symbol usage, not grep.
- **Migration ordering** — State-mutating commands (Group A: US-007) must migrate before read-only (Group B: US-008).

## Progress Report Format

APPEND to `scripts/ralph/progress.txt` (never replace, always append):
```
## [Date/Time] - [Story ID]
- What was implemented
- Files changed
- **Learnings for future iterations:**
  - Patterns discovered (e.g., "this codebase uses X for Y")
  - Gotchas encountered (e.g., "don't forget to update Z when changing W")
  - Useful context (e.g., "the supervisor state machine is in engine/mod.rs")
---
```

The learnings section is critical — it helps future iterations avoid repeating mistakes and understand the codebase better.

## Consolidate Patterns

If you discover a **reusable pattern** that future iterations should know, add it to the `## Codebase Patterns` section at the TOP of `scripts/ralph/progress.txt` (create it if it doesn't exist). This section should consolidate the most important learnings:

```
## Codebase Patterns
- Example: Supervisor handle methods use oneshot channels for request-response
- Example: Always call supervisor.refresh_status() after load_model/unload_model
- Example: The EngineLifecycleState enum must derive serde::Serialize with tag="state"
```

Only add patterns that are **general and reusable**, not story-specific details.

## Update CLAUDE.md Files

Before committing, check if any edited files have learnings worth preserving in nearby CLAUDE.md files:

1. **Identify directories with edited files** — Look at which directories you modified
2. **Check for existing CLAUDE.md** — Look for CLAUDE.md in those directories or parent directories
3. **Add valuable learnings** — If you discovered something future developers/agents should know

**Do NOT add:**
- Story-specific implementation details
- Temporary debugging notes
- Information already in progress.txt

## Quality Requirements

- ALL commits must pass project quality checks (cargo check, clippy, svelte-check, lint)
- Do NOT commit broken code
- Keep changes focused and minimal
- Follow existing code patterns in the codebase

## Stop Condition

After completing a user story, check if ALL stories have `passes: true`.

If ALL stories are complete and passing, reply with:
<promise>COMPLETE</promise>

If there are still stories with `passes: false`, end your response normally (another iteration will pick up the next story).

## Important

- Work on ONE story per iteration
- Commit frequently
- Keep CI green
- Read the Codebase Patterns section in progress.txt before starting
