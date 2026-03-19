---
name: "claude-codex-handoff"
description: "Prepare a high-signal handoff from Codex to Claude in a dual-agent workflow. Use when Codex has completed scoped work and Claude needs a concise summary of changes, checks, machine-local setup, open questions, and what should be finalized next."
---

# Claude Codex Handoff

Use this skill when a supervising Claude session needs a clean summary of what Codex actually did.

## Goal

Reduce handoff loss. Claude should be able to read one summary and know:

- what was changed
- what was only changed locally on this machine
- what was validated
- what remains open
- what Claude should finalize

## Workflow

1. Read the repository `AGENTS.md` and respect the declared role split.
2. Capture the current branch and note whether the worktree was already dirty before your changes.
3. Separate repository changes from machine-local Codex changes.
4. Record validation commands that were run and whether they passed, failed, or were not run.
5. Summarize unresolved risks, assumptions, and follow-up choices for Claude.

## Required Sections

### Objective

State the specific task Codex handled.

### Repository Changes

List only files Codex changed in the repository and the purpose of each change.

### Machine-Local Changes

List changes outside the repository, such as:

- `~/.codex/AGENTS.md`
- installed or patched user skills
- local Codex configuration changes

Claude cannot infer these from the repo diff, so call them out explicitly.

### Validation

For every check, say one of:

- passed
- failed
- not run

If not run, say why.

### Open Items

Call out anything Claude still needs to decide, review, or merge.

### Next Actions For Claude

End with a short action list Claude can execute directly.

## Guardrails

- Do not blur repository changes and machine-local changes.
- Do not claim a restart happened unless it did.
- If a new skill was installed, note that Codex may need a restart to pick it up reliably.
- If the repository had pre-existing dirty files, say so and avoid claiming ownership of them.

## Output Template

Use this structure when asked for a handoff:

```md
Objective
- ...

Repository changes
- ...

Machine-local changes
- ...

Validation
- ...

Open items
- ...

Next actions for Claude
- ...
```
