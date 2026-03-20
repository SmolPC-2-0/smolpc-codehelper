# Phase 9: Source-Parity Session Resume Hardening + Safety UX

Date: 2026-03-20  
Status: Completed (Step 3 docs finalized locally; push blocked by non-fast-forward remote)  
Owner: `apps/libreoffice-assistant`

## Goal

Harden Phase 8 source-parity chat/session persistence in `apps/libreoffice-assistant` with safer resume/new-session UX, bounded persistence behavior, and explicit corrupted-payload recovery signaling, while preserving engine-only + current MCP workflow behavior.

## Scope

In scope:

1. Add session-restore metadata in source-parity chat store (restore happened, restored count, persisted saved-at timestamp).
2. Add resume UX in `SourceParityPanel`:
   - visible resumed-session banner/state
   - explicit `Start New Session` action with confirmation
3. Add bounded persisted session history so localStorage payload size cannot grow unbounded.
4. Keep malformed/corrupt payload handling graceful and non-crashing, with explicit operator-facing signal when persisted session data is reset.
5. Preserve persisted tool trace and workflow outcome metadata behavior.
6. Preserve tool-first guard and MCP tool-selection behavior.

## Out Of Scope

1. Reintroducing Ollama/provider runtime logic.
2. Backend command-surface expansion unless strictly required.
3. Changes outside `apps/libreoffice-assistant`.
4. Source-parity settings persistence behavior changes.

## Acceptance Criteria

1. Chat store exposes restore metadata indicating whether restore occurred and how many messages were restored.
2. Restored metadata includes persisted `saved_at_iso` timestamp when available.
3. Chat panel shows a clear resumed-session indicator when prior messages were restored.
4. Chat panel exposes explicit `Start New Session` action that requires operator confirmation before clearing local session history.
5. Persisted chat history is bounded to latest N messages, preventing unbounded localStorage growth.
6. Malformed/corrupt persisted payloads do not crash and are reset safely.
7. Operator is explicitly informed when prior session data was reset due to malformed/corrupt payload.
8. Tool trace messages and workflow outcome metadata continue to persist/restore as before.
9. Tool-first MCP-selection guard behavior remains unchanged.
10. Required validation gates pass.

## Risks And Mitigations

1. Risk: bounded persistence drops context that operators expect.
   - Mitigation: cap only persisted history (not in-memory live session), keep most recent messages, and communicate resumed count clearly.
2. Risk: restore metadata introduces state drift or stale UI banners.
   - Mitigation: centralize restore metadata in chat store and reset it explicitly on new-session/clear actions.
3. Risk: corrupt-payload detection is too strict and clears recoverable data.
   - Mitigation: keep normalization permissive for valid messages, only reset on schema mismatch/non-object/non-array/all-invalid-message conditions.
4. Risk: UX changes accidentally alter tool-first behavior.
   - Mitigation: keep existing send guard logic untouched and limit UI changes to session actions/banner.

## Validation Gates

1. `npm run check:libreoffice`
2. `npm run build:libreoffice`
3. `cargo test -p smolpc-libreoffice-assistant --lib`

## Manual QA Checklist

1. Fresh launch with no saved chat: no resume banner, no reset warning, normal chat behavior.
2. Send user prompt in `mcp_assisted` mode and receive assistant response: messages persist after reload.
3. Confirm tool trace + workflow outcome metadata still appear and survive reload.
4. Reload with saved session: resumed banner appears, restored count is correct, saved timestamp is shown when present.
5. Click `Start New Session`, cancel confirmation: session remains.
6. Click `Start New Session`, confirm: messages clear, persisted session removed, resume banner/reset warning cleared.
7. Enable `tool_first` with no selected MCP tool: send remains blocked with existing guard message.
8. Save more than N messages, reload: only latest N restored, app remains responsive.
9. Corrupt session payload in localStorage (invalid schema/object): app does not crash, session resets, explicit reset warning is shown.
10. Corrupt payload containing partial malformed messages: valid messages restore, malformed entries dropped safely.

## Execution Result (2026-03-20)

1. `npm run check:libreoffice`
   - Result: pass (`svelte-check found 0 errors and 0 warnings`)
2. `npm run build:libreoffice`
   - Result: pass (`vite build` completed successfully)
3. `cargo test -p smolpc-libreoffice-assistant --lib`
   - Result: pass (`12 passed; 0 failed`)

## Manual QA Checklist Results (2026-03-20)

1. Interactive UI checklist execution in this terminal session:
   - Result: pending (requires live app interaction and browser localStorage mutation checks).
2. Static/code-path verification completed for checklist logic:
   - resumed-session banner + restored-count/saved-at rendering: implemented
   - `Start New Session` explicit confirmation flow: implemented
   - corrupt payload reset signaling banner: implemented
   - bounded persistence (latest N messages): implemented (`MAX_PERSISTED_CHAT_MESSAGES = 200`)
