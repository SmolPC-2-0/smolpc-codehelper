# LibreOffice Assistant Phase 3 Teammate Results Template

Use this template to submit one result per Windows machine/lane for Phase 3 acceptance.

## Run Metadata

1. Tester name:
2. Date:
3. Branch + commit:
4. Machine (CPU/GPU/NPU summary):
5. Driver versions (GPU/NPU if applicable):

## Runtime Lane and Model

1. Expected lane (`cpu` or `directml`):
2. Observed `active_backend` from `/engine/status`:
3. Model loaded:
4. `selection_reason`:

## Workflow Steps Run

1. MCP status check completed (`running: true`):
2. MCP tools loaded count:
3. Tool invoked (`list_documents` expected for baseline):
4. Workflow mode used:
   - `Run Tool-First Fast Path`
   - `Run MCP-Assisted Flow` (optional)

## Results

1. Tool call outcome:
   - Success/Failure
   - Error text (if any)
2. Final response outcome:
   - Model-assisted response
   - Local fallback summary
   - Timeout/failure
3. Notes on latency/usability:

## Required Artifacts (attach all)

1. Screenshot of `Backend Status` (`active_backend`, `selection_reason` visible).
2. `Workflow Trace` capture.
3. Successful MCP tool result JSON.
4. One final workflow response output.
5. If failure:
   - terminal log excerpt
   - issue report JSON
   - exact error text

## Acceptance Checklist

1. [ ] Engine started and healthy.
2. [ ] MCP running with non-zero tools.
3. [ ] Baseline tool call succeeded.
4. [ ] Workflow produced final response.
5. [ ] Artifacts attached.
