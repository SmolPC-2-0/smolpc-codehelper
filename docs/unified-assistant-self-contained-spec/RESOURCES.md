# External Resources

Canonical reference for all external links, documentation sources, GitHub repos, and API references used during SmolPC Unified Assistant development.

**Purpose:** Future AI sessions (Claude, Codex, etc.) should consult this document to find up-to-date docs without web searches. Every entry includes what it covers, when to use it, and relevant caveats.

**Last Updated:** 2026-03-17

---

## Self-Contained Delivery Sources

| Resource                    | Source                                                                                           | Why it matters for the self-contained line                                                                                                 |
| --------------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Blender addon payload       | `apps/blender-assistant/blender_addon/blender_helper_http.py` and companion addon files          | This is the in-repo source that will be repackaged under unified app resources for automatic Blender addon provisioning.                   |
| LibreOffice runtime scripts | `apps/codehelper/src-tauri/resources/libreoffice/mcp_server/`                                    | Writer and Slides already depend on these bundled scripts. The self-contained line uses them as the baseline for removing external Python. |
| GIMP MCP/plugin runtime     | Pending pinned upstream `gimp-mcp` snapshot                                                      | The self-contained line needs a vendored, provenance-tracked import before GIMP can be provisioned and launched automatically.             |
| Bundled Python packaging    | `uv`, `uv tool`, `uv pip`, and packaged wheel/runtime inputs                                     | This is the planned app-private Python foundation for LibreOffice first, and likely for GIMP-side runtime ownership later.                 |
| Bundled model packaging     | Engine model registry plus packaged resource layout under `apps/codehelper/src-tauri/resources/` | The self-contained finish line requires shipping a default model payload instead of expecting external model installation.                 |

---

## Table of Contents

