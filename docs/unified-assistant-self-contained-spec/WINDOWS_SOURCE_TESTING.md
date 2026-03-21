# Windows Source Testing Guide

**Last Updated:** 2026-03-21
**Status:** Pre-upgrade functional validation gate for `dev/unified-assistant-self-contained` before engine reconciliation, packaging, or installer work

## 1. Purpose

Use this guide to test the actual unified app behavior on Windows from source.

This is intentionally narrower than Phase 6 packaging work:

- in scope:
  - source-based functional testing on Windows
  - verifying the unified shell, setup panel, and live modes work as expected
  - collecting repeatable tester results before any platform upgrades
- out of scope:
  - installer testing
  - clean-machine packaged validation
  - engine upgrade work from `main`
  - packaging or installer changes

## 2. Branch And Clone Rules

- branch to test:
  - `origin/dev/unified-assistant-self-contained`
- use a separate clean clone for this work
- do not test from a stale local `main` checkout
- do not mix this testing pass with engine/platform reconciliation branches

Recommended clone command:

```bash
git clone --branch dev/unified-assistant-self-contained --single-branch https://github.com/SmolPC-2-0/smolpc-codehelper.git
cd smolpc-codehelper
```

Record the exact tested commit in your results.

## 3. Windows Prerequisites

### Required developer tooling

- Node.js 18+
- Rust toolchain through `rustup`
- Windows Tauri prerequisites for a normal Rust/Tauri dev build
  - MSVC C++ build tools
  - WebView2 runtime

### Required host apps

- GIMP 3.x
- Blender
- LibreOffice or Collabora

Phase 5 only supports GIMP 3.x. If the machine only has GIMP 2.x, treat that as
an expected unsupported-host result, not as a mysterious regression.

### Optional but often needed for local source testing

- Python on PATH, or a repo-local `.venv`, for source-mode GIMP and
  Writer/Slides fallback behavior
- a working shared model root for Code mode

Code mode is functional only when the engine can resolve a model. The simplest
paths are:

- an existing `SMOLPC_MODELS_DIR`
- or `npm run model:setup:qwen3`

## 4. Bootstrap The Clone

From the repo root:

```bash
npm ci
npm run check
cargo test -p smolpc-code-helper
node apps/codehelper/scripts/self-contained/validate-resource-manifests.mjs
```

If you plan to test Code mode and do not already have a shared model root:

```powershell
npm run model:setup:qwen3
```

This functional test gate is not trying to prove packaged payload staging yet.
It is only trying to prove that the current unified app behavior works from
source on real Windows laptops.

## 5. Launch The Unified App

Start from the repo root:

```powershell
npm run tauri:dev
```

This wrapper:

- builds `smolpc-engine-host`
- shuts down stale local engine-host processes first
- starts the unified app in Tauri dev mode

Use the setup panel and per-mode status surfaces as the source of truth during
testing.

## 6. Expected Source-Mode Behavior

These behaviors are expected in the current validation pass:

- `bundled_model` may still show `not_prepared` if the packaged model payload
  has not been staged into `apps/codehelper/src-tauri/resources/models/`
- `bundled_python` may still show `not_prepared` if the packaged Python payload
  has not been staged into `apps/codehelper/src-tauri/resources/python/payload/`
- that alone does not mean the branch is broken for source-based functional
  testing
- GIMP source mode can use:
  - prepared bundled Python if present
  - repo `.venv` Python if present
  - PATH Python as a last resort in debug/source mode
- Writer and Slides source mode can use:
  - prepared bundled Python if present
  - repo `.venv` Python if present
  - PATH Python as a last resort in debug/source mode
- Code mode still requires a real model to be discoverable by the shared engine

Treat source-mode usability and honest status/error reporting as the goal here.
Do not treat missing packaged payloads as a release regression during this pass.

## 7. Functional Test Matrix

### Setup And Shell

Verify:

- the app launches from source
- the setup banner and setup panel render
- setup item states are readable and believable
- setup details distinguish missing host apps from not-prepared app-owned assets

### Code Mode

With a model configured:

- engine starts automatically
- one simple prompt completes successfully

Without a model configured:

- the app reports the failure honestly
- the result is logged in the tester report as an expected environment gap, not
  an unexplained regression

### GIMP Mode

Verify:

- GIMP 3.x is detected
- `setup_prepare()` can provision or repair the bundled plugin/runtime without
  launching the interactive GIMP UI
- first GIMP use provisions when needed
- first GIMP use launches GIMP only when no suitable process is already running
- already-running GIMP is reused
- GIMP tools connect successfully through the bundled bridge on `127.0.0.1:10008`
- missing GIMP or GIMP 2.x surfaces honest detail

### Blender Mode

Verify:

- Blender is detected
- `setup_prepare()` provisions the addon without launching Blender UI
- first Blender use launches Blender only when needed
- already-running Blender is reused
- Blender tools connect successfully

### Writer And Slides

Verify:

- LibreOffice or Collabora is detected
- first Writer use launches the runtime plus the host app when needed
- first Slides use launches the runtime plus the host app when needed
- missing LibreOffice surfaces honest detail

### Calc

Verify:

- Calc remains visible
- Calc remains intentionally disabled

## 8. What To Capture

Use the results template in:

- `docs/unified-assistant-self-contained-spec/WINDOWS_SOURCE_TEST_RESULTS_TEMPLATE.md`

At minimum, record:

- tested commit
- Windows version
- host app versions
- whether Code mode had a configured model
- whether PATH Python or a repo `.venv` was used
- exact failing step, if any
- exact setup or mode detail text shown by the app
- terminal output from `npm run tauri:dev` when something fails

## 9. Escalation Rules

- If a step only fails because the tester skipped a documented prerequisite,
  fix the environment and continue.
- If a step fails even though the documented prerequisites are satisfied, treat
  it as a branch-local functional issue on
  `dev/unified-assistant-self-contained`.
- Do not pull in engine-upgrade or packaging work just to explain a current
  unified-app failure during this pass.

## 10. Exit Condition For This Gate

This gate is successful when another developer can follow this guide from a
fresh Windows clone and complete the functional matrix without relying on
unstated tribal knowledge.
