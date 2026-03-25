# Standalone Engine Usage Guide

Use this guide when you want to run and call `smolpc-engine-host` directly, without going through CodeHelper.

This is the right path for:

- manual engine validation
- non-Rust apps or external wrappers
- direct localhost API testing
- debugging startup, auth, or runtime issues outside the app shell

If you are building a Rust or Tauri app in this repo, prefer `smolpc-engine-client` instead. If you need the full endpoint reference, use [docs/ENGINE_API.md](./ENGINE_API.md). If you are onboarding another app, use [docs/APP_ONBOARDING_PLAYBOOK.md](./APP_ONBOARDING_PLAYBOOK.md).

Current `main` assumptions:

- the engine is localhost-only
- every request requires `Authorization: Bearer <token>`
- one local engine process serves `http://127.0.0.1:19432`

## What The Engine Is

`smolpc-engine-host` is the shared localhost inference server for SmolPC apps.

It serves:

- `/engine/*` for lifecycle, readiness, model loading, and diagnostics
- `/v1/*` for OpenAI-compatible inference calls

The host binary does not bootstrap its own auth token. For direct standalone use, the token must already exist or be created manually before startup, and the same token must be exposed to the process via `SMOLPC_ENGINE_TOKEN`.

## Prerequisites

This guide is Windows-first and assumes you are working from a current checkout of this repo.

For the closest match to current app behavior, stage the same assets current apps rely on before following the manual startup steps below.

From the repo root:

```powershell
npm ci
cargo build -p smolpc-engine-host
npm run runtime:setup:openvino
npm run model:setup:qwen25-instruct
npm run model:setup:qwen3-4b
```

Optional DirectML runtime staging:

```powershell
cd apps/codehelper
npm run runtime:setup:dml
cd ../..
```

Notes:

- `runtime:setup:openvino` stages the runtime bundle used by CPU and OpenVINO-backed host flows.
- `runtime:setup:dml` is only needed if you want to validate DirectML.
- `runtime:setup:python` is part of current app packaging, but it is not required for plain engine-only `/engine/*` and `/v1/*` testing.
- The examples below assume models are under `%LOCALAPPDATA%\SmolPC\models\`, which matches current shared app behavior.

## Runtime Layout

The recommended standalone layout should mirror the app-managed runtime layout on current `main`:

- shared runtime root:
  `%LOCALAPPDATA%\SmolPC\engine-runtime`
- token file:
  `%LOCALAPPDATA%\SmolPC\engine-runtime\engine-token.txt`
- PID file:
  `%LOCALAPPDATA%\SmolPC\engine-runtime\engine.pid`
- spawn log:
  `%LOCALAPPDATA%\SmolPC\engine-runtime\engine-spawn.log`
- host data dir:
  `%LOCALAPPDATA%\SmolPC\engine-runtime\host-data`
- shared models root:
  `%LOCALAPPDATA%\SmolPC\models`

The host binary itself has a fallback `--data-dir` default if you omit that argument, but the recommended standalone workflow should keep using the app-compatible `engine-runtime\host-data` layout so that logs, tokens, and troubleshooting match current app behavior.

`engine.pid` and `engine-spawn.log` are normally created by the app/client spawn path. The manual startup example below writes them into the same locations so your standalone flow matches the current app layout.

## Manual Startup

The primary example below uses the repo-built host binary and the current repo resource layout.

### 1. Prepare PowerShell variables and token

Run this from the repo root in PowerShell:

```powershell
$repoRoot = (Get-Location).Path
$runtimeRoot = Join-Path $env:LOCALAPPDATA "SmolPC\engine-runtime"
$dataDir = Join-Path $runtimeRoot "host-data"
$modelsDir = Join-Path $env:LOCALAPPDATA "SmolPC\models"
$tokenPath = Join-Path $runtimeRoot "engine-token.txt"
$pidPath = Join-Path $runtimeRoot "engine.pid"
$spawnLog = Join-Path $runtimeRoot "engine-spawn.log"
$hostBin = Join-Path $repoRoot "target\debug\smolpc-engine-host.exe"
$resourceDir = Join-Path $repoRoot "apps\codehelper\src-tauri"

New-Item -ItemType Directory -Force -Path $runtimeRoot | Out-Null
New-Item -ItemType Directory -Force -Path $dataDir | Out-Null

if (!(Test-Path $tokenPath)) {
    $token = ([guid]::NewGuid().ToString("N") + [guid]::NewGuid().ToString("N")).Substring(0, 48)
    Set-Content -Path $tokenPath -Value $token -NoNewline
}

$token = (Get-Content $tokenPath -Raw).Trim()
$env:SMOLPC_ENGINE_TOKEN = $token
$env:SMOLPC_MODELS_DIR = $modelsDir
```

This does three important things:

- creates the app-compatible runtime directories
- creates `engine-token.txt` if it does not already exist
- exports the same token into `SMOLPC_ENGINE_TOKEN`, which the host requires at startup

### 2. Optional backend override

Leave backend selection on automatic mode unless you are doing targeted validation.

Examples:

```powershell
$env:SMOLPC_FORCE_EP = "cpu"
```

```powershell
$env:SMOLPC_FORCE_EP = "dml"
```

```powershell
$env:SMOLPC_FORCE_EP = "openvino_npu"
```

If you want automatic selection instead:

```powershell
Remove-Item Env:SMOLPC_FORCE_EP -ErrorAction SilentlyContinue
```

### 3. Start the host

```powershell
$arguments = @(
    "--port", "19432",
    "--data-dir", $dataDir,
    "--resource-dir", $resourceDir,
    "--app-version", "standalone-dev"
)

