# Native OpenVINO GenAI Plan

Checked on: 2026-03-10

## Scope

- Windows x64 only
- weak Intel laptops are the primary KPI
- backend-specific model variants are acceptable
- the actual shipping model is not fixed yet
- packaging details matter only as far as runtime viability and end-user reliability

## Final Architecture Decision

- Primary lane: `native OpenVINO GenAI` on `NPU`
- Fallback lane: `DirectML + ONNX Runtime GenAI`
- Terminal fallback: `ORT CPU`
- Removed path: `ORT + OpenVINO EP`

OpenVINO is a dedicated native runtime lane, not another ORT execution provider.

## Current Branch Baseline

Implemented on this branch as of 2026-03-10:

- backend persistence is now keyed by a full selection fingerprint and stored in `backend_decisions.v2.json`
- multiple persisted records for the same model are retained when the fingerprint differs
- `GET /engine/status` now exposes lane-based readiness for `openvino_npu`, `directml`, and `cpu`
- `POST /engine/check-model` now returns readiness by lane instead of `{ "exists": bool }`
- `decision_persistence_state` now distinguishes `persisted` from `temporary_fallback`
- OpenVINO lane artifacts are validated via `openvino_npu/manifest.json`
- `engine-host` now launches a dedicated machine-scoped OpenVINO startup probe and reports truthful OpenVINO device/driver diagnostics into lane status
- model load now applies the `30 seconds` OpenVINO preflight budget and temporary-fallback semantics before falling through to `directml` or `cpu`

Still pending for the remaining Phase 1 work:

- native `openvino_npu` runtime adapter and `runtime_engine=ov_genai_npu`
- successful OpenVINO compile and first-token preflight
- automatic live selection order `openvino_npu -> directml -> cpu`
- lane-specific manifest rollout and default catalog migration away from `qwen3-4b-instruct-2507`

## Responsibilities

- `engine-host`
  - final backend selector
  - startup probe owner
  - preflight owner
  - fallback/demotion owner
  - telemetry and diagnostics owner
- launcher/setup/apps
  - consume shared runtime status only
  - may cache user-facing inference mode
  - must not rank backends or override engine policy by default

## Final Runtime Shape

```text
launcher / setup / apps
  -> engine-host
     -> ov_genai_npu
     -> genai_dml
     -> ort_cpu
```

`engine-host` owns policy across all three lanes. Runtime engines are allowed to differ behind that boundary.

## Backend Selection Policy

Selection order:

1. `openvino_npu`
2. `directml`
3. `cpu`

Current branch behavior:

- automatic selection still resolves to `directml -> cpu`
- OpenVINO is probed, validated, and can drive `temporary_fallback` status semantics now
- successful native OpenVINO activation still cannot happen because the runtime adapter is not implemented yet

Rules:

- first run for a given model performs startup probe plus model-specific preflight
- Phase 1 is capability-first, not benchmark-first
- if `openvino_npu` completes preflight successfully inside the preflight budget, select it
- if `openvino_npu` does not complete preflight in time, fall back for the current load only and do not persist the fallback as a final winner
- later runs reuse a previously persisted eligible lane immediately
- persisted eligibility is keyed per full selection fingerprint, not one record per model
- full re-evaluation only happens when a material input changes or the cached lane fails

## Startup Probe And First-Run Preflight

### Capability probe

`engine-host` should gather:

- current hardware profile via existing hardware detection
- best DirectML candidate and driver version
- Intel NPU presence
- OpenVINO runtime bundle presence
- OpenVINO available devices
- OpenVINO NPU driver version when available
- model artifact availability for each lane

The startup probe is machine-scoped only. It must not persist a negative or fallback backend decision for a specific model.

### Native OpenVINO preflight

The OpenVINO lane is only considered viable if all of the following pass:

- bundled OpenVINO GenAI/runtime/tokenizers tuple is present
- required plugins are present
- model artifact for `openvino_npu/` is complete
- OpenVINO reports `NPU` as available
- compile smoke test succeeds
- first-token smoke test succeeds

### Preflight timeout

