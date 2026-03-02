# Shared Engine Handoff Readiness (2026-03-01)

## Purpose

Final readiness audit before onboarding additional apps (Blender helper, GIMP helper, etc.) onto the shared engine contract.

## Validation Executed

1. `cargo check --workspace` -> pass
2. `cargo test --workspace` -> pass
3. `npm run check` -> pass (1 existing Svelte a11y warning)
4. Live host smoke against `http://127.0.0.1:19432`:
   - `GET /engine/meta` -> pass (`protocol_version: 1.0.0`)
   - `POST /engine/load` -> pass
   - `POST /v1/chat/completions` non-stream -> pass with populated `smolpc_metrics`
   - `POST /v1/chat/completions` stream -> pass (`chat.completion.metrics` event + `[DONE]`)

## Current Contract/Runtime State

1. Shared architecture is in place and wired:
   - `smolpc-engine-core`
   - `smolpc-engine-host`
   - `smolpc-engine-client`
2. Tauri app command surface is routed through shared engine client.
3. Host sidecar packaging flow is present in release workflow and Tauri resources.
4. Backend status contract is exposed for diagnostics (`active_backend`, `selection_reason`, `selected_device_name`, etc.).
5. Streaming semantics are contract-safe:
   - structured error events (not token-injected errors)
   - metrics event in stream path

## Findings (Ordered by Severity)

## P1 (Open, known/deferred)

1. Auto-selection can false-negative to CPU on some DML-capable systems due to startup probe gating behavior.
   - Tracked in `codex/WORKING_ISSUES.md` as issue #5.
   - Impact: some machines may start on CPU unless probe/runtime path recovers.
   - Status: explicitly deferred for later implementation.

## P2 (Non-blocking for app onboarding)

1. Strict clippy gate is not green yet:
   - `cargo clippy --workspace --all-targets -- -D warnings` fails.
   - Most findings are `uninlined_format_args`; notable structural findings include:
     - `len_without_is_empty` (`LayerCache`)
     - `large_enum_variant` (`InferenceRuntimeAdapter`)
2. No dedicated engine-host integration test suite yet for reconnect/fallback queue matrix.
   - Current confidence is from workspace tests + smoke validation.

## P3 (Minor)

1. Existing frontend warning:
   - `src/lib/components/chat/ConversationView.svelte` a11y warning (`a11y_no_static_element_interactions`).

## Go/No-Go Decision

`GO (controlled onboarding)` for internal app integration against current shared-engine contract.

Rationale:

1. Core contract and runtime behavior are stable enough for consumer integration.
2. Critical build/test gates are green.
3. Sidecar packaging and host discovery path are in place.
4. Remaining risks are known, documented, and mostly hardening/quality depth.

## Onboarding Guardrails

1. Integrate consumer apps against a tagged release, not branch head.
2. Require each integration issue report to include:
   - request payload
   - HTTP status/response body
   - `/engine/status` snapshot
   - app version + hardware details
3. Keep stabilization fixes on `codex/shared-engine-v1`; run enhancements separately.

## Recommended Immediate Follow-up (parallel to onboarding)

1. Implement probe-to-runtime validation fix (working issue #5).
2. Add host integration tests:
   - reconnect after host exit
   - idle exit respawn
   - stream cancel/error semantics
   - queue full/timeout paths
3. Start clippy debt burn-down with staged policy:
   - first enforce on touched files/modules
   - then move toward full `-D warnings`
