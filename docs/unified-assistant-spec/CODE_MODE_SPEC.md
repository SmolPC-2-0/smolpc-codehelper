# Code Mode Specification

**Last Updated:** 2026-03-16
**Status:** Canonical definition of the in-app Code mode

## 1. Purpose

Code mode preserves the real Codehelper experience inside the unified app. It is
not a placeholder tab, and it is not delegated to a launcher or external editor
for this implementation plan.

The source baseline is the current `apps/codehelper/` app.

## 2. What Code Mode Preserves

### Core experience

- coding-focused chat workflow
- current conversation view and composer behavior
- markdown and code-block rendering
- chat export
- keyboard shortcuts
- model readiness and backend status visibility

### Current Codehelper shell elements

- sidebar with chat history
- conversation area
- workspace header
- model info panel
- hardware panel
- benchmark panel

### Current backend behaviors

- shared engine usage
- startup/readiness flow
- generation streaming
- model and backend diagnostics

## 3. Panels And Controls

### Shared across all modes

- model info panel
- hardware panel
- status indicators
- sidebar / history shell

### Code-only in v1

- benchmark panel
- code-oriented workspace controls
- code-specific quick examples or replacements for them
- any code-only local actions exposed through the local provider

## 4. Retained V1 Features

Code mode v1 retains:

1. coding tutor persona and prompt behavior
2. current multi-chat workflow
3. model/backend visibility
4. export and diagnostics surfaces
5. shared engine contract

## 5. Phase 2 Shell Behavior

During Phase 2 shell implementation:

1. Code mode remains the default active mode.
2. Code mode alone uses the current Codehelper send/generate path.
3. Code mode alone keeps export enabled.
4. Code mode alone keeps benchmark surfaces enabled.
5. Code mode keeps context controls visible while non-Code modes hide them.
6. Code chats live in fresh unified storage and are not migrated from the old
   standalone storage keys.

## 6. Deferred From This Plan

The following are intentionally deferred:

- a separate VS Code extension
- editor-native inline completions
- deep IDE workspace integration beyond what current Codehelper already offers
- any requirement that Code mode leave the unified Tauri app

## 7. Local Provider Expectations

Code mode uses a `local` provider, which means:

- no external MCP process is required
- mode-specific behavior can call in-app Tauri commands
- the assistant orchestrator still treats Code mode through the same provider
  boundary used by other modes

This keeps the unified backend consistent while allowing Code mode to preserve
its existing behavior.

## 8. Regression Rule

Unified shell work must not reduce Code mode to a minimal placeholder while
other modes gain functionality.

Any shell refactor should be judged against this question:

> Does Code mode still feel like the current Codehelper app, only inside a
> unified mode system?

If the answer is no, the refactor is not complete.
