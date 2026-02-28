# Shared Engine Production Audit (2026-02-28)

## Scope

Audit target:

1. Shared engine host/runtime behavior (`crates/smolpc-engine-host`)
2. Shared backend status contract (`crates/smolpc-engine-core`)
3. Frontend/backend visibility contract (`src/lib/stores/inference.svelte.ts`, `src/lib/components/StatusIndicator.svelte`)
4. Integration and handoff documentation

Audit method:

1. Static review of changed runtime selection/fallback code
2. Build and test gates
3. Runtime smoke validation against host API in auto and forced-DML modes
4. Contract/documentation parity check

## Build/Test Evidence

Executed successfully:

1. `cargo check --workspace`
2. `cargo test --workspace`
3. `npm run check` (1 pre-existing Svelte accessibility warning, no errors)

## Runtime Smoke Evidence

## Auto Mode (`SMOLPC_FORCE_EP` unset)

Observed:

1. Startup probe surfaced `available_backends: ["cpu","directml"]`
2. Load selected `active_backend: "directml"` with `runtime_engine: "genai_dml"`
3. Streaming emitted token SSE chunks + structured metrics event + `[DONE]`
4. Non-stream response included populated `smolpc_metrics`

Key status fields observed:

1. `selection_state: "ready"`
2. `selection_reason: "default_directml_candidate"`
3. `selected_device_name: "NVIDIA GeForce RTX 4050 Laptop GPU"`

## Forced DML Mode (`SMOLPC_FORCE_EP=dml`)

Observed:

1. Load selected and stayed on `directml`
2. `force_override: "directml"` present in status
3. Generation succeeded with expected metrics payload

## Fixes Applied During Audit

1. Added stable, explicit decision reason serialization for DirectML-related variants to avoid split-initialism drift in public payloads.
2. Normalized host-side selection reason codes to stable snake_case values.
3. Fixed decision/status semantics so active decision reason reflects effective runtime path after fallback.
4. Kept `forced dml` behavior strict on runtime failure path (no silent forced-mode demotion).
5. Improved status consistency:
1. `available_backends` reflects startup capability detection.
2. `directml_probe_passed` represents hardware detection state.
3. `dml_gate_state` distinguishes `selected`, `fallback_cpu`, `artifact_missing`, `cpu_only`.
6. Added forced-mode error status updates for missing artifact and DirectML init failures.
7. Removed unused internal `session_demoted` state field.

## Current Readiness Assessment

## Ready for Team Handoff

1. Engine contract is stable and observable at runtime.
2. Backend selection/fallback behavior is deterministic and inspectable.
3. Status payload now contains enough context for consumer-team diagnostics.
4. Documentation coverage is sufficient for external teams to integrate without reading internals.

## Remaining Gaps Before "Strict Production Bar"

1. `clippy -D warnings` is not green across `smolpc-engine-core`.
1. Large existing warning debt in formatting style lints and API design lints (`large_enum_variant`, `len_without_is_empty`, `uninlined_format_args`, etc.).
2. No dedicated host integration-test suite for fallback and reconnect scenarios.
1. Current verification is runtime smoke + workspace tests.
3. Forced DML device-id validation depends on runtime behavior.
1. Invalid/high device ids may still be accepted by the underlying runtime.

## Recommended Next Improvements

Priority 0:

1. Add engine-host integration tests for:
1. load auto-select
2. init failure fallback
3. runtime failure fallback
4. forced override strictness

Priority 1:

1. Add a CI lane with scoped clippy policy:
1. start by enforcing clippy on changed modules
2. then incrementally burn down existing warning debt to reach `-D warnings`

Priority 2:

1. Add structured error codes in host load path responses for:
1. artifact missing
2. DirectML init failure
3. invalid force override configuration

Priority 3:

1. Add explicit runtime metric/telemetry events for backend transition moments (selected, fallback, demoted).

## Conclusion

The shared engine path is functionally stable for current handoff scope (CPU + DirectML, Windows-first), with validated auto-selection, forced-mode operation, and status observability. Remaining risk is primarily test-depth and clippy debt, not immediate correctness regressions in the new selector/fallback behavior.
