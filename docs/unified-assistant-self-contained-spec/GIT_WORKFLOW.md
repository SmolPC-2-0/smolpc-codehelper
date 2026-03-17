# Git Workflow For The Self-Contained Unified Assistant Line

**Last Updated:** 2026-03-17
**Status:** Required workflow after the demo freeze

## 1. Branch Roles

| Branch                                       | Role                                   |
| -------------------------------------------- | -------------------------------------- |
| `dev/unified-assistant`                      | Frozen demo implementation baseline    |
| `docs/unified-assistant-spec`                | Frozen demo/spec baseline              |
| `dev/unified-assistant-self-contained`       | Self-contained implementation mainline |
| `docs/unified-assistant-self-contained-spec` | Self-contained canonical spec branch   |
| `codex/*`                                    | Narrow task branches                   |

## 2. Branch Cut Rule

The branch cut is complete.

Do not branch new self-contained work from:

- `dev/unified-assistant`
- `docs/unified-assistant-spec`

Use those branches only for explicit demo hotfixes.

## 3. Docs-First Rule

Every self-contained phase follows this exact sequence:

1. create `codex/<phase>-docs` from
   `origin/docs/unified-assistant-self-contained-spec`
2. merge into `docs/unified-assistant-self-contained-spec`
3. create `codex/<phase>-docs-sync` from
   `origin/dev/unified-assistant-self-contained`
4. merge updated `docs/unified-assistant-self-contained-spec` into
   `codex/<phase>-docs-sync`
5. merge `codex/<phase>-docs-sync` into
   `dev/unified-assistant-self-contained`
6. create `codex/<phase>` from
   `origin/dev/unified-assistant-self-contained`
7. merge into `dev/unified-assistant-self-contained`
8. create `codex/<phase>-status-docs` from
   `origin/docs/unified-assistant-self-contained-spec`
9. merge into `docs/unified-assistant-self-contained-spec`
10. create `codex/<phase>-status-sync` from
    `origin/dev/unified-assistant-self-contained`
11. merge updated `docs/unified-assistant-self-contained-spec` into
    `codex/<phase>-status-sync`
12. merge `codex/<phase>-status-sync` into
    `dev/unified-assistant-self-contained`

## 4. Clone Rule

Use separate clones, not worktrees.

Rationale: this repository has repeatedly run into Tauri/Cargo artifact bleed,
branch confusion, and AI-session context mistakes when multiple active
workstreams share one checkout. Separate clones keep the frozen demo line and
the self-contained line physically isolated.

Recommended clone set:

- one docs clone tracking `docs/unified-assistant-self-contained-spec`
- one implementation clone tracking `dev/unified-assistant-self-contained`

Do not do self-contained work from the stale local `main` checkout.

## 5. Merge Policy

- no direct pushes to either self-contained mainline
- narrow `codex/*` branch per phase
- use merge-only sync branches for docs-to-dev alignment
- merge demo hotfixes forward only when still relevant
- do not merge self-contained work back into the demo line

## 6. Demo Hotfix Rule

Allowed on the frozen demo line:

- issue blocking live demos
- small copy/status fixes needed for demonstration
- no architectural or packaging drift

If a demo hotfix is also relevant to self-contained work:

1. land it on the frozen demo line
2. cherry-pick or re-implement it onto the self-contained line
3. do not reverse-merge the self-contained line into the demo line

## 7. Branch Boundaries

### Docs branches

Allowed:

- spec updates
- roadmap changes
- packaging and provenance documentation
- API contract documentation

Avoid:

- code changes

### Foundation / provisioning branches

Allowed:

- setup subsystem
- host-app detection
- provisioning manifests
- bundled runtime ownership

Avoid:

- mode-surface expansion unrelated to self-contained ownership

### Mode-specific self-contained branches

Each mode branch should change only:

- provider-owned runtime/provisioning logic for that mode
- the smallest necessary shell/setup integration points
- bundled provider assets for that mode

Avoid:

- rewriting standalone apps wholesale
- unrelated engine redesign

## 8. Provenance Rule

Before importing third-party runtime or plugin assets, the docs branch for that
phase must update `THIRD_PARTY_PROVENANCE.md` with:

- upstream source location
- exact pinned commit/tag/version
- license
- files to import
- local modifications expected

No third-party asset import should land without provenance recorded first.

## 9. PR Targets

| Branch type    | PR target                                    |
| -------------- | -------------------------------------------- |
| phase docs     | `docs/unified-assistant-self-contained-spec` |
| docs sync      | `dev/unified-assistant-self-contained`       |
| implementation | `dev/unified-assistant-self-contained`       |
| closeout docs  | `docs/unified-assistant-self-contained-spec` |
| status sync    | `dev/unified-assistant-self-contained`       |

## 10. Per-Phase Checklist

Use this checklist for every self-contained phase:

- docs branch opened from `origin/docs/unified-assistant-self-contained-spec`
- docs branch merged into `docs/unified-assistant-self-contained-spec`
- docs-sync branch opened from `origin/dev/unified-assistant-self-contained`
- docs-sync branch merged into `dev/unified-assistant-self-contained`
- implementation branch opened from updated `origin/dev/unified-assistant-self-contained`
- implementation branch merged into `dev/unified-assistant-self-contained`
- status-docs branch opened from updated `origin/docs/unified-assistant-self-contained-spec`
- status-docs branch merged into `docs/unified-assistant-self-contained-spec`
- status-sync branch opened from updated `origin/dev/unified-assistant-self-contained`
- status-sync branch merged into `dev/unified-assistant-self-contained`

## 11. First Branch In This Line

The first required branch is:

- `codex/unified-self-contained-master-plan-docs`

No implementation branch should open on the self-contained line until that
master-plan docs branch is merged through both new mainlines.
