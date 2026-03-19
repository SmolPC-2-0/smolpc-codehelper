# Native OpenVINO GenAI Plan Pack

Checked on: 2026-03-19
Scope: Windows x64 only, canonical planning + contract docs, weak Intel laptops are the primary KPI.

This folder is the canonical plan pack for the SmolPC Intel acceleration path.

## Implementation Status

As of 2026-03-19, this branch has completed native runtime activation and Windows archive-based runtime bring-up for the OpenVINO smoke path:

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
- `npm run runtime:setup:openvino` now downloads the official 2026 Windows OpenVINO GenAI archive, verifies its SHA256, validates the `openvino_genai_c.dll` exports, and stages the app-local bundle into `apps/codehelper/src-tauri/libs/openvino`
- `npm run model:setup:qwen25-instruct` stages the supported `qwen2.5-1.5b-instruct` shared model with official OpenVINO IR + DirectML artifacts
- `npm run model:setup:qwen3-4b` stages the supported `qwen3-4b` shared model with official `OpenVINO/Qwen3-4B-int4-ov` IR plus a validated DirectML self-build from `Qwen/Qwen3-4B`
- the `qwen3-4b` DirectML builder path now uses an isolated Python `3.14` venv with pinned official packages:
  - `onnxruntime==1.24.2`
  - `onnxruntime-directml==1.24.2`
  - `onnxruntime-genai==0.12.2`
  - `onnxruntime-genai-directml==0.12.2`
- `apps/codehelper/scripts/setup-qwen3-4b-model.ps1` keeps `self_build` as the supported default DML source mode and exposes `fallback_snapshot` only as an explicit recovery path; the public model id remains `qwen3-4b`
- DirectML export logs are kept under `%LOCALAPPDATA%/SmolPC/logs/dml-export/`
- OpenVINO CPU and OpenVINO NPU now use structured chat history by default; the prompt-string path remains only for explicit legacy ChatML input compatibility
- OpenVINO `qwen3-4b` is currently non-thinking only and follows upstream non-thinking defaults (`temperature=0.7`, `top_p=0.8`, `top_k=20`, `presence_penalty=1.5`)
- the native OpenVINO lane now applies NPU creation defaults that work on this PC:
  - `MAX_PROMPT_LEN=512`
  - `MIN_RESPONSE_LEN=1024`
- those NPU defaults can be overridden for debugging with:
  - `SMOLPC_OPENVINO_NPU_MAX_PROMPT_LEN`
  - `SMOLPC_OPENVINO_NPU_MIN_RESPONSE_LEN`

Still pending for the remaining Phase 1 / Phase 1b work:

- final end-to-end Intel NPU validation inside the full app flow on this machine
- installer-time OpenVINO bundle population
- workload tuning, cache policy, and prompt-default calibration

## Final Decision

- Primary Intel lane: `native OpenVINO GenAI` on `NPU`
- Windows fallback lane: `DirectML + ONNX Runtime GenAI`
- Terminal fallback lane: `CPU`
- Removed from scope: `ORT + OpenVINO EP`

OpenVINO is native GenAI exclusive in this plan. Do not plan or implement an ORT/OpenVINO EP lane unless the plan is explicitly reopened.

## Windows Bring-Up Baseline

Windows native staging uses the official 2026 OpenVINO GenAI archive, not the PyPI wheels. The wheel-based `openvino-genai` package does not expose the `ov_genai_*` C ABI that the Rust adapter calls, while the archive ships `openvino_genai_c.dll` and the native headers needed for this integration.

Primary 2026 references for this repo state:
- OpenVINO GenAI install guide: `https://docs.openvino.ai/2026/get-started/install-openvino/install-openvino-genai.html`
- OpenVINO GenAI on NPU: `https://docs.openvino.ai/2026/openvino-workflow-generative/inference-with-genai/inference-with-genai-on-npu.html`
- Upstream C samples: `https://github.com/openvinotoolkit/openvino.genai/tree/master/samples/c`

## Folder Layout

- `PLAN.md`
  - final architecture, probe flow, packaging posture, rollout phases
- `RESEARCH_SUMMARY_2026-03-09.md`
  - dated volatile facts rechecked against official primary sources
- `RESEARCH_SUMMARY_2026-03-18_ORT_DML.md`
  - dated ORT GenAI / DirectML source-of-truth and validation results for the unified `qwen3-4b` path
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
5. `RESEARCH_SUMMARY_2026-03-18_ORT_DML.md`
6. `RESEARCH_SUMMARY_2026-03-09.md`

## Implementation Planning Workflow

This pack is intentionally structured for short-lived, focused implementation-planning sessions.

Recommended planning boundaries:

1. lane-specific manifests, artifact layout, and default catalog migration
2. exact-parity OpenVINO export and benchmark refresh
3. Intel NPU validation and packaging hardening
4. workload tuning, cache policy, and prompt-default calibration

Each future Codex session should take one workstream or one subsection of a phase, produce an implementation plan for that slice only, and stop before broad execution planning.

Planner and implementer prompts should explicitly require small, frequent checkpoint commits. Do not carry a large dirty worktree across sessions when the work can be split into coherent checkpoints.

The existing benchmark frontend shell stays in scope. The current legacy Ollama benchmark backend is a temporary holdover and should be replaced with runtime-aware benchmarking later, not deleted outright.

## Removed Assumptions

- No ORT-based OpenVINO EP rollout
- No launcher/setup EP ranking policy
- No provider-internal `AUTO`/`HETERO` as product policy
- No machine-global backend choice that ignores version, model, or driver drift
