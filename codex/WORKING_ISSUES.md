# Codex Working Issues

Last updated: 2026-02-09
Base branch for stacked work: `fix/stop-token-chatml`

## Scope
- Backend focus: `src-tauri/src/inference`, `src-tauri/src/models`
- Goal: fix runtime safety and modular model support issues incrementally using child branches

## Issue Tracking

1. Issue: ONNX output access and tensor shape safety in inference generator
Status: Completed in child branch
Branch: `fix/inference-safe-output-shapes`
PR: https://github.com/SmolPC-2-0/smolpc-codehelper/pull/26
Notes:
- Replaced panic-prone output indexing with required-output checks
- Added strict rank/dimension/data-length validation
- Added focused unit tests for validation helpers

2. Issue: Model runtime spec and architecture contract (1.5B-first)
Status: Completed in child branch
Branch: `fix/inference-runtime-spec-per-model`
PR: https://github.com/SmolPC-2-0/smolpc-codehelper/pull/27
Notes:
- Added runtime spec abstraction (`ModelRuntimeSpec`) for model architecture + I/O naming + stop tokens
- Integrated runtime spec validation into model load path
- Updated generator/input builder to be runtime-spec-driven
- Removed 7B from visible model list until supported
- Added guardrail tests for 1.5B-only registry/runtime mapping

3. Issue: Single-flight generation and cancellation scoping
Status: Pending
Planned branch: `fix/inference-single-flight-cancel-scope`

4. Issue: Deterministic model path resolution (remove CWD dependency)
Status: Pending
Planned branch: `fix/models-path-resolution`

5. Issue: InputBuilder allocation and key handling improvements
Status: Pending
Planned branch: `fix/inference-inputbuilder-allocs`

## Implementation Rules
- Keep each issue isolated in its own child branch and PR
- Base each child PR on `fix/stop-token-chatml`
- Exclude unrelated local changes:
  - `.claude/settings.local.json`
  - `package-lock.json`
- Run at least:
  - `cargo check`
  - targeted tests for touched modules
