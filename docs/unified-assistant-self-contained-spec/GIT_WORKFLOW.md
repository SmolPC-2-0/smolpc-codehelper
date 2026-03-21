# Git Workflow For The Self-Contained Unified Assistant Line

**Last Updated:** 2026-03-17
**Status:** Required workflow after the Phase 3 single-mainline transition

## 1. Branch Roles

| Branch                                       | Role                                   |
| -------------------------------------------- | -------------------------------------- |
| `dev/unified-assistant`                      | Frozen demo implementation baseline    |
| `docs/unified-assistant-spec`                | Frozen demo/spec baseline              |
| `dev/unified-assistant-self-contained`       | Sole active self-contained mainline    |
| `docs/unified-assistant-self-contained-spec` | Frozen self-contained archive snapshot |
| `codex/*`                                    | Narrow task branches                   |

## 2. Branch Cut Rule

The branch cut is complete.

Do not branch new self-contained work from:

- `dev/unified-assistant`
- `docs/unified-assistant-spec`

Use those branches only for explicit demo hotfixes.

## 3. Historical Note

Phases 0 through 2 intentionally used a dual-mainline workflow:

- `docs/unified-assistant-self-contained-spec`
- `dev/unified-assistant-self-contained`

That branch structure was useful while the self-contained line was being cut,
documented, and stabilized. The resulting PR stack proved heavier than needed
once the docs tree was already fully in sync with the implementation mainline.

The historical merged PRs and branch names from that period are correct and
should not be rewritten. Phase 3 onward uses the simplified workflow below.

## 4. Docs-First Rule

Every self-contained phase now follows this exact sequence:

1. create `codex/<phase>-docs` from
   `origin/dev/unified-assistant-self-contained`
2. merge the docs-only preflight PR into
   `dev/unified-assistant-self-contained`
3. create `codex/<phase>` from
   updated `origin/dev/unified-assistant-self-contained`
4. merge the implementation PR into
   `dev/unified-assistant-self-contained`
5. create `codex/<phase>-status-docs` from
   updated `origin/dev/unified-assistant-self-contained`
6. merge the closeout docs PR into
   `dev/unified-assistant-self-contained`

No future self-contained PR should target
`docs/unified-assistant-self-contained-spec`.

## 5. Clone Rule

Use separate clones, not worktrees.

Rationale: this repository has repeatedly run into Tauri/Cargo artifact bleed,
branch confusion, and AI-session context mistakes when multiple active
workstreams share one checkout. Separate clones keep the frozen demo line and
the self-contained line physically isolated.

Recommended clone set:

- one active self-contained clone tracking `dev/unified-assistant-self-contained`
- one optional frozen demo clone only if demo hotfixes are needed

Do not do self-contained work from the stale local `main` checkout.

## 6. Merge Policy

- no direct pushes to `dev/unified-assistant-self-contained`
- narrow `codex/*` branch per phase
- merge demo hotfixes forward only when still relevant
- do not merge self-contained work back into the demo line
- no new PRs target `docs/unified-assistant-self-contained-spec`

## 7. Demo Hotfix Rule

Allowed on the frozen demo line:

- issue blocking live demos
- small copy/status fixes needed for demonstration
- no architectural or packaging drift

If a demo hotfix is also relevant to self-contained work:

1. land it on the frozen demo line
2. cherry-pick or re-implement it onto the self-contained line
3. do not reverse-merge the self-contained line into the demo line

## 8. Branch Boundaries

### Docs branches

Allowed:

- spec updates
- roadmap changes
- packaging and provenance documentation
- API contract documentation
- workflow migration or phase-closeout docs that belong on the active mainline

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

## 9. Provenance Rule

Before importing third-party runtime or plugin assets, the docs-only preflight
branch for that phase must update `THIRD_PARTY_PROVENANCE.md` with:

- upstream source location
- exact pinned commit/tag/version
- license
- files to import
- local modifications expected

No third-party asset import should land without provenance recorded first.

## 10. PR Targets

| Branch type    | PR target                              |
| -------------- | -------------------------------------- |
| phase docs     | `dev/unified-assistant-self-contained` |
| implementation | `dev/unified-assistant-self-contained` |
| closeout docs  | `dev/unified-assistant-self-contained` |

## 11. Per-Phase Checklist

Use this checklist for every self-contained phase:

- docs-only branch opened from `origin/dev/unified-assistant-self-contained`
- docs-only branch merged into `dev/unified-assistant-self-contained`
- implementation branch opened from updated `origin/dev/unified-assistant-self-contained`
- implementation branch merged into `dev/unified-assistant-self-contained`
- status-docs branch opened from updated `origin/dev/unified-assistant-self-contained`
- status-docs branch merged into `dev/unified-assistant-self-contained`

## 12. Archived Docs Branch Rule

`docs/unified-assistant-self-contained-spec` remains in the remote as a frozen
reference snapshot at `06d32a5219b69d8182079843c79661aca98ad220`.

Use it only as historical reference for the dual-mainline transition period.
Do not open new PRs against it.

## 13. First Branch In This Line

The first required branch is:

- `codex/unified-self-contained-master-plan-docs`

No implementation branch should open on the self-contained line until that
master-plan docs branch is merged through both new mainlines.
