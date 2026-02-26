# PR #37 Audit: DirectML + CPU Fallback (Rust/ORT)

Date: 2026-02-26
Reviewer: Codex (GPT-5)
PR scope audited: `origin/main...0c998fb` (commits `5f8cf76` -> `0c998fb`)

## Scope and Method

### Code scope reviewed
- `src-tauri/Cargo.toml`
- `scripts/setup-libs.sh`
- `src-tauri/src/inference/{mod.rs,session.rs,backend.rs,backend_store.rs,generator.rs}`
- `src-tauri/src/commands/inference.rs`
- `src-tauri/src/hardware/{detector.rs,types.rs}`
- `src-tauri/src/lib.rs`
- `src/lib/types/hardware.ts`

### Validation performed
- Static code audit (correctness, Rust idioms, failure handling, persistence semantics, concurrency boundaries).
- ORT/EP best-practice cross-check against official docs (listed in References).
- Local test run:
  - `RUSTC=$HOME/.rustup/toolchains/1.88.0-aarch64-apple-darwin/bin/rustc $HOME/.rustup/toolchains/1.88.0-aarch64-apple-darwin/bin/cargo test --lib -- --nocapture`
  - Result: `70 passed, 0 failed, 9 ignored`.

## Findings (Ordered by Severity)

### [P1] Benchmark timeout + sticky persistence can permanently disable DirectML on weaker devices
**Where**
- `src-tauri/src/inference/backend.rs:4`
- `src-tauri/src/commands/inference.rs:532-570`
- `src-tauri/src/commands/inference.rs:572-580`
- `src-tauri/src/commands/inference.rs:631-637`

**Issue**
- Selection benchmark has a hard 2s global budget (`BENCHMARK_SELECTION_BUDGET_MS = 2000`) for both CPU and DirectML runs.
- On timeout, CPU is selected and persisted immediately.
- On future loads, persisted CPU decision short-circuits selection, so DirectML is no longer retried for that key.

**Impact**
- Weak systems (especially those this feature targets) are most likely to exceed this budget, causing long-term CPU lock-in and no acceleration.
- This is a functional risk, not just tuning.

**Recommendation**
- Do not persist `BenchmarkBudgetExceeded` as a hard CPU decision.
- Use a retryable state with TTL/backoff, or benchmark asynchronously after first usable load.

---

### [P1] No adapter/device selection: DirectML defaults to GPU 0, which may be the wrong GPU
**Where**
- `src-tauri/src/inference/session.rs:65`
- `src-tauri/src/commands/inference.rs:174-217`

**Issue**
- Session uses `ep::DirectML::default().build()` without `with_device_id(...)`.
- Decision key captures adapter identity, but the selected DirectML adapter is never explicitly bound.

**Impact**
- On hybrid iGPU+dGPU systems, ORT docs note GPU 0 may not be the most performant adapter.
- You can benchmark/persist against one perceived adapter identity but actually run another.

**Recommendation**
- Explicitly select DirectML device ID and persist that ID in decision metadata.
- Add an override (`SMOLPC_DML_DEVICE_ID`) for diagnostics.

---

### [P2] DirectML dynamic-shape perf guidance is not applied
**Where**
- `src-tauri/src/inference/session.rs:52-81`
- (absence of `with_dimension_override` usage)

**Issue**
- ORT docs call out DirectML performance sensitivity to known/static dimensions.
- Current session config does not set any dimension overrides.

**Impact**
- For decode workloads with dynamic dimensions, DirectML may underperform or miss expected gains.
- Most visible on weaker GPUs where overhead margin is small.

**Recommendation**
- Apply dimension overrides where feasible (at least stable axes like batch=1).
- Validate shape strategy against the Qwen ONNX graph input names.

---

### [P2] Single transient DirectML init failure becomes sticky CPU with no re-probe policy
**Where**
- `src-tauri/src/commands/inference.rs:600-620`
- `src-tauri/src/commands/inference.rs:631-637`
- `src-tauri/src/commands/inference.rs:312-319`

**Issue**
- If DirectML load fails once, final persisted backend becomes CPU.
- Subsequent loads prefer persisted CPU and skip DirectML attempt.

**Impact**
- Temporary runtime problems (e.g., missing DLL at first launch, transient environment issue) can disable DirectML indefinitely for that decision key.

