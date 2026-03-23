# Ralph Iteration: Engine Supervisor Redesign

You are implementing the Engine Supervisor redesign for SmolPC Code Helper. Each iteration you complete ONE user story from the PRD.

## Instructions

1. Read `scripts/ralph/prd.json` — find the FIRST story with `"passes": false`
2. Read the full design spec at `docs/superpowers/specs/2026-03-23-engine-supervisor-redesign.md`
3. Read `CLAUDE.md` (project root) for conventions
4. Read `PROMPT.md` for detailed implementation guidance per phase
5. Implement the story — follow its acceptance criteria exactly
6. Verify ALL acceptance criteria (run the commands listed, check output)
7. Commit with conventional commit format: `feat(engine): US-XXX — <title>`
8. Update `scripts/ralph/prd.json` — set the completed story's `"passes": true`
9. Also update `tasks/prd.json` to keep it in sync
10. Commit the prd.json update: `chore: mark US-XXX complete in prd.json`
11. Push: `git push`

## Completion Check

After marking a story complete, check if ALL stories have `"passes": true`. If yes:

<promise>COMPLETE</promise>

If not, exit normally. Ralph will call you again for the next story.

## Critical Rules

- ONE story per iteration. Do not attempt multiple.
- Run verification commands from the acceptance criteria BEFORE marking passes:true.
- If a story depends on previous stories' code, it should already exist (stories are ordered by dependency).
- Do NOT modify the design spec or PROMPT.md.
- Do NOT skip acceptance criteria — every checkbox must be verified.
- Use LSP (findReferences, goToDefinition) when tracing symbol usage, not grep.
- EngineClient is Clone (Arc-based). get_client() returns cheap clones.
- Tauri already wraps managed state in Arc — do NOT add redundant Arc inside the handle.
- On Windows, DETACHED_PROCESS means child.wait() doesn't work. Use HTTP health + PID checks.
- Token regeneration on restart is CRITICAL — stale tokens cause "unhealthy after idle".
