param(
    [switch]$ReuseCargoProcesses,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$TauriArgs
)

$ErrorActionPreference = "Stop"

function Stop-ProcessIfRunning {
    param([string]$Name)

    $processes = Get-Process -Name $Name -ErrorAction SilentlyContinue
    if ($null -eq $processes) {
        return
    }

    Write-Host "Stopping stale process '$Name'..."
    $processes | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300
}

function Stop-WorkspaceCargoProcesses {
    param(
        [string]$WorkspaceRoot,
        [switch]$AllowReuse
    )

    if ($AllowReuse) {
        Write-Host "Reusing existing Cargo/rustc processes."
        return
    }

    $workspacePattern = ($WorkspaceRoot -replace '\\', '\\\\')
    $candidates = Get-CimInstance Win32_Process -Filter "name = 'cargo.exe' or name = 'rustc.exe'" |
        Where-Object { $_.CommandLine -like "*$workspacePattern*" }

    if ($null -eq $candidates -or $candidates.Count -eq 0) {
        return
    }

    foreach ($proc in $candidates) {
        try {
            Stop-Process -Id $proc.ProcessId -Force -ErrorAction SilentlyContinue
        } catch {
            # Ignore failures; process may have already exited.
        }
    }
    Start-Sleep -Milliseconds 300
}

function Remove-StaleCargoLock {
    param([string]$TargetDir)

    $lockPath = Join-Path $TargetDir "debug\.cargo-lock"
    if (!(Test-Path $lockPath)) {
        return
    }

    try {
        Remove-Item $lockPath -Force -ErrorAction SilentlyContinue
        Write-Host "Removed stale Cargo lock at $lockPath"
    } catch {
        Write-Warning "Failed to remove Cargo lock at ${lockPath}: $($_.Exception.Message)"
    }
}

function Ensure-RustToolchain {
    $rustcOutput = cmd /c "rustc -vV 2>&1"
    if ($LASTEXITCODE -eq 0 -and ($rustcOutput -match "(?m)^host:\s+")) {
        return
    }

    $fallbackToolchain = "stable-x86_64-pc-windows-msvc"
    $env:RUSTUP_TOOLCHAIN = $fallbackToolchain
    Write-Warning "Pinned Rust toolchain is unavailable; falling back to $fallbackToolchain for launcher dev."
}

function Ensure-BuildScriptShims {
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

        $canonicalNoExt = Join-Path $dir.FullName "build-script-build"
        if (!(Test-Path $canonicalNoExt)) {
            try {
                Copy-Item -Force $candidateExe.FullName $canonicalNoExt
                $fixed += 1
            } catch {
                # Ignore; some directories deny this rename target.
            }
        }
    }

    return $fixed
}

function Ensure-RustflagsBuildDirBypass {
    $metadataFlag = "-C metadata=launcherdev_fix"

    if ([string]::IsNullOrWhiteSpace($env:RUSTFLAGS)) {
        $env:RUSTFLAGS = $metadataFlag
    } elseif ($env:RUSTFLAGS -notmatch "metadata=") {
        $env:RUSTFLAGS = "$($env:RUSTFLAGS) $metadataFlag"
    }

    Write-Host "Using RUSTFLAGS=$($env:RUSTFLAGS)"
}

# Release any lock on launcher/src-tauri/libs/DirectML.dll before tauri build.rs runs.
Stop-ProcessIfRunning -Name "smolpc-engine-host"
Stop-ProcessIfRunning -Name "smolpc-launcher"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$launcherRoot = Split-Path -Parent $scriptRoot
$repoRoot = Split-Path -Parent $launcherRoot
$targetDir = Join-Path $repoRoot "target\launcher"

New-Item -ItemType Directory -Force -Path $targetDir | Out-Null
$env:CARGO_TARGET_DIR = $targetDir
Write-Host "Using CARGO_TARGET_DIR=$targetDir"

Stop-WorkspaceCargoProcesses -WorkspaceRoot $repoRoot -AllowReuse:$ReuseCargoProcesses
Remove-StaleCargoLock -TargetDir $targetDir
Ensure-RustToolchain
Ensure-RustflagsBuildDirBypass

Push-Location $launcherRoot
try {
    $maxAttempts = 4
    for ($attempt = 1; $attempt -le $maxAttempts; $attempt++) {
        if ($TauriArgs.Count -gt 0) {
            npx tauri dev @TauriArgs
        } else {
            npx tauri dev
        }

        $exitCode = $LASTEXITCODE
        if ($exitCode -eq 0) {
            exit 0
        }

        if ($attempt -lt $maxAttempts) {
            $fixed = Ensure-BuildScriptShims -TargetDir $targetDir
            if ($fixed -gt 0) {
                Write-Host "Applied build-script shim fix to $fixed crate(s); retrying tauri dev..."
                continue
            }
        }

        exit $exitCode
    }
} finally {
    Pop-Location
}
