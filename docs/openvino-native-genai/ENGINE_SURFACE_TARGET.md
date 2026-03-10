# Engine Surface Target

Checked on: 2026-03-09
Purpose: define the target engine status and model-readiness contract for the native OpenVINO rollout.

This file describes the target Phase 0 contract. It does not claim the current implementation already matches it.

## Principles

- status is lane-based, not DML-specific
- readiness is reported independently for `openvino_npu`, `directml`, and `cpu`
- startup probe and model preflight are separate states
- temporary fallback must be distinguishable from persisted selection

## Target `GET /engine/status`

Top-level backend surface:

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

Lane readiness surface:

- `backend_status.lanes.openvino_npu`
- `backend_status.lanes.directml`
- `backend_status.lanes.cpu`

Each lane should expose:

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

## Target `POST /engine/check-model`

Do not return a single boolean.

Return readiness by lane, for example:

```json
{
  "model_id": "qwen2.5-coder-1.5b",
  "lanes": {
    "openvino_npu": {
      "artifact_ready": true,
      "bundle_ready": true,
      "ready": false,
      "reason": "preflight_not_run"
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

## Failure-Class Vocabulary

OpenVINO:

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

DirectML:

- `directml_candidate_missing`
- `directml_artifact_missing`
- `directml_initialization_failed`
- `directml_runtime_failed`

CPU:

- `cpu_artifact_missing`
- `cpu_initialization_failed`

## Persistence Rules Visible In Status

- temporary fallbacks must be visible as temporary
- timeout-based fallbacks must not be reported as persisted winners
- persisted records should reference the selection fingerprint, not just the backend id

## Implementation Boundary

This contract should be implemented before launcher/app integration for the native OpenVINO lane proceeds.
