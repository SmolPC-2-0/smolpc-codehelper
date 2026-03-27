# Testing

## Philosophy

SmolPC has two testing tiers:

1. **Unit tests validate logic.** ChatML rendering, config parsing, backend selection state machines, thinking filter, error classification, type serialization contracts, path validation — all testable without hardware.
2. **Live tests validate hardware paths.** After changing generation config, backend selection, or model loading, start the engine with `SMOLPC_FORCE_EP=<backend>` and curl a streaming chat completion. Verify output quality, not just "no crash."

Unit tests run in CI on every push to main and on all pull requests. Live tests run manually on physical hardware — there is no way to simulate a real NPU or discrete GPU in CI.

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

# Desktop app tests
cargo test -p smolpc-desktop

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

**40 tests across 7 files** — inline `#[cfg(test)]` modules in source files:

**inference/backend.rs (12 tests):**
- `decision_key_fingerprint_changes_when_gpu_driver_changes` / `_when_openvino_bundle_changes` / `_when_openvino_npu_tuning_changes` — decision key fingerprint sensitivity
- `benchmark_gate_requires_speedup_and_ttft_guardrail` — DirectML benchmark gate enforces speedup ratio and TTFT regression limit
- `directml_is_demoted_after_three_consecutive_failures` — demotion threshold enforcement
- `ttft_ratio_is_infinite_when_cpu_ttft_is_zero_and_directml_non_zero` / `ttft_ratio_is_one_when_both_ttft_values_are_zero` — edge cases in TTFT ratio math
- `directml_backend_serializes_without_split_initialism` / `openvino_backend_serializes_with_lane_name` — serde wire format stability
- `backend_status_serializes_lane_based_surface` — lane-based JSON structure, no legacy field leaks
- `check_model_response_any_ready_requires_a_ready_lane` / `_is_false_when_only_artifacts_exist` — readiness vs artifact availability distinction

**models/loader.rs (6 tests):**
- `default_models_dir_is_absolute_and_points_to_models_folder` — path contract
- `resolve_models_dir_uses_non_empty_override` / `_falls_back_for_empty_override` — env override resolution
- `backend_dir_names_are_stable` — `DirectML.as_dir() == "dml"`
- `openvino_manifest_file_uses_lane_directory` — manifest path structure
- `default_models_dir_prefers_models_suffix` — suffix stability

**inference/genai/directml.rs (6 tests):**
- `last_logits_row_bounds_rank1_tensor` / `_rank3_tensor` — logits tensor row extraction for different ranks
- `last_logits_row_bounds_rejects_non_positive_dims` / `_rejects_empty_dims` — error handling for invalid dimensions
- `missing_genai_dll_only_disables_directml_lane` / `missing_directml_dll_only_disables_directml_lane` — partial bundle degradation (ORT core stays valid)

**inference/genai/openvino.rs (5 tests):**
- `metric_ms_pair_rounds_float_values` — metric rounding (12.6 -> 13)
- `runtime_available_rejects_invalid_bundle` — bundle validation rejection
- `inject_nothink_prepends_to_existing_system_message` / `_creates_system_message_if_absent` / `_is_idempotent` — `/nothink` injection for NPU non-thinking mode

**inference/runtime_loading.rs (4 tests):**
- `fingerprint_changes_when_bundle_root_changes` / `_when_versions_change` — fingerprint determinism
- `runtime_library_loader_requires_absolute_paths` — path validation (relative paths rejected)
- `runtime_loading_is_centralized` — source-invariant scan that fails CI if any file outside `runtime_loading.rs` contains `Library::new()` or `load_with_flags()`

**inference/backend_store.rs (4 tests):**
- `round_trip_persistence` — serialize/deserialize backend decision through JSON file
- `multiple_records_for_same_model_are_retained_when_fingerprints_differ` — fingerprint-keyed storage
- `records_can_persist_failure_counters_without_persisted_winner` — failure tracking without a decision
- `invalid_json_store_is_reset_to_empty` — corrupt file recovery

