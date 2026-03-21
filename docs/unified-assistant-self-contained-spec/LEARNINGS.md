# Learnings

> **Purpose:** Session-tracked corrections, discoveries, and gotchas accumulated during development. Each session appends new learnings. This document prevents future sessions from repeating mistakes.
>
> **Audience:** Every AI session. Read relevant sections before working on that subsystem.
>
> **Last Updated:** 2026-03-21

---

## Self-Contained Line

- **Freeze the demo line before productization work starts** (2026-03-17): `dev/unified-assistant` and `docs/unified-assistant-spec` are now the frozen demo baseline. Self-contained delivery work continues on `dev/unified-assistant-self-contained`, with the archived `docs/unified-assistant-self-contained-spec` branch retained only as reference for the branch-cut period.

- **Dual-mainline docs control was a transition tool, not a permanent policy** (2026-03-17): The separate self-contained docs branch was useful while the branch cut, master plan, and Phase 2 foundation were still settling. Once the docs tree was already fully present on `dev/unified-assistant-self-contained`, the extra docs-sync/status-sync PRs created more CI noise and branch churn than protection. Phase 3 onward keeps docs-first rigor but lands docs directly on the implementation mainline.

- **Host apps may remain external while integrations become app-owned** (2026-03-17): The self-contained finish line does not require bundling GIMP, Blender, or LibreOffice themselves. It does require the unified app to own everything else: model files, Python runtime, MCP servers, plugins, addons, provisioning, and launch orchestration.

- **GIMP is the hardest self-contained gap** (2026-03-17): Blender already has an app-owned bridge and LibreOffice already has bundled runtime scripts, but GIMP still depends on an externally managed plugin/server arrangement. The self-contained roadmap must treat GIMP vendoring, provisioning, and launch ownership as the longest integration track.

- **Bundled Python is the cross-mode enabler** (2026-03-17): Removing external Python is not only a LibreOffice requirement. The managed Python runtime is also the cleanest foundation for GIMP-side runtime ownership and future packaging consistency across provider-owned resources.

- **LibreOffice must not silently swap interpreters once bundled Python owns the runtime** (2026-03-17): After Phase 3, the bundled LibreOffice runtime must keep using the exact interpreter that launched `main.py`. Letting the runtime rediscover and switch to an office-bundled Python would undermine the whole packaged-mode ownership contract and make setup status misleading.

- **Blender provisioning must be session-aware, not restart-driven** (2026-03-17): Phase 4 is safer when addon provisioning and enablement run through Blender CLI background execution, while live sessions are left untouched. If Blender is already running, report that one reopen may be needed instead of killing or force-restarting the session.

- **Provenance must be documented before third-party imports land** (2026-03-17): Any vendored runtime, addon, plugin, or model payload needs a pinned source reference, license notes, and local modification tracking in `THIRD_PARTY_PROVENANCE.md` before implementation branches import it.

- **Keep setup state app-level and phase-limited at first** (2026-03-17): Phase 2 stayed low-risk because it introduced setup state, detection, manifests, and a lightweight UI without changing any mode activation paths. That kept the foundation branch additive and made later runtime/provisioning phases narrower.

- **Track resource roots before payloads land** (2026-03-17): Adding manifests, placeholder READMEs, and staging hooks in Phase 2 is worthwhile even before the large Python/model payloads are imported. It gives the setup subsystem something honest to validate and avoids mixing contract work with heavy runtime imports.

- **The archive docs branch is reference-only, not a sync target** (2026-03-21): Once `dev/unified-assistant-self-contained` became the sole active self-contained mainline, the frozen `docs/unified-assistant-self-contained-spec` branch stopped being a status mirror. Keep the archive branch pinned for history, but treat the docs tree on the implementation mainline as the only source of truth.

- **Generic GIMP install paths need version probing, not just path heuristics** (2026-03-21): Windows often exposes `gimp-3.exe`, but Linux and some macOS installs surface generic paths like `/usr/bin/gimp`. Phase 5 needed `gimp --version` fallback logic so the unified app can reject GIMP 2.x honestly instead of silently provisioning against the wrong host.

