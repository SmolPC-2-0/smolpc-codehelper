# Native OpenVINO GenAI Plan Pack

Checked on: 2026-03-09
Scope: Windows x64 only, planning/docs only, weak Intel laptops are the primary KPI.

This folder is the canonical plan pack for the SmolPC Intel acceleration path.

## Final Decision

- Primary Intel lane: `native OpenVINO GenAI` on `NPU`
- Windows fallback lane: `DirectML + ONNX Runtime GenAI`
- Terminal fallback lane: `CPU`
- Removed from scope: `ORT + OpenVINO EP`

OpenVINO is native GenAI exclusive in this plan. Do not plan or implement an ORT/OpenVINO EP lane unless the plan is explicitly reopened.

## Folder Layout

- `PLAN.md`
  - final architecture, probe flow, packaging posture, rollout phases
- `RESEARCH_SUMMARY_2026-03-09.md`
  - dated volatile facts rechecked against official primary sources
- `MODEL_STRATEGY.md`
  - bring-up model, backup model, export rules, artifact layout
- `ENGINE_SURFACE_TARGET.md`
  - target engine status and model-readiness contract for the native OpenVINO rollout
- `REPO_CONTEXT.md`
  - current repo seams that must change and the boundaries that must not
- `OFFICIAL_DOCS_INDEX.md`
  - primary-source map for future implementation sessions

## Review Order

1. `PLAN.md`
2. `MODEL_STRATEGY.md`
3. `ENGINE_SURFACE_TARGET.md`
4. `REPO_CONTEXT.md`
5. `RESEARCH_SUMMARY_2026-03-09.md`

## Implementation Planning Workflow

This pack is intentionally structured for short-lived, focused implementation-planning sessions.

Recommended planning boundaries:

1. runtime loading and lane isolation
2. probe, preflight, timeout, and fallback semantics
3. persistence fingerprint and cache invalidation
4. engine status and `/engine/check-model` contract
5. model manifests, artifact layout, and default catalog migration
6. native OpenVINO runtime adapter implementation

Each future Codex session should take one workstream or one subsection of a phase, produce an implementation plan for that slice only, and stop before broad execution planning.

## Removed Assumptions

- No ORT-based OpenVINO EP rollout
- No launcher/setup EP ranking policy
- No provider-internal `AUTO`/`HETERO` as product policy
- No machine-global backend choice that ignores version, model, or driver drift
