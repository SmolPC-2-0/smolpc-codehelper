# LibreOffice Assistant Windows Phase 1 Verification

Use this runbook on a Windows test machine to capture review-ready evidence.

For MCP bridge validation added in Phase 2, use:

1. `apps/libreoffice-assistant/WINDOWS_PHASE2_MCP_VERIFICATION.md`

## Preconditions

1. LibreOffice Assistant build from branch `codex/libreoffice-port-track-a` is installed or running via `tauri:dev:libreoffice`.
2. Shared engine model bootstrap has been run at least once (`qwen3-4b-instruct-2507` preferred).
3. You can open the LibreOffice Assistant Phase 1 diagnostics UI.

## Verification Flow

1. Click `Ensure Engine Started`.
2. Click `Refresh Models` and select `qwen3-4b-instruct-2507`.
3. Click `Load Model`.
4. Run `Generate (Non-Stream)` and confirm output + metrics.
5. Run `Generate (Stream)` and confirm stream chunks appear.
6. Click `Cancel` during an active stream once to verify cancel behavior.
7. Click `Run Verification Checklist` and review pass/fail rows.

## Issue Report Capture

1. In `Integration Issue Report`, paste the request payload used for the failing or suspect run.
2. Fill `HTTP Status` and `Response Body` (if available).
3. Click `Generate Issue Report`.
4. Click `Copy JSON` and archive in your test notes.

## Evidence Bundle Export

1. Click `Export Evidence Bundle`.
2. Note the exported file path shown under `Evidence file`.
3. Attach the exported JSON file to your PR/review thread.

## Required Artifacts For Team Review

1. One screenshot of runtime checklist results.
2. One issue report JSON snapshot.
3. One exported evidence bundle JSON file path + file contents.
4. Note whether `SMOLPC_FORCE_EP` or `SMOLPC_DML_DEVICE_ID` was set.

## Notes

1. The exported evidence bundle contains both:
   - runtime verification result
   - integration issue report payload
2. If DirectML is unavailable on the test machine, checklist details should still be attached to explain lane state/reason codes.
