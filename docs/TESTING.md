# Testing

## Philosophy

SmolPC has two testing tiers:

1. **Unit tests validate logic.** ChatML rendering, config parsing, backend selection state machines, thinking filter, error classification, type serialization contracts, path validation — all testable without hardware.
2. **Live tests validate hardware paths.** After changing generation config, backend selection, or model loading, start the engine with `SMOLPC_FORCE_EP=<backend>` and curl a streaming chat completion. Verify output quality, not just "no crash."

Unit tests run in CI on every push. Live tests run manually on physical hardware — there is no way to simulate a real NPU or discrete GPU in CI.

## Running Tests

```bash
# Engine and connector tests
cargo test -p smolpc-engine-core
cargo test -p smolpc-engine-host
cargo test -p smolpc-engine-client
cargo test -p smolpc-connector-common
cargo test -p smolpc-connector-blender
cargo test -p smolpc-connector-gimp
cargo test -p smolpc-connector-libreoffice

# All workspace tests
cargo test --workspace

# Frontend checks
cd app && npm run check && npm run lint
```

## Unit Test Inventory

### Engine Host Tests

**Source:** `engine/crates/smolpc-engine-host/src/tests.rs` (~49 test functions)

The largest test file, covering the engine's core logic:

**ChatML parsing and request conversion (5 tests):**
- `request_to_prompt_renders_chatml` — basic ChatML message formatting with `<|im_start|>`/`<|im_end|>` tags
- `request_to_prompt_preserves_preformatted_chatml_single_user_message` — pre-formatted ChatML strings pass through unchanged
- `request_to_prompt_injects_nothink_in_system_message` — `/nothink` directive appended to system message for Qwen3
- `request_to_prompt_prepends_nothink_system_message_when_none_present` — creates a system message if absent
- `request_to_prompt_nothink_false_does_not_inject` — respects the disable flag

**Thinking filter (4 tests):**
- `model_has_thinking_mode_matches_qwen3` — Qwen3 identified, Qwen2.5 not
- `thinking_filter_streaming_suppresses_think_block` — `<think>...</think>` tags stripped from token stream
- `thinking_filter_streaming_split_tags` — handles tags split across token boundaries
- `thinking_filter_passes_non_thinking_text` — non-thinking text passes through unchanged

**Request configuration (5 tests):**
- `request_to_config_rejects_zero_max_tokens` — zero is invalid
- `request_to_config_caps_max_tokens_to_hard_limit` — enforces `OPENVINO_MAX_TOKENS_HARD_CAP`
- `openvino_qwen3_request_defaults_match_upstream_non_thinking_guidance` — temperature=0.7, top_p=0.8, top_k=20
- `structured_messages_follow_loaded_openvino_runtime_not_backend_name` — structured chat depends on runtime, not backend name

**Authentication (1 test):**
- `auth_compare_is_constant_time_functionally` — timing-safe comparison correctness

**Engine startup and degradation (4 tests):**
- `engine_state_startup_succeeds_with_missing_ort_bundle` — missing ORT DLLs don't crash
- `engine_state_startup_succeeds_with_missing_openvino_bundle` — missing OpenVINO DLLs don't crash
- `engine_state_startup_succeeds_with_missing_openvino_plugin` — missing NPU plugin handled gracefully
- Tests use `tempdir()` with mock DLL files to simulate partial runtime installations

**Backend selection state machine (6 tests):**
- `backend_selection_prefers_directml_when_both_candidates_ready` — DirectML is default preference
- `backend_selection_prefers_directml_when_openvino_is_unavailable` — fallback chain works
- `directml_model_reload_releases_current_adapter_before_rebuild` — adapter release logic
- `backend_selection_keeps_persisted_cpu_choice` — persisted decisions survive restart
- `backend_selection_keeps_persisted_openvino_choice_when_candidate_is_ready` — NPU persistence
- `backend_selection_falls_back_when_persisted_openvino_candidate_is_unavailable` — fallback from stale persistence

**Model readiness and probe integration (5 tests):**
- `check_model_response_reports_lane_readiness` — CPU/DirectML/OpenVINO lanes reported separately
- `check_model_response_keeps_directml_ready_while_probe_is_pending` — DML stays ready during NPU probe
- `check_model_response_blocks_directml_when_probe_confirms_no_candidate` — DML blocked when no discrete GPU
- `check_model_response_reports_openvino_lane_ready_when_probe_is_ready` — NPU lane tracks probe
- `check_model_response_reports_shared_openvino_artifact_readiness` — shared artifacts affect multiple lanes

