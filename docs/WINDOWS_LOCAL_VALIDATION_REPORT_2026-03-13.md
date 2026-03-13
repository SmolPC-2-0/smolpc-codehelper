# Windows Local Validation Report (CPU Baseline)

## Environment
- Date: 2026-03-13
- Machine: Windows (Intel Iris Xe, integrated GPU)
- Driver: Intel Iris Xe `32.0.101.7084` (2026-01-15)
- RAM: 34 GB system memory
- Repo: `C:\Users\mathi\smolpc\smolpc-codehelper`
- Branch: `codex/libreoffice-port-track-a`

## Summary
- CPU path is stable and usable locally.
- DML path with `qwen3-4b-instruct-2507` crashes host process on this machine.
- Local development should use CPU + `qwen2.5-coder-1.5b`.
- DML validation should be completed by teammates on stronger/different hardware.

## Key Findings

### 1) Engine Runtime Bundle / Startup
- Initial blocker: runtime bundle path resolution (`missing_root`) when running host manually.
- Resolved by setting:
  - `SMOLPC_ORT_BUNDLE_ROOT=C:\Users\mathi\smolpc\smolpc-codehelper\target\debug`

### 2) DML Path
- `ensure-started` + `load` could succeed for `qwen3-4b-instruct-2507` on DML.
- Generation call caused host crash:
  - Exit code: `0xc0000409`
  - Status: `STATUS_STACK_BUFFER_OVERRUN`

### 3) CPU Path
- `qwen3-4b-instruct-2507` cannot run in forced CPU mode on this branch by design:
  - Error: `Model 'qwen3-4b-instruct-2507' currently requires DirectML backend... forced CPU mode is not supported`
- Switched to `qwen2.5-coder-1.5b` for CPU baseline.
- After installing `qwen2.5-coder-1.5b` CPU artifact, CPU path worked end-to-end.

## CodeHelper Validation (CPU)

### Launch mode
- Started CodeHelper via:
  - `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\run-tauri-dev.ps1 -ForceEp cpu`

### Terminal API checks (passed)
- `GET /engine/health` -> `ok: true`
- `GET /v1/models` -> includes:
  - `qwen3-4b-instruct-2507`
  - `qwen2.5-coder-1.5b`
- `POST /engine/ensure-started` with startup policy `qwen2.5-coder-1.5b` -> success
- `POST /engine/load` model `qwen2.5-coder-1.5b` -> success
- `GET /engine/status` ->
  - `current_model: qwen2.5-coder-1.5b`
  - `backend: cpu`
  - `selection_reason: forced_override`

### Non-stream generation sample (passed)
- Request model: `qwen2.5-coder-1.5b`
- Prompt: `Reply exactly: terminal load OK`
- Response text: `terminal password OK` (semantic success; output variance acceptable)
- Metrics:
  - `time_to_first_token_ms`: 983
  - `tokens_per_second`: 1.3758
  - `total_time_ms`: 2180
  - `total_tokens`: 3

## CPU Benchmark Baseline (qwen2.5-coder-1.5b)
- Run 1: `5.51s`, `2.41 tok/s`, `13 tokens`
- Run 2: `6.51s`, `2.61 tok/s`, `17 tokens`
- Run 3: `6.20s`, `2.58 tok/s`, `16 tokens`

Interpretation:
- Stable but slow, expected for this CPU/iGPU class.
- Good enough for local correctness testing.

## LibreOffice Assistant Status
- MCP bridge was previously validated:
  - `running: true`
  - `tools_loaded: 27`
- Tool call success example:
  - `list_documents` returned expected files in `C:\Users\mathi\Documents`.
- Workflow summary can time out on this machine; local fallback summary path is functioning.

## Known Constraints on This Machine
1. DML generation stability issue (`0xc0000409`) with `qwen3-4b-instruct-2507`.
2. CPU-only fallback for local dev requires `qwen2.5-coder-1.5b`.
3. UI model controls may appear inconsistent; terminal API calls are reliable for validation.

## Recommended Next Actions

### For local (this machine)
1. Continue dev/testing with:
   - `SMOLPC_FORCE_EP=cpu`
   - model `qwen2.5-coder-1.5b`
2. Validate functionality, not throughput.

### For teammates (other hardware)
1. Re-run DML path with `qwen3-4b-instruct-2507`.
2. Capture:
   - `/engine/status`
   - DML generation metrics
   - Any crash/event logs
3. Confirm whether DML crash reproduces or is device/driver-specific.

## Artifacts to Attach
1. Terminal logs with DML crash (`0xc0000409`).
2. CPU benchmark outputs (3-run table).
3. `/engine/status` snapshots for CPU and DML attempts.
4. MCP tool success JSON (`list_documents` example).
