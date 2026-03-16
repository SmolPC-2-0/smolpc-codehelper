# ONNX Migration - Current State

> **Note:** This document is a historical snapshot from the DirectML integration phase.
> For current project state, see `CLAUDE.md` at the repo root.

Last updated: 2026-02-28
Branch: `codex/directml-inferencing` (historical — this branch has been merged)
Phase: 2A DirectML acceleration (GenAI C-FFI path) completed and validated

---

## Executive Status

DirectML inferencing is now operational through ONNX Runtime GenAI C-FFI on Windows, with:

- Automatic startup capability probing and backend selection (DirectML-first when available)
- Forced backend override (`SMOLPC_FORCE_EP=dml|cpu`)
- Device override (`SMOLPC_DML_DEVICE_ID`)
- Preflight probing and finite-logit validation
- Persistent backend decisioning and CPU fallback/demotion safety

CPU inferencing remains on the ORT generator path.

---

## Key Commits (Merge-Critical)

1. `0b38f67` - `feat(inference): add DirectML GenAI runtime path and DML export tooling`
2. `477ca60` - `refactor(inference): remove dead code and clean warning surface`
3. `7460015` - `docs(dml): add full GenAI DirectML rundown and ignore local artifacts`

---

## Implemented Runtime Architecture

### Backend/runtime split

- `ort_cpu`:
  - `InferenceSession` + `Generator` (existing ORT path)
- `genai_dml`:
  - `GenAiDirectMlGenerator` via C-FFI to `onnxruntime-genai.dll`
  - provider setup: clear providers -> append `dml` -> optional hardware device id

### Adapter layer

- `src-tauri/src/inference/runtime_adapter.rs` now dispatches:
  - `InferenceRuntimeAdapter::Ort`
  - `InferenceRuntimeAdapter::GenAiDirectMl` (Windows)

### Model artifact layout

Required:

```text
src-tauri/models/<model_id>/
  cpu/model.onnx
  dml/model.onnx
  tokenizer.json
```

CPU keeps legacy fallback to `<model_id>/model.onnx`; DML does not.

---

## DML Runtime Dependencies

`scripts/setup-libs.sh` installs (Windows):

- `onnxruntime.dll`
- `onnxruntime_providers_shared.dll`
- `DirectML.dll`
- `onnxruntime-genai.dll`

`tauri.conf.json` bundles `libs/*` into app resources.

---

## Validation Snapshot

Validation snapshot at the time of writing (2026-02-28, on `codex/directml-inferencing`):

- `cargo check --all-targets`: pass (warning-clean)
- `cargo test --lib -- --nocapture`: pass (`79 passed, 0 failed, 9 ignored`)
- `npm run check`: pass (1 existing frontend accessibility warning, unrelated)

> **2026-03-16 update:** These results are stale. The codebase has since moved to
> shared engine architecture with OpenVINO NPU support. Run verification commands
> from `CLAUDE.md` for current status.

---

## Current Gates and Runtime Controls

- Force backend:
  - `SMOLPC_FORCE_EP=dml`
  - `SMOLPC_FORCE_EP=cpu`
- Optional device pin:
  - `SMOLPC_DML_DEVICE_ID=<non-negative int>`

---

## What Is Complete

- DirectML runtime integration through GenAI C-FFI
- Startup backend probing with capability-first DirectML auto-selection
- Multi-GPU DirectML ranking (discrete-first, then VRAM)
- Load-time DML init + preflight probe
- Runtime failure tracking and demotion behavior
- Backend status surface fields:
  - `runtime_engine`
  - `dml_gate_state`
  - `dml_gate_reason`
- DML export and run helper scripts
- Dead-code cleanup in touched modules
- Meeting handoff documentation for DML path internals

---

## Remaining Work (Not Blocking Current DML PR)

1. Packaging hardening for production model path:
   - current model default path logic is dev-oriented (`CARGO_MANIFEST_DIR/models`)
   - move to runtime app data model directory + first-run copy/sync strategy
2. Frontend surfacing of backend diagnostics:
   - consume `get_inference_backend_status` in UI
3. OpenVINO acceleration design/implementation:
   - choose ORT EP-first path vs GenAI OpenVINO build-from-source path
4. Windows clean-machine installer validation:
   - verify all bundled runtime binaries and model artifacts on a fresh environment

---

## Reference Docs

- Canonical integration plan:
  - `docs/new_onnx_plan/DIRECTML_CPU_FALLBACK_INTEGRATION_PLAN.md`
- DML export/layout contract:
  - `docs/DML_plans/DML_EXPORT_AND_LAYOUT.md`
- Detailed technical rundown (for stakeholder meetings):
  - `docs/DML_plans/DIRECTML_GENAI_FULL_RUNDOWN.md`
- Historical audit context:
  - `docs/new_onnx_plan/PR37_DIRECTML_CODE_AUDIT.md`