**Error classification (5 tests):**
- `classify_startup_model_error_flags_unknown_model_as_non_retryable`
- `classify_startup_model_error_flags_missing_assets_as_non_retryable`
- `classify_startup_model_error_flags_memory_pressure` — retryable with memory hint injection
- `classify_startup_model_error_ignores_non_memory_substrings` — avoids false positives
- `with_memory_pressure_hint_is_idempotent` — no double-hinting

**RAM-based model selection (4 tests):**
- `ram_selection_32gb_picks_largest` / `ram_selection_16gb_picks_largest_at_threshold` / `ram_selection_12gb_picks_smaller` / `ram_selection_4gb_returns_none`

**Audio and TTS (7 tests):**
- `audio_body_must_be_f32_aligned` — 4-byte alignment for Whisper input
- `f32_le_bytes_round_trip` — serialization roundtrip
- `whisper_model_path_structure` — model directory layout
- `tts_binary_not_in_resource_dir` / `tts_binary_found_in_binaries_dir` — sidecar discovery
- `audio_speech_request_defaults` / `audio_speech_request_with_overrides` — TTS request JSON

**Configuration (3 tests):**
- `process_idle_exit_is_disabled_by_default` / `idle_timeout_zero_disables_timer` / `model_idle_unload_keeps_default_when_unset`

### Engine Core Tests

Inline `#[cfg(test)]` modules in source files:

- **runtime_loading.rs** — fingerprint determinism, path validation, centralized DLL loading enforcement (source-invariant scan)
- **openvino.rs** — metric rounding, bundle validation rejection, `/nothink` injection (prepend, create, idempotent)

### Type Contract Tests

**Source:** `crates/smolpc-assistant-types/tests/contracts.rs` (5 tests)

These tests lock the JSON serialization format to prevent accidental breaking changes in the wire protocol:

- `app_mode_serializes_to_expected_wire_values` — `AppMode::Impress` → `"impress"`
- `mode_config_uses_camel_case_keys` — verifies `providerKind`, `systemPromptKey`, `showExport` (not snake_case)
- `assistant_stream_events_use_kind_tag` — event envelope uses `"kind"` discriminator
- `assistant_response_uses_tool_results_key` — `toolResults` not `tool_results`
- `setup_status_uses_expected_wire_keys_and_values` — `overallState`, `canPrepare`, enum values

These tests matter because the Svelte frontend consumes these JSON payloads directly — a renamed field breaks the UI silently.

### Security Tests

**Source:** `app/src-tauri/src/security/tests.rs` (7 tests)

Content and file size validation:

- `test_valid_content` / `test_empty_content` / `test_content_at_limit` — pass cases
- `test_content_over_limit` — content exceeding 10 MB rejected with "too large" message
- `test_large_content_error_message` — error format includes "MB" and "max"
- `test_file_size_validation` — small file passes, >10 MB fails
- `test_file_size_validation_error_message` — error includes formatted size

### Connector Tests

**Blender connector** (14 tests across 4 modules):

- **setup.rs** (5): addon provisioning, marker short-circuit, probe/enable failure messages
- **state.rs** (2): default cache returns disconnected, fresh data returns connected
- **provider.rs** (7): tool definitions after connect, port-occupied error, scene_current execution, RAG retrieval, graceful degradation on bad metadata, live scene detail in status, reconnect restores tools
- **rag.rs** (2): top match retrieval, disabled index returns empty

**GIMP connector** (9 tests across 2 modules):

- **runtime.rs** (4): Python resolution priority (prepared > venv > system), strict mode enforcement
- **planner.rs** (5): JSON parsing, code fence extraction, invalid tool rejection, non-JSON fallback, retry logic

**LibreOffice connector** (13 tests across 2 modules):

- **executor.rs** (9): Writer tool execution + streaming, Impress JSON fallback parsing, stringified JSON arguments, trailing garbage recovery, comma-for-colon repair, summary failure fallback, cancellation fallback, timeout fallback
- **resources.rs** (4): nested path fallback, direct path preference, missing asset error context, dev mode resolution

### Engine Client Test Utilities

**Source:** `engine/crates/smolpc-engine-client/src/test_utils.rs`

Not tests themselves, but test infrastructure:

