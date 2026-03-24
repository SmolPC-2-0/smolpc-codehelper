# Phase 1: Structural Move — `apps/codehelper/` → `app/`

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename `apps/codehelper/` to `app/`, update all config references, delete dead directories, and verify the entire build still works.

**Architecture:** Pure file moves and config path updates. No Rust code changes, no new crates, no import rewiring. The Cargo package name stays `smolpc-code-helper` in this phase (renamed in Phase 6).

**Tech Stack:** git mv, Cargo workspace, npm workspaces, Tauri, PowerShell scripts, GitHub Actions YAML

**Spec reference:** `docs/superpowers/specs/2026-03-24-repo-restructure-design.md` — Sections 1, 4 (Phase 1), 6, 0a

---

### Task 1: Move the app directory

**Files:**
- Move: `apps/codehelper/` → `app/`
- Delete: `apps/` (will be empty after move)

- [ ] **Step 1: Move the directory with git mv**

```bash
git mv apps/codehelper app
```

Note: `git mv` preserves history. The `apps/` parent directory will be removed automatically by git when empty.

- [ ] **Step 2: Verify the move**

```bash
ls app/src-tauri/Cargo.toml && ls app/src/App.svelte && ls app/package.json
```

Expected: All three files exist at new paths.

- [ ] **Step 3: Verify apps/ is gone**

```bash
ls apps/ 2>&1
```

