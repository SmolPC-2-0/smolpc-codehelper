# LibreOffice Assistant Windows Phase 3 Workflow Verification

Use this runbook on Windows machines to validate Phase 3 chat/tool workflow behavior across different hardware lanes.

## Goal

Verify that the app can:

1. Execute MCP tools as part of workflow orchestration.
2. Feed MCP tool results back into the response path.
3. Complete workflow behavior on both:
   - CPU-only lane (with local summary fallback if needed)
   - Accelerator lane (DirectML/GPU preferred, model-assisted summary expected)

## Preconditions

1. Branch includes Phase 3 preview workflow changes.
2. Windows machine has:
   - Node.js and npm
   - Rust toolchain
   - Python 3.12+
   - LibreOffice (or Collabora Office) installed
3. Shared model setup completed at least once:
   - `npm run model:setup:qwen3`
4. MCP venv setup completed:
   - `apps/libreoffice-assistant/src-tauri/resources/mcp_server/.venv`
5. App launches with:
   - `npm run tauri:dev:libreoffice`

## Common startup

1. Click `Ensure Engine Started`.
2. Click `Refresh Models`.
3. Select and load `qwen3-4b-instruct-2507`.
4. In `MCP Bridge`:
   - `Refresh MCP Status`
   - `Start MCP Server` if needed
   - `Refresh MCP Tools`
5. Confirm:
   - `running: true`
   - `tools_loaded > 0`

## Test Matrix

### A) CPU lane validation (required)

1. Confirm backend lane in UI:
   - `Backend Status -> active_backend: cpu`
2. In `MCP Bridge`:
   - Tool: `list_documents`
   - Args:

```json
{
  "directory": "C:\\Users\\<YOUR_USER>\\Documents"
}
```

3. In `Phase 3 Workflow (Preview)`:
   - Set prompt requesting document listing + short summary.
   - Click `Run Tool-First Fast Path`.

Expected:

1. `Workflow Trace` includes:
   - `Fast path: invoking 'list_documents' directly.`
   - `Tool call succeeded: list_documents`
   - `CPU backend detected; skipped summary model turn.` or
   - `Summary turn timed out; used local fallback summary.`
2. `Workflow Final Response` is non-empty and references discovered docs.
3. `MCP tool result` includes document listing output.

### B) Accelerator lane validation (teammate lane)

Run this on a teammate machine with DirectML-capable hardware.

1. Confirm backend lane in UI:
   - `Backend Status -> active_backend: directml` (preferred)
2. Repeat the same workflow prompt and run:
   - `Run Tool-First Fast Path`
3. Optional: run full model-first path:
   - `Run MCP-Assisted Flow`

Expected:

1. Tool call succeeds.
2. Summary/model response returns without timeout.
3. `Workflow Final Response` includes model-assisted output (not just local fallback).

### C) Helper recovery behavior (required once)

1. Trigger a tool call after helper instability (or after app restart cycles).
2. If helper refusal appears, verify recovery behavior:
   - automatic retry path in workflow OR
   - manual stop/start MCP in bridge panel.

Expected:

1. Tool call eventually succeeds and returns content.
2. No persistent `Connection refused` after one restart/retry cycle.

## Required artifacts for review

For each machine/lane tested:

1. Screenshot of `Backend Status` panel showing `active_backend`.
2. Screenshot or copied text of `Workflow Trace`.
3. `MCP tool result` JSON for successful `list_documents` call.
4. One final response output from `Phase 3 Workflow (Preview)`.
5. If failure occurs:
   - exact error text
   - terminal log excerpt around failure
   - issue report JSON from app UI

## Notes

1. CPU-only machines may not reliably complete the summary model turn within desired latency.
2. CPU lane acceptance for this phase preview allows local deterministic summary fallback when tool result is correct.
3. Accelerator lane teammate validation is required before promoting Phase 3 preview to final acceptance.
