# Engine Surface Target

Checked on: 2026-03-12
Purpose: define the target engine status and model-readiness contract for the native OpenVINO rollout, and record the implemented branch baseline that the remaining Phase 1 work should assume.

## Principles

- status is lane-based, not DML-specific
- readiness is reported independently for `openvino_npu`, `directml`, and `cpu`
- startup probe and model preflight are separate states
- temporary fallback is distinguishable from persisted selection
- `engine-host` remains the only selector, probe owner, and persistence owner

## Implemented Baseline (2026-03-12)

### `GET /engine/status`

Top-level fields now implemented:

- `active_backend`
- `active_artifact_backend`
- `runtime_engine`
- `selection_state`
- `selection_reason`
- `decision_persistence_state`
  - `none`
  - `persisted`
  - `temporary_fallback`
- `available_backends`
- `selected_device`
- `selection_fingerprint`
- `decision_key`
- `last_decision`
- `runtime_bundles`
- `failure_counters`
- `force_override`
- `store_path`

Current lane surface:

- `backend_status.lanes.openvino_npu`
- `backend_status.lanes.directml`
- `backend_status.lanes.cpu`

Each implemented lane now exposes:

- `detected`
- `bundle_ready`
- `artifact_ready`
- `startup_probe_state`
  - `not_started`
  - `ready`
  - `error`
- `preflight_state`
  - `not_started`
  - `pending`
  - `ready`
  - `timeout`
  - `error`
- `persisted_eligibility`
- `last_failure_class`
- `last_failure_message`
- `driver_version`
- `runtime_version`
- `cache_state`
  - `unknown`
  - `cold`
  - `warm`
- `device_id`
- `device_name`

Current truthfulness limits:

- `directml` startup detection is implemented now
- `directml` preflight is only reported after an actual DirectML load attempt
- `openvino_npu` bundle/artifact readiness is reported now
- `openvino_npu` startup-probe truth is reported now (`detected`, `device_name`, `driver_version`, `startup_probe_state`, `last_failure_class`)
- `runtime_engine` now emits `ort_cpu`, `genai_dml`, or `ov_genai_npu`
- `openvino_npu.preflight_state` now reflects real compile plus first-token preflight during `/engine/load`
- selector handoff now prefers `openvino_npu -> directml -> cpu` when the OpenVINO lane passes preflight

### `POST /engine/check-model`

Implemented now:

- response is lane-based and no longer returns a single boolean
- primary app-facing readiness surfaces are:
  - HTTP `POST /engine/check-model`
  - `EngineClient::check_model_readiness()`
  - Tauri command `check_model_readiness(model_id)`
- compatibility shims are:
  - `EngineClient::check_model_exists()`
  - Tauri command `check_model_exists(model_id)`
  - these return `true` only when at least one lane has `ready = true`
  - new callers should prefer the readiness API above
- current shape:

```json
{
  "model_id": "qwen2.5-coder-1.5b",
  "lanes": {
    "openvino_npu": {
      "artifact_ready": false,
      "bundle_ready": true,
      "ready": false,
      "reason": "artifact_missing"
    },
    "directml": {
      "artifact_ready": true,
      "bundle_ready": true,
      "ready": true,
      "reason": "ready"
    },
    "cpu": {
      "artifact_ready": true,
      "bundle_ready": true,
      "ready": true,
      "reason": "ready"
    }
  }
}
```

Current reason codes include:

- `ready`
- `unknown_model`
- `artifact_missing`
- `artifact_invalid`
- `artifact_incomplete`
- `startup_probe_pending`
- `startup_probe_failed`
- `directml_candidate_missing`
- runtime-bundle validation failure codes such as `missing_root`, `directml_missing`, `openvino_npu_plugin_missing`
- blocking OpenVINO startup-probe failure classes such as `no_npu_hardware`, `openvino_npu_driver_missing`, and `openvino_npu_plugin_unavailable`

Current implemented reason semantics:

- `cpu`
  - `ready` when artifact and ORT bundle are both ready
  - `artifact_missing` when the CPU artifact is incomplete
  - otherwise the current ORT bundle failure code
- `directml`
  - `artifact_missing` when the DirectML artifact is incomplete
  - then the DirectML bundle failure code
  - then `startup_probe_pending` before the startup probe finishes
  - then `directml_candidate_missing` when the probe finished without a DirectML-capable adapter
  - otherwise `ready`
- `openvino_npu`
  - `artifact_missing` when the OpenVINO manifest is missing
  - `artifact_invalid` or `artifact_incomplete` when the OpenVINO manifest exists but is not usable
  - then the OpenVINO bundle failure code
  - then `startup_probe_pending` while the async OpenVINO startup probe is still running
  - then a blocking OpenVINO startup-probe failure class when the startup probe completed but the lane is unusable
  - then `startup_probe_failed` if the startup probe completed without a usable result but also without a blocking classified failure
  - otherwise `ready`
  - OpenVINO lane `ready=true` means the lane is viable enough to attempt `/engine/load`; the final native preflight still runs during model load

## Remaining Target For The Rest Of Phase 1

### `GET /engine/status`

The remaining Phase 1 work still needs to make the implemented baseline fully match the final target by adding:

- cache-state truth for the OpenVINO lane
- final Intel NPU validation against the real bundled runtime/model inventory

### `POST /engine/check-model`

The remaining Phase 1 work still needs:

- any explicit preflight-on-demand readiness API if product needs readiness to include the model-scoped OpenVINO smoke test
- validation that app-local bundle staging matches the current contract on target Intel laptops

## Failure-Class Vocabulary

OpenVINO current + target vocabulary:

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

DirectML current + target vocabulary:

- `directml_candidate_missing`
- `directml_artifact_missing`
- `directml_initialization_failed`
- `directml_runtime_failed`

CPU current + target vocabulary:

- `artifact_missing`
- ORT runtime-bundle validation failure codes when the CPU lane bundle is incomplete

## Persistence Rules Visible In Status

- `decision_persistence_state=temporary_fallback` means the active backend is a temporary fallback and the host did not replace a prior good persisted decision
- `lanes.<lane>.persisted_eligibility=true` identifies which lane is currently persisted for the active selection fingerprint
- `selection_fingerprint` and `decision_key` now reference the full persisted fingerprint, not just the backend id

## Implementation Boundary

The lane-based surface, fingerprinted persistence contract, and OpenVINO startup-probe surfaces are now implemented on this branch. The next planner session should stay focused on native OpenVINO activation, live selection, and model/catalog migration rather than reopening the status or persistence schema unless testing finds a regression.
