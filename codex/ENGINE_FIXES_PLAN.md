# Engine Fixes Plan — Step 1

Last updated: 2026-03-20
Status: **Implemented** — Tiers 1-3 complete (except 3.1 token ACL and 3.5 structured errors, deferred)
Branch: `fix/engine-prod-readiness` from `main` at `9c0c0ca`

---

## Goal

Make the engine production-ready: fix all known bugs, close stability gaps, ensure auto-selection works without env vars, and harden for deployment on unknown hardware. No refactoring for aesthetics — only changes that fix bugs, prevent crashes, or close security gaps.

---

## Issue Registry

Every issue found during the five-agent audit, organized by implementation priority. Each fix is scoped to be independently testable.

---

### Tier 1 — Must Fix (blocks deployment)

#### 1.1 — ~~Idle unload after 30s breaks readiness~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-host/src/main.rs`
- **Location:** Lines 525 (default timeout), 3416-3422 (idle timer), 1511-1517 (mark_readiness_idle), 2304-2337 (unload_model)
- **Bug:** `model_idle_unload` defaults to `Some(0)` which clamps to 30 seconds. After 30s of inactivity, the idle timer fires `unload_model(false)`, which calls `mark_readiness_idle_after_unload()`, setting state to `Idle`. `/engine/status` then reports `ready: false`. Client shows "reconnecting" forever.
- **Fix:** Change the default `model_idle_unload` from `Some(0)` (30s) to `None` (disabled). The engine should keep the model loaded until explicitly unloaded or a new model is loaded. If idle unload is desired in the future, it should be opt-in via a CLI flag, not the default.
- **Verification:** Start engine, load model, wait 5 minutes, curl `/engine/status` — should still report `ready: true`.

#### 1.2 — ~~Flip backend priority to DirectML > NPU > CPU~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-host/src/main.rs`
- **Location:** `choose_preferred_backend()` function, lines 2494-2545
- **Current:** Checks `has_openvino_candidate` (line 2529) before `has_dml_candidate` (line 2538).
- **Fix:** Swap the order: check `has_dml_candidate` first, then `has_openvino_candidate`. Also update the persisted-decision freshness preference block (lines 2504-2527) to match.
- **Test update:** `backend_selection_prefers_openvino_when_candidate_is_ready` (line ~3768) needs renaming and logic update to reflect DirectML preference.
- **Also update:** CLAUDE.md already updated to `directml > openvino_npu > cpu`.
- **Verification:** Start engine without `SMOLPC_FORCE_EP`, load model — should select DirectML if DLL bundle is present.

#### 1.3 — ~~OpenVINO startup probe has no timeout~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-host/src/main.rs`
- **Location:** Lines 1387-1401 (`spawn_blocking` for `probe_openvino_startup`)
- **Bug:** The DirectML probe (lines 1334-1362) is wrapped in `probe_budget` timeout, but the OpenVINO probe runs unbounded inside `spawn_blocking`. If NPU driver hangs during device enumeration, the blocking thread is leaked and startup hangs.
- **Fix:** Wrap the `spawn_blocking` call in `tokio::time::timeout(OPENVINO_STARTUP_PROBE_WAIT, ...)`, consistent with the DirectML probe pattern. On timeout, return a degraded probe result (same pattern as DirectML timeout at lines 1359-1362).
- **Verification:** Can be unit tested by mocking a slow probe. In practice, verified by observing that startup completes even if NPU driver is unresponsive.

#### 1.4 — ~~No logger initialization~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-host/src/main.rs`
- **Location:** `main()` function, lines 3391-3467
- **Bug:** The crate uses `log::info!`, `log::warn!` extensively but never initializes a logger backend. All diagnostic output (backend selection, probe failures, DLL loading, template patching) is invisible.
- **Fix:** Add `env_logger` to `Cargo.toml` dependencies. Add `env_logger::init()` at the start of `main()`. This respects `RUST_LOG` env var for filtering.
- **Verification:** Run engine with `RUST_LOG=info` — should see startup diagnostics, backend selection decisions, DLL loading messages.

#### 1.5 — ~~CARGO_MANIFEST_DIR fallback won't work in production~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-core/src/models/loader.rs`
- **Location:** Line 40
- **Bug:** `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models")` is evaluated at compile time. In a release binary installed to `C:\Program Files\SmolPC\`, this path points to the build machine's source tree, which won't exist on end-user machines.
- **Fix:** Gate the `CARGO_MANIFEST_DIR` fallback behind `#[cfg(debug_assertions)]`. In release builds, the fallback should be `std::env::current_exe().parent().join("models")` (models adjacent to exe) or return `None` to force the `%LOCALAPPDATA%\SmolPC\models` path.
- **Verification:** Build in release mode, check that `ModelLoader::new()` resolves to `%LOCALAPPDATA%\SmolPC\models` without `SMOLPC_MODELS_DIR` set.