- `RuntimeEnvGuard` — RAII guard that saves and restores environment variables on drop
- `set_env_var()` / `remove_env_var()` — helpers for test env manipulation
- Used by engine host tests to safely modify `SMOLPC_*` env vars without leaking state between tests

### Benchmark Statistics Tests

**Source:** `engine/crates/smolpc-benchmark/src/stats.rs` (5 tests)

- `empty_returns_none` — empty input produces no stats
- `single_value` — single value: mean=median=min=max, std_dev=0
- `known_values` — [10,20,30,40,50]: mean=30, std_dev≈14.14
- `percentiles_interpolate` — linear interpolation: P50=5.5, P90=9.1 for 1..=10
- `two_values` — [100,200]: mean=150, median=150

## Integration Testing

Integration tests require real hardware and are not automated in CI.

### Protocol

After any change to generation config, backend selection, or model loading:

1. Start the engine with a forced backend:
   ```powershell
   $env:SMOLPC_FORCE_EP = "cpu"           # or "directml", "openvino_npu"
   $env:SMOLPC_ENGINE_TOKEN = "test-token"
   cargo run -p smolpc-engine-host
   ```

2. Send a streaming chat completion:
   ```bash
   curl -s http://localhost:19432/v1/chat/completions \
     -H "Authorization: Bearer test-token" \
     -H "Content-Type: application/json" \
     -d '{"messages":[{"role":"user","content":"Write a Python function that checks if a number is prime"}],"stream":true,"max_tokens":512}'
   ```

3. Verify:
   - Response streams tokens (not a single dump)
   - Output is coherent code, not garbage or repetition
   - Generation stops at a reasonable length (not runaway)
   - TTFT and tokens/sec are within expected range for the backend
   - No crash or "unknown exception" errors

### Hardware Coverage

Three test machine configurations cover the backend matrix:

| Machine | CPU | GPU | NPU | RAM | Backends |
|---|---|---|---|---|---|
| Core Ultra | Intel Core Ultra | Intel Arc (integrated) | Intel AI Boost | 16 GB | CPU, NPU |
| Intel CPU | Intel i-series | None | None | 8 GB | CPU only |
| RTX workstation | Intel i5 | NVIDIA RTX 2000 | None | 16 GB | CPU, DirectML |

Each machine tests a different backend path. The Core Ultra machine is the primary NPU test target. The RTX machine validates DirectML with a discrete GPU. The Intel CPU machine validates the fallback path.

## CI Pipeline

**Source:** `.github/workflows/ci.yml`

7 jobs run on every push:

### 1. Frontend Quality

- TypeScript and Svelte type checking (`npm run check`)
- NPM vulnerability audit (high+ severity)
- Runs on: `windows-latest`, Node 23

### 2. Boundary Enforcement

- Runs `check-boundaries.ps1` (see below)
- Prevents architectural regression

### 3. Engine Tests (MSRV — Rust 1.88.0)

- Tests 7 crates: `smolpc-engine-core`, `smolpc-engine-client`, `smolpc-engine-host`, `smolpc-connector-common`, `smolpc-connector-blender`, `smolpc-connector-gimp`, `smolpc-connector-libreoffice`
- Validates minimum supported Rust version

### 4. Engine Tests (Stable)

- Same 7 crates, latest stable Rust
- Catches issues with newer compiler versions

### 5. Tauri Build Check

- `cargo check -p smolpc-desktop` — verifies the desktop app compiles
- Does not produce a binary (that requires runtime DLLs)

### 6. Incremental Style Gates

- Only checks changed files (not the whole codebase)
- **Prettier** for `.js`, `.ts`, `.tsx`, `.svelte`, `.json`, `.md`, `.css`, `.yml`, `.html`
- **ESLint** for `.js`, `.ts`, `.tsx`, `.svelte`
- **rustfmt** for `.rs` files

### 7. Rust Security Audit

- `cargo audit` — checks for known vulnerabilities in dependencies

## Boundary Enforcement

**Source:** `scripts/check-boundaries.ps1`

9 rules enforced in CI that prevent architectural regression:

### File presence rules (must NOT exist)

| Path | Why |
|---|---|
| `app/src-tauri/src/inference` | Legacy app-owned inference module — engine owns all inference |
| `app/src-tauri/src/models` | Legacy app-owned model module — engine owns model management |
| `app/src-tauri/src/commands/ollama.rs` | Legacy Ollama integration — fully removed |
| `app/src/lib/stores/ollama.svelte.ts` | Legacy Ollama frontend store — fully removed |
| `app/src/lib/types/ollama.ts` | Legacy Ollama frontend types — fully removed |

