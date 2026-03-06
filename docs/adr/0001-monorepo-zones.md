# ADR 0001: Three-Zone Monorepo Structure

- Status: Accepted
- Date: 2026-03-06

## Decision

Adopt a single monorepo with strict zones:

- `engine/`
- `launcher/`
- `apps/`

CodeHelper is moved to `apps/codehelper`, and engine crates are moved to `engine/crates`.

## Rationale

- Enables parallel development without coupling app logic to engine internals.
- Preserves shared CI/workspace tooling while improving ownership clarity.
- Makes future extraction into separate repos possible without blocking current velocity.

## Consequences

- Path references and tooling configs must use new zone paths.
- Boundary checks are required to prevent reintroduction of app-owned inference logic.
- Historical docs may require gradual path cleanup; architecture docs define source of truth.
