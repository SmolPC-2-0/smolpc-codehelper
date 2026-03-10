# Repo Context

Checked on: 2026-03-09
Purpose: describe the current code shape that the native OpenVINO GenAI plan must fit.

## Current Strengths

- `engine-host` already owns startup probe, backend persistence, and failure demotion
- the repo already supports multiple runtime engines behind one host boundary
- DirectML already exists as a dedicated runtime lane, not just a generic ORT session option
- model loading already understands backend-specific artifact directories

## Current Code Shape

1. ORT workspace pin
   - `Cargo.toml`
   - current pin: `ort = "=2.0.0-rc.11"` with `load-dynamic` and `directml`

2. Runtime adapter split
   - `crates/smolpc-engine-core/src/inference/runtime_adapter.rs`
   - current runtime variants:
     - `Ort`
     - `GenAiDirectMl`

3. Backend vocabulary
   - `crates/smolpc-engine-core/src/inference/backend.rs`
   - current backend enum only has:
     - `cpu`
     - `directml`

4. Model artifact loader
   - `crates/smolpc-engine-core/src/models/loader.rs`
   - current backend-aware directories only cover:
     - `cpu`
     - `dml`

5. Host probe and selection
   - `crates/smolpc-engine-host/src/main.rs`
   - current startup probe only ranks DirectML candidates
   - current persisted-decision key includes model, adapter identity, driver version, app version, ORT version, and optional DML device id
   - current load path waits for a short startup-probe budget and may persist the resulting fallback decision

6. Public status contract
   - `docs/ENGINE_API.md`
   - current status and examples are DML-specific

7. Global ORT initialization and loading
   - `crates/smolpc-engine-host/src/main.rs`
   - `crates/smolpc-engine-core/src/inference/mod.rs`
   - the process currently initializes ORT during startup, before backend selection
   - the ORT loader still allows bare-name / non-app-local resolution

## What Must Stay True

- `engine-host` remains the final selector
- launcher/apps do not duplicate ranking logic
- setup caches user intent, not backend policy
- runtime failure is visible and recoverable
- CPU remains the safe terminal fallback

## What The New Plan Requires

### Runtime adapters

Add a third runtime lane:

- `OpenVinoGenAi`

Final runtime engine identities should look like:

- `ov_genai_npu`
- `genai_dml`
- `ort_cpu`

### Backend vocabulary

Generalize backend enums, status, and reason codes to cover:

- `openvino_npu`
- `directml`
- `cpu`

### Host probe

Extend the startup probe to classify:

- Intel NPU hardware presence
- OpenVINO bundle availability
- OpenVINO NPU device visibility
- OpenVINO NPU driver version
- OpenVINO model artifact readiness

Also split:

- machine-scoped startup probe
- model-scoped preflight

The startup probe must not persist a negative model decision. Timed-out preflight must produce a temporary fallback only.

### Persisted decision key

Extend the stored key with native runtime tuple data:

- OpenVINO version
- OpenVINO GenAI version
- OpenVINO Tokenizers version
- NPU driver version
- OpenVINO model artifact version or hash
- prompt and generation buckets that materially affect native NPU behavior

### Loader expectations

Add a dedicated `openvino_npu/` artifact lane rather than overloading `cpu/` or `dml/`.

Use lane-specific manifests. Do not keep a shared root tokenizer contract.

### Diagnostics

Expose enough status for the launcher to show targeted messages such as:

- Intel NPU detected but driver missing
- Intel NPU detected but driver recommended for update
- OpenVINO runtime bundle incomplete
- OpenVINO preflight timed out and the current load fell back temporarily

### Runtime loading

Phase 0 must remove two current mismatches with the plan:

- ORT must not hard-fail engine startup before lane selection
- production runtime loading must not fall back to PATH or bare filenames

The final design requires lane-local lazy initialization and deterministic app-local absolute paths.

## Current Files To Reopen Before Implementation

1. `Cargo.toml`
2. `docs/ENGINE_API.md`
3. `crates/smolpc-engine-host/src/main.rs`
4. `crates/smolpc-engine-core/src/inference/runtime_adapter.rs`
5. `crates/smolpc-engine-core/src/inference/backend.rs`
6. `crates/smolpc-engine-core/src/models/loader.rs`
7. `crates/smolpc-engine-core/src/inference/genai/directml.rs`