#### 1.6 — ~~Health endpoint always returns ok:true~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-host/src/main.rs`
- **Location:** Lines 2998-3005
- **Bug:** `/engine/health` always returns `{"ok": true}` as long as auth passes. A load balancer, launcher, or client cannot detect that the engine has failed, has no model loaded, or is in `Failed` state.
- **Fix:** Return `{"ok": false, "state": "<current_state>"}` with HTTP 503 when readiness state is `Failed`. Return `{"ok": true, "state": "<current_state>"}` otherwise. This preserves backward compat (clients checking `ok` field) while adding diagnostic info.
- **Verification:** Force a load failure, curl `/engine/health` — should return `ok: false` with 503.

---

### Tier 2 — Should Fix (stability & resilience)

#### 2.1 — ~~expect() panics in production paths~~ ✅ FIXED

- **Files:** `engine/crates/smolpc-engine-host/src/main.rs`
- **Locations:**
  - Line 3146: `serde_json::to_value(readiness).expect("check-model response should serialize")`
  - Line 1938: `.expect("OpenVINO startup probe should exist when artifact is ready")`
- **Fix:** Replace both with proper error returns (HTTP 500 for the handler, `Err(...)` for the load path).

#### 2.2 — ~~Idle timer and /engine/load can race~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-host/src/main.rs`
- **Location:** Lines 3416-3422 (idle timer) vs line 1731 (load_model)
- **Bug:** The idle timer checks `generating.load()` and `current_model` without a unified lock. Between checking and calling `unload_model`, a new `/engine/load` could start. The two could interleave.
- **Fix:** Add an `AtomicBool` guard `model_transition_in_progress` that is set before load/unload and checked by the idle timer. The idle timer skips its tick if a transition is in progress.
- **Note:** With idle unload disabled by default (fix 1.1), this race is much less likely, but should still be guarded for when idle unload is explicitly enabled.

#### 2.3 — ~~Preflight has no timeout for CPU and DirectML paths~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-host/src/main.rs`
- **Locations:** Line 2600 (`build_openvino_cpu_runtime_adapter`), Line 2615 (`build_directml_runtime_adapter`)
- **Bug:** `run_preflight` calls have no timeout. If a malformed model causes OpenVINO CPU or DirectML to hang during warmup, the load path hangs indefinitely.
- **Fix:** Wrap `run_preflight` in a `tokio::time::timeout` (30s for CPU, 60s for DirectML). On timeout, return a specific error.

#### 2.4 — ~~Template patch failure only warned on NPU~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-host/src/openvino.rs`
- **Location:** Line 514
- **Bug:** If `ensure_qwen3_nothink_template` fails, it logs a warning and continues. On NPU, the un-patched template defaults to thinking mode, causing runaway generation.
- **Fix:** When target is NPU and template patch fails, return `OpenVinoPreflightResult::Failed` with class `"openvino_npu_template_patch_failed"`. On CPU, keep the warning (thinking mode is less harmful on CPU).

#### 2.5 — ~~chat_template can be an array in tokenizer_config.json~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-host/src/openvino.rs`
- **Location:** `extract_chat_template_from_tokenizer_config` function, lines 456-459
- **Bug:** HuggingFace `tokenizer_config.json` can store `chat_template` as either a string or an array of `{name, template}` objects. Current code only handles the string form.
- **Fix:** If `v.as_str()` returns `None`, try `v.as_array()` and look for the entry with `name: "default"` (or first entry). Extract its `template` field.

#### 2.6 — ~~Shutdown doesn't cancel active generation~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-host/src/main.rs`
- **Location:** Lines 3455-3464
- **Bug:** On shutdown, no cancellation token is set for any active generation. The blocking generation thread continues running until the tokio runtime is forcefully dropped.
- **Fix:** Set the cancel token for any active generation during the shutdown handler, then add a brief `tokio::time::sleep(Duration::from_secs(2))` grace period before exiting.

