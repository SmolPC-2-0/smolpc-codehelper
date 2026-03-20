# Packaging & Finalization Plan

Last updated: 2026-03-20
Status: Planning complete, awaiting user-defined deliverables for Phase 2+

---

## Deliverable Phases

### Phase 1 — Bundled Offline Folder (IMMEDIATE)

**Goal:** A folder containing the executable installer, all DLLs, and all models. 100% offline, zero network. Drop on a USB and go.

**Ships:** Engine + Code Helper only (no launcher, no other apps)

**Contents:**
- `smolpc-engine-host.exe` (or embedded in Tauri app)
- All runtime DLLs (OpenVINO, ORT, DirectML, TBB)
- Pre-bundled models:
  - `qwen2.5-1.5b-instruct/openvino/` (INT4, ~1.3 GB) — default
  - `qwen3-4b/openvino/` (INT8_SYM, ~3.75 GB) — upgrade tier
  - `qwen3-4b/dml/` (~3.5 GB) — DirectML fallback
- Tauri Code Helper app
- No network calls, no telemetry, no cloud

**Open questions for this phase:**
- Installer format: NSIS, WiX, Inno Setup, or just a zip?
- Model path resolution: where does the installed app look for models?
- DLL staging: which DLLs go alongside the exe vs in a subdirectory?
- Do we bundle ALL backend DLLs or detect hardware and include only relevant ones?

### Phase 2 — Single Self-Contained Executable

**Goal:** One `.exe` that downloads models on first run from a hosted source.

**Ships:** Engine + Code Helper (later: + launcher + extension apps from other devs)

**User experience:**
- Run the exe → setup wizard
- Choose model tier: Tier 1 (Qwen2.5, lighter), Tier 2 (Qwen3-4B, better), or both
- Models download from hosted location
- After setup, fully offline

**Deferred decisions:**
- Model hosting (GitHub releases, HuggingFace, self-hosted?)
- Download UX (progress bar, resume support, verification)
- Extension app integration (other devs building those)

---

## Backend Selection — Production

**Auto-select priority:** DirectML (if available) > NPU > CPU

This differs from the dev priority (NPU > DirectML > CPU) because DirectML has broader hardware support. NPU is only on Core Ultra.

**Rules:**
- Auto-selection must work out of the box with zero env vars
- User can switch runtimes and models on demand via the UI
- Default to Qwen2.5-1.5B (weaker, safer on unknown hardware)
- User can upgrade to Qwen3-4B themselves
- Future: heuristic to suggest stronger model when hardware supports it

---

## Cleanup Priority Order

### Priority 1 — Bugs & UX Stability
- End-to-end UX functional (load model → chat → streaming → stop)
- Engine runs and reboots cleanly
- Stress test under realistic usage patterns
- Idle health check recovery (issue #4)
- Graceful degradation when preferred backend unavailable

### Priority 2 — Codebase Cleanup
- Legacy code paths and dead code removal
- Break up 4000+ line files
- Remove debug/dev-only endpoints
- Tighten error handling at system boundaries
- User will provide exact scope/deliverables

### Priority 3 — Documentation
- After code is clean and stable
- API docs, deployment guide, contributor docs

---

## Test Hardware Matrix

| Machine | CPU | GPU | NPU | Tests |
|---------|-----|-----|-----|-------|
| Dev (SmolPC) | Core Ultra | iGPU | Yes | All three backends |
| High-RAM Intel | Intel (unknown gen) | None | Unknown | CPU fallback, possibly NPU |
| Mid-tier | i5 | RTX 2000 series | No | DirectML with discrete GPU |
| Professor's machine(s) | Unknown | Unknown | Unknown | Full auto-selection validation |

**Critical path:** CPU fallback must be rock-solid — it's the only backend guaranteed on all machines.

---

## Current State (2026-03-20)

### What's Working
- All inference paths live-tested (NPU INT8, NPU INT4, CPU, DirectML)
- Both models (Qwen2.5-1.5B, Qwen3-4B) produce coherent output
- Chat template auto-patching for Qwen3
- Streaming SSE responses
- Backend auto-selection (dev mode)

### What's Not Ready
- Model path resolution hardcoded to dev paths
- No installer or packaging pipeline
- No clean-machine validation
- Idle health check bug (issue #4)
- UI doesn't surface backend diagnostics fully
- Codebase needs significant cleanup
- Production backend priority not yet implemented (DirectML > NPU > CPU)

### Open Issues (from WORKING_ISSUES.md)
1. ~~Qwen3-4B NPU~~ — RESOLVED via INT8
2. Model path resolution — needs app-data paths for packaging
3. Installer/runtime validation — no test matrix executed yet
4. Idle health check — NPU goes unhealthy after idle
5. Backend diagnostics in UI — partially surfaced