Expected: Directory not found (git removed it since it's empty after the move).

---

### Task 2: Update root `Cargo.toml` workspace member

**Files:**
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Update the workspace member path**

Change line 3 from:
```toml
  "apps/codehelper/src-tauri",
```
to:
```toml
  "app/src-tauri",
```

- [ ] **Step 2: Verify Cargo workspace resolves**

```bash
cargo metadata --no-deps --format-version 1 2>&1 | head -5
```

Expected: No errors. Metadata output starts with `{"packages":[`.

---

### Task 3: Update root `package.json` workspace and scripts

**Files:**
- Modify: `package.json` (workspace root)

- [ ] **Step 1: Update workspace array**

Change line 6 from:
```json
  "workspaces": ["apps/codehelper"],
```
to:
```json
  "workspaces": ["app"],
```

- [ ] **Step 2: Update all 14 script references**

Replace every `--workspace apps/codehelper` with `--workspace app` in the `"scripts"` block (lines 9-23). All 14 entries:
- `dev`, `build`, `preview`, `check`, `format`, `lint`, `tauri`, `tauri:dev`, `tauri:dml`
- `runtime:setup:openvino`, `runtime:setup:python`
- `model:export:dml`, `model:setup:qwen25-instruct`, `model:setup:qwen3-4b`

- [ ] **Step 3: Verify npm workspace resolves**

```bash
npm ls --workspaces 2>&1 | head -5
```

Expected: Shows `app` workspace, no errors about missing `apps/codehelper`.

---

### Task 4: Update `tauri.conf.json` schema path

**Files:**
- Modify: `app/src-tauri/tauri.conf.json`

- [ ] **Step 1: Fix the `$schema` relative path**

The file moved from `apps/codehelper/src-tauri/` (3 levels deep) to `app/src-tauri/` (2 levels deep).

Change line 2 from:
```json
  "$schema": "../../../node_modules/@tauri-apps/cli/config.schema.json",
```
to:
```json
  "$schema": "../../node_modules/@tauri-apps/cli/config.schema.json",
```

---

### Task 5: Update `scripts/check-boundaries.ps1`

**Files:**
- Modify: `scripts/check-boundaries.ps1`

- [ ] **Step 1: Replace all 8 `apps/codehelper` path references**

Replace every occurrence of `apps/codehelper` with `app` in the file. The 8 locations:
- Lines 21-25: `Assert-PathAbsent` paths
- Line 27: `$commandsModPath`
- Line 35: `$appCargoPath`
- Line 46: `rg` search path
- Line 52: `Get-ChildItem -Path`
- Line 59: violation message

Use find-and-replace: `apps/codehelper` → `app` across the entire file.

---

### Task 6: Update `.github/workflows/ci.yml`

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Replace `apps/codehelper` references**

4 locations to update:
- Line 24: `npm audit --workspace apps/codehelper` → `npm audit --workspace app`
- Line 133: `$_ -like 'apps/codehelper/*'` → `$_ -like 'app/*'`
- Line 134: `$_ -notlike 'apps/codehelper/*'` → `$_ -notlike 'app/*'`
- Lines 137, 140: `Push-Location apps/codehelper` → `Push-Location app` and `$file.Substring('apps/codehelper/'.Length)` → `$file.Substring('app/'.Length)`

---

### Task 7: Update `.github/workflows/release.yml`

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Replace all `apps/codehelper` references (14 locations)**

Use find-and-replace across the entire file: `apps/codehelper` → `app`. Locations:
- Lines 51-53: cache paths and hash key
- Lines 59, 65, 71: PowerShell script paths
- Lines 79, 85: sidecar binary staging
- Lines 91-98: artifact validation paths
- Line 111: `projectPath: apps/codehelper` → `projectPath: app`

---

### Task 8: Update dev-mode resource validation

**Files:**
- Modify: `app/src-tauri/src/app_paths.rs`

- [ ] **Step 1: Trim KNOWN_BUNDLED_RESOURCE_ROOTS**

Per spec Section 0a, change line 3-4 from:
```rust
const KNOWN_BUNDLED_RESOURCE_ROOTS: [&str; 5] =
    ["python", "models", "gimp", "blender", "libreoffice"];
```
to:
```rust
const KNOWN_BUNDLED_RESOURCE_ROOTS: [&str; 2] = ["python", "models"];
```

- [ ] **Step 2: Update tests that create single resource roots**

The test `bundled_resource_dir_resolution_uses_debug_fallback_when_tauri_is_unusable` (line 136) creates only `gimp/`. After trimming the constant, this won't validate. Change it to create `python/` instead:

Line 138: `write_resource_root(dev.path(), "gimp");` → `write_resource_root(dev.path(), "python");`

Similarly, `bundled_resource_dir_resolution_skips_debug_fallback_outside_debug_builds` (line 157) creates only `blender/`. Change line 160: `write_resource_root(dev.path(), "blender");` → `write_resource_root(dev.path(), "models");`

---

### Task 9: Delete dead directories

**Files:**
- Delete: `launcher/` (empty placeholder)

- [ ] **Step 1: Remove the empty launcher directory**

```bash
rm -rf launcher/
git add -u launcher/
```

Note: `apps/` was already removed by `git mv` in Task 1. The stubs `apps/blender-assistant/` and `apps/gimp-assistant/` don't exist on the current main branch (already cleaned up in earlier PRs).

---

### Task 10: Verify everything builds

- [ ] **Step 1: Cargo workspace check**

```bash
cargo check --workspace
```

Expected: Compiles with only pre-existing warnings. No errors.

- [ ] **Step 2: Cargo tests**

```bash
cargo test -p smolpc-engine-core && cargo test -p smolpc-engine-host -- --skip development_mode_prefers_discovered_dev_roots
```

Expected: All tests pass (skip the pre-existing dev roots test).

- [ ] **Step 3: App-specific test (includes app_paths tests)**

```bash
cargo test -p smolpc-code-helper
```

Expected: All tests pass, including the updated `app_paths` tests.

- [ ] **Step 4: Frontend checks**

```bash
cd app && npm run check
```

Expected: 0 errors, 0 warnings.

- [ ] **Step 5: Clippy**

```bash
cargo clippy --workspace
```

Expected: Only pre-existing warnings.

- [ ] **Step 6: Format check**

```bash
cargo fmt -- --check
```

Expected: No diffs.

---

### Task 11: Commit Phase 1

- [ ] **Step 1: Stage all changes**

```bash
git add -A
```

Review staged changes — should be:
- Renamed directory: `apps/codehelper/` → `app/`
- Modified: `Cargo.toml`, `package.json`, `scripts/check-boundaries.ps1`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `app/src-tauri/tauri.conf.json`, `app/src-tauri/src/app_paths.rs`
- Deleted: `launcher/`

- [ ] **Step 2: Commit**

```bash
git commit -m "refactor: phase 1 — rename apps/codehelper to app

Move the single Tauri app from apps/codehelper/ to app/ at root level.
Update all workspace configs, CI workflows, boundary scripts, and
dev-mode resource validation to reference new paths.
Delete empty launcher/ placeholder directory.

Part of connector-first architecture restructure.
Spec: docs/superpowers/specs/2026-03-24-repo-restructure-design.md"
```

- [ ] **Step 3: Update progress tracker**

In `docs/superpowers/specs/2026-03-24-repo-restructure-progress.md`, change Phase 1 status from `pending` to `completed`.

```bash
git add docs/superpowers/specs/2026-03-24-repo-restructure-progress.md
git commit -m "docs: mark phase 1 complete in progress tracker"
```
