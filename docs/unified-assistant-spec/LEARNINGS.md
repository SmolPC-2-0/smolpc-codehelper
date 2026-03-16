# Learnings

> **Purpose:** Session-tracked corrections, discoveries, and gotchas accumulated during development. Each session appends new learnings. This document prevents future sessions from repeating mistakes.
>
> **Audience:** Every AI session. Read relevant sections before working on that subsystem.
>
> **Last Updated:** 2026-03-13

---

## How to Use This Document

1. Before working on a subsystem, read the relevant section
2. When you discover a correction or gotcha, append it to the relevant section
3. Include: what happened, why it's wrong, what's correct
4. Date your entries

---

## Engine / Inference

### Rust / Tauri

- **Use Channels over Events for streaming** (2026-02): `tauri::ipc::Channel<T>` is command-scoped and ordered. Events are global broadcast with no lifecycle tie to `invoke()`. Channels eliminate listener race conditions by design. This was discovered when Events caused tokens to be dropped because the listener wasn't set up before the first token was emitted.

- **Use `OnceLock<Result>` over `Once` for fallible init** (2026-02): `Once::call_once` doesn't return values. If you need to cache and return a result from one-time init, use `OnceLock::get_or_init()`. We wasted time implementing workarounds with `Once` before discovering this.

- **Fatal init in production, non-fatal in dev** (2026-02): `if !cfg!(debug_assertions) { return Err(...) }` in Tauri's `.setup()` — lets dev work continue without model files. Without this, every dev session requires having the model files present.

- **Use dedicated `AtomicBool` over `try_lock()` for state tracking** (2026-02): `try_lock()` creates TOCTOU races. An explicit flag set/cleared in the function lifecycle is reliable. We had a bug where two generation requests could start simultaneously because `try_lock` released between check and use.

### ONNX / OpenVINO

- **ONNX Runtime version split** (2026-02): v1.22.1 only ships Windows builds. Use v1.22.0 for macOS/Linux cross-compilation. Don't use latest if you need cross-platform CI.

- **Tauri `resources` glob must match files** (2026-02): `bundle.resources: ["libs/*"]` fails at compile time if no files match. Use a README.md as glob satisfier with `.gitignore` exception. This caused CI builds to fail silently.

- **Qwen2.5 has TWO stop tokens** (2026-02): `<|endoftext|>` (token ID 151643) for raw completion EOS and `<|im_end|>` (token ID 151645) for ChatML turn end. Without ChatML, the model only emits 151643, so you must check both. Missing this caused infinite generation loops.

- **ChatML is mandatory for chat behavior** (2026-02): Without `<|im_start|>system/user/assistant<|im_end|>` formatting, Qwen falls into pretraining-data completion patterns (self-Q&A, training data regurgitation). The model generates coherent but useless output that looks like training data.

- **Set `add_special_tokens: false` when embedding special tokens in prompt** (2026-02): If the prompt already contains ChatML tokens, don't let the tokenizer add its own — they'd be duplicated or wrong. This caused malformed token sequences that the model couldn't interpret.

- **OpenVINO NPU requires INT4 symmetric quantization** (2026-03): Asymmetric quantization is not supported on Intel NPU. Models quantized with asymmetric INT4 will fail to load. Always use `--sym` flag in `optimum-cli export`.

- **NPU first load is slow (blob compilation)** (2026-03): First time a model is loaded on NPU, it's compiled to NPU-specific blob format. This takes 30-60 seconds. Subsequent loads use the cached blob and are 5-10 seconds. Cache blobs in `<model_dir>/cache/`.

- **No runtime device switching in OpenVINO** (2026-03): You cannot switch between CPU and NPU on the same LLMPipeline instance. You need separate pipeline instances per device. This means the engine creates separate adapter instances for CPU and NPU.

- **Raw `ort` crate is being deprecated** (2026-03): CPU backend is migrating from raw `ort` crate to either onnxruntime-genai (with CPU EP) or native openvino_genai (CPU device). The raw `ort` backend requires ~500+ lines of manual KV cache, tokenization, and sampling code that both alternatives provide built-in.

---

## Frontend / Svelte

- **Tauri Channel pattern** (2026-02): Create `new Channel<T>()`, set `onmessage`, pass to `invoke()`. The `invoke()` promise resolves with the command's return value only after all channel messages are delivered. No manual listener cleanup needed. This is different from WebSocket or EventSource patterns where you manage lifecycle manually.

- **Single source of truth for state** (2026-02): Don't duplicate reactive state between component and store. If the store tracks `isGenerating` with proper `finally` cleanup, the component should read from the store — not maintain its own copy. We had a bug where the component's local `isSending` and the store's `isGenerating` got out of sync.

