# Codex -> Claude Handoff (Benchmark Session, 2026-03-21)

Objective
- Prepare this laptop for benchmark practice on `feat/benchmark`, validate the "installed app + `--resource-dir`" path, and capture reproducible failures for Claude cleanup.

Repository changes
- Fast-forwarded local `feat/benchmark` from `e5075a6` to `c8a8662`.
- Updated [`codex/WORKING_ISSUES.md`](./WORKING_ISSUES.md):
  - `Last updated` date -> `2026-03-21`.
  - Added open issue `#3` for benchmark combo skips and error details.
  - Added fast repro command (~20s) for the CPU model-switch conflict.
- Added benchmark handoff report: [`codex/CLAUDE_HANDOFF_BENCHMARK_2026-03-21.md`](./CLAUDE_HANDOFF_BENCHMARK_2026-03-21.md).
- Added benchmark evidence files:
  - `benchmark-results/benchmark-igpu-32gb-2026-03-21.json`
  - `benchmark-results/benchmark-igpu-32gb-cpu-qwen3-smoke-2026-03-21.json`
  - `benchmark-results/benchmark-igpu-32gb-cpu-two-model-smoke-2026-03-21.json`

Machine-local changes
- Ran runtime setup scripts for this workspace:
  - `apps/codehelper/scripts/setup-directml-runtime.ps1`
  - `apps/codehelper/scripts/setup-openvino-runtime.ps1`
- Copied staged runtime libs into `target/debug/libs` for local debug benchmark discovery.
- Built local binaries in `target/`:
  - `smolpc-engine-host` (debug)
  - `smolpc-benchmark` (debug + release)
- Installed npm deps in `apps/codehelper` via `npm ci` to run check/lint locally.

Validation
- `cargo check --workspace`: passed
- `cargo clippy --workspace`: passed
- `cargo test -p smolpc-engine-core`: passed
- `cargo test -p smolpc-engine-host`: passed
- `cd apps/codehelper && npm run check`: passed
- `cd apps/codehelper && npm run lint`: failed
  - Prettier check reports style drift in 67 files under `apps/codehelper` (broad pre-existing formatting mismatch; not touched in this task to avoid unrelated mass diff).

Benchmark command verification
- Claude-proposed command (release + installed app resource dir) executed successfully and produced a result file:
  - `cargo run --release -p smolpc-benchmark -- --machine igpu-32gb --resource-dir "%LOCALAPPDATA%\\SmolPC Code Helper"`
- Observed outcomes:
  - Success: `cpu / qwen2.5-1.5b-instruct`
  - Skipped: `cpu / qwen3-4b` with `HTTP 409 STARTUP_POLICY_CONFLICT`
  - Skipped: `openvino_npu / qwen2.5-1.5b-instruct` with `HTTP 503 STARTUP_MODEL_LOAD_FAILED` (NPU detected but not exposed by OpenVINO)
  - Skipped: `openvino_npu / qwen3-4b` with same `HTTP 503`
- Fast repro (no generation, ~20s):
  - `cargo run --release -p smolpc-benchmark -- --machine igpu-32gb-cpu-two-model-smoke --backends cpu --models qwen2.5-1.5b-instruct,qwen3-4b --runs 0 --warmup 0 --cooldown 1 --resource-dir "%LOCALAPPDATA%\\SmolPC Code Helper"`
  - Reproduces `qwen3-4b` `STARTUP_POLICY_CONFLICT`.
- Single-model control:
  - `cargo run --release -p smolpc-benchmark -- --machine igpu-32gb-cpu-qwen3-smoke --backends cpu --models qwen3-4b --runs 0 --warmup 0 --cooldown 1 --resource-dir "%LOCALAPPDATA%\\SmolPC Code Helper"`
  - Passes, confirming conflict is tied to multi-model sequence within one backend run.

Open items
- Fix benchmark lifecycle/model transition logic so second model on same backend does not fail with `STARTUP_POLICY_CONFLICT`.
- Tighten backend auto-detect/preflight gating so NPU lane is not scheduled when runtime exposes no usable OpenVINO NPU device.
- Decide whether to enforce CPU-only on this machine until NPU gating is corrected.
- Optional hygiene follow-up (separate task): resolve repo-wide Prettier drift in `apps/codehelper` so `npm run lint` can pass.

Next actions for Claude
1. Reproduce quickly with the fast command above (`cpu`, both models, `runs=0`, `warmup=0`).
2. Patch benchmark engine lifecycle around per-combo startup policy/model switching.
3. Patch backend detection/gating for false-positive NPU presence.
4. Re-run full benchmark command with `--resource-dir` and confirm all intended combos either run or are cleanly skipped by design.
5. Decide whether to keep benchmarking artifacts under `benchmark-results/` in this branch or archive externally after triage.