#### 2.7 — ~~Client: 120s global timeout kills long streaming requests~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-client/src/lib.rs`
- **Location:** Lines 295-299
- **Bug:** The `reqwest::Client` has a 120s global timeout. For streaming SSE, this applies to the entire response, not per-chunk. On slow hardware, a complex generation could exceed 120s.
- **Fix:** Remove the global timeout. Add per-request timeouts for non-streaming endpoints (30s). For streaming endpoints, implement a per-chunk idle timeout (60s with no new data = abort).

#### 2.8 — ~~Client: No crash recovery / reconnection~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-client/src/lib.rs`
- **Location:** Lines 569-630, 664-729
- **Bug:** If the engine crashes mid-stream, the client gets a generic HTTP error. No automatic reconnection or restart. The UI shows an opaque error.
- **Fix:** Add an `EngineClientError::EngineCrashed` variant that detects connection-refused/reset errors. The Tauri command layer can then trigger `connect_or_spawn` automatically and surface "Engine restarting..." to the UI.

---

### Tier 3 — Should Fix (security & correctness)

#### 3.1 — Windows token file has no ACL restriction

- **File:** `engine/crates/smolpc-engine-client/src/lib.rs`
- **Location:** Lines 1095-1101 (existing TODO)
- **Bug:** On Windows, `engine-token.txt` is created with default ACLs. On shared school computers, another user can read the token.
- **Fix:** Use `%LOCALAPPDATA%\SmolPC\` for token storage (per-user ACLs by default) and set `FILE_ATTRIBUTE_HIDDEN`.

#### 3.2 — ~~min_new_tokens API loaded despite known GenAI 2026 ban~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-core/src/inference/genai/openvino.rs`
- **Location:** Lines 83, 195-198, 843-848
- **Bug:** The `min_new_tokens` field exists and is applied when `Some(...)`. Per CLAUDE.md, any value >= 1 permanently suppresses EOS on GenAI 2026.0.0.
- **Fix:** Add a runtime guard at line 843: if `min_new_tokens >= 1`, log a warning and skip the call. Or remove the field entirely.

#### 3.3 — ~~Windows spawn doesn't redirect stdio to null~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-client/src/lib.rs`
- **Location:** Lines 1140-1146
- **Bug:** Windows path sets `DETACHED_PROCESS` but doesn't redirect stdio. If inherited handles are invalid, the engine could panic on first stdout write.
- **Fix:** Add `.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())` matching the Unix path.

#### 3.4 — ~~Client: duplicated SSE parsing logic~~ ✅ FIXED

- **File:** `engine/crates/smolpc-engine-client/src/lib.rs`
- **Location:** Lines 541-634 vs 636-729
- **Bug:** ~90 lines of identical SSE parsing in `generate_stream` and `generate_stream_messages`. Bug fixes must be applied twice.
- **Fix:** Extract into a private `consume_sse_stream()` method.

#### 3.5 — Client: catch-all Message(String) error variant

- **File:** `engine/crates/smolpc-engine-client/src/lib.rs`
- **Location:** Lines 152-162
- **Bug:** `EngineClientError::Message(String)` conflates timeout, busy, not-found, startup-failed. UI cannot programmatically distinguish failure modes.
- **Fix:** Add structured variants: `Timeout`, `EngineBusy`, `EngineNotFound`, `StartupFailed { retryable }`.

---

### Tier 4 — Nice to Have (polish)

#### 4.1 — DefaultHasher fingerprint not stable across Rust versions

- **File:** `engine/crates/smolpc-engine-core/src/inference/runtime_loading.rs`, line 749
- **Impact:** Rust toolchain update invalidates all cached backend decisions, forcing re-evaluation.
- **Fix:** Replace with a stable hash (e.g., FNV or xxhash with fixed seed).

#### 4.2 — Dead stub functions: model_requires_directml/openvino

- **File:** `engine/crates/smolpc-engine-host/src/main.rs`, lines 581-589
- **Impact:** Always return `false`. Dead code.
- **Fix:** Remove or implement.

#### 4.3 — State transition guard doesn't update timestamp during retries after failure

- **File:** `engine/crates/smolpc-engine-host/src/main.rs`, lines 182-187
- **Impact:** UI shows stale `state_since` during retry sub-phases.
- **Fix:** Add `|| matches!(self.state, ReadinessState::Failed)` to the transition guard.

#### 4.4 — Non-streaming completions rejected with 400

- **File:** `engine/crates/smolpc-engine-host/src/main.rs`, lines 3380-3388
- **Impact:** OpenAI-compatible clients sending default requests (no `stream` field) get an error.
- **Fix:** Default `stream` to `true` if unset, or document the requirement.

