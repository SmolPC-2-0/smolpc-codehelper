# OpenVINO Migration Docs

This folder is the isolated documentation zone for the OpenVINO-first EP migration strategy.

## Source of Truth

- `OPENVINO_EP_ACCELERATION_PLAN_2026-03-06.md`
- `research/RESEARCH_INDEX.md`

## Isolation Rule

- Keep new OpenVINO EP strategy/planning docs in this folder.
- Treat legacy planning docs under:
  - `docs/new_onnx_plan/`
  - `docs/onnx plan/`
  as historical reference unless explicitly migrated here.

## Research Pack

Use `research/` as the cached research baseline for future sessions to reduce context bloat:

- `research/RESEARCH_INDEX.md` (entrypoint)
- `research/RESEARCH_SNAPSHOT_2026-03-06.md` (dated baseline claims)
- `research/ORT_DOCS_RELEVANT_NOTES.md` (Rust `ort` behavior notes)
- `research/SOURCE_REGISTRY_2026-03-06.md` (primary source map)