- [Inference Runtimes](#inference-runtimes)
  - [OpenVINO](#openvino)
  - [ONNX Runtime](#onnx-runtime)
  - [Model Export and Quantization](#model-export-and-quantization)
- [Models](#models)
  - [Qwen3 Family (NPU-Verified)](#qwen3-family-npu-verified)
  - [Qwen3.5 Family (Not Yet NPU-Verified)](#qwen35-family-not-yet-npu-verified)
  - [Other NPU-Verified Models](#other-npu-verified-models)
  - [Benchmarks and Leaderboards](#benchmarks-and-leaderboards)
- [MCP (Model Context Protocol)](#mcp-model-context-protocol)
  - [MCP Specification](#mcp-specification)
  - [MCP Servers](#mcp-servers)
- [Historical VS Code Extension Research](#historical-vs-code-extension-research)
  - [Core APIs](#core-apis)
  - [Reference Implementations](#reference-implementations)
- [Frontend Framework](#frontend-framework)
- [Packaging and Distribution](#packaging-and-distribution)
- [Python Tooling](#python-tooling)
- [Rust Ecosystem](#rust-ecosystem)
- [Data Protection](#data-protection)
- [SmolPC Internal](#smolpc-internal)

---

## Inference Runtimes

### OpenVINO

**Primary acceleration target for Intel NPU hardware.**

| Resource                    | URL                                                                                                              | Purpose                                                                                                                                                                   |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OpenVINO 2025 documentation | https://docs.openvino.ai/2025/                                                                                   | Main documentation portal. Covers workflows, deployment, API reference. Start here for any OpenVINO question.                                                             |
| OpenVINO release notes      | https://docs.openvino.ai/2025/about-openvino/release-notes-openvino.html                                         | Track version changes, new device support, breaking changes. Check before upgrading the pinned DLL bundle.                                                                |
| OpenVINO GenAI GitHub       | https://github.com/openvinotoolkit/openvino.genai                                                                | Source for GenAI runtime, C API headers, and sample code. The C API surface is what our Rust FFI wraps.                                                                   |
| GenAI supported models      | https://openvinotoolkit.github.io/openvino.genai/docs/supported-models/                                          | Authoritative list of architectures supported by GenAI pipelines. Check before adopting a new model family.                                                               |
| GenAI NPU inference guide   | https://docs.openvino.ai/2025/openvino-workflow-generative/inference-with-genai/inference-with-genai-on-npu.html | NPU-specific pipeline setup, cache directory, prompt budget, and performance tuning. Essential reading for NPU lane work.                                                 |
| openvino-rs crate           | https://crates.io/crates/openvino                                                                                | Rust bindings for the core OpenVINO C API (v0.9.1). Covers model loading and tensor manipulation only. Does NOT wrap GenAI. We use it for probe/detection, not inference. |

**Caveats:**

- The `openvino-rs` crate wraps the core API only. GenAI (LlmPipeline, StreamerCallback) is accessed through our own FFI layer against `openvino_genai.dll`.
- The GenAI C API exists for FFI wrapping from Rust. See `openvino.genai` repo under `src/c/` for headers.
- All three components (`openvino`, `openvino_genai`, `openvino_tokenizers`) must be from the same release tuple. Mixing versions breaks ABI.

**Related research:**

- NITRO paper (NPU optimization techniques): available on arXiv. Describes NPU compilation strategies relevant to first-load latency.

**Verified:** 2026-03-13

---

### ONNX Runtime

**Fallback runtime for CPU and DirectML GPU acceleration.**

| Resource                   | URL                                                      | Purpose                                                                                                                            |
| -------------------------- | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| ORT GenAI GitHub           | https://github.com/microsoft/onnxruntime-genai           | Source for the GenAI runtime used by the DirectML lane. Our DML FFI wraps this library's C API.                                    |
| ORT GenAI model builder    | https://onnxruntime.ai/docs/genai/howto/build-model.html | Instructions for converting and optimizing models for ORT GenAI. Reference when preparing DML model artifacts.                     |
| ORT GenAI config reference | https://onnxruntime.ai/docs/genai/reference/config.html  | Schema for `genai_config.json` used in `dml/` model artifact directories. Covers search parameters, token IDs, and model settings. |
| ort crate (Rust)           | https://docs.rs/ort/                                     | Rust bindings for ONNX Runtime. Used for the CPU inference lane (KV Cache + Attention Sinks).                                      |

**Caveats:**

- The project is migrating AWAY from the raw `ort` crate for CPU inference long-term. OpenVINO NPU is the primary target. CPU lane via `ort` remains as the final fallback.
- `ort` crate version is pinned to `2.0.0-rc.11` (hardcoded as `ORT_CRATE_VERSION`). Do not upgrade without updating the constant and re-validating the DML lane.
- DirectML lane uses native FFI against `onnxruntime-genai.dll`, not the `ort` crate.

**Verified:** 2026-03-13

---

### Model Export and Quantization

| Resource                           | URL                                                               | Purpose                                                                                                                                                                       |
| ---------------------------------- | ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Optimum-Intel (OpenVINO IR export) | https://huggingface.co/docs/optimum/main/en/intel/openvino/export | Export HuggingFace models to OpenVINO IR format (.xml + .bin). Use when preparing new model artifacts for the NPU lane.                                                       |
| NNCF                               | https://github.com/openvinotoolkit/nncf                           | Neural Network Compression Framework. Handles INT4/INT8 quantization for OpenVINO models. Reference when customizing quantization beyond pre-quantized HuggingFace artifacts. |

**Caveats:**

- Use INT4 quantization, not NF4, for broad NPU compatibility. NF4 is only supported on Intel Core Ultra Series 2 and later.
- Pre-quantized models from the `OpenVINO/` HuggingFace organization are preferred over manual export when available.

**Verified:** 2026-03-13

---

## Models

### Qwen3 Family (NPU-Verified)

**Current primary model family. All sizes below have been verified on NPU hardware.**

| Model      | Notes                                                     |
| ---------- | --------------------------------------------------------- |
| Qwen3-1.7B | Smallest viable Qwen3. Good fit for 8GB RAM constraint.   |
| Qwen3-4B   | Mid-range option. Better quality, higher memory pressure. |
| Qwen3-8B   | Best quality in the family. Tight fit on 8GB devices.     |

| Resource                              | URL                                                           | Purpose                                                                                                                 |
| ------------------------------------- | ------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Qwen3 technical report                | https://arxiv.org/pdf/2505.09388                              | Architecture details, training data, benchmark results. Reference for understanding model capabilities and limitations. |
| Qwen2.5-1.5B-Instruct INT4 (OpenVINO) | https://huggingface.co/OpenVINO/Qwen2.5-1.5B-Instruct-int4-ov | Pre-quantized OpenVINO IR artifact. Currently used as the primary bring-up model for NPU validation.                    |
| Qwen3-8B INT4 (OpenVINO)              | https://huggingface.co/OpenVINO/Qwen3-8B-int4-ov              | Pre-quantized Qwen3 for OpenVINO. Available for NPU deployment.                                                         |

**Verified:** 2026-03-13

---

### Qwen3.5 Family (Not Yet NPU-Verified)

**Released February-March 2026. Promising candidates, but NPU support is unconfirmed.**

| Resource         | URL                                                         | Purpose                                                                                        |
| ---------------- | ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Qwen3.5 analysis | https://artificialanalysis.ai/articles/qwen3-5-small-models | Independent benchmarks and architectural analysis. Good overview of capabilities across sizes. |
| Qwen3.5 GitHub   | https://github.com/QwenLM/Qwen3.5                           | Official repository. Model cards, usage instructions, and architecture documentation.          |

**Available sizes:** 0.8B, 2B, 4B, 9B -- all potential candidates for this project.

**Architecture notes:**

- Hybrid architecture: Gated DeltaNet + MoE (Mixture of Experts)
- 262K context window
- Apache 2.0 license (no usage restrictions)

**Caveats:**

- NPU verification is pending. The hybrid architecture (DeltaNet + MoE) may require specific OpenVINO GenAI support. Do not adopt until verified on target hardware.
- Qwen3.5-4B and 9B scored 80-82% on the AA-Omniscience hallucination benchmark. This is a concern for a student-facing tool -- monitor improvements.

**Verified:** 2026-03-13

---

### Other NPU-Verified Models

These models have been confirmed to work with OpenVINO GenAI on NPU hardware.

| Model                  | Notes                                                      |
| ---------------------- | ---------------------------------------------------------- |
| Gemma-3-1B-it          | Google. Smallest option. Limited capability but very fast. |
| Gemma-3-4B-it          | Google. Good balance of speed and quality.                 |
| Phi-3-Mini-4K-Instruct | Microsoft. 4K context. Solid code generation.              |
| Phi-4-mini-reasoning   | Microsoft. Reasoning-focused variant.                      |
| AFM-4.5B               | Apple Foundation Model. 4.5B parameters.                   |

**Pre-quantized artifacts:**

| Artifact                        | URL                                                                   | Purpose                                                           |
| ------------------------------- | --------------------------------------------------------------------- | ----------------------------------------------------------------- |
| Phi-4-mini INT4 (OpenVINO, NPU) | https://huggingface.co/FluidInference/phi-4-mini-instruct-int4-ov-npu | Pre-quantized and NPU-optimized Phi-4-mini. Ready for deployment. |

**Verified:** 2026-03-13

---

### Benchmarks and Leaderboards

| Resource                        | URL                                              | Purpose                                                                                                                                                   |
| ------------------------------- | ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| BFCL leaderboard (tool calling) | https://gorilla.cs.berkeley.edu/leaderboard.html | Berkeley Function Calling Leaderboard. Use to compare model performance on tool/function calling tasks. Critical for MCP tool-calling quality assessment. |
| HumanEval                       | (standard benchmark, no single URL)              | Code generation benchmark. Use published scores from model cards to compare code quality across candidates.                                               |

**Verified:** 2026-03-13

---

## MCP (Model Context Protocol)

### MCP Specification

| Resource          | URL                                   | Purpose                                                                                                                                                         |
| ----------------- | ------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| MCP specification | https://spec.modelcontextprotocol.io/ | Full protocol spec. Reference for implementing MCP client in the unified assistant. Covers message formats, capability negotiation, and transport requirements. |

**Protocol details:**

- Wire format: JSON-RPC 2.0
- Transports: stdio, TCP, HTTP (SSE)
- Our implementation will likely use TCP transport for desktop app servers (GIMP, Blender, LibreOffice)

**Verified:** 2026-03-13

---

### MCP Servers

Servers the unified assistant will connect to for application control.

| Server          | GitHub                 | Stars  | Transport  | Notes                                                                                                                    |
| --------------- | ---------------------- | ------ | ---------- | ------------------------------------------------------------------------------------------------------------------------ |
| GIMP MCP        | `maorcc/gimp-mcp`      | ~58    | TCP :10008 | Provides `call_api` escape hatch for arbitrary GIMP Script-Fu execution. Primary integration surface for GIMP assistant. |
| Blender MCP     | `ahujasid/blender-mcp` | ~17.7K | TCP :9876  | Provides `execute_blender_code` for arbitrary Blender Python execution. Very popular, well-maintained.                   |
| LibreOffice MCP | `patrup/mcp-libre`     | --     | stdio      | 14 tools in standalone mode, 73 tools in extension mode. Listed in the official MCP server directory.                    |

**Emerging standard:**

- MCPB (MCP Bundle) format: `.mcpb` zip archive containing a `manifest.json` and server implementation. Watch for adoption across the ecosystem.

**Verified:** 2026-03-13

---

## Historical VS Code Extension Research

These resources are retained for historical context and possible future work.
They are not part of the active unified frontend implementation path.

### Core APIs

| Resource                     | URL                                                                                  | Purpose                                                                                                                               |
| ---------------------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------- |
| VS Code Extension API        | https://code.visualstudio.com/api                                                    | Top-level entry point for all extension development. Start here for activation events, contribution points, and extension manifest.   |
| InlineCompletionItemProvider | https://code.visualstudio.com/api/references/vscode-api#InlineCompletionItemProvider | API for providing inline code completions (ghost text). This is how the code assistant surfaces suggestions in the editor.            |
| Webview API                  | https://code.visualstudio.com/api/extension-guides/webview                           | Building custom UI panels inside VS Code. Relevant for chat panels, settings UI, and diagnostics views.                               |
| Language Server Protocol     | https://microsoft.github.io/language-server-protocol/                                | LSP specification. Reference if implementing language-aware features (diagnostics, hover, go-to-definition) beyond simple completion. |

**Verified:** 2026-03-13

---

### Reference Implementations

Study these for patterns, architecture decisions, and pitfalls.

| Project      | URL                                     | Size      | Notes                                                                                                                                                                                          |
| ------------ | --------------------------------------- | --------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Continue.dev | https://github.com/continuedev/continue | ~50K LOC  | Full-featured AI coding assistant. Good reference for extension architecture, multi-provider support, and chat UI. Large codebase -- focus on specific modules rather than reading everything. |
| Twinny       | https://github.com/rjmacarthy/twinny    | ~3.6K LOC | Minimal local AI coding assistant. Good reference for understanding the minimum viable VS Code AI extension. **Archived** -- no longer maintained, but code is still instructive.              |
| Cline        | https://github.com/cline/cline          | ~30K LOC  | Agentic AI assistant. Good reference for tool use patterns, file editing, and terminal integration from within VS Code.                                                                        |

**Verified:** 2026-03-13

---

## Frontend Framework

| Resource               | URL                                           | Purpose                                                                                                                                                     |
| ---------------------- | --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tauri 2 docs           | https://v2.tauri.app/                         | Main documentation for the desktop app framework. Covers commands, plugins, window management, and build configuration.                                     |
| Tauri 2 IPC / Channels | https://v2.tauri.app/develop/calling-rust/    | IPC mechanism between frontend and Rust backend. Includes Channel-based streaming, which is how token streaming reaches the UI.                             |
| Svelte 5 runes         | https://svelte.dev/docs/svelte/what-are-runes | Reactivity system for Svelte 5. This project uses runes (`$state`, `$derived`, `$effect`) exclusively. Do NOT use Svelte 4 stores (`writable`, `readable`). |
| Tailwind CSS 4         | https://tailwindcss.com/docs                  | Utility-first CSS framework. Version 4 does NOT support `@apply`. Use utility classes directly in templates.                                                |
| SvelteKit              | https://svelte.dev/docs/kit                   | Full-stack Svelte framework. Referenced for routing patterns and layout conventions even though the app runs in Tauri, not a traditional web server.        |

**Caveats:**

- Svelte 5 runes are mandatory. Any code using `import { writable } from 'svelte/store'` is wrong for this project.
- Tailwind 4 removed `@apply`. Do not use it. Apply utility classes directly to elements.
- Tauri Channels (not Events) are the correct pattern for streaming. Channels are command-scoped and ordered; Events are global broadcast.

**Verified:** 2026-03-13

---

## Packaging and Distribution

| Resource                       | URL                                                                | Purpose                                                                                                             |
| ------------------------------ | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- |
| Tauri Windows installer (NSIS) | https://v2.tauri.app/distribute/windows-installer/                 | NSIS-based installer configuration. Covers install paths, shortcuts, registry entries, and uninstall behavior.      |
| Tauri code signing             | https://v2.tauri.app/distribute/sign/windows/                      | Windows code signing setup for Tauri apps. Required for production distribution to avoid SmartScreen warnings.      |
| Tauri updater plugin           | https://v2.tauri.app/plugin/updater/                               | Auto-update mechanism. Covers update server configuration, signature verification, and rollback.                    |
| Azure Trusted Signing          | https://azure.microsoft.com/en-in/pricing/details/trusted-signing/ | Cloud-based code signing service. $9.99/month. Alternative to purchasing a traditional EV code signing certificate. |

**Verified:** 2026-03-13

---

## Python Tooling

**Python is used for model export/conversion scripts and MCP server sidecar processes.**

| Resource           | URL                                   | Purpose                                                                                                                                                |
| ------------------ | ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| uv (Astral)        | https://docs.astral.sh/uv/            | Fast Python package installer and resolver. Used for managing Python dependencies in the project.                                                      |
| uv offline usage   | https://ajfriend.com/uv_offline/      | Guide for using uv without internet access. Critical for offline-first deployment -- pre-cache packages for air-gapped installs.                       |
| uv-pack            | https://github.com/davnn/uv-pack      | Creates portable offline Python environment bundles. Use for packaging Python MCP servers or export scripts as sidecars.                               |
| Tauri sidecar docs | https://v2.tauri.app/develop/sidecar/ | How to bundle and launch external binaries (including Python scripts) from a Tauri app. Covers permissions, path resolution, and lifecycle management. |

**Verified:** 2026-03-13

---

## Rust Ecosystem

| Resource         | URL                         | Purpose                                                                                                                                                                |
| ---------------- | --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ort crate        | https://docs.rs/ort/        | ONNX Runtime Rust bindings. Used for the CPU inference lane. Note: DEPRECATED for this project's CPU lane long-term; OpenVINO NPU is the migration target.             |
| libloading crate | https://docs.rs/libloading/ | Safe DLL loading. Used in `runtime_loading.rs` for loading OpenVINO and ORT GenAI DLLs at runtime. All `Library::new()` calls must go through this centralized module. |
| Axum             | https://docs.rs/axum/       | HTTP server framework. Powers `smolpc-engine-host`. Handles routing, SSE streaming, and request/response types.                                                        |
| tokio            | https://docs.rs/tokio/      | Async runtime. Foundation for the engine host server and all async inference operations.                                                                               |
| serde            | https://docs.rs/serde/      | Serialization/deserialization. Used throughout for JSON API payloads, configuration files, and model manifests.                                                        |

**Verified:** 2026-03-13

---

## Data Protection

| Framework | Notes                                                                                                                         |
| --------- | ----------------------------------------------------------------------------------------------------------------------------- |
| UK GDPR   | Governs processing of personal data for UK users. Relevant because the target audience includes UK secondary school students. |
| FERPA     | US Federal Educational Rights and Privacy Act. Governs student education records. Relevant if deployed in US school settings. |

**Key compliance position:** The offline-first architecture is the strongest possible compliance stance. No student data leaves the device. No telemetry, no cloud inference, no external API calls. Document this prominently in any compliance review.

**Verified:** 2026-03-13

---

## SmolPC Internal

### GitHub

| Resource            | URL                                             | Purpose                                                                                      |
| ------------------- | ----------------------------------------------- | -------------------------------------------------------------------------------------------- |
| GitHub organization | https://github.com/SmolPC-2-0/                  | All SmolPC repositories.                                                                     |
| Main monorepo       | https://github.com/SmolPC-2-0/smolpc-codehelper | Primary development repository containing engine, apps, utilities, and the unified app work. |

### Monorepo Zone Map

These are the key internal paths within the monorepo. Reference `docs/ARCHITECTURE.md` for the full zone map and ownership rules.

| Zone                  | Path                                  | Purpose                                                                                                                                                              |
| --------------------- | ------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Engine host           | `engine/crates/smolpc-engine-host/`   | Axum HTTP server. Backend selection policy, load/inference handlers, startup probes. Entry point for the shared engine process.                                      |
| Engine core           | `engine/crates/smolpc-engine-core/`   | Inference runtimes (ORT CPU, DirectML FFI, OpenVINO GenAI FFI), model loading, backend status, benchmarking. All DLL loading is centralized in `runtime_loading.rs`. |
| Engine client         | `engine/crates/smolpc-engine-client/` | Typed HTTP client for apps to communicate with the engine. Used by Tauri app backends.                                                                               |
| Hardware detection    | `engine/crates/hardware_query/`       | System capability detection (NPU presence, GPU info, memory). Used during backend selection.                                                                         |
| GIMP assistant        | `apps/gimp-assistant/`                | Reference app for GIMP integration via MCP. Currently being migrated to use engine-client.                                                                           |
| Blender assistant     | `apps/blender-assistant/`             | Reference app for Blender integration via MCP.                                                                                                                       |
| Code Helper           | `apps/codehelper/`                    | Primary Tauri desktop app. Svelte 5 frontend + Tauri backend with engine client integration.                                                                         |
| LibreOffice assistant | `apps/libreoffice-assistant/`         | LibreOffice integration app.                                                                                                                                         |
| Launcher              | `launcher/`                           | Historical / optional utility zone. Not part of the active unified frontend architecture.                                                                            |

**Verified:** 2026-03-13

---

## How to Use This Document

1. **Starting a new feature:** Check the relevant category above for official docs, API references, and caveats before writing code.
2. **Evaluating a new model:** Check [Models](#models) for NPU verification status and pre-quantized artifacts. Do not adopt unverified models for the NPU lane.
3. **Debugging runtime issues:** Check the caveats under [OpenVINO](#openvino) and [ONNX Runtime](#onnx-runtime) for known gotchas (DLL load order, version pinning, quantization format).
4. **Adding a new MCP integration:** Check [MCP Servers](#mcp-servers) for existing server implementations and transport details.
5. **Updating dependencies:** Check version caveats before upgrading. OpenVINO components must stay in sync. ORT crate version is pinned.

**Keeping this document current:** When a resource URL changes, a new version is adopted, or a model's NPU status is verified, update the relevant entry and bump the "Last Updated" date at the top. Add a "Verified" date to individual sections when re-checked.
