# Windows Gotchas

SmolPC Code Helper is a Windows-first application. This document collects platform-specific issues discovered during development — DLL loading quirks, process lifecycle traps, tool behavior differences, and workarounds that are not obvious from documentation.

## DLL Loading

### Load Order Matters

Windows resolves DLL dependencies implicitly when you load a library. If library A depends on library B, Windows tries to find B using a search order that includes the application directory, System32, and PATH. If B has not been loaded yet and is not in the search path, A fails to load — but the error message names A, not B.

OpenVINO has 14 DLLs with a deep dependency chain. Loading `openvino_genai_c.dll` before `tbb12.dll` fails because the GenAI DLL implicitly depends on TBB. The error says "Failed to load openvino_genai_c.dll" — it does not mention TBB.

**Solution:** Load all DLLs in explicit dependency order in `runtime_loading.rs`. A CI test enforces that no other file in the workspace calls `Library::new()` or `load_with_flags()`.

### Load-With-Flags for Search Path Control

The default `LoadLibrary` search order includes the working directory and PATH, which can pick up wrong DLL versions. We use `LoadLibraryExW` with flags:

```rust
let flags = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32;
```

This restricts dependency resolution to the DLL's own directory (where our bundled DLLs live) plus System32 (for OS dependencies like `kernel32.dll`). Without these flags, a stale `tbb12.dll` on the user's PATH could be loaded instead of our bundled version, causing ABI mismatches.

### Extended-Length Path Prefix

Tauri's `resource_dir()` can return paths with the `\\?\` extended-length prefix on some Windows configurations. This prefix is incompatible with `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR` in `LoadLibraryExW`. The `strip_extended_length_prefix()` function in `runtime_bundles.rs` removes it before constructing DLL paths.

## Process Lifecycle

### Detached Processes Need PID Files

Processes spawned with `DETACHED_PROCESS` survive their parent's exit. This is intentional (the engine should outlive the app), but it means there is no automatic cleanup. Rules:

- Write a PID file immediately after spawn
- Check the PID file on startup to detect already-running instances
- Verify process identity before killing via a stale PID (see below)

### PID Reuse

Windows aggressively reuses PIDs. A stale PID file from a crashed engine might now point to an unrelated process (e.g., Explorer, a game, another app). **Always verify the process name/path matches the expected binary before killing.**

The engine client uses Toolhelp32 (`OpenProcess`) for PID liveness checks in `is_lock_holder_dead()`. It does not use `taskkill` for liveness detection.

### taskkill.exe Can Hang

`taskkill.exe /F /IM <name>` can hang for 60+ seconds on some Windows machines, particularly when the target process is stuck in a driver call. The hang is in `taskkill.exe` itself — it cannot be timed out from the calling process.

**Workaround:** Use `taskkill /F /IM` only for cleanup (with `CREATE_NO_WINDOW` to suppress the console), not for time-sensitive operations. For liveness checks, use Toolhelp32 directly.

### CREATE_NO_WINDOW

Background processes (engine, TTS sidecar) must be spawned with `CREATE_NO_WINDOW` (0x08000000). Without this flag, spawning a console application briefly flashes a black console window. Students would see a flickering rectangle every time the engine restarts.

### Stderr Capture

**Never use `Stdio::null()` for detached daemon stderr.** When the engine crashes due to an FFI error (segfault in OpenVINO, GPU driver hang), the only diagnostic is the stderr output. Redirecting stderr to `/dev/null` makes crashes invisible.

Always redirect stderr to a log file:

```rust
let stderr_file = File::create(log_path)?;
cmd.stderr(Stdio::from(stderr_file));
```

The engine writes to `engine-spawn.log` in the engine runtime directory.

### Tauri Exit Hook

Tauri 2's `Builder::run()` does not provide access to `RunEvent`. To hook `ExitRequested` for graceful engine shutdown, use `Builder::build()` + `App::run(callback)`:

```rust
let app = tauri::Builder::default().build()?;
app.run(|_handle, event| {
    if let RunEvent::ExitRequested { .. } = event {
        tauri::async_runtime::block_on(async {
            // Shut down engine gracefully
        });
    }
});
```

The closure is sync (`FnMut`), so async cleanup requires `block_on()`.

## WMI Queries

`Get-WmiObject` and `Get-CimInstance` can hang indefinitely on some Windows machines. The hang is in the WMI service and cannot be timed out from PowerShell or Rust.

