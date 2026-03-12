param(
    [ValidateSet("none", "cpu", "dml")]
    [string]$ForceEp = "none",
    [int]$DeviceId = -1,
    [switch]$ReuseEngine
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
        # Ignore shutdown errors; host may already be offline.
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

function Assert-PortAvailable {
    param(
        [int]$Port,
        [string]$ServiceName
    )

    $listener = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $listener) {
        return
    }

    $owningProcessId = $listener.OwningProcess
    $processName = $null
    if ($owningProcessId -gt 0) {
        $proc = Get-Process -Id $owningProcessId -ErrorAction SilentlyContinue
        if ($null -ne $proc) {
            $processName = $proc.ProcessName
        }
    }

    if ($null -ne $processName) {
        throw "$ServiceName port $Port is already in use by process '$processName' (PID $owningProcessId). Stop that process, then retry."
    }

    Write-Warning "$ServiceName port $Port appears to be in use by PID $owningProcessId, but process details are unavailable. Continuing; if startup fails, free this port and retry."
}

function Ensure-BuildScriptLinks {
    param([string]$TargetDir)

    $buildRoot = Join-Path $TargetDir "debug\build"
    if (!(Test-Path $buildRoot)) {
        return 0
    }

    $fixed = 0
    $buildDirs = Get-ChildItem -LiteralPath $buildRoot -Directory -ErrorAction SilentlyContinue
    foreach ($dir in $buildDirs) {
        $candidateExe = Get-ChildItem -LiteralPath $dir.FullName -File -Filter "build_script_build-*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($null -eq $candidateExe) {
            continue
        }

        $canonicalExe = Join-Path $dir.FullName "build-script-build.exe"
        if (Test-Path $canonicalExe) {
            continue
        }

        try {
            New-Item -ItemType HardLink -Path $canonicalExe -Target $candidateExe.FullName -Force | Out-Null
        } catch {
            Copy-Item -Force $candidateExe.FullName $canonicalExe
        }

        $fixed += 1
    }

    return $fixed
}

function Ensure-RuntimeLibs {
    param(
        [string]$AppRoot,
        [string]$ScriptRoot
    )

    $libsDir = Join-Path $AppRoot "src-tauri\libs"
    $required = @(
        "onnxruntime.dll",
        "onnxruntime_providers_shared.dll",
        "DirectML.dll",
        "onnxruntime-genai.dll"
    )

    $missing = @()
    foreach ($name in $required) {
        if (-not (Test-Path (Join-Path $libsDir $name))) {
            $missing += $name
        }
    }

    $needsSetup = $missing.Count -gt 0
    $ortPath = Join-Path $libsDir "onnxruntime.dll"
    if (-not $needsSetup -and (Test-Path $ortPath)) {
        $rawVersion = (Get-Item $ortPath).VersionInfo.FileVersion
        $match = [regex]::Match($rawVersion, "\d+\.\d+\.\d+(\.\d+)?")
        if ($match.Success) {
            $currentVersion = [version]$match.Value
            if ($currentVersion -lt [version]"1.23.0") {
                Write-Host "Found outdated onnxruntime.dll version ($rawVersion). Need >= 1.23.0."
                $needsSetup = $true
            }
        }
    }

    if ($needsSetup) {
        if ($missing.Count -gt 0) {
            Write-Host "Missing runtime libs: $($missing -join ', ')"
        }
        Write-Host "Running runtime setup to install ONNX Runtime 1.23.x libs..."
        $setupScript = Join-Path $ScriptRoot "setup-libs.ps1"
        & $setupScript
        if ($LASTEXITCODE -ne 0) {
            throw "Runtime setup failed. Run 'npm run runtime:setup' and retry."
        }
    }

    foreach ($name in $required) {
        if (-not (Test-Path (Join-Path $libsDir $name))) {
            throw "Required runtime lib missing after setup: $name (expected under $libsDir)"
        }
    }
}

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$appRoot = Split-Path -Parent $scriptRoot
$appsDir = Split-Path -Parent $appRoot
$repoRoot = Split-Path -Parent $appsDir
$cargoTargetDir = Join-Path $repoRoot "target\blender-assistant"
$hostBinary = Join-Path $cargoTargetDir "debug\smolpc-engine-host.exe"

New-Item -ItemType Directory -Force -Path $cargoTargetDir | Out-Null
$env:CARGO_TARGET_DIR = $cargoTargetDir
Write-Host "Using CARGO_TARGET_DIR=$cargoTargetDir"

Ensure-RuntimeLibs -AppRoot $appRoot -ScriptRoot $scriptRoot
if ($ReuseEngine) {
    Write-Host "Reusing existing engine host if available (no forced restart)."
} else {
    Request-EngineShutdown
}
Assert-PortAvailable -Port 1420 -ServiceName "Vite dev server"
Assert-PortAvailable -Port 5179 -ServiceName "Scene bridge"

Write-Host "Building smolpc-engine-host (debug) for Blender Assistant dev runtime..."
Push-Location $repoRoot
try {
    cargo build -p smolpc-engine-host
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
} finally {
    Pop-Location
}

if (!(Test-Path $hostBinary)) {
    throw "Expected engine host binary not found at: $hostBinary"
}

$env:SMOLPC_ENGINE_HOST_BIN = $hostBinary
if ($ReuseEngine) {
    Clear-EnvVar -Name "SMOLPC_ENGINE_DEV_FORCE_RESPAWN"
} else {
    $env:SMOLPC_ENGINE_DEV_FORCE_RESPAWN = "1"
}

switch ($ForceEp) {
    "cpu" {
        $env:SMOLPC_FORCE_EP = "cpu"
        Clear-EnvVar -Name "SMOLPC_DML_DEVICE_ID"
        Write-Host "Starting Blender Assistant with forced CPU backend..."
    }
    "dml" {
        $env:SMOLPC_FORCE_EP = "dml"
        if ($DeviceId -ge 0) {
            $env:SMOLPC_DML_DEVICE_ID = "$DeviceId"
            Write-Host "Starting Blender Assistant with forced DirectML backend (device=$DeviceId)..."
        } else {
            Clear-EnvVar -Name "SMOLPC_DML_DEVICE_ID"
            Write-Host "Starting Blender Assistant with forced DirectML backend (auto device)..."
        }
    }
    default {
        Clear-EnvVar -Name "SMOLPC_FORCE_EP"
        Clear-EnvVar -Name "SMOLPC_DML_DEVICE_ID"
        Write-Host "Starting Blender Assistant with automatic backend selection..."
    }
}

Push-Location $appRoot
try {
    $maxAttempts = 2
    for ($attempt = 1; $attempt -le $maxAttempts; $attempt++) {
        npx tauri dev
        $exitCode = $LASTEXITCODE
        if ($exitCode -eq 0) {
            exit 0
        }

        if ($attempt -lt $maxAttempts) {
            $fixed = Ensure-BuildScriptLinks -TargetDir $cargoTargetDir
            if ($fixed -gt 0) {
                Write-Host "Applied Cargo build-script link fix to $fixed crate(s); retrying tauri dev..."
                continue
            }
        }

        exit $exitCode
    }
} finally {
    Pop-Location
}
