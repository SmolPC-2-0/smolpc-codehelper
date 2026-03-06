# OpenVINO Migration Research Index

Date created: 2026-03-06  
Purpose: Keep a local, reusable research baseline to avoid context bloat in future planning sessions.

## Files

- `RESEARCH_SNAPSHOT_2026-03-06.md`
  - Dated, high-confidence claims used to shape the migration plan.
- `ORT_DOCS_RELEVANT_NOTES.md`
  - Relevant `ort.pyke.io` and `docs.rs/ort` behavior notes for EP selection/runtime policy.
- `SOURCE_REGISTRY_2026-03-06.md`
  - Source-to-claim mapping (primary sources only).

## Usage Guidance

1. Start with `RESEARCH_SNAPSHOT_2026-03-06.md`.
2. Use `ORT_DOCS_RELEVANT_NOTES.md` for implementation details in Rust host/runtime wiring.
3. Re-verify volatile items (latest releases/compatibility) before execution or merge.

## Volatility Notes

- Release numbers and compatibility matrices are volatile.
- ORT/OpenVINO compatibility should be treated as tuple-gated and revalidated on each rollout phase.

