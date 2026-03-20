# Finalization Roadmap

Last updated: 2026-03-20
Status: Step 1 complete, Step 2 next

---

## Step 1 — Engine Fixes & Robustness ✅ COMPLETE

20 production fixes merged as PR #107 (`3d7460e`). Backend priority flipped, idle bug fixed, timeouts added, logger initialized, health endpoint returns real state. Live-tested on hardware.

**Remaining deferred items:** See `codex/ENGINE_FIXES_PLAN.md` (3.1 token ACL, 3.5 structured errors, Tier 4 polish).

---

## Step 2 — Packaging Proof-of-Concept (NEXT)

Build on `feat/windows-dml-packaging` (our prior work). Extend local-bundle path to include OpenVINO DLLs alongside DirectML. Start with one model (Qwen2.5), prove the package installs and runs.

**Owner:** Engine team (us)
**Depends on:** Step 1 complete ✅
**Blocks:** Step 3
**Starting point:** `feat/windows-dml-packaging` branch (7 commits, 2 working paths)

---

## Step 3 — Clean-Machine Validation

Test the package on all three hardware targets:
- Machine 1: Core Ultra (NPU + DirectML + CPU)
- Machine 2: Intel CPU, high RAM, no discrete GPU (CPU fallback)
- Machine 3: i5 + RTX 2000 series (discrete GPU DirectML)

**Owner:** Engine team + professor
**Depends on:** Step 2 complete

---

## Step 4 — Engine Codebase Cleanup

Break up monolithic files (main.rs at 4,431 LOC), remove dead code, remove legacy paths. User will define exact scope when this begins. Moved after packaging — cleanup once build is confirmed.

**Owner:** Engine team (us)
**Depends on:** Steps 1-3 stable

---

## Step 5 — Integration with Other Apps

siddh-m rebases PR #59 (launcher + blender + libreoffice) onto clean main. GIMP work (PRs #58/#60) follows. We support but don't own.

**Owner:** siddh-m, n0ssy, aishah, mts934
**Depends on:** Step 1 complete (they rebase onto our engine)

---

## Step 6 — Full Bundle

All apps + launcher + engine in one package. Phase 2 delivery (single .exe with model download). Requires model hosting decision. Engine lifecycle (shutdown) owned by launcher — see `project_engine_lifecycle.md`.

**Owner:** Full team
**Depends on:** Steps 2-5