#### 4.5 — ICU DLL validation reports misleading error class

- **File:** `engine/crates/smolpc-engine-host/src/runtime_bundles.rs`, lines 377-388
- **Impact:** Diagnostic confusion only.
- **Fix:** Add `OpenVinoIcuMissing` variant.

#### 4.6 — OpenVINO cache directory grows unbounded

- **File:** `engine/crates/smolpc-engine-host/src/main.rs`, lines 1231-1244
- **Impact:** On 8GB RAM budget hardware with limited storage, NPU compiled blob cache could fill the drive over many model updates.
- **Fix:** Add a cache size limit or eviction policy. Low priority for initial deployment.

---

## Research Insights (from web search)

### NPU Health Recovery

- `ZE_RESULT_ERROR_DEVICE_LOST` (0x70000001) is a known NPU failure mode after sleep/wake cycles or driver timeouts (OpenVINO GitHub #29386).
- On Windows, there is **no user-mode NPU driver reset API**. The only recovery is destroying the pipeline object and creating a new one.
- **Recommendation:** If a generation fails with a device error, the engine should destroy the adapter, wait briefly, and attempt pipeline re-creation. This is beyond the scope of fix 1.1 (which prevents the idle unload bug) but should be tracked as a future resilience improvement.

### NPU Driver Version Sensitivity

- Driver version `.4023` introduced regressions causing compilation failures. Version `.3717` is known-good in multiple reports.
- **Recommendation:** Document the validated NPU driver version in system requirements.

### DirectML Is in Maintenance Mode

- Microsoft has shifted new development to **WinML**, which auto-selects execution providers.
- DirectML continues to be supported but will not receive new features.
- **No immediate action needed** — the engine's centralized backend policy isolates this future concern.

### Tauri DLL Bundling

- Tauri 2 `resources` config is the correct pattern for DLL bundling (not sidecars).
- DLLs land in `$RESOURCE/`, not the app root — explicit path-based loading is required, which is what `runtime_loading.rs` already does.
- **No changes needed** to the bundling approach.

---

## Implementation Order

Recommended sequence for a single implementation session:

```
1.4  Logger init              (5 min, unlocks diagnostics for everything else)
1.1  Disable idle unload      (5 min, fixes the known bug)
1.2  Flip backend priority    (15 min, includes test update)
1.6  Health endpoint          (10 min)
1.5  Model path fallback      (10 min)
1.3  OpenVINO probe timeout   (15 min)
2.1  Remove expect() panics   (10 min)
2.4  Template patch → error   (10 min)
2.5  Array chat_template      (15 min)
2.3  Preflight timeouts       (10 min)
2.6  Shutdown cancellation    (10 min)
2.2  Idle/load race guard     (15 min)
3.3  Windows stdio redirect   (5 min)
3.2  min_new_tokens guard     (5 min)
--- checkpoint: cargo check && cargo clippy && cargo test ---
2.7  Client timeout fix       (20 min)
2.8  Client crash recovery    (20 min)
3.4  SSE dedup                (15 min)
3.5  Structured errors        (20 min)
3.1  Token file ACL           (15 min)
--- checkpoint: cargo check && cargo clippy && cargo test ---
4.x  Polish items as time permits
```

**Estimated total: ~4-5 hours of implementation**

---

## Verification Plan

After all fixes:

1. `cargo check --workspace && cargo clippy --workspace` — clean
2. `cargo test -p smolpc-engine-core && cargo test -p smolpc-engine-host` — all pass
3. **Live test: DirectML auto-selection**
   - Start engine WITHOUT `SMOLPC_FORCE_EP`
   - Load qwen2.5-1.5b-instruct → should auto-select DirectML
   - Stream a chat completion → coherent output
4. **Live test: NPU forced**
   - `SMOLPC_FORCE_EP=openvino_npu` → load qwen3-4b → coherent INT8 output
5. **Live test: CPU fallback**
   - `SMOLPC_FORCE_EP=cpu` → load qwen2.5-1.5b-instruct → coherent output
6. **Live test: idle stability**
   - Load model → wait 5+ minutes → curl status → still `ready: true`
   - Send a chat completion after idle → works without restart
7. **Live test: health endpoint**
   - Healthy engine → `{"ok": true, "state": "ready"}`
   - Failed load → `{"ok": false, "state": "failed"}` with HTTP 503
8. **Stress test: rapid load/unload**
   - 5x load/unload cycles → no crashes, no leaked resources
9. `cd apps/codehelper && npm run check && npm run lint` — frontend still clean
