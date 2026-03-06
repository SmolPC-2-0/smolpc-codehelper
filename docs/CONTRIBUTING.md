# Contributing and Git Practices

## Branching

- Use short-lived branches with one clear scope.
- Prefix branch names by zone when possible:
  - `engine/...`
  - `launcher/...`
  - `apps/codehelper/...`

## Commits

- Use scoped commit messages:
  - `feat(engine): ...`
  - `fix(launcher): ...`
  - `refactor(codehelper): ...`
- Keep commits reviewable and revertable.

## Pull Requests

- Keep PRs small and zone-focused.
- Include boundary impact, test evidence, and docs updates.
- If contract changes affect multiple zones, land engine contract changes first, then consumers.

## Merge Conflict Control

- Rebase/merge active branches frequently.
- Avoid concurrent edits to the same contract types across zones.
- Route shared contract changes through a designated integration branch/PR.

## Required Checks

Run before opening PR:

```bash
npm run check
npm run boundary:check
cargo test -p smolpc-engine-core -p smolpc-engine-client -p smolpc-engine-host
cargo check -p smolpc-code-helper
```

## Documentation Quality Bar

Every behavior change must update docs in the same PR:

1. zone README
2. API/contract docs when interface changes
3. onboarding docs if integration flow changed