**Recommendation**
- Persist failure counters/reason, but schedule periodic re-probe attempts (e.g., every N loads or after cooldown).

---

### [P2] DirectML candidate gating depends on hardware detector string semantics
**Where**
- `src-tauri/src/commands/inference.rs:174-217`
- `src-tauri/src/commands/inference.rs:510`
- `src-tauri/src/hardware/detector.rs:120-126`

**Issue**
- Candidate detection relies on a specific backend string (`"DirectX 12"`) from `hardware-query` mapping.
- If detection fails or backend labeling diverges, DirectML selection path is skipped.

**Impact**
- False negatives can prevent acceleration on actually-capable hardware.

**Recommendation**
- Treat hardware detection as hinting, not a hard gate.
- Prefer an actual EP registration probe for final capability.

---

### [P3] Backend store replace sequence is not crash-safe
**Where**
- `src-tauri/src/inference/backend_store.rs:136-141`

**Issue**
- Existing store file is removed before rename.
- Crash between delete and rename can lose persistence file.

**Impact**
- Loss of diagnostic/history metadata; not model-breaking.

**Recommendation**
- Use atomic replace semantics (`rename` over existing where supported, or `tempfile` persisted with platform-safe replace).

---

### [P3] Minor maintainability drift in decision model
**Where**
- `src-tauri/src/inference/backend.rs:38` (`RuntimeFailureFallback`)
- not used in selector/load paths.

**Issue**
- Unused reason variant and slightly diverged nomenclature from actual transitions.

**Impact**
- Low runtime impact; moderate clarity cost for future debugging.

**Recommendation**
- Remove or wire this reason into explicit transitions.

## Addendum: Claude Audit Cross-Check (Validated)

This section records which items from a secondary audit were validated against the current codebase and elevated to required work.

### Shipping Gate Must-Do (Validated)

1. **Non-preemptive benchmark timeout in `load_model`**
   - `tokio::time::timeout` wraps async code that performs long synchronous ORT work (`commit_from_file`, `session.run`), so timeout does not enforce a hard wall-clock budget for those blocking spans.
   - Required action:
     - Run benchmark orchestration off the async command path (`spawn_blocking`/worker thread + message passing).
     - Make timeout non-blocking for UI responsiveness; treat late benchmark result as optional post-load refinement.

2. **No user-visible progress/status during long model load + backend selection**
   - Frontend currently awaits `invoke('load_model', ...)` without progress stream.
   - `get_inference_backend_status` is registered in backend but not consumed in frontend, and there is no typed status contract in TS.
   - Required action:
     - Add load progress events/states (`validating`, `benchmarking_cpu`, `benchmarking_dml`, `initializing_session`, `ready`, `fallback`).
     - Add TypeScript types for backend status and surface backend + reason in UI.
     - Surface demotion/fallback events to users.

3. **Cancelled generation currently resets DirectML failure streak**
   - Success reset is called before cancellation check in both generation command paths.
   - Required action:
     - Check cancellation before calling `reset_directml_failures_on_success`.

4. **Backend decision store persistence ordering can record decisions before model load is truly usable**
   - Decision is persisted before tokenizer + generator construction completes.
   - Required action:
     - Persist decision only after generator construction succeeds.
     - On failure, persist explicit failed-load reason/state (or skip persist entirely).

5. **Backend decision store replace path is not crash-safe**
   - Current `remove_file` then `rename` can lose state on crash.
   - Required action:
     - Use atomic replace semantics (platform-safe replace implementation).

6. **Demotion has no natural recovery path**
   - After 3 consecutive failures, persisted CPU decision can remain sticky indefinitely.
   - Required action:
     - Add demotion TTL/cooldown or explicit retry policy.
     - Add manual “retry DirectML” control path.

7. **Benchmark methodology bias and instability**
   - Current benchmark is CPU-first only, 8 generated tokens, no warmup.
   - This is vulnerable to order effects and startup/JIT noise.
   - Required action:
     - Add warmup pass.
     - Increase token count for measurement stability.
     - Randomize or alternate run order, or normalize for order effects.

8. **`directml_ttft_ratio` zero-baseline handling masks regressions**
   - CPU TTFT `0` currently maps to ratio `1.0`.
   - Required action:
     - Return `INFINITY` when CPU TTFT is zero and DirectML TTFT is non-zero, and gate accordingly.

