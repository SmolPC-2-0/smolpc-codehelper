# Codex Working Issues

Last updated: 2026-02-24
Base branch for stacked work: `codex/directml-inferencing`

## DirectML Execution Tracker

1. Milestone 1 - Toolchain + Runtime Packaging
Status: Completed
Notes:
- Rust MSRV moved to 1.88, ORT upgraded to `2.0.0-rc.11`
- `scripts/setup-libs.sh` now downloads checksum-verified DirectML runtime bundles
- Windows runtime bundle now includes `onnxruntime_providers_shared.dll` and `DirectML.dll`
- ORT rc.11 compatibility updates applied in inference init/session metadata accessors

2. Milestone 2 - Backend Domain + Persistence
Status: Completed
Notes:
- Added `src-tauri/src/inference/backend.rs` with backend decision + benchmark gate + failure counter domain types
- Added `src-tauri/src/inference/backend_store.rs` with versioned JSON persistence and atomic writes
- Added tests for:
  - key/fingerprint mutation on driver changes
  - demotion threshold behavior at 3 consecutive DirectML failures
  - persistence round-trip + invalidation on key changes
  - invalid JSON recovery

3. Milestone 3 - Hardware Identity Enrichment
Status: Completed
Notes:
- Added `driver_version` + `pci_device_id` to `src-tauri/src/hardware/types.rs::GpuInfo`
- Populated fields in `src-tauri/src/hardware/detector.rs`
- Mirrored optional fields in `src/lib/types/hardware.ts`

4. Milestone 4 - Session Builder + Fallback
Status: Completed
Notes:
- `src-tauri/src/inference/session.rs` now supports explicit backend creation (`Cpu` / `DirectML`)
- DirectML session options use ORT EP registration with `error_on_failure`, sequential execution, disabled memory pattern, and Level3 optimization
- `src-tauri/src/inference/mod.rs` now preloads `DirectML.dll` on Windows before ORT init
- `src-tauri/src/commands/inference.rs` now contains same-request fallback helper for DirectML init failure -> CPU session

5. Milestone 5 - Selector + Benchmark Gate + Demotion
Status: Pending

6. Milestone 6 - Diagnostics Command + Structured Logs
Status: Pending

## Active Risks / Notes

1. Toolchain invocation quirk on this workstation
Status: Open
Impact:
- `cargo check` may still invoke Homebrew Rust 1.87 by default
Mitigation:
- Use explicit Rust 1.88 toolchain binaries with `RUSTC=$HOME/.rustup/toolchains/1.88.0-aarch64-apple-darwin/bin/rustc`
- Keep `rust-toolchain.toml` committed so CI/other workstations are deterministic

## Scope
- Backend focus: `src-tauri/src/inference`, `src-tauri/src/models`
- Goal: fix runtime safety and modular model support issues incrementally using child branches

## Issue Tracking

1. Issue: ONNX output access and tensor shape safety in inference generator
Status: Completed in child branch
Branch: `fix/inference-safe-output-shapes`
PR: https://github.com/SmolPC-2-0/smolpc-codehelper/pull/26
Notes:
- Replaced panic-prone output indexing with required-output checks
- Added strict rank/dimension/data-length validation
- Added focused unit tests for validation helpers

2. Issue: Model runtime spec and architecture contract (1.5B-first)
Status: Completed in child branch
Branch: `fix/inference-runtime-spec-per-model`
PR: https://github.com/SmolPC-2-0/smolpc-codehelper/pull/27
Notes:
- Added runtime spec abstraction (`ModelRuntimeSpec`) for model architecture + I/O naming + stop tokens
- Integrated runtime spec validation into model load path
- Updated generator/input builder to be runtime-spec-driven
- Removed 7B from visible model list until supported
- Added guardrail tests for 1.5B-only registry/runtime mapping

3. Issue: Single-flight generation and cancellation scoping
Status: Pending
Planned branch: `fix/inference-single-flight-cancel-scope`

4. Issue: Deterministic model path resolution (remove CWD dependency)
Status: Pending
Planned branch: `fix/models-path-resolution`

5. Issue: InputBuilder allocation and key handling improvements
Status: Pending
Planned branch: `fix/inference-inputbuilder-allocs`

## Implementation Rules
- Keep each issue isolated in its own child branch and PR
- Base each child PR on `fix/stop-token-chatml`
- Exclude unrelated local changes:
  - `.claude/settings.local.json`
  - `package-lock.json`
- Run at least:
  - `cargo check`
  - targeted tests for touched modules
