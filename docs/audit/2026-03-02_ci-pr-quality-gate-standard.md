# CI / PR Quality Gate Standard

Date: 2026-03-02  
Applies to: `codex/shared-engine-v1` and descendants  
Goal: prevent regressions and sloppy changes while allowing incremental cleanup of historical debt.

## Required CI Checks (Immediate)

From `.github/workflows/ci.yml`:

1. `Frontend Quality`
2. `Engine Tests (Rust 1.88.0)`
3. `Engine Tests (stable)`
4. `Tauri Build Check`
5. `Incremental Style Gates`
6. `Rust Security Audit`

## Gate Definitions

### Frontend Quality

Checks:

1. `npm ci`
2. `npm run check`
3. `npm audit --audit-level=high`

Pass criteria:

1. No TypeScript/Svelte check errors.
2. No high/critical npm vulnerabilities.

### Engine and Build Jobs

Checks:

1. Engine tests on Rust `1.88.0`.
2. Engine tests on Rust `stable`.
3. Tauri crate compile check on Windows.

Pass criteria:

1. All tests compile and pass.
2. App crate compiles without errors.

### Incremental Style Gates

Policy:

1. Strict style/lint checks run only on changed files in the PR/push diff.
2. Full-repo style debt is tracked separately and does not block unrelated changes.

Checks:

1. Prettier check on changed web/config/doc files.
2. ESLint with `--max-warnings 0` on changed JS/TS/Svelte files.
3. `rustfmt --check` on changed Rust files.

Pass criteria:

1. No formatting or lint violations in changed files.

### Rust Security Audit

Checks:

1. Install `cargo-audit`.
2. Run `cargo audit`.

Pass criteria:

1. No unresolved Rust dependency advisories at audit failure threshold.

## Non-Goals of Immediate Gate

1. Full-repo `cargo clippy -D warnings` enforcement (currently too much historical debt).
2. Full-repo `npm run lint` enforcement (format drift across many files).

These remain backlog work and should be introduced once debt is reduced.

## Branch Protection Policy (Post-Demo)

Target: protect `main` immediately after demo.

Required settings:

1. Require pull request before merge.
2. Require at least 1 approval review.
3. Require status checks listed above.
4. Dismiss stale approvals on new commits.
5. Restrict direct pushes to `main`.

## Change Management Rules

1. New CI checks can be added only with clear failure messages and deterministic behavior.
2. Any intentionally skipped gate must be documented in PR description with expiration date.
3. `claude-review` is advisory unless explicitly promoted to required.

## Maintenance Cadence

1. Weekly: review failing trends and flaky checks.
2. Before release tags: ensure all required checks pass on the tagged commit.
3. Monthly: reassess readiness to promote full-repo clippy/lint enforcement.
