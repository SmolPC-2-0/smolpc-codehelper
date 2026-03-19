---
paths:
  - "engine/**/*.rs"
---

# Model-Specific Rules

- Qwen2.5 has TWO stop tokens: `<|endoftext|>` (151643) + `<|im_end|>` (151645) — check both
- Qwen3 OpenVINO support is currently non-thinking only; align temperature, top_p, top_k, and presence_penalty with the upstream non-thinking guidance
- Qwen3 NPU requires template patching: the enable_thinking Jinja condition must default to non-thinking when the variable is undefined (NPU StaticLLMPipeline does not support extra_context)