**models/registry.rs (3 tests):**
- `available_models_ordering` — model list order: `qwen2.5-1.5b-instruct` before `qwen3-4b`
- `available_models_include_supported_ids` — supported model IDs present
- `get_model_returns_supported_models_only` — unknown model IDs return `None`

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

### Desktop App Tests

**Source:** `app/src-tauri/src/` (76 tests across 19 modules, excluding security tests listed above)

Run command: `cargo test -p smolpc-desktop`

**engine/handle.rs (10 tests):**
- `handle_is_clone` — `EngineSupervisorHandle` implements `Clone`
- `get_client_if_ready_returns_none_when_no_client` / `_returns_client_after_broadcast` — synchronous client access
- `get_client_times_out_when_not_running` / `_returns_error_on_terminal_state` / `_returns_client_when_broadcast_before_state` — async client acquisition with timeout, terminal state detection
- `ensure_started_sends_command` / `shutdown_sends_command` — command dispatch via mpsc channel
- `refresh_status_is_fire_and_forget` / `set_runtime_mode_sends_command` — fire-and-forget operations

**commands/engine_client_adapter.rs (8 tests):**
- `startup_mode_maps_to_runtime_mode` / `_reads_runtime_env_overrides` — startup mode to runtime mode conversion
- `normalize_contract_state_rejects_unknown_values` — unknown state string handling
- `map_engine_status_prefers_canonical_contract_fields` / `_falls_back_to_last_error_payload` — DTO mapping from engine status
- `normalize_startup_policy_trims_and_validates` — input sanitization
- `ensure_started_request_dto_serializes_with_contract_field_names` / `readiness_dto_deserializes_from_contract_shape` — wire format stability

**commands/inference.rs (8 tests):**
- `sysinfo_memory_unit_heuristic_targets_supported_ram_range` / `sysinfo_032_contract_matches_ci_host_expectation` — memory detection heuristics
- `unload_in_progress_error_detection_matches_engine_message` — error classification
- `classify_memory_level_uses_warning_and_critical_thresholds` — memory warning/critical thresholds
- `recommend_switch_returns_false_when_current_model_is_already_recommended` / `_triggers_for_heavy_mode_even_when_level_is_normal` — model recommendation logic
- `heavy_mode_detection_flags_blender_and_gimp` — mode-based memory classification
- `memory_message_reports_auto_unload_when_triggered` — auto-unload messaging

**lib.rs (6 tests):**
- `app_local_data_resolution_prefers_tauri_path_when_available` / `_uses_dirs_local_data_fallback_in_debug_mode` / `_uses_last_resort_fallback_when_dirs_root_is_missing` / `_stays_unavailable_outside_debug_mode` — app local data dir resolution chain
- `ensure_fallback_app_local_data_dir_creates_directory_on_disk` — directory creation
- `build_managed_state_passes_app_local_data_dir_to_setup_and_providers` — state wiring

**engine/mod.rs (6 tests):**
- `is_running_returns_true_for_running_state` / `_returns_false_for_non_running_states` — lifecycle state query
- `is_terminal_only_for_failed` — terminal state detection
- `valid_transitions_are_accepted` / `invalid_transitions_are_rejected` — lifecycle state machine transition rules
- `lifecycle_state_serializes_with_tag` — serialization format

**app_paths.rs (5 tests):**
- `bundled_resource_dir_resolution_preserves_direct_tauri_root` / `_normalizes_nested_resources_root` — Tauri resource path normalization
- `_uses_debug_fallback_when_tauri_is_unusable` / `_skips_debug_fallback_outside_debug_builds` — debug vs release behavior
- `_returns_none_for_unusable_candidates` — graceful degradation

**commands/audio.rs (5 tests):**
- `mono_mix_stereo_to_mono` — stereo-to-mono channel mixing
- `resample_passthrough_at_16k` / `resample_48k_to_16k_reduces_samples` / `resample_44100_to_16k_reduces_samples` — sample rate conversion for Whisper input
- `recording_buffer_cap_constant_is_sane` — buffer size bounds

