# Fix Backlog and Execution Plan

Date: 2026-03-02  
Priority model: functional/security first, then maintainability and hygiene.

## Priority 0: Immediate (Before Merging Demo PR Stack)

### P0-1 Remove legacy Ollama runtime surface from app command layer

Scope:

1. Stop exporting/using `commands::ollama` in active Tauri command registration.
2. Decouple benchmark command from Ollama state path.
3. Ensure frontend cannot invoke stale Ollama commands.

Files:

1. `src-tauri/src/commands/mod.rs`
2. `src-tauri/src/lib.rs`
3. `src-tauri/src/commands/benchmark.rs`
4. Any additional command/state wiring impacted by removal

Acceptance criteria:

1. No `commands::ollama` references in active invoke handler path.
2. App starts and inference flows work with shared engine only.
3. Benchmark feature is either shared-engine compatible or intentionally disabled with clear user messaging.

### P0-2 Fix accessibility warning in conversation container

Scope:

1. Resolve touch-interaction ARIA warning in conversation view.

File:

1. `src/lib/components/chat/ConversationView.svelte`

Acceptance criteria:

1. `npm run check` reports zero warnings/errors for this component.

### P0-3 Merge CI hardening child PR into base branch

Scope:

1. Adopt stricter required checks implemented in `.github/workflows/ci.yml`.

Acceptance criteria:

1. New checks run and pass on target branch.
2. Team agrees these check names become required status checks post-demo.

## Priority 1: Stability and Architecture Integrity

### P1-1 Resolve duplicated inference/core module drift

Problem:

1. Duplicated modules exist under both `crates/smolpc-engine-core/src` and `src-tauri/src`, with content drift in multiple files.

Execution:

1. Inventory each duplicate and identify runtime ownership.
2. Move runtime authority to shared engine crates.
3. Remove stale duplicates or keep thin wrappers with explicit forwarding.

Acceptance criteria:

1. No divergent duplicate source files for core inference/hardware/model logic.
2. Ownership map is documented.

### P1-2 Convert strict clippy from failing aspirational check to enforceable gate

Execution:

1. Triage current clippy findings into:
   - mechanical format modernizations,
   - API-shape improvements (`len_without_is_empty`, `err_expect`),
   - performance/sizing advisories (`large_enum_variant` decision).
2. Fix high-value lints first; allowlist consciously deferred items with justification.

Acceptance criteria:

1. `cargo clippy --workspace --all-targets -- -D warnings` either passes or has a documented minimal exception list.

### P1-3 Full-repo formatting stabilization

Execution:

1. Run formatting campaign in dedicated branch.
2. Separate pure formatting commits from behavior changes.

Acceptance criteria:

1. `cargo fmt --all -- --check` passes.
2. `npm run lint` passes or has documented scoped exceptions.

## Priority 2: Test Coverage Hardening

### P2-1 Add frontend tests for demo-critical flows

Scope:

1. Message send/stream/cancel behavior.
2. Runtime/model selection panel behavior.
3. Backend status rendering consistency.

Acceptance criteria:

1. New deterministic tests cover critical UI flow regressions.

### P2-2 Reduce dependency on ignored model-asset tests in CI confidence narrative

Scope:

1. Keep ignored integration tests, but add lightweight deterministic unit tests around key behavior where possible.

Acceptance criteria:

1. Core behavior confidence not dependent on manual local model assets.

## Rollout Sequence

1. Merge CI hardening PR (this child branch).
2. Execute P0-1 and P0-2 as next PR.
3. Run demo on cleaned branch.
4. Post-demo: enable branch protection and execute P1/P2 workstream.

## Risk Controls

1. Keep each backlog item in separate PR with explicit acceptance criteria.
2. Avoid mixing architecture refactors with behavioral fixes in one change set.
3. Keep CI signal deterministic; no flaky/non-deterministic gates.