- **First-use provisioning in async providers should move blocking work off the async lane** (2026-03-21): GIMP Phase 5 showed that version probing, recursive asset copy, and marker repair can happen during the first provider connect. Wrapping that path in `tokio::task::spawn_blocking` keeps the shared async runtime responsive while the provider acquires ownership of bundled assets.

- **Setup tests that mutate `HOME` or `APPDATA` need an env lock** (2026-03-21): The new app-level provisioning tests for bundled integrations can race when they mutate profile-root env vars in parallel. Reuse a single env guard helper anywhere setup tests touch process-wide profile variables.

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

- **Root Prettier needs a Svelte-only override** (2026-03): The repo-level `.prettierrc` cannot safely apply `prettier-plugin-tailwindcss` to `.svelte` files during root incremental checks. Keep Tailwind's plugin at the top level, but load `prettier-plugin-svelte` inside the `.svelte` override instead.

- **The shell needs local fallback mode configs** (2026-03): If the unified shell relies entirely on `list_modes()` during startup, one command failure can leave the mode selector empty and strand the user in the current mode. Keep a local fallback copy of the six mode configs so the shell stays navigable even when backend mode discovery fails.

- **Top-level async app init should always be caught** (2026-03): The unified shell startup sequence chains several async steps (`modeStore.initialize()`, inference startup, cached hardware load). Calling that sequence without a top-level `.catch()` risks unhandled startup failures that are hard to diagnose. Catch once at the shell entrypoint and surface the warning in the UI.

- **Live modes should surface live runtime state, not scaffold status copy** (2026-03): Once a mode already has a real execution path, the shell should present that mode's actual runtime state rather than reusing placeholder provider messaging for consistency. In Phase 3, Code mode needed `inferenceStore.status` as the visible shell status source to avoid making a working path look fake.

- **Keep non-active modes phase-neutral in shared shell copy** (2026-03): Phase-specific wording in multi-mode UI goes stale quickly once the next phase lands. Keep copy specific for the mode under active implementation, but keep untouched modes neutral unless the phase is explicitly about them.

- **Optional message metadata is the safest way to mix mode-specific UIs** (2026-03): Phase 4 needed GIMP assistant messages to carry `explain`, `undoable`, `toolResults`, and `plan` without breaking existing Code chats. Extending the shared `Message` shape with optional fields preserved backward compatibility while still allowing mode-specific rendering and actions.

- **Live non-Code modes still need a single shared-generation gate** (2026-03): Phase 5 made Blender a second live non-Code mode, but it still shares the same engine runtime as Code. The shell can allow mode switching during Blender generation, yet it must block starting any competing live-mode request until the active non-Code run finishes or is cancelled.

- **Side-effectful document modes should not inherit replay-style chat actions** (2026-03): Writer and Slides tool calls mutate real documents, so automatically replayable actions like `Regenerate`, `Continue`, and `Branch Chat` are a bad fit by default. Keep those affordances on tutoring/code modes unless the document workflow has explicit idempotency or undo semantics.

---

## MCP

- **Make the shared MCP client async before real transports land** (2026-03): stdio and TCP transports are inherently async. If the shared JSON-RPC client starts synchronous, every real implementation either blocks a thread or forces a breaking trait change later.

- **Shared providers must be mode-aware at the provider boundary** (2026-03): LibreOffice serves Writer, Calc, and Slides from one runtime. Patching `ProviderStateDto.mode` later in the command layer is too fragile; the provider contract itself must accept the requested mode for status, tool discovery, undo, and execution.

- **Planner allowlists are not enough for side-effectful tools** (2026-03): Once Writer and Slides went live, the backend still had to enforce the per-mode tool allowlist in `list_tools()` and `execute_tool()`. Relying on prompt instructions alone would leave document-modifying tools exposed to planner mistakes or malformed fallback payloads.

- **GIMP MCP uses TCP, not stdio** (2026-03): `maorcc/gimp-mcp` connects to GIMP's Script-Fu console via TCP. The MCP server itself listens on TCP port 10008. This is different from most MCP servers which use stdio.

- **`call_api` is the escape hatch** (2026-03): The GIMP MCP server's `call_api` tool can execute arbitrary GIMP PDB procedures. This means the model isn't limited to predefined tools — it can generate any GIMP API call. This is powerful but requires careful prompt engineering.

