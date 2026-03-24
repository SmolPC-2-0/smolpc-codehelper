# AGENTS.md — Codex CLI Instructions

## Project

SmolPC Code Helper — offline AI coding assistant for secondary school students. Rust engine (axum HTTP server) + Tauri 2 + Svelte 5 + Tailwind 4.

**Architecture:** `engine/` = shared inference server (`smolpc-engine-host` + `smolpc-engine-core`). `apps/` = product apps. `launcher/` = suite shell. Engine runs as local HTTP server on port 19432.

**Backend priority:** `directml` (discrete GPU only) > `openvino_npu` > `cpu`

---

## Your Role

You are the Codex session in a dual-agent workflow. Claude Code handles architecture decisions, multi-file refactors, and complex debugging. You handle well-scoped, single-purpose tasks.

**You should:**
- Complete the specific task assigned to you
- Work only on your assigned branch (`codex/<topic>`)
- Run pre-commit checks before committing
- Use conventional commits with scope: `feat(engine):`, `fix(openvino):`, `docs:`, etc.

**You should NOT:**
- Make architecture decisions or change backend selection policy
- Touch files outside your task scope
- Merge your own branches to main (Claude reviews and merges)
- Modify `CLAUDE.md`, `.claude/rules/`, or workflow docs

---

## Conventions

**DLL loading is centralized.** All `Library::new()` / `load_with_flags()` calls live in `runtime_loading.rs`.

**OpenVINO DLL load order matters.** `tbb12 -> openvino -> openvino_c -> openvino_ir_frontend -> openvino_intel_cpu_plugin -> openvino_intel_npu_plugin -> openvino_tokenizers -> openvino_genai`

**OpenVINO models = IR format.** `.xml` + `.bin` artifacts, not ONNX.

**Svelte 5 runes only.** No `writable`/`readable`. Use `$state`, `$derived`, `$effect`.

**Tailwind 4.** No `@apply`.

**Pre-commit checks:**
```bash
cargo check --workspace && cargo clippy --workspace
cargo test -p smolpc-engine-core && cargo test -p smolpc-engine-host
cd apps/codehelper && npm run check && npm run lint
```

---

## Key Learnings

- OpenVINO GenAI handles its own tokenization — Rust `TokenizerWrapper` is not involved for NPU/CPU lanes
- Qwen2.5 has TWO stop tokens: `<|endoftext|>` (151643) + `<|im_end|>` (151645)
- Qwen3 OpenVINO = non-thinking only; align temperature/top_p/top_k/presence_penalty with upstream guidance
- Do NOT set `min_new_tokens` on OpenVINO GenAI 2026.0.0 — suppresses EOS detection
- Use `OnceLock<Result>` over `Once` for fallible init
- Use `AtomicBool` over `try_lock()` for state tracking
- Don't dismiss broken checks as "pre-existing" — fix them

---

## Coordination

**Workflow doc:** [`docs/AI_WORKFLOW.md`](docs/AI_WORKFLOW.md) — read this at session start for full context on the dual-agent workflow.

**Shared state:** Update `codex/WORKING_ISSUES.md` when your task is complete. Format:
```
- Status: Resolved (codex/<branch-name>, commit <hash>)
```

If you discover a new issue during your task, add it to WORKING_ISSUES.md with status Open.

**Handoff:** When your task is done, use the `$claude-codex-handoff` skill (in `.codex/skills/`) to prepare a structured summary for Claude. Include: objective, repo changes, machine-local changes, validation results, open items, next actions.