9. **DirectML DLL bare-name fallback is risky and non-deterministic**
   - Preload candidate list includes bare `DirectML.dll` path resolution.
   - Required action:
     - Remove bare-name fallback from preload path.
     - Fail loudly/diagnostically when bundled DLL cannot be resolved.

10. **DirectML KV cache path likely incurs heavy host->device transfer overhead**
    - Decode loop rebuilds many layer KV tensors from CPU-side arrays each token.
    - Required action:
      - Treat this as a required perf investigation for DirectML viability.
      - Add targeted profiling + optimization plan (IO binding/device memory strategy).

### Reviewed but Not Confirmed as Stated

1. **“State corruption window during demotion due interleaving `load_model`”**
   - Not confirmed in the specific form claimed.
   - Rationale:
     - `load_model`/`unload_model` are guarded by `generating` and return early during active generation.
     - Demotion reload is invoked while generation permit is still active, preventing model-change command interleaving.
   - Note:
     - There is still room to simplify and harden state management by consolidating related fields under one state mutex, but this is not currently a proven P0/P1 corruption bug.

## ORT Best-Practice Alignment

### What is well aligned
- Explicit EP registration via `with_execution_providers(...)` in ordered form.
- Use of `error_on_failure()` to avoid silent all-EP registration fallback ambiguity.
- Correct DirectML safety config:
  - `with_parallel_execution(false)`
  - `with_memory_pattern(false)`
- Dynamic runtime bundling and preloading strategy for Windows DirectML DLLs is coherent and checksum-verified in setup script.

### What is partially aligned / missing
- EP docs emphasize user configurability and explicit EP behavior; current selection policy is overly sticky under timeout/failure.
- DirectML docs expose `with_device_id`; implementation does not use it.
- DirectML perf guidance for fixed dimensions (`with_dimension_override`) is not leveraged.

## Functional Assessment: Will DirectML speed up weak hardware?

Short answer: **sometimes, but current policy will likely under-realize gains on exactly the low-end targets.**

### Weak integrated GPU (iGPU)
- Expected with current implementation: **often CPU remains selected**.
- Reasons:
  - 2s two-backend benchmark budget is strict for low-end devices.
  - Selection requires strong decode gain (`>=1.30x`) with tight TTFT guard (`<=1.15x`).
  - Dynamic-shape overhead can erode DirectML gains.

### Weak discrete GPU (dGPU)
- Expected with current implementation: **possible speedup, but not guaranteed to be selected**.
- Reasons:
  - dGPU can improve sustained decode throughput.
  - But default GPU0 selection may pick the wrong adapter on hybrid systems.
  - One timeout or transient init issue can persist CPU and block future DirectML attempts.

### Net assessment for PR #37
- Architecture is sound (explicit DirectML registration + fallback + diagnostics).
- Selection policy is conservative to a fault on weaker hardware; likely to bias toward CPU and reduce observed acceleration.

## Suggested Refinement Order (Next Session)

1. Make timeout/failure decisions retryable (non-sticky) instead of hard-persisting CPU.
2. Add DirectML device ID selection and persistence.
3. Introduce dimension overrides for stable axes and re-benchmark.
4. Replace hardware-string gating with EP registration probing.
5. Harden backend store atomic replace behavior.
6. Expand benchmark policy (more representative token count/profile and per-backend timing budget).

## References (Official Docs)

1. ORT crate execution provider re-exports:
   - https://docs.rs/ort/2.0.0-rc.11/ort/ep/index.html#reexports
2. ORT crate session EP registration guidance (`with_execution_providers`):
   - https://docs.rs/ort/2.0.0-rc.11/ort/session/struct.SessionBuilder.html#method.with_execution_providers
3. ORT crate DirectML EP docs (`with_device_id`, perf considerations):
   - https://docs.rs/ort/2.0.0-rc.11/ort/ep/struct.DirectML.html
4. ORT execution provider behavior and fallback notes:
   - https://ort.pyke.io/perf/execution-providers
5. ONNX Runtime EP architecture and priority/fallback model:
   - https://onnxruntime.ai/docs/execution-providers
6. ONNX Runtime DirectML EP official guidance:
   - https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html