$engine = Start-Process `
    -FilePath $hostBin `
    -ArgumentList $arguments `
    -PassThru `
    -WindowStyle Hidden `
    -RedirectStandardError $spawnLog

Set-Content -Path $pidPath -Value $engine.Id -NoNewline
```

Notes:

- `--resource-dir` is optional, but recommended in repo-dev/manual workflows because the host resolves runtime bundles from `resource_dir\libs`.
- `SMOLPC_MODELS_DIR` remains the supported way to point the host at the shared models root.
- If you are using a packaged install instead of a repo checkout, the host binary and resource root will come from the packaged app resources rather than `target\debug` and `apps\codehelper\src-tauri`.

### 4. Confirm the process is up

```powershell
Get-Process -Id $engine.Id
```

If that succeeds, you can begin calling the API with the same `$token` you loaded above.

## Manual Request Flow

Use `curl.exe`, not bare `curl`, so PowerShell does not route the command through `Invoke-WebRequest`.

### 1. Health check

```powershell
curl.exe -s `
  -H "Authorization: Bearer $token" `
  http://127.0.0.1:19432/engine/health
```

Expected shape:

```json
{"ok":true}
```

### 2. Read engine metadata

```powershell
curl.exe -s `
  -H "Authorization: Bearer $token" `
  http://127.0.0.1:19432/engine/meta
```

Use this to confirm:

- the engine is reachable
- the protocol version matches expectations
- the host PID is the process you just started

### 3. Recommended startup handshake: `ensure-started`

Set a small default model first:

```powershell
$ensureStartedBody = '{"mode":"auto","startup_policy":{"default_model_id":"qwen2.5-1.5b-instruct"}}'

curl.exe -s `
  -X POST `
  -H "Authorization: Bearer $token" `
  -H "Content-Type: application/json" `
  --data "$ensureStartedBody" `
  http://127.0.0.1:19432/engine/ensure-started
```

This is the recommended startup handshake because it gives you a blocking readiness path instead of jumping straight to raw generation calls.

### 4. Check current readiness and backend state

```powershell
curl.exe -s `
  -H "Authorization: Bearer $token" `
  http://127.0.0.1:19432/engine/status
```

At this point, you should verify:

- `ready` or `state=ready`
- `current_model` / `active_model_id`
- `backend_status.active_backend`
- `backend_status.runtime_engine`

For full field meanings, use [docs/ENGINE_API.md](./ENGINE_API.md).

### 5. Explicitly load or switch models with `/engine/load`

Use this when you want to change models directly after startup:

```powershell
$loadBody = '{"model_id":"qwen3-4b"}'

curl.exe -s `
  -X POST `
  -H "Authorization: Bearer $token" `
  -H "Content-Type: application/json" `
  --data "$loadBody" `
  http://127.0.0.1:19432/engine/load
```

Then re-check status:

```powershell
curl.exe -s `
  -H "Authorization: Bearer $token" `
  http://127.0.0.1:19432/engine/status
```

### 6. Generate text with the OpenAI-compatible surface

```powershell
$chatBody = '{"model":"qwen2.5-1.5b-instruct","stream":false,"messages":[{"role":"user","content":"Say hello from the standalone engine."}],"max_tokens":64,"temperature":0.7}'

curl.exe -s `
  -X POST `
  -H "Authorization: Bearer $token" `
  -H "Content-Type: application/json" `
  --data "$chatBody" `
  http://127.0.0.1:19432/v1/chat/completions
```

For streaming requests and the full payload contract, use [docs/ENGINE_API.md](./ENGINE_API.md).

## Troubleshooting

### Missing token or token mismatch

Symptoms:

- the host fails immediately because `SMOLPC_ENGINE_TOKEN` is missing
- API requests return auth failures

Check:

- `%LOCALAPPDATA%\SmolPC\engine-runtime\engine-token.txt` exists
- the token in that file matches the token you send in `Authorization: Bearer ...`
- the same token is exported in `SMOLPC_ENGINE_TOKEN` before startup

### Port already in use

If `19432` is already occupied, either shut down the other engine process or start this one on a different port and update your request URLs to match.

### Host fails health within timeout

If `/engine/health` never comes up after startup, check:

- `%LOCALAPPDATA%\SmolPC\engine-runtime\engine-spawn.log`
- the process listed in `%LOCALAPPDATA%\SmolPC\engine-runtime\engine.pid`
- whether the host binary actually started successfully

### Missing models or runtime bundles

If startup or load fails, confirm:

- models exist under `%LOCALAPPDATA%\SmolPC\models\`
- `SMOLPC_MODELS_DIR` points at the same directory
- runtime assets were staged with the current repo setup scripts
- `--resource-dir` points at a root that contains `libs\`

### Need the app-integration version instead

If your real goal is embedding the engine into another app rather than calling it manually:

- use [docs/APP_ONBOARDING_PLAYBOOK.md](./APP_ONBOARDING_PLAYBOOK.md) for integration guidance
- use `smolpc-engine-client` for Rust/Tauri consumers
