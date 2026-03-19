# Phase 8: Source-Parity Chat Session Persistence + Resume UX

Date: 2026-03-19  
Status: Planned (Step 1 docs push pending)  
Owner: `apps/libreoffice-assistant`

## Goal

Implement source-parity chat/session persistence so chat state survives app reloads and operators can safely resume or clear sessions, while preserving the existing engine-only runtime and MCP workflow behavior.

## Scope

In scope:

1. Persist source-parity chat messages to local storage with schema versioning.
2. Persist and restore source-parity workflow metadata needed for resume UX (including tool traces and workflow outcome metadata represented in chat history).
3. Restore persisted chat session on app startup with safe parsing/normalization.
4. Add clear-session UX with explicit operator confirmation.
5. Handle malformed/corrupt storage payloads gracefully with fallback behavior and no crash.
6. Keep existing source-parity settings persistence behavior intact.
7. Keep tool-first behavior and MCP tool-selection behavior unchanged.

Out of scope:

1. Any Ollama/provider runtime reintroduction.
2. Backend command-surface expansion unless strictly required.
3. MCP Python helper implementation changes.
4. Launcher catalog/installer/registration changes.
5. Additional source-parity screens unrelated to chat/session persistence.

## Acceptance Criteria

1. [ ] Source-parity chat history persists in local storage and survives reloads.
2. [ ] Persisted payload uses explicit schema versioning and supports safe normalization.
3. [ ] Session restore hydrates message history (including tool trace and workflow outcome metadata) at startup without blocking existing chat workflow.
4. [ ] Clear-session action requires explicit confirmation and removes persisted chat session data.
5. [ ] Malformed/corrupt payloads do not crash the app and fall back to empty/default session state.
6. [ ] Tool-first workflow guard behavior remains intact (send blocked when no MCP tool selected).
7. [ ] Existing source-parity settings storage behavior remains functional.
8. [ ] Validation gates pass:
   - `npm run check:libreoffice`
   - `npm run build:libreoffice`
   - `cargo test -p smolpc-libreoffice-assistant --lib`

## Risks and Mitigations

1. Risk: persisted schema drift causes runtime hydration errors.
   - Mitigation: introduce versioned payload type + strict runtime normalization with typed defaults.
2. Risk: malformed localStorage payload crashes Source-Parity panel render path.
   - Mitigation: defensive parse/validation and fallback to empty chat session.
3. Risk: clear action accidentally removes other persisted data (settings).
   - Mitigation: isolate chat storage key and only remove the chat-session entry.
4. Risk: persistence side effects alter tool-first/MCP flow behavior.
   - Mitigation: preserve existing send guards and controller orchestration; limit persistence to message/session state only.

## Validation Gates

Required gates for this phase:

1. `npm run check:libreoffice`
2. `npm run build:libreoffice`
3. `cargo test -p smolpc-libreoffice-assistant --lib`

Phase completion requires all three gates to pass after implementation.

## Mandatory 3-Step Workflow Record

1. Step 1: plan/docs commit + push
2. Step 2: implementation commit + push
3. Step 3: post-implementation docs finalization commit + push

No phase is complete until all three steps are pushed.
