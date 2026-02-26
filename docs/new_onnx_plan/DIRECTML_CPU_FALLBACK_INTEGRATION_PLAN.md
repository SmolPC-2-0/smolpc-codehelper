# DirectML + CPU Fallback Integration Plan (with Execution Discipline)

Last updated: 2026-02-24
Status: Baseline plan implemented on `codex/directml-inferencing`

## Summary
Integrate DirectML acceleration on Windows (10 20H1+ / 11) with automatic CPU fallback, benchmark-gated selection, persistent backend decisions, and auto-demotion after 3 failures. Upgrade ORT stack to `ort = 2.0.0-rc.11`, bundle required runtime binaries, and keep UI unchanged (log-only visibility for now).

## Execution Discipline (Mandatory)
1. Commit frequency: one commit per logical unit (no mixed concerns).
2. Commit size: small and reviewable; each commit maps to a single milestone step.
3. Commit quality gate: touched code must `cargo check`; targeted tests for changed module must pass before commit.
4. Doc parity: every code commit also updates handoff docs in the same commit.
5. Handoff docs per commit:
   - `docs/new_onnx_plan/CURRENT_STATE.md`: what changed, what remains, exact next action.
   - `codex/WORKING_ISSUES.md`: bug/risk table with "introduced in commit", status, mitigation.
   - `SESSION_LOG.md`: session handoff with "last known good commit" + "resume from step".
6. Bug traceability: all discovered issues are logged with commit hash and reproduction notes.
7. Branch hygiene: no force-push, no squash-amend during active review cycle unless explicitly requested.

## Scope and Locked Decisions
1. Backend scope: DirectML + CPU fallback only.
2. ORT crate: `2.0.0-rc.11`.
3. Packaging: bundle everything required.
4. OS target: Windows 10 20H1+ and Windows 11.
5. Selection thresholds (current default): DirectML requires `+30% decode tok/s` with max `+15% TTFT` regression.
6. Selection time budget: up to 2s at first model load.
7. Visibility: logs now, UI later.
8. Persistence key: model + adapter identity + driver + app version + ORT version.
9. Demotion: DirectML demoted to CPU after 3 init/runtime failures.

## Public/API Changes
1. Add backend diagnostics command:
   `get_inference_backend_status() -> BackendStatus`.
2. Additive GPU fields in IPC:
   `driver_version?: string`, `pci_device_id?: string`.
3. Keep existing generation/load command signatures unchanged.

## Milestone Plan

### Milestone 1: Toolchain + Runtime Packaging
1. Update `src-tauri/Cargo.toml`:
   `rust-version = "1.88"`, `ort = "=2.0.0-rc.11"`.
2. Add `rust-toolchain.toml` pinned to `1.88`.
3. Update `scripts/setup-libs.sh`:
   pull `Microsoft.ML.OnnxRuntime.DirectML` (ORT runtime DLLs) and `Microsoft.AI.DirectML` (`DirectML.dll`) with checksum verification.
4. Ensure `src-tauri/tauri.conf.json` keeps bundling `libs/*`.

### Milestone 2: Backend Domain + Persistence
1. Add `inference/backend.rs`:
   `InferenceBackend`, `BackendDecision`, `DecisionReason`, `BackendBenchmark`, `FailureCounters`.
2. Add `inference/backend_store.rs`:
   versioned JSON store, atomic writes, invalidation on key changes.
3. Persist decision by agreed key fields.

### Milestone 3: Hardware Identity Enrichment
1. Extend backend hardware types in `src-tauri/src/hardware/types.rs` with `driver_version` and `pci_device_id`.
2. Populate in `src-tauri/src/hardware/detector.rs`.
3. Mirror optional fields in `src/lib/types/hardware.ts`.

### Milestone 4: Session Builder + Fallback
1. Refactor `src-tauri/src/inference/session.rs`:
   add backend-aware session creation (`Cpu` / `DirectML`).
2. DirectML config:
   `with_execution_providers([ep::DirectML...error_on_failure()])`,
   `with_parallel_execution(false)`,
   `with_memory_pattern(false)`,
   `Level3` optimization.
3. In `src-tauri/src/inference/mod.rs`, preload `DirectML.dll` on Windows before ORT init.
4. If DirectML init fails, immediately fallback to CPU in same load flow.

### Milestone 5: Selector + Benchmark Gate + Demotion
1. Add backend selection flow in `src-tauri/src/commands/inference.rs`.
2. First-run A/B micro-benchmark (bounded by 2s).
3. Promote DirectML only if thresholds pass; otherwise keep CPU.
4. Count DirectML failures; demote after 3.
5. Add hidden override `SMOLPC_FORCE_EP=cpu|dml` for diagnostics.

### Milestone 6: Diagnostics + Logging
1. Add structured logs for candidate ranking, benchmark result, selection reason, fallback cause, demotion count.
2. Add `get_inference_backend_status` command and register in `src-tauri/src/lib.rs`.
3. No UI changes in this phase.

## Test Cases and Scenarios
1. Unit:
   key generation, persistence round-trip, invalidation, demotion threshold behavior.
2. Integration:
   DirectML init failure -> CPU fallback works in same request.
3. Regression:
   cancellation, single-flight generation lock, load/unload lifecycle.
4. Manual Windows matrix:
   DirectML-capable machine, incompatible/failing DirectML machine, repeated-failure demotion scenario.
5. Performance validation:
   verify benchmark gate enforces policy and remains within 2s budget.

## Assumptions and Defaults
1. CPU remains safe default when selection data is incomplete/timeout.
2. This phase is Windows-first for DirectML runtime delivery.
3. IoBinding optimization is deferred until after stable backend selection/fallback.
4. Runtime failure during generation aborts that request; fallback applies to subsequent requests.
5. UI backend indicator is intentionally deferred; logs + diagnostics command are sufficient now.