- **blender-assistant is production-ready v7.0.0** (2026-03): Despite appearing to be scaffolding, the Blender assistant has a full HTTP bridge, RAG system, shared engine integration, and build pipeline. Don't discard it — use hybrid approach (keep HTTP bridge + add blender-mcp).

- **LibreOffice `mcp-libre` has two modes** (2026-03): Standalone mode has 14 tools. Extension mode (with LibreOffice extension installed) has 73 tools. Extension mode is recommended for full capability.

- **All MCP servers are Python** (2026-03): gimp-mcp, blender-mcp, and mcp-libre are all Python. They're lightweight and don't run inference (inference goes to the engine). Python is managed by bundled `uv`.

- **Mode-by-mode activation can reuse one command surface** (2026-03): Phase 4 made `assistant_send`, `mode_status`, `mode_refresh_tools`, and `mode_undo` real for GIMP without changing command names or DTOs. The stable command surface from Phase 1 was sufficient; only the mode-specific implementation behind it changed.

- **GIMP status must attempt a real connection to be useful** (2026-03): A cached placeholder state is not enough once GIMP becomes a live mode. The unified GIMP provider needs `status()` to attempt live MCP initialization and tool discovery so the header and welcome state can show an honest connected/disconnected result immediately on mode switch.

- **Scene-query heuristics must stay narrower than workflow questions** (2026-03): Blender retrieval should be skipped for pure scene-state questions like "What is in my scene right now?", but not for workflow questions that happen to mention scene nouns such as "selected object". Broad substring heuristics accidentally suppress retrieval on legitimate tutoring questions.

- **Stdio MCP transports need stderr capture and response timeouts** (2026-03): A child-process MCP transport that drops stderr or waits forever for a matching response ID turns routine runtime failures into opaque hangs. Capture recent stderr lines and bound response waits so the caller gets an actionable transport error instead of a stuck request.

- **Scaffolding phases should land honest provider status before runtime import** (2026-03): Phase 6A stayed merge-safe because LibreOffice modes gained a real shared provider, resource validation, and mode-aware disabled copy without activating `assistant_send` or importing the evolving standalone runtime. That makes the later activation branch a narrower, more auditable diff.

---

## Model Selection

- **BFCL scores for sub-8B models are not published** (2026-03): The Berkeley Function Calling Leaderboard doesn't have scores for Qwen3 small models (1.7B, 4B). Tool calling quality must be evaluated through hands-on testing, not benchmarks.

- **Qwen3.5 is NOT NPU-verified** (2026-03): As of March 2026, Qwen3.5 models (released Feb-March 2026) have NOT been verified to work on Intel NPU via OpenVINO. Qwen3 IS verified. Don't commit to Qwen3.5 until NPU support is confirmed.

- **Qwen3.5 has high hallucination rates** (2026-03): 80-82% on AA-Omniscience for 4B and 9B. This is concerning for an educational tool. The Intelligence Index improvements (27 for 4B vs 18 for Qwen3-4B) may come at the cost of factual accuracy.

- **Qwen3.5-35B-A3B is NOT viable** (2026-03): Despite only 3B active parameters (MoE architecture), the total model weights are ~24GB at INT4. Far too large for any target hardware.

---

## Packaging

- **Track placeholder resource directories for Tauri CI** (2026-03): If `bundle.resources` references a directory such as `apps/codehelper/src-tauri/libs/openvino/`, that directory must exist in git on a clean checkout even when the real Windows DLLs are staged later by scripts. A tracked `README.md` placeholder is enough to satisfy Tauri's resource resolution.

- **PyInstaller causes Windows Defender false positives** (2026-03): Bundling Python apps with PyInstaller frequently triggers antivirus alerts on school machines. Use `uv` + source Python instead.

- **Can't reuse application-bundled Python** (2026-03): GIMP, Blender, and LibreOffice all bundle their own Python, but they install to Program Files (needs admin), have different versions, and modifying them risks breaking the host application.

- **`uv` can run offline with pre-cached deps** (2026-03): `uv pip install --offline --find-links ./wheels` installs from local wheel files without internet. Use this as fallback for firewalled schools.

