# Native OpenVINO GenAI Plan Pack

Checked on: 2026-03-12
Scope: Windows x64 only, canonical planning + contract docs, weak Intel laptops are the primary KPI.

This folder is the canonical plan pack for the SmolPC Intel acceleration path.

## Implementation Status

As of 2026-03-12, this branch has completed the first native runtime-activation slice:

- selection persistence is keyed by full fingerprint and keeps multiple records per model
- `GET /engine/status` is lane-based instead of DML-only
- `POST /engine/check-model` reports readiness by lane instead of a single boolean
- `openvino_npu/manifest.json` inspection and artifact validation are implemented
- an async OpenVINO startup probe classifies hardware, device visibility, driver version, and startup failure class
- a native OpenVINO GenAI runtime adapter is implemented in `engine-core`
- model load now runs real OpenVINO compile plus first-token preflight under the `30 seconds` budget
- successful OpenVINO preflight now activates `runtime_engine=ov_genai_npu`
- automatic selection now prefers `openvino_npu -> directml -> cpu` when the OpenVINO lane is viable
- the selection fingerprint now uses the `openvino_native_v1` profile so stale pre-activation records do not block rollout

Still pending for the remaining Phase 1 / Phase 1b work:

- lane-specific manifest rollout and default catalog migration away from `qwen3-4b-instruct-2507`
- app-local/runtime-bundle population for real Windows validation and packaging
- workload tuning, cache policy, and prompt-default calibration

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

1. lane-specific manifests, artifact layout, and default catalog migration
2. app-local runtime-bundle staging and Intel NPU validation
3. workload tuning, cache policy, and prompt-default calibration
4. benchmark refresh so users can compare inference/runtime choices on their own machine

Each future Codex session should take one workstream or one subsection of a phase, produce an implementation plan for that slice only, and stop before broad execution planning.

Planner and implementer prompts should explicitly require small, frequent checkpoint commits. Do not carry a large dirty worktree across sessions when the work can be split into coherent checkpoints.

The existing benchmark frontend shell stays in scope. The current legacy Ollama benchmark backend is a temporary holdover and should be replaced with runtime-aware benchmarking later, not deleted outright.

## Removed Assumptions

- No ORT-based OpenVINO EP rollout
- No launcher/setup EP ranking policy
- No provider-internal `AUTO`/`HETERO` as product policy
- No machine-global backend choice that ignores version, model, or driver drift
