param(
    [string]$Target = "x86_64-pc-windows-msvc",
    [string]$ModelsRoot = "",
    [string]$OutputDir = "",
    [long]$OnnxExternalDataChunkBytes = 1073741824,
    [switch]$SkipEngineCheck,
    [switch]$SkipModels
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Resolve-RepoRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Resolve-WorkspaceRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path
}

function Resolve-OutputDir {
    param([string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return Join-Path (Resolve-WorkspaceRoot) "dist\smolpc-codehelper-offline"
    }

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }

    return Join-Path (Resolve-WorkspaceRoot) $Path
}

function Resolve-ModelsRoot {
    param([string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return Join-Path $env:LOCALAPPDATA "SmolPC\models"
    }

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }

    return Join-Path (Resolve-WorkspaceRoot) $Path
}

$repoRoot = Resolve-RepoRoot
$workspaceRoot = Resolve-WorkspaceRoot
$outputDir = Resolve-OutputDir -Path $OutputDir
$modelsRoot = Resolve-ModelsRoot -Path $ModelsRoot
$bundleScript = Join-Path $repoRoot "scripts\build-windows-local-bundle.ps1"

Write-Host "=========================================="
Write-Host "  SmolPC Code Helper - Offline Packager"
Write-Host "=========================================="
Write-Host ""
Write-Host "  Repo root:    $repoRoot"
Write-Host "  Workspace:    $workspaceRoot"
Write-Host "  Models root:  $modelsRoot"
Write-Host "  Output dir:   $outputDir"
Write-Host "  Target:       $Target"
Write-Host ""

if (-not $SkipEngineCheck) {
    Write-Host "Pre-flight: verifying engine compiles..."
    Set-Location $workspaceRoot
    cargo check -p smolpc-engine-host
    if ($LASTEXITCODE -ne 0) {
        throw "Engine check failed. Fix compilation errors before packaging."
    }
    Write-Host "  Engine check passed."
    Write-Host ""
}

$bundleArgs = @(
    "-Target", $Target,
    "-OutputDir", $outputDir,
    "-OnnxExternalDataChunkBytes", $OnnxExternalDataChunkBytes
)
if (-not [string]::IsNullOrWhiteSpace($ModelsRoot)) {
    $bundleArgs += @("-ModelsRoot", $modelsRoot)
}

Write-Host "Launching full bundle build..."
Write-Host ""
& powershell -NoProfile -ExecutionPolicy Bypass -File $bundleScript @bundleArgs
if ($LASTEXITCODE -ne 0) {
    throw "Bundle build failed with exit code $LASTEXITCODE."
}

Write-Host ""
Write-Host "=========================================="
Write-Host "  Packaging complete!"
Write-Host ""
Write-Host "  Offline bundle: $(Resolve-Path $outputDir)"
Write-Host ""
Write-Host "  To install on a clean machine:"
Write-Host "    1. Copy the folder to USB or network share"
Write-Host "    2. Double-click Install-CodeHelper.cmd"
Write-Host "    3. App auto-detects best backend on first launch"
Write-Host "=========================================="