- **Tauri NSIS `currentUser` mode needs no admin** (2026-03): Installs to `%AppData%`, uses HKCU registry. Students can install without IT admin help. But some schools with strict WDAC policies may still block per-user installs.

---

## Workflow

- **`npm audit fix --package-lock-only` is worth trying before package range changes** (2026-03): The foundation branch's `undici` advisory was resolved by refreshing the lockfile only. That kept the dependency ranges stable while still moving the vulnerable transitive resolution out of range.

- **Launcher path tests need platform-specific absolute fixtures** (2026-03): A Windows-style absolute path string is not absolute on macOS/Linux. Tests that validate launcher manifest rules should use a fixture that is absolute on the current platform or they can fail for the wrong reason.

- **Root incremental style gates need root frontend config entrypoints** (2026-03): CI runs `prettier` and `eslint` from the repo root against `apps/codehelper/...` paths. That requires a root `eslint.config.mjs` that re-exports the workspace config, plus a real `apps/codehelper/.gitignore` so `includeIgnoreFile()` does not crash.

- **Docs-only PRs still need repo-root Prettier hygiene** (2026-03-21): Incremental style gates check changed Markdown files too, not just code. For self-contained docs branches, run Prettier from the repo root on the touched `docs/unified-assistant-self-contained-spec/*.md` files before opening the PR or the branch can merge with avoidable CI noise.

- **Keep shell diffs off legacy lint surfaces unless they are truly needed** (2026-03): Incremental style gates only lint changed files. Touching a legacy frontend file with an unrelated existing lint violation can turn that old issue into a new CI blocker for the current phase.

- **Parallel specialist reviews catch cascading issues** (2026-02): Running Rust, Frontend, and Architecture reviews in parallel, then fixing and re-reviewing, is effective for thorough audits. Issues in one layer often reveal issues in others.

- **No git worktrees** (2026-03): User preference — use separate clones instead. Worktrees have caused issues in this project.

- **Context compaction is the biggest risk** (2026-03): Long AI sessions lose synthesized research when context compacts. Always persist findings to documentation files before they're lost. This entire `docs/unified-assistant-self-contained-spec` directory exists to prevent research loss on the self-contained line.

- **Dirty clones need selective staging, not cleanup churn** (2026-03): The shared clone used for unified work can contain unrelated local diffs from other workstreams. For implementation branches, stage only the files that belong to the phase and leave unrelated dirt alone instead of widening the branch scope with cleanup commits.

- **Reference-branch runtime imports should be pinned to an exact commit** (2026-03): Pulling large runtime assets from an active standalone branch is only reviewable if the unified branch records the exact source commit it imported from. That keeps follow-up syncs auditable and avoids silently drifting with unrelated standalone work.

- **Bridge handles need explicit drop cleanup or Rust tests can hang** (2026-03): Lazy-start provider runtimes that spawn local servers must stop those tasks when the handle is dropped. Without explicit cleanup, the lib test binary can finish its assertions but never exit because detached bridge tasks are still alive.

- **Local helper sockets still need auth and framing limits** (2026-03): Binding a provider helper to loopback is not enough by itself once the runtime becomes live. The LibreOffice helper path needed a per-runtime auth token, hard frame-size ceilings, and strict response validation before Phase 7 could treat it as shippable.

- **Fake dependency shims are enough to regression-test local runtimes** (2026-03): The Phase 7 hardening branch could test the LibreOffice helper auth/framing behavior and the LibreOffice MCP client's response validation without a real LibreOffice install by spawning the imported Python scripts with fake UNO/MCP shim modules. That is a practical way to keep protocol hardening covered in `cargo test`.

---

## Historical VS Code Extension Research

- **Chat Participant API requires Copilot** (2026-03): Historical finding from the old extension-first direction. The VS Code Chat Participant API (for contributing to the Copilot chat panel) only works when GitHub Copilot is installed. Don't use it for the current unified app.

- **InlineCompletionItemProvider is the autocomplete API** (2026-03): Historical extension research. Same API that GitHub Copilot uses for ghost text. It is not part of the active unified-app plan.

- **1.5B model viable for completions, not agents** (2026-03): A 1.5-3B model can handle short code completions and explain-code tasks. It is NOT capable of agentic multi-file edits (too slow, context too limited, instruction following too weak).