**Never use WMI for GPU detection.** DXGI adapter enumeration (`IDXGIFactory6`) completes in ~14ms consistently and is the authoritative GPU detection path. The app-side hardware detector (`app/src-tauri/src/hardware/detector.rs`) does not attempt GPU detection because `sysinfo` v0.32.1 lacks GPU support — the engine's DXGI probe in `probe.rs` is the only GPU detection.

## PowerShell

### Stderr Interception

PowerShell treats stderr output from external tools as error records. In strict mode (`$ErrorActionPreference = 'Stop'`), a tool writing to stderr (even informational messages) can throw a terminating exception.

**Workaround:** Coerce stderr records to plain strings:

```powershell
$output = & cargo build 2>&1 | ForEach-Object { "$_" }
```

### Compress-Archive Has a 2 GB Limit

`Compress-Archive` silently corrupts archives larger than 2 GB. Model archives frequently exceed this.

**Use `tar.exe` instead:**

```powershell
& "$env:SystemRoot\System32\tar.exe" -czf archive.tar.gz -C source_dir .
```

### Use System32 tar.exe, Not Git Bash tar

Git Bash's `tar` cannot handle Windows-style paths (`C:\Users\...`). Always use the full path to the system tar:

```powershell
& "$env:SystemRoot\System32\tar.exe" ...
```

If you just call `tar`, the shell might resolve to Git Bash's version first if Git is on PATH.

### Splatting and Array Arguments

PowerShell splatting (`@args`) can silently flatten or reformat arguments. For cargo flags, use explicit `if/else` blocks rather than building argument arrays dynamically:

```powershell
# Avoid:
$flags = @("--release")
if ($target) { $flags += "--target", $target }
cargo build @flags

# Prefer:
if ($target) {
    cargo build --release --target $target
} else {
    cargo build --release
}
```

### $PSScriptRoot with Paste

When pasting PowerShell commands from documentation into a terminal, `$PSScriptRoot` is empty (it is only set when running from a `.ps1` file). Scripts that use `$PSScriptRoot` for relative paths silently use the current directory instead.

## NSIS Installer

### Kebab-Cased Output Names

Tauri's NSIS builder kebab-cases the output filename: `SmolPC Code Helper` becomes `smol-pc-code-helper_1.0.0_x64-setup.exe` (or similar). **Do not hardcode the exact output filename.** Glob for `*-setup.exe` in the output directory.

### Installer Source Path ($EXEDIR)

The NSIS post-install hook writes `$EXEDIR` (the directory containing the installer) to `installer-source.txt`. This is the directory the installer was launched from, not the install target. On a USB install, this points back to the USB drive.

## File Locking

When a file cannot be deleted or renamed because it is locked, **investigate the lock holder before deleting the lock file.** Common culprits:

- **Antivirus.** Windows Defender can hold locks on newly extracted DLLs for scanning. Retry with exponential backoff (the extractor retries removal up to 3 times with 500ms delays).
- **Engine process.** A stale engine process may hold the DLL file. Verify the process is dead before attempting cleanup.
- **Explorer.** Windows Explorer can lock files in directories it is displaying.

Never blindly delete a lock file — the lock may represent legitimate in-progress work (e.g., concurrent provisioning protected by the global mutex).

## Orphaned Processes

Model export and build scripts that use `timeout` or `Start-Process` can leave orphaned Python processes after a timeout kills the parent. These orphaned processes hold file locks and prevent subsequent operations.

**Always check for orphaned processes after a timeout:**

```powershell
Get-Process -Name "python*" -ErrorAction SilentlyContinue |
    Where-Object { $_.StartTime -gt $scriptStart } |
    Stop-Process -Force
```

## Environment Variable Propagation

Environment variables set in the shell (`$env:SMOLPC_FORCE_EP`) do **not** automatically reach the engine process. The `EngineSupervisor` explicitly controls the engine's environment via `cmd.env()` / `cmd.env_remove()` in `spawn_engine`. To override the backend:

- Use the dev script's `-ForceEp` parameter, or
- Use the frontend's runtime mode preference (which sends a command through the supervisor)

Setting `$env:SMOLPC_FORCE_EP` in the terminal only affects `cargo run -p smolpc-engine-host` when running the engine directly — it does not reach the engine when spawned by the Tauri app.
