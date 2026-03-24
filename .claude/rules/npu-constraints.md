---
paths:
  - "engine/**/*.rs"
  - "apps/**/modes/**/*.rs"
---

# NPU (OpenVINO StaticLLMPipeline) Constraints

- Greedy decoding only — always force `do_sample=false`
- `presence_penalty` is incompatible with greedy decoding — skip it on NPU
- Do NOT set `min_new_tokens` — any value >= 1 permanently suppresses EOS detection, causing runaway generation
- No `extra_context` API for thinking control — inject `/nothink` into system message content instead
- Fixed context window: `MAX_PROMPT_LEN` (input) + `MIN_RESPONSE_LEN` (output). Exceeding `MAX_PROMPT_LEN` crashes with "unknown exception". Intel default is 1024.
- No tokenizer in the OpenVINO GenAI C API — token counting from Rust requires the `tokenizers` crate with model's `tokenizer.json`, or a character heuristic (~3.5 chars/token for Qwen)
- Qwen3 template must be patched to default to non-thinking when `enable_thinking` is undefined — patch failure must be a hard error (un-patched defaults to thinking mode → runaway generation)
- Qwen3-4B INT4 produces garbage on NPU — use INT8_SYM per-channel (`nncf.compress_weights`). INT4 stays for CPU only.
- Compilation is slow on first load but fast after — `CACHE_DIR` enables compiled blob reuse
