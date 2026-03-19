---
paths:
  - "engine/**/*.rs"
  - "scripts/**/*openvino*"
  - "scripts/**/*ov*"
---

# OpenVINO Engine Rules

- OpenVINO GenAI handles its own tokenization — Rust `TokenizerWrapper` is not involved for the NPU lane
- OpenVINO 2026.0.0 is the pinned tuple — `openvino`, `openvino_genai`, `openvino_tokenizers` must match; mixing breaks ABI
- Use INT4, not NF4, for broad NPU compatibility — NF4 only works on Core Ultra Series 2+
- OpenVINO GenAI chat requests must use structured message history on CPU and NPU; keep the preformatted ChatML string path only for explicit legacy compatibility
- NPU compilation is slow on first load but fast after — `CACHE_DIR` enables compiled blob reuse
- Do NOT set `min_new_tokens` on OpenVINO GenAI 2026.0.0 — any value >= 1 permanently suppresses EOS detection, causing runaway generation
