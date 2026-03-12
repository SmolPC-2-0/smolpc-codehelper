param(
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

# Release any lock on launcher/src-tauri/libs/DirectML.dll before tauri build.rs runs.
Stop-ProcessIfRunning -Name "smolpc-engine-host"
Stop-ProcessIfRunning -Name "smolpc-launcher"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$launcherRoot = Split-Path -Parent $scriptRoot

Push-Location $launcherRoot
try {
    if ($TauriArgs.Count -gt 0) {
        npx tauri dev @TauriArgs
    } else {
        npx tauri dev
    }

    exit $LASTEXITCODE
} finally {
    Pop-Location
}
