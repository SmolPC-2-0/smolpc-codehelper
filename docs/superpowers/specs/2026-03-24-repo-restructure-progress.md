# Repo Restructure — Phase Progress Tracker

**Spec:** `docs/superpowers/specs/2026-03-24-repo-restructure-design.md`

**Workflow per phase:**
1. Read spec + this tracker to determine current phase
2. Use `superpowers:writing-plans` to write a detailed implementation plan for the current phase
3. Implement based on plan
4. Dispatch `superpowers:requesting-code-review` agent to review against plan
5. Fix issues found
6. Run verification: `cargo check --workspace && cargo clippy --workspace && cargo test --workspace`
7. Commit with phase label (e.g., `refactor: phase 1 — rename apps/codehelper to app`)
8. Update this tracker

---

## Phase 1: Structural move (`apps/codehelper/` → `app/`)
- **Status:** completed
- **Commit:** d3c048d
- **Key spec sections:** 4 (file moves), 6 (config updates), 0a (dev-mode resource fix)
- **Gate:** `cargo check --workspace` + `cd app && npm run check`

## Phase 2: Create `smolpc-connector-common` crate
- **Status:** pending
- **Key spec sections:** 2 (exports/deps), 4 (file moves), 5 (import rewiring), 0b-0d (sysinfo, CancellationToken, MockCancellationToken), 0g (TextStreamer error message)
- **Gate:** `cargo check --workspace` + `cargo test -p smolpc-connector-common`

## Phase 3: Extract Blender connector
- **Status:** pending
- **Key spec sections:** 3 (connector deps), 4 (file moves), 5 (import rewiring — both app-side and intra-connector)
- **Gate:** `cargo check --workspace` + `cargo test -p smolpc-connector-blender`

## Phase 4: Extract GIMP connector
- **Status:** pending
- **Key spec sections:** 3 (connector deps), 4 (file moves), 5 (import rewiring — note GIMP runtime.rs/transport.rs intra-connector rewiring)
- **Gate:** `cargo check --workspace` + `cargo test -p smolpc-connector-gimp`

## Phase 5: Extract LibreOffice connector
- **Status:** pending
- **Key spec sections:** 3 (connector deps), 4 (file moves), 5 (import rewiring)
- **Gate:** `cargo check --workspace` + `cargo test -p smolpc-connector-libreoffice`

## Phase 6: Cleanup and identity
- **Status:** pending
- **Key spec sections:** 6 (all config updates), 0e-0h (constants, scripts, identifier), 10 (full verification), 11 (risk mitigations)
- **Gate:** Full verification checklist from spec Section 10
