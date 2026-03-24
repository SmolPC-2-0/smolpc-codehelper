---
paths:
  - "engine/**/src/*.rs"
  - "apps/**/supervisor.rs"
  - "apps/**/handle.rs"
  - "engine/**/tts_sidecar.rs"
  - "apps/**/modes/**/runtime.rs"
---

# Windows Process Lifecycle Rules

- Detached processes (`DETACHED_PROCESS`) survive parent exit — always write PID to file on spawn
- Before force-killing via stale PID file, verify the process identity (name/path) — PIDs are reused by the OS
- Clean up PID file on both graceful exit and after force-kill
- Redirect stderr to a log file, never `Stdio::null()` — makes crash diagnosis impossible on user machines
- Use `CREATE_NO_WINDOW` for background sidecars to prevent console flash
- Tauri 2 `Builder::run()` has no RunEvent access — use `Builder::build()` + `App::run(callback)` to hook `ExitRequested`
- Use `tauri::async_runtime::block_on()` for async cleanup in `RunEvent::ExitRequested` — the closure is sync (`FnMut`)