- **Svelte 5 runes only** (2026-02): This project uses `$state`, `$derived`, `$effect`, `$props`. Never use `writable`, `readable`, `derived` from `svelte/store`. Never use `$:` reactive declarations. These are Svelte 4 patterns and will not work correctly.

- **Tailwind 4 has no `@apply`** (2026-02): `@apply` was removed in Tailwind 4. Use utility classes directly in templates. For reusable styles, extract into components or use CSS variables.

---

## MCP

- **GIMP MCP uses TCP, not stdio** (2026-03): `maorcc/gimp-mcp` connects to GIMP's Script-Fu console via TCP. The MCP server itself listens on TCP port 10008. This is different from most MCP servers which use stdio.

- **`call_api` is the escape hatch** (2026-03): The GIMP MCP server's `call_api` tool can execute arbitrary GIMP PDB procedures. This means the model isn't limited to predefined tools — it can generate any GIMP API call. This is powerful but requires careful prompt engineering.

- **blender-assistant is production-ready v7.0.0** (2026-03): Despite appearing to be scaffolding, the Blender assistant has a full HTTP bridge, RAG system, shared engine integration, and build pipeline. Don't discard it — use hybrid approach (keep HTTP bridge + add blender-mcp).

- **LibreOffice `mcp-libre` has two modes** (2026-03): Standalone mode has 14 tools. Extension mode (with LibreOffice extension installed) has 73 tools. Extension mode is recommended for full capability.

- **All MCP servers are Python** (2026-03): gimp-mcp, blender-mcp, and mcp-libre are all Python. They're lightweight and don't run inference (inference goes to the engine). Python is managed by bundled `uv`.

---

## Model Selection

- **BFCL scores for sub-8B models are not published** (2026-03): The Berkeley Function Calling Leaderboard doesn't have scores for Qwen3 small models (1.7B, 4B). Tool calling quality must be evaluated through hands-on testing, not benchmarks.

- **Qwen3.5 is NOT NPU-verified** (2026-03): As of March 2026, Qwen3.5 models (released Feb-March 2026) have NOT been verified to work on Intel NPU via OpenVINO. Qwen3 IS verified. Don't commit to Qwen3.5 until NPU support is confirmed.

- **Qwen3.5 has high hallucination rates** (2026-03): 80-82% on AA-Omniscience for 4B and 9B. This is concerning for an educational tool. The Intelligence Index improvements (27 for 4B vs 18 for Qwen3-4B) may come at the cost of factual accuracy.

- **Qwen3.5-35B-A3B is NOT viable** (2026-03): Despite only 3B active parameters (MoE architecture), the total model weights are ~24GB at INT4. Far too large for any target hardware.

---

## Packaging

- **PyInstaller causes Windows Defender false positives** (2026-03): Bundling Python apps with PyInstaller frequently triggers antivirus alerts on school machines. Use `uv` + source Python instead.

- **Can't reuse application-bundled Python** (2026-03): GIMP, Blender, and LibreOffice all bundle their own Python, but they install to Program Files (needs admin), have different versions, and modifying them risks breaking the host application.

- **`uv` can run offline with pre-cached deps** (2026-03): `uv pip install --offline --find-links ./wheels` installs from local wheel files without internet. Use this as fallback for firewalled schools.

- **Tauri NSIS `currentUser` mode needs no admin** (2026-03): Installs to `%AppData%`, uses HKCU registry. Students can install without IT admin help. But some schools with strict WDAC policies may still block per-user installs.

---

## Workflow

- **Parallel specialist reviews catch cascading issues** (2026-02): Running Rust, Frontend, and Architecture reviews in parallel, then fixing and re-reviewing, is effective for thorough audits. Issues in one layer often reveal issues in others.

- **No git worktrees** (2026-03): User preference — use separate clones instead. Worktrees have caused issues in this project.

- **Context compaction is the biggest risk** (2026-03): Long AI sessions lose synthesized research when context compacts. Always persist findings to documentation files before they're lost. This entire docs/unified-assistant-spec directory was created specifically to prevent research loss.

---

## Historical VS Code Extension Research

- **Chat Participant API requires Copilot** (2026-03): Historical finding from the old extension-first direction. The VS Code Chat Participant API (for contributing to the Copilot chat panel) only works when GitHub Copilot is installed. Don't use it for the current unified app.

- **InlineCompletionItemProvider is the autocomplete API** (2026-03): Historical extension research. Same API that GitHub Copilot uses for ghost text. It is not part of the active unified-app plan.

- **1.5B model viable for completions, not agents** (2026-03): A 1.5-3B model can handle short code completions and explain-code tasks. It is NOT capable of agentic multi-file edits (too slow, context too limited, instruction following too weak).