**provisioning/extractor.rs (4 tests):**
- `test_extract_zip_creates_files` / `_strips_common_prefix` — ZIP extraction with prefix normalization
- `test_extract_zip_cancel` — cancellation during extraction
- `test_copy_dir_recursive` — recursive directory copy

**provisioning/manifest.rs (4 tests):**
- `test_parse_valid_manifest` / `test_reject_unsupported_version` — manifest parsing and version validation
- `test_verify_sha256_correct` / `_mismatch` — SHA256 integrity verification

**setup/provision.rs (4 tests):**
- `prepare_setup_creates_app_local_directories_and_python_runtime` — directory and runtime preparation
- `_does_not_touch_external_user_profile_roots` — isolation from user profile
- `_provisions_blender_addon_when_blender_is_detected` — conditional Blender addon setup
- `_provisions_gimp_plugin_runtime_when_gimp_is_detected` — conditional GIMP plugin setup

**provisioning/source.rs (3 tests):**
- `test_models_exist_false_when_no_dir` — model detection with missing directory
- `test_detect_sources_no_internet` / `_with_internet` — source detection modes

**Remaining modules (1-2 tests each):**
- **hardware/detector.rs** (2): memory unit heuristic, raw memory to GB conversion
- **modes/config.rs** (2): mode config list ordering, Impress slides label
- **setup/status.rs** (2): collect setup status returns all required items, missing Python manifest reported
- **setup/models.rs** (2): bundled model missing manifest, bundled model ready when manifest exists
- **setup/state.rs** (2): setup cache round-trip through disk, backup used when primary is corrupt
- **modes/code.rs** (1): code provider returns idle state
- **modes/registry.rs** (1): LibreOffice modes share one provider family
- **commands/modes.rs** (1): mode status DTO uses camelCase keys

### Connector Tests

**Blender connector** (32 tests across 7 modules):

- **bridge.rs** (9): token exact-match comparison, token round-trip persistence, health route is public (no auth), scene_current requires valid bearer token, scene_current returns snapshot payload, scene_update mutates shared cache, scene_update rejects oversized bodies, port conflict produces friendly error, bridge stop shuts down background task
- **provider.rs** (9): tool definitions after bridge start, port-occupied error, scene_current tool returns snapshot, RAG context retrieval with loaded metadata, retrieval load failure degrades gracefully, live scene detail in status, reconnect restores tools after disconnect, status reports missing when host unavailable, provider starts bridge before addon provisioning
- **setup.rs** (5): addon provisioning, marker short-circuit, probe/enable failure messages, not-prepared status when Blender installed
- **executor.rs** (3): scene query skips RAG lookup, workflow question uses RAG and emits tokens, cancellation returns cancelled result
- **state.rs** (2): default cache returns no-scene message, fresh data returns connected scene
- **rag.rs** (2): top match retrieval, disabled index returns empty results
- **response.rs** (2): Blender messages marked non-undoable, RAG context list parsing

**GIMP connector** (28 tests across 6 modules):

- **setup.rs** (7): GIMP 2 path rejection, major version CLI output parsing, leading number handling, ambiguous path validation via version command, GIMP 3 acceptance, plugin not-prepared status, plugin provisioning + marker write
- **planner.rs** (5): JSON selector parsing, code fence extraction, invalid tool rejection, non-JSON fallback, retry on first non-JSON output
- **executor.rs** (5): info query success, fast-path edit success, planned call-api edit, answer without connection when selector chooses none, user-friendly error when edit requires connection
- **runtime.rs** (4): Python resolution priority (prepared > venv > system), strict mode enforcement when dev fallback disabled
- **provider.rs** (4): disconnected state reports honest error, connected state includes tool definitions, status reports missing GIMP, rejects GIMP 2 installs
- **heuristics.rs** (3): metadata query detection, region blur fast path detection, rotate fast path detection

**LibreOffice connector** (26 tests across 5 modules):

