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
Status: In progress
Branch: `fix/inference-runtime-spec-per-model`
Planned outcomes:
- Introduce model runtime spec for inference/model contract
- Keep code modular for future model variants
- Remove 7B model from list until supported
- Fail fast on unsupported model runtime specs

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
