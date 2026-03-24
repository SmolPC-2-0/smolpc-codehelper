---
paths:
  - "apps/**/src-tauri/**/*.rs"
---

# Tauri 2 App Patterns

- Use `tauri::async_runtime::spawn` not `tokio::spawn` in `Builder::setup()` — Tauri manages its own Tokio runtime
- Engine lifecycle is owned by `EngineSupervisor` — commands send via mpsc channel, read state via watch channel, never spawn/kill directly
- Get engine client via `supervisor.get_client(timeout)` (blocking) or `get_client_if_ready()` (non-blocking)
- Runtime mode preference must be in the Start command (`StartupConfig`) — never apply post-spawn
- Use `tauri::ipc::Channel<T>` for streaming (command-scoped, ordered), not global Events
- Tauri resource map format (`"libs/": "libs/"`) recursively copies directories — preferred over glob arrays for nested DLL layouts
- NSIS `installMode: "currentUser"` installs to `%LOCALAPPDATA%\<productName>\` and kebab-cases the binary name