- **executor.rs** (9): Writer tool execution + streaming, Impress JSON fallback parsing, stringified JSON arguments, trailing brace garbage recovery, trailing whitespace recovery, comma-for-colon repair (ignored — see below), summary failure fallback, cancellation fallback, timeout fallback
- **provider.rs** (7): Writer status connects and filters tools, Impress status filters slides tools, unsupported mode returns error, runtime start failure honest error, missing resources error, concurrent status refresh survival, retryable runtime failure detection
- **resources.rs** (4): nested path fallback, direct path preference, missing asset error context, dev mode resolution
- **response.rs** (3): fallback summary uses document count/names, error payload marks result as error, messages marked non-undoable
- **runtime.rs** (3): config from layout sets entrypoint, stdio transport config sets log dir env, summary describes document server

### Connector-Common Tests

**Source:** `crates/smolpc-connector-common/src/` (16 tests across 5 modules)

Run command: `cargo test -p smolpc-connector-common`

- **launch.rs** (5): process detection skips spawn when already running, spawns when absent, executable path exact-match, GIMP launch errors for missing executable, LibreOffice launch errors for missing executable
- **python.rs** (4): bundled Python reports not-prepared when payload missing, reports staged payload can prepare, prepare copies staged payload, resolve prepared Python command detects runtime
- **host_apps.rs** (3): cached path wins when it still exists, standard path used when cached is missing, path lookup accepts GIMP 3 executable name
- **manifests.rs** (3): validate manifest rejects empty values, missing expected paths reports relative entries, load manifest parses camelCase file
- **text_generation.rs** (1): TextStreamer trait propagates cancelled result

### Engine Client Tests

**Source:** `engine/crates/smolpc-engine-client/src/lib.rs` (18 tests)

Run command: `cargo test -p smolpc-engine-client`

**Runtime env override parsing (3 tests):**
- `runtime_env_overrides_default_when_unset` — defaults to `Auto` mode when no env vars
- `runtime_env_overrides_parse_force_ep_tokens` — parses `cpu`, `DIRECTML`, `dml`, ignores `unknown`
- `runtime_env_overrides_parse_dml_device_id` — parses numeric device ID, ignores non-numeric

**Error and metrics (2 tests):**
- `parse_error_message_extracts_nested_message` — extracts `error.message` from JSON
- `fallback_stream_metrics_reflects_emitted_chunks` — token count and TTFT tracking

**Model response parsing (3 tests):**
- `parse_models_response_rejects_missing_data_array` — validates `data` array exists
- `parse_models_response_rejects_unknown_only_models` — unknown model IDs rejected
- `parse_models_response_accepts_known_model` — `qwen2.5-1.5b-instruct` accepted

**Running host policy (2 tests):**
- `running_host_policy_restarts_when_protocol_is_incompatible_and_idle` — incompatible+idle triggers restart
- `running_host_policy_rejects_forced_override_when_busy` — forced override while busy produces rejection with context

**API version compatibility (2 tests):**
- `version_major_extracts_major_component` — parses `"2.3.4"` -> `2`, handles edge cases
- `engine_api_major_compatible_requires_equal_or_higher_major` — major version gate

**Token and serialization (2 tests):**
- `load_or_create_token_creates_private_file` — creates 48-char alphanumeric token, idempotent reload
- `startup_mode_serializes_as_contract_value` — `DirectmlRequired` -> `"directml_required"`

**Engine status parsing (4 tests):**
- `engine_status_parses_canonical_readiness_fields` — full status payload deserialization
- `engine_status_keeps_legacy_payload_compatible` — minimal legacy payloads still parse
- `engine_status_readiness_prefers_ready_flag_and_state` — `is_ready()` logic
- `engine_status_failure_message_prefers_last_startup_error` — `is_failed()` and failure message extraction

### Engine Client Test Utilities

**Source:** `engine/crates/smolpc-engine-client/src/test_utils.rs`

Not tests themselves, but test infrastructure:

