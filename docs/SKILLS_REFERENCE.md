# Skills Reference

Ad-hoc skill lookup for development workflow. Skills are loaded on-demand from [alirezarezvani/claude-skills](https://github.com/alirezarezvani/claude-skills), never pre-loaded.

**Rule:** One skill per focused session task. Don't preload.

---

## Structural (concepts adopted, always active)

| Concept | Source Skill | What We Use | Where It Lives |
|---|---|---|---|
| Scoped rules | `self-improving-agent` | `.claude/rules/` files with path matchers | `.claude/rules/*.md` |
| Promotion lifecycle | `self-improving-agent` | Memory → CLAUDE.md → rules pipeline | `docs/AI_WORKFLOW.md` |
| Error capture | `self-improving-agent` | PostToolUse hook on Bash errors | `.claude/scripts/error-capture.sh` |
| Branch isolation | `git-worktree-manager` | One agent per branch, port offsets | `docs/AI_WORKFLOW.md` |

---

## Phase-Triggered (load ad-hoc when needed)

### `pr-review-expert`

**When:** Before merging any significant branch to main.
**Path:** `engineering/pr-review-expert/SKILL.md`
**What to use:** Blast radius checklist, security scan, breaking change detection, test coverage delta.
**How to load:**
```
Fetch and follow: https://github.com/alirezarezvani/claude-skills/blob/main/engineering/pr-review-expert/SKILL.md
```

### `release-manager`

**When:** Cutting a handoff release.
**Path:** `engineering/release-manager/SKILL.md`
**What to use:** Semantic version bump, changelog generation from conventional commits, release readiness checklist.
**Scripts:** `changelog_generator.py`, `version_bumper.py`, `release_planner.py`

### `codebase-onboarding`

**When:** Generating handoff documentation.
**Path:** `engineering/codebase-onboarding/SKILL.md`
**What to use:** Repo analysis, onboarding template, audience-tiered docs.

### `dependency-auditor`

**When:** Pre-release security sweep.
**Path:** `engineering/dependency-auditor/SKILL.md`
**What to use:** Cross-ecosystem scan (Cargo.toml + package.json), CVE matching, license compliance.

---

## Evaluated and Skipped

| Skill | Reason |
|---|---|
| `senior-architect` | Architecture is mature. Low ROI at this stage. |
| `tech-debt-tracker` | Already tracked via `codex/WORKING_ISSUES.md`. |
| `senior-secops` | Fully offline app, no cloud surface. Overkill. |
| `monorepo-navigator` | Designed for Turborepo/Nx, not Cargo workspaces. |
| `tdd-guide` | Rust testing is native. Only relevant for Svelte frontend tests. |
| `changelog-generator` | Subset of `release-manager`. Redundant. |
| `runbook-generator` | Lower priority than release and onboarding docs. |

---

## Promotion Scoring (from self-improving-agent)

When deciding whether a learning should move from memory to CLAUDE.md or `.claude/rules/`:

| Dimension | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| Durability | One-time fix | Temp workaround | Stable pattern | Architectural truth |
| Impact | Nice-to-know | Saves 1 minute | Prevents mistakes | Prevents breakage |
| Scope | One file | One directory | Entire project | All projects |

**Promote when total >= 6.** Target: CLAUDE.md for project-wide, `.claude/rules/` for path-scoped.