### Content rules (must NOT contain)

| File | Pattern | Why |
|---|---|---|
| `commands/mod.rs` | `\bollama\b` | No Ollama references in command routing |
| `app/src-tauri/Cargo.toml` | `smolpc-engine-host` | Desktop app must not depend on engine-host directly (only via engine-client) |
| `app/src-tauri/src/**/*.rs` | `use smolpc_engine_host` | No direct imports of engine-host in the app |

The engine-host boundary rule is critical: the desktop app communicates with the engine over HTTP via `smolpc-engine-client`. If the app imports `smolpc-engine-host` directly, it defeats process isolation and makes FFI crashes take down the UI.

## Release Pipeline

**Source:** `.github/workflows/release.yml`

Triggered by git tags matching `v*.*.*` or manual dispatch.

### Build steps

1. Setup: Node 23, Rust 1.88.0
2. Runtime staging (with caching):
   - DirectML runtime DLLs
   - OpenVINO runtime DLLs
   - Bundled Python 3.12 runtime
3. Build engine host sidecar (release mode)
4. Stage sidecar binary to `app/src-tauri/binaries/`

### Artifact validation

8 required files must be present before the Tauri build:

| Artifact | Purpose |
|---|---|
| `libs/onnxruntime.dll` | ONNX Runtime core |
| `libs/DirectML.dll` | DirectML provider |
| `libs/onnxruntime-genai.dll` | ORT GenAI extension |
| `libs/openvino/openvino_genai_c.dll` | OpenVINO GenAI C API |
| `libs/openvino/tbb12.dll` | Intel TBB (OpenVINO dependency) |
| `libs/openvino/openvino_intel_cpu_plugin.dll` | OpenVINO CPU plugin |
| `resources/python/payload/python.exe` | Bundled Python runtime |
| `binaries/smolpc-engine-host-*.exe` | Engine sidecar binary |

### Size gate

After the Tauri build completes, the NSIS installer must be >200 MB. An installer <50 MB fails the pipeline — this catches builds where resources were not properly bundled.

## Benchmark Infrastructure

**Source:** `engine/crates/smolpc-benchmark/`

A standalone CLI tool for performance measurement across backends.

### Modules

| Module | Purpose |
|---|---|
| `main.rs` | CLI (clap), subcommands: `run`, `compare` |
| `config.rs` | Benchmark configuration (backends, warmup, cooldown) |
| `runner.rs` | Test execution loop with measurement |
| `prompts.rs` | Standard prompt set for consistent comparison |
| `stats.rs` | Descriptive statistics: mean, median, p90, p95, std_dev, min, max |
| `output.rs` | Report types: `BenchmarkReport`, `BackendModelResult`, `HardwareSnapshot` |
| `compare.rs` | Side-by-side comparison of two benchmark reports |
| `resource_sampler.rs` | Process-specific CPU% and memory sampling via `sysinfo` |
| `engine_lifecycle.rs` | Engine spawn/health/shutdown for benchmark runs |
| `reliability.rs` | Error rate and recovery tracking |

### Metrics collected

- **TTFT** (time to first token) — milliseconds
- **Total time** — end-to-end generation duration
- **Tokens/sec** — decode throughput
- **Memory** — before, during, after, peak (process-specific via sysinfo)
- **CPU utilization** — percentage during generation

### Statistical analysis

The `Stats` struct computes population statistics with linear interpolation for percentiles:

```rust
pub struct Stats {
    pub mean: f64,
    pub median: f64,
    pub p90: f64,
    pub p95: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
}
```

### Usage

```bash
cargo run -p smolpc-benchmark -- run \
  --machine "core-ultra-16gb" \
  --backends cpu,openvino_npu \
  --warmup 2 \
  --cooldown 5
```

The `compare` subcommand diffs two JSON reports side-by-side.

## Test Count Summary

| Crate / Module | Test Functions |
|---|---|
| Engine host (`tests.rs`) | ~49 |
| Engine core (inline) | ~10 |
| Type contracts | 5 |
| Security | 7 |
| Blender connector | 14 |
| GIMP connector | 9 |
| LibreOffice connector | 13 |
| Benchmark stats | 5 |
| **Total** | **~112** |

All engine and connector tests run on both MSRV (Rust 1.88.0) and stable in CI.