- `RuntimeEnvGuard` — RAII guard that saves and restores environment variables on drop
- `set_env_var()` / `remove_env_var()` — helpers for test env manipulation
- `with_runtime_env()` — closure-based helper that acquires the env lock, sets vars, runs the test, and restores on drop
- Used by engine client and engine host tests to safely modify `SMOLPC_*` env vars without leaking state between tests

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

7 jobs run on every push to main and on all pull requests:

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
- **Prettier** for `.cjs`, `.mjs`, `.js`, `.jsx`, `.ts`, `.tsx`, `.json`, `.md`, `.css`, `.scss`, `.yml`, `.yaml`, `.html`, `.svelte`
- **ESLint** for `.js`, `.jsx`, `.ts`, `.tsx`, `.svelte`
- **rustfmt** for `.rs` files

### 7. Rust Security Audit

- `cargo audit` — checks for known vulnerabilities in dependencies

## Boundary Enforcement

**Source:** `scripts/check-boundaries.ps1`

8 rules enforced in CI that prevent architectural regression (5 `Assert-PathAbsent` + 3 content checks):

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
| `app/src-tauri/src/commands/mod.rs` | `\bollama\b` | No Ollama references in command routing |
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

After the Tauri build completes, the NSIS installer size is checked. An installer smaller than 50 MB fails the pipeline — this catches builds where runtime DLLs, models, or Python were not properly bundled. A properly bundled installer is expected to exceed 200 MB.

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
| Engine host (`tests.rs`) | 49 |
| Engine core (7 files) | 40 |
| Engine client | 18 |
| Type contracts | 5 |
| Security | 7 |
| Connector-common (5 modules) | 16 |
| Blender connector (7 modules) | 32 |
| GIMP connector (6 modules) | 28 |
| LibreOffice connector (5 modules) | 26 |
| Benchmark stats | 5 |
| Desktop app (19 modules) | 76 |
| **Total** | **~302** |

All engine and connector tests run on both MSRV (Rust 1.88.0) and stable in CI. Desktop app tests run via `cargo test -p smolpc-desktop` (included in `cargo test --workspace`).

## Test Isolation Patterns

Tests that modify global state use several isolation patterns to avoid flaky failures:

**Environment variable isolation:**
- `EnvVarGuard` (engine host) — RAII struct that saves the current value of an env var on construction and restores it on drop, even if the test panics
- `RuntimeEnvGuard` (engine client) — similar RAII guard specialized for `SMOLPC_FORCE_EP` and `SMOLPC_DML_DEVICE_ID`
- `with_runtime_env()` (engine client `test_utils.rs`) — closure-based helper that acquires a global lock, sets env vars, runs the test, and restores on drop

**Serializing env-mutating tests:**
- Global `OnceLock<Mutex<()>>` guards are used in both the engine host (`env_lock()`) and engine client (`env_lock()`) test modules. Tests that mutate environment variables acquire this lock before running. This prevents parallel test execution from producing non-deterministic results, since `cargo test` runs tests in parallel by default and env vars are process-global.

**Filesystem isolation:**
- `tempdir()` (from the `tempfile` crate) creates a unique temporary directory per test. Used extensively in runtime loading, backend store, connector setup, provisioning, and extractor tests. The directory is automatically deleted when the `TempDir` guard drops.

**Mock implementations:**
- Connector crates use `#[cfg(test)]` mock implementations for bridge, runtime, and transport layers. This allows provider and executor tests to run without real Blender, GIMP, or LibreOffice processes.

**When to use `#[test]` vs `#[tokio::test]`:**
- Use `#[test]` for synchronous logic: pure functions, serialization, path resolution, state machine transitions
- Use `#[tokio::test]` for async code: provider status checks, executor flows, channel-based handle operations, setup provisioning

## Ignored Tests

Tests annotated with `#[ignore = "reason"]` document known limitations and are excluded from CI. Run them explicitly with:

```bash
cargo test -- --ignored
```

Currently ignored:
- `extract_tool_call_repairs_comma_for_colon_in_nested_objects` (LibreOffice executor) — `repair_json` heuristic cannot reliably distinguish comma-as-separator from comma-as-colon in nested JSON objects
