param(
    [ValidateSet("none", "cpu", "dml")]
    [string]$ForceEp = "none",
    [int]$DeviceId = -1
)

$ErrorActionPreference = "Stop"

function Clear-EnvVar {
    param([string]$Name)
    if (Test-Path "Env:$Name") {
        Remove-Item "Env:$Name"
    }
}

function Request-EngineShutdown {
    $runtimeRoot = Join-Path $env:LOCALAPPDATA "SmolPC\engine-runtime"
    $tokenPath = Join-Path $runtimeRoot "engine-token.txt"

    if (!(Test-Path $tokenPath)) {
        return
    }

    $token = (Get-Content $tokenPath -Raw).Trim()
    if ([string]::IsNullOrWhiteSpace($token)) {
        return
    }

    try {
        Invoke-RestMethod -Uri "http://127.0.0.1:19432/engine/shutdown" -Method Post -Headers @{ Authorization = "Bearer $token" } | Out-Null
        Start-Sleep -Milliseconds 400
    } catch {
        # Ignore shutdown errors - host may already be offline.
    }

    $deadline = (Get-Date).AddSeconds(4)
    while ($null -ne (Get-Process -Name "smolpc-engine-host" -ErrorAction SilentlyContinue)) {
        if ((Get-Date) -gt $deadline) {
            break
        }
        Start-Sleep -Milliseconds 200
    }

    $remaining = Get-Process -Name "smolpc-engine-host" -ErrorAction SilentlyContinue
    if ($null -ne $remaining) {
        Write-Host "Force-stopping stale smolpc-engine-host process(es) to release binary lock..."
        $remaining | Stop-Process -Force -ErrorAction SilentlyContinue
        Start-Sleep -Milliseconds 300
    }
}

Request-EngineShutdown

Write-Host "Building smolpc-engine-host (debug) for deterministic dev runtime..."
cargo build -p smolpc-engine-host
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$env:SMOLPC_ENGINE_DEV_FORCE_RESPAWN = "1"

switch ($ForceEp) {
    "cpu" {
        $env:SMOLPC_FORCE_EP = "cpu"
        Clear-EnvVar -Name "SMOLPC_DML_DEVICE_ID"
        Write-Host "Starting Tauri dev with forced CPU backend..."
    }
    "dml" {
        $env:SMOLPC_FORCE_EP = "dml"
        if ($DeviceId -ge 0) {
            $env:SMOLPC_DML_DEVICE_ID = "$DeviceId"
            Write-Host "Starting Tauri dev with forced DirectML backend (device=$DeviceId)..."
        } else {
            Clear-EnvVar -Name "SMOLPC_DML_DEVICE_ID"
            Write-Host "Starting Tauri dev with forced DirectML backend (auto device)..."
        }
    }
    default {
        Clear-EnvVar -Name "SMOLPC_FORCE_EP"
        Clear-EnvVar -Name "SMOLPC_DML_DEVICE_ID"
        Write-Host "Starting Tauri dev with automatic backend selection..."
    }
}

npx tauri dev
exit $LASTEXITCODE