- preflight budget for native OpenVINO on first load: `30 seconds`
- if preflight completes successfully inside the budget, `openvino_npu` is eligible and may be persisted
- if preflight times out, current load falls through to `directml` or `cpu`
- a timeout result is recorded as `temporary_fallback`, not as a persisted negative decision
- an incomplete or timed-out preflight must not overwrite an existing successful persisted OpenVINO record

### Phase 1 selection rule

If both `openvino_npu` and `directml` are viable, Phase 1 still selects `openvino_npu` first after successful preflight.

Head-to-head performance calibration is deferred. It may be added later, but it is not part of the initial native OpenVINO rollout.

## Selection Fingerprint And Persistence

Persist per:

- `model_id`
- model artifact version or hash
- app version
- runtime engine id
- OpenVINO version
- OpenVINO GenAI version
- OpenVINO Tokenizers version
- ORT runtime version
- NPU driver version
- GPU adapter identity and driver version
- machine adapter identity
- OpenVINO pipeline configuration that materially affects performance or compilation:
  - `MAX_PROMPT_LEN` bucket
  - `MIN_RESPONSE_LEN` bucket
  - `PREFILL_HINT`
  - `GENERATE_HINT`
  - cache mode / cache path policy

Persistence rules:

- keep multiple records by full fingerprint
- do not prune to one record per model
- persist only completed preflight outcomes
- timeout or incomplete probe results may be logged, but must not become the final persisted winner

Implemented baseline on this branch as of 2026-03-10:

- current fingerprint fields include model id, computed model artifact fingerprint, app version, selector engine id, ORT/OpenVINO version metadata, runtime bundle fingerprints, GPU identity/driver/device id, and future-ready NPU fields
- store records now separate `persisted_decision` from `failure_counters`
- `persisted_decision` may be `null` so temporary fallbacks can update counters without overwriting a prior good persisted record
- repeated DirectML failures still demote to persisted CPU after the existing threshold; single-load fallbacks stay `temporary_fallback`

## Failure Handling

### Expected behavior

- missing or unusable OpenVINO NPU must not break the app
- the engine ignores that lane and falls through to `directml` or `cpu`
- runtime failure after selection demotes the failing lane and retries on the next lane

### Diagnostic classes to expose

- `no_npu_hardware`
- `openvino_bundle_missing`
- `openvino_npu_plugin_unavailable`
- `openvino_npu_driver_missing`
- `openvino_npu_driver_unknown`
- `openvino_npu_driver_recommended_update`
- `openvino_npu_driver_unusable`
- `openvino_npu_preflight_timeout`
- `openvino_npu_compile_failed`
- `openvino_npu_runtime_failed`
- `directml_candidate_missing`
- `directml_runtime_failed`

Popup guidance:

- no popup when no Intel NPU hardware exists
- actionable popup when Intel NPU hardware exists but the driver is missing or unusable
- soft update guidance when the driver is below the current recommended troubleshooting floor
- internal error surface when the bundled OpenVINO runtime itself is incomplete

## Packaging Posture

- bundle OpenVINO app-local with the launcher/app suite
- do not require users to install OpenVINO separately
- do not rely on a user-installed OpenVINO toolkit
- do not ship precompiled NPU blobs as the primary installer strategy
- compile and cache on the user machine
- production builds must resolve ORT and OpenVINO from app-local absolute paths only
- production builds must not resolve ORT or OpenVINO from `PATH`, bare filenames, or a user-installed toolkit
- fallback lanes are lazy-initialized; OpenVINO failure must not block ORT fallback lanes, and ORT failure must not block native OpenVINO bring-up

Minimum OpenVINO bundle expectation:

- `openvino`
- `openvino_c` when using the C ABI bridge
- `openvino_intel_npu_plugin`
- `openvino_intel_cpu_plugin`
- `openvino_ir_frontend`
- OpenVINO GenAI native library
- OpenVINO Tokenizers native library
- TBB dependencies
- Visual C++ redistributable prerequisite on Windows

## Model Artifact Layout

Model directories should become lane-specific:

```text
models/<model_id>/
  cpu/
    manifest.json
  dml/
    manifest.json
  openvino_npu/
    manifest.json
```

