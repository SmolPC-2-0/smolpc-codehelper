# Engine Fixes Plan — Step 1

Last updated: 2026-03-20
Status: **Complete** — merged as PR #107 (`3d7460e`)

---

## Summary

20 fixes implemented across engine host, client, and core. All Tier 1 (deployment blockers) and Tier 2 (stability) resolved. Most Tier 3 (security/correctness) resolved. Live-tested on hardware.

**Commits:** 6 on `fix/engine-prod-readiness`, squash-merged to main.

---

## Remaining (deferred)

### 3.1 — Windows token file has no ACL restriction
- **File:** `engine/crates/smolpc-engine-client/src/lib.rs`
- Needs `windows` crate dependency. Defer to packaging phase — token storage path may change.

### 3.5 — Client: catch-all Message(String) error variant
- **File:** `engine/crates/smolpc-engine-client/src/lib.rs`
- Would add structured variants: `Timeout`, `EngineBusy`, `EngineNotFound`, `StartupFailed { retryable }`. Changes public API surface — needs coordination with Tauri app.

### Tier 4 — Polish (do if time permits during cleanup phase)

| ID | Issue | Impact |
|----|-------|--------|
| 4.1 | DefaultHasher fingerprint not stable across Rust versions | Cache invalidation on toolchain update |
| 4.2 | Dead stub functions: model_requires_directml/openvino | Dead code |
| 4.3 | State transition guard stale timestamp during retries | UI shows stale `state_since` |
| 4.4 | Non-streaming completions rejected with 400 | OpenAI compat gap |
| 4.5 | ICU DLL validation misleading error class | Diagnostic confusion |
| 4.6 | OpenVINO cache directory grows unbounded | Storage on budget hardware |

---

## Research Insights (retained for reference)

### NPU Health Recovery
- `ZE_RESULT_ERROR_DEVICE_LOST` (0x70000001) is a known failure mode after sleep/wake. No user-mode reset API — must destroy and recreate pipeline.

### NPU Driver Version Sensitivity
- Driver `.4023` has regressions. Driver `.3717` is known-good.

### Tauri DLL Bundling
- Tauri 2 `resources` config is correct for DLL bundling. DLLs land in `$RESOURCE/`, not app root — explicit path-based loading required (already implemented in `runtime_loading.rs`).
