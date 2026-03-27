# Security and Privacy

SmolPC Code Helper is deployed to secondary school students (ages 11-18). This creates specific security and privacy requirements: the app handles minors' data, runs on shared school machines, and must comply with data protection regulations without relying on cloud infrastructure.

## Privacy by Design

The core principle: **no user data leaves the device.**

- **Offline-first.** The app works without internet. All inference runs locally on the student's CPU, GPU, or NPU. There is no cloud backend.
- **No telemetry.** The app does not phone home. There are no analytics, crash reporters, usage trackers, or update checks.
- **No user accounts.** There is no login, no registration, no user profile. The app stores chat history locally and does not associate it with an identity.
- **No model training.** User conversations are not used to fine-tune or improve the AI model. The model weights are read-only artifacts shipped with the installer.
- **No network listeners.** The engine listens on `127.0.0.1:19432` (localhost only). It is not accessible from other machines on the network. The TTS sidecar listens on `127.0.0.1:19433`.

## GDPR and FERPA Compliance

**GDPR** (General Data Protection Regulation) applies because the project is deployed in the UK to minors. **FERPA** (Family Educational Rights and Privacy Act) is relevant as a reference framework for educational data privacy.

**How the architecture supports compliance:**

| Requirement | How We Meet It |
|---|---|
| Data minimization | No personal data is collected. Chat history is stored locally and never transmitted. |
| Purpose limitation | The app has one purpose: coding assistance. No data is repurposed. |
| Storage limitation | Chat history persists only on the local machine. No server-side retention. |
| Right to erasure | Uninstalling the app removes all data from `%LOCALAPPDATA%`. |
| Data protection by design | Offline-first architecture eliminates data transmission risks. |
| Parental consent (under 16) | Not required — no personal data is processed by a data controller. |
| Cross-border transfer | Not applicable — data never leaves the device. |

The key insight: by keeping everything local, most GDPR obligations around data controllers, processors, and cross-border transfers simply do not apply. There is no data controller because there is no centralized data processing.

## Content Security Policy

**Source:** `app/src-tauri/tauri.conf.json`

The Tauri app enforces a Content Security Policy on the WebView:

```
default-src 'self';
script-src 'self';
style-src 'self' 'unsafe-inline';
img-src 'self' data: blob:;
font-src 'self';
connect-src 'self';
object-src 'none';
base-uri 'self';
form-action 'self'
```

| Directive | Value | Rationale |
|---|---|---|
| `script-src` | `'self'` | Only scripts bundled with the app can execute. No inline scripts, no external scripts. |
| `style-src` | `'self' 'unsafe-inline'` | Bundled styles plus inline styles (needed by Svelte's scoped CSS and dynamic styling). |
| `img-src` | `'self' data: blob:` | Bundled images plus `data:` URIs for programmatic images (e.g., base64 icons) and `blob:` for generated content. |
| `connect-src` | `'self'` | Network requests only to the app's own origin (the Tauri IPC bridge). The engine API on `localhost:19432` is accessed through Tauri invoke, not direct fetch. |
| `object-src` | `'none'` | No plugins (Flash, Java applets, etc.). |
| `form-action` | `'self'` | Forms can only submit to the app itself. Prevents form-based data exfiltration. |

## Path Validation

**Source:** `app/src-tauri/src/security/mod.rs`

All file paths received from the frontend are validated before any file operation:

1. **Canonicalization.** Paths are resolved via `std::fs::canonicalize()` to eliminate `..`, `.`, and symlinks. This prevents symlink escape attacks where a path appears to be within an allowed directory but actually points elsewhere.
2. **Allowlist check.** The canonical path must be a descendant of one of three allowed directories: `app_data_dir`, `app_cache_dir`, or `app_local_data_dir`. Any path outside these is rejected.
3. **Non-existent path rejection.** `canonicalize()` requires the path to exist. Paths to non-existent files fail validation, preventing attacks that construct paths to files that will be created.

This approach prevents directory traversal attacks (e.g., `../../etc/passwd`) and symlink escapes (e.g., a symlink inside the app data directory pointing to a system file).

## File Size Validation

**Source:** `app/src-tauri/src/security/mod.rs`

All file operations enforce a 10 MB size limit (`MAX_FILE_SIZE = 10 * 1024 * 1024`):

- `validate_content_size(content)` — checks string length before writing
- `validate_file_size(path)` — checks file metadata before reading

This prevents memory exhaustion attacks where a malicious or corrupted file causes the app to allocate unbounded memory. The 10 MB limit is generous for chat exports and configuration files but blocks multi-gigabyte reads.

## Authentication

**Source:** `engine/crates/smolpc-engine-host/src/auth.rs`

The engine requires a Bearer token on every HTTP request (except `/engine/health`):

- **Token generation.** A UUID is auto-generated on first run and written to the engine runtime directory.
- **Token rotation.** The token file is deleted and regenerated each time the engine spawns. Old tokens become invalid immediately.
- **Constant-time comparison.** Token verification uses XOR-based byte comparison that takes the same time regardless of where a mismatch occurs. This prevents timing side-channel attacks where an attacker measures response time to guess the token byte-by-byte.
- **Per-installation.** Each machine gets its own token. There is no shared secret across installations.

The health endpoint (`/engine/health`) is unauthenticated so that the supervisor can check engine liveness without a token. It returns only a boolean readiness status — no sensitive information.

## Network Isolation

The engine binds to `127.0.0.1`, not `0.0.0.0`. This means:

- Other machines on the same network cannot reach the engine
- Other users on the same machine can reach it (localhost is shared), but they need the auth token
- The Tauri app communicates via localhost HTTP, which never touches the network stack

The TTS sidecar (`localhost:19433`) has no authentication because it is only reachable from localhost and is proxied through the engine host.

Connector bridges use the same localhost-only pattern:
- Blender bridge: `127.0.0.1:5179` with Bearer token authentication
- GIMP bridge: `127.0.0.1:10008` (loopback only, no auth — same process tree)
- LibreOffice MCP server: stdio transport (no network exposure)

## Model Data

The AI models (Qwen 2.5, Qwen 3) are pre-trained open-weight models from HuggingFace:

- **No personal data in weights.** The models were trained on public internet data, not student data.
- **No fine-tuning.** The app does not modify model weights. They are read-only artifacts.
- **No retrieval augmentation with user data.** The Blender connector uses RAG over bundled Blender API documentation, not user content.
- **Model outputs are ephemeral.** Generated text exists only in the chat UI and local chat history. It is not stored server-side or transmitted.

## Threat Model Summary

| Threat | Mitigation |
|---|---|
| Data exfiltration via network | No outbound network access; engine and bridges on localhost only |
| Unauthorized engine access | Bearer token auth with constant-time comparison, rotated per spawn |
| Path traversal / symlink escape | Canonicalization + allowlist validation |
| Memory exhaustion via large files | 10 MB file size limit |
| XSS in WebView | Content Security Policy blocks inline scripts and external resources |
| Timing attack on auth token | Constant-time XOR comparison |
| Partial model extraction | Atomic extraction prevents corrupted model state |
| Stale PID file exploitation | Process identity verification before kill (PIDs are reused by Windows) |
| Concurrent provisioning corruption | Windows global mutex (`Global\SmolPC-Provisioning`) serializes access |
| Model poisoning | SHA-256 verification of archives before extraction |