Manifest rules:

- each lane owns its own manifest and referenced assets
- tokenizer and generation-config references are lane-local through the manifest
- there is no shared root tokenizer contract in the final design
- the OpenVINO lane should prefer OpenVINO IR artifacts, not ONNX

## Engine Status And Model Readiness Contract

Phase 0 had to define a per-lane readiness model before workstream 2 planning could proceed.

Implemented baseline on this branch as of 2026-03-10:

- `GET /engine/status` reports lane readiness and the selected runtime lane
- `POST /engine/check-model` reports readiness by lane, not a single boolean
- readiness must cover:
  - runtime bundle integrity
  - artifact completeness
  - startup probe result
  - preflight result
  - persisted-eligibility state
  - last failure class
- OpenVINO lane startup probe truth is now surfaced in status (`detected`, `device_name`, `driver_version`, `startup_probe_state`, `last_failure_class`)
- `POST /engine/check-model` can already report OpenVINO startup-probe failures and `runtime_unavailable`

Still pending for the remaining Phase 1 work:

- `openvino_npu.ready=true` after a real native OpenVINO preflight
- truthful OpenVINO preflight outcomes driven by compile/first-token smoke tests rather than the current `runtime_unavailable` stub
- real `openvino_npu` selection and runtime-engine activation

See `ENGINE_SURFACE_TARGET.md` for the target contract.

## Implementation Direction

### Phase 0

- completed:
  - make this folder the canonical plan pack
  - generalize backend vocabulary and status surfaces away from DML-only language
  - define strict lane-local runtime loading and validation rules
  - define the persisted selection fingerprint and invalidation rules
  - define the per-lane engine status and model-readiness contract
  - split the machine-scoped OpenVINO startup probe from the model-scoped preflight entrypoint in host code
- still pending before broader rollout:
  - define lane-specific manifest layout and the default catalog migration away from `qwen3-4b-instruct-2507`

### Phase 1

- add a native `OpenVINO GenAI` runtime adapter
- replace the current `runtime_unavailable` short-circuit with real compile and first-token preflight
- switch automatic selection from `directml -> cpu` to `openvino_npu -> directml -> cpu` once the OpenVINO lane is genuinely viable
- surface explicit driver/runtime diagnostics
- validate on the Intel NPU laptop first

### Phase 1b

- tune cache policy, warmup behavior, prompt defaults, and workload buckets
- broaden benchmarking to weaker Intel laptops
- decide whether later benchmark-driven lane selection is worth the added complexity

### Phase 2

- refresh benchmarking around the native runtime architecture
- keep the existing benchmark frontend shell and replace the disabled legacy Ollama benchmark backend
- benchmark real runtime/lane choices on the user's own hardware so they can see which engine performs best for that machine and model
- decide how benchmark results should influence future user guidance and optional backend recommendations

## Explicit Non-Goals

- do not add `ORT + OpenVINO EP`
- do not use ORT `AUTO` device policy for product backend selection
- do not move EP ownership out of `engine-host`
- do not over-design plugin/package ecosystems before native OpenVINO viability is proven

## Planning Discipline

Implementation should be planned in isolated workstreams with fresh sessions.

Planner and implementer prompts must require small, frequent checkpoint commits. Do not let multi-workstream WIP accumulate in one dirty tree when the work can be split into reviewable checkpoints.

Recommended next workstream order from this branch baseline:

1. native OpenVINO runtime adapter and successful activation path
2. automatic selector handoff to `openvino_npu -> directml -> cpu`
3. lane-specific manifests and default catalog migration
4. workload tuning and cache policy refinement
5. benchmark refresh for cross-lane, per-machine comparison once the runtime stack is stable enough to measure

## Merge Gate For Implementation Start

- one canonical doc pack only
- final runtime order is agreed: `openvino_npu -> directml -> cpu`
- model artifact policy is agreed
- persisted decision key and invalidation rules are agreed
- NPU driver classification behavior is agreed
- per-lane readiness contract is agreed
- strict app-local runtime loading rules are agreed
