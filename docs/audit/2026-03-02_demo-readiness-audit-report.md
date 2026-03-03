# Demo Readiness Audit Report

Date: 2026-03-02  
Scope baseline: `origin/codex/shared-engine-v1` (audited from child branch `codex/demo-audit-ci-hardening`)  
Platform focus: Windows (demo target)

## Executive Verdict

Current state is close to demo-ready, but not clean enough for a strict "no sloppy code" standard without follow-up fixes.

Go/No-Go recommendation:

1. `Go` for functional demo if the team avoids legacy Ollama benchmark paths.
2. `No-Go` for "highest-standard merge" until the blocking findings below are resolved.

## Evidence Baseline

Commands run locally during audit:

1. `cargo test --workspace` -> pass.
2. `cargo check -p smolpc-code-helper` -> pass.
3. `npm run check` -> pass with one Svelte accessibility warning.
4. `npm audit --audit-level=high` -> pass (0 vulnerabilities).
5. `cargo clippy --workspace --all-targets -- -D warnings` -> fail (style and API-shape lint debt in engine-core).
6. `cargo fmt --all -- --check` -> fail (format drift in current baseline).
7. CI status for PR checks on base branch prior to this child-branch work -> pass on Windows.

## Blocking Findings (Functional/Security/Reliability)

### HIGH-01: Legacy Ollama command surface is still registered in the Tauri app

Evidence:

1. `src-tauri/src/commands/mod.rs:6` still exports `ollama`.
2. `src-tauri/src/lib.rs:14-17` imports Ollama command/state types.
3. `src-tauri/src/lib.rs:34-37` manages Ollama state objects.
4. `src-tauri/src/lib.rs:43-46` registers Ollama commands in invoke handler.
5. `src-tauri/src/commands/benchmark.rs:3,12-14` benchmark command still depends on Ollama client/config state.

Risk:

1. Two inference paths remain in shipped app surface (shared engine + Ollama legacy).
2. Demo behavior can become inconsistent if benchmark or legacy command paths are triggered.
3. Increases support/debug complexity during live demo and for external reviewers.

Recommendation:

1. Remove/disable Ollama command registration and Ollama-backed benchmark path for this branch, or explicitly gate/hide benchmark UI for demo.

### HIGH-02: `main` branch is not protected

Evidence:

1. GitHub API query for branch protection returned `404 Branch not protected`.

Risk:

1. Any user with push rights can bypass checks and merge unreviewed or failing code.
2. Undermines all CI investment and weakens demo confidence narrative.

Recommendation:

1. Apply branch protection immediately after demo (as requested policy), with required checks listed in CI standard doc.

## Significant Non-Blocking Findings

### MEDIUM-01: Quality debt is large enough to hide regressions

Evidence:

1. `cargo clippy -D warnings` fails on many warnings/errors (examples include `generator.rs`, `input_builder.rs`, `kv_cache.rs`, `runtime_adapter.rs` in `crates/smolpc-engine-core/src/inference/`).
2. `cargo fmt --all -- --check` fails on current baseline.
3. `npm run lint` fails due broad historical formatting drift.

Risk:

1. Useful static checks are not currently enforceable repo-wide.
2. Review signal/noise ratio is poor; real regressions are easier to miss.

Recommendation:

1. Enforce incremental strict checks on changed files now (implemented in this child branch).
2. Burn down full-repo lint debt in a dedicated stabilization pass.

### MEDIUM-02: Core logic is duplicated and divergent between `crates/` and `src-tauri/src/`

Evidence:

1. Duplicate relative-path modules detected across both trees.
2. 11 duplicate files differ in content (including `inference/generator.rs`, `inference/kv_cache.rs`, `models/registry.rs`, `hardware/detector.rs`).

Risk:

1. Behavior drift and maintenance errors.
2. Fixes can land in one copy and be absent in another.

Recommendation:

1. Consolidate to single source of truth (prefer shared engine crates), then remove or hard-deprecate duplicate modules.

### LOW-01: Accessibility warning in conversation view

Evidence:

1. `src/lib/components/chat/ConversationView.svelte:67-73` has touch handlers on a `<div>` without ARIA role.

Risk:

1. Accessibility lint warning persists in demo and CI logs.

Recommendation:

1. Add appropriate ARIA role or refactor interaction target to semantic element.

## Demo-Critical Scenario Matrix

1. Shared engine model load/generation path: pass in local checks.
2. Runtime switch and backend status UI: pass in current manual flow.
3. CI required tests (base branch): pass on Windows.
4. Legacy path isolation: fail (Ollama path still wired).
5. Strict static quality gate readiness: fail (full clippy/fmt/lint debt).

## Immediate Actions Before Merge

1. Keep this child-branch CI hardening PR and merge it into `codex/shared-engine-v1`.
2. Remove or gate legacy Ollama command surface.
3. Fix conversation accessibility warning.
4. Decide whether to accept clippy/fmt debt as tracked non-blocking backlog or begin cleanup immediately.
