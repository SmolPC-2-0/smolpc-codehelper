param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$TauriArgs
)

$ErrorActionPreference = "Stop"

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

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$appRoot = Split-Path -Parent $scriptRoot
$cargoTargetDir = Join-Path $env:LOCALAPPDATA "SmolPC\cargo-target\blender-assistant"

New-Item -ItemType Directory -Force -Path $cargoTargetDir | Out-Null
$env:CARGO_TARGET_DIR = $cargoTargetDir
Write-Host "Using CARGO_TARGET_DIR=$cargoTargetDir"

if ($TauriArgs.Count -gt 0 -and $TauriArgs[0] -eq "dev") {
    $devScript = Join-Path $scriptRoot "run-tauri-dev.ps1"
    & $devScript
    exit $LASTEXITCODE
}

Push-Location $appRoot
try {
    $maxAttempts = 2
    for ($attempt = 1; $attempt -le $maxAttempts; $attempt++) {
        if ($TauriArgs.Count -gt 0) {
            npx tauri @TauriArgs
        } else {
            npx tauri
        }

        $exitCode = $LASTEXITCODE
        if ($exitCode -eq 0) {
            exit 0
        }

        if ($attempt -lt $maxAttempts) {
            $fixed = Ensure-BuildScriptLinks -TargetDir $cargoTargetDir
            if ($fixed -gt 0) {
                Write-Host "Applied Cargo build-script link fix to $fixed crate(s); retrying tauri command..."
                continue
            }
        }

        exit $exitCode
    }
} finally {
    Pop-Location
}
