param(
    [string]$Target = "x86_64-pc-windows-msvc",
    [string]$ModelsRoot = "",
    [string]$OutputDir = "",
    [long]$OnnxExternalDataChunkBytes = 1073741824
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Resolve-RepoRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Resolve-WorkspaceRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path
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

function Find-NsisInstaller {
    param([string[]]$SearchRoots)

    $candidates = foreach ($root in $SearchRoots) {
        if (-not (Test-Path $root -PathType Container)) {
            continue
        }

        Get-ChildItem -Path $root -Recurse -File -Filter "*-setup.exe" |
            Where-Object { $_.FullName -match [regex]::Escape("\bundle\nsis\") }
    }

    return $candidates | Sort-Object LastWriteTime -Descending | Select-Object -First 1
}

function New-InstallModelsPowerShell {
    param([string]$Path)

    $content = @'
param(
    [string]$ManifestPath = "",
    [switch]$Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Resolve-ManifestPath {
    param([string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) "model-archives.json"
    }

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }

    return Join-Path (Get-Location).Path $Path
}

$manifestPath = Resolve-ManifestPath -Path $ManifestPath
$manifestDirectory = Split-Path -Parent $manifestPath
$modelsRoot = Join-Path $env:LOCALAPPDATA "SmolPC\models"

if (-not (Test-Path $manifestPath -PathType Leaf)) {
    throw "Missing model manifest at '$manifestPath'."
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($null -eq $manifest.models -or $manifest.models.Count -eq 0) {
    throw "Model manifest is empty."
}

Write-Host "Installing model archives to $modelsRoot"
New-Item -ItemType Directory -Force -Path $modelsRoot | Out-Null

$tempRoot = Join-Path $env:TEMP ("smolpc-model-install-" + [Guid]::NewGuid().ToString("N"))
try {
    New-Item -ItemType Directory -Force -Path $tempRoot | Out-Null

    foreach ($model in $manifest.models) {
        $modelId = [string]$model.id
        $backend = [string]$model.backend
        $archiveName = [string]$model.archive_name
        $archivePath = [string]$model.archive_path
        $expectedSha256 = [string]$model.sha256

        if ([string]::IsNullOrWhiteSpace($modelId) -or [string]::IsNullOrWhiteSpace($backend)) {
            throw "Manifest entry missing 'id' or 'backend'."
        }

        $targetDir = Join-Path $modelsRoot "$modelId\$backend"
        if (-not $Force -and (Test-Path $targetDir -PathType Container)) {
            $fileCount = (Get-ChildItem -LiteralPath $targetDir -File).Count
            if ($fileCount -gt 0) {
                Write-Host "Skipping $modelId/$backend (already installed, $fileCount files)."
                continue
            }
        }

        Write-Host ""
        Write-Host "Installing $modelId ($backend)..."

        $localArchive = Join-Path $manifestDirectory $archivePath
        if (-not (Test-Path $localArchive -PathType Leaf)) {
            throw "Archive file not found: $localArchive"
        }

        if (-not [string]::IsNullOrWhiteSpace($expectedSha256)) {
            $actualSha256 = (Get-FileHash -Path $localArchive -Algorithm SHA256).Hash.ToLowerInvariant()
            if ($actualSha256 -ne $expectedSha256.ToLowerInvariant()) {
                throw "Checksum mismatch for '$archiveName'. Expected $expectedSha256, got $actualSha256."
            }
        }

        $extractRoot = Join-Path $tempRoot ("extract-$modelId-$backend")
        if (Test-Path $extractRoot) {
            Remove-Item -LiteralPath $extractRoot -Recurse -Force
        }

        Expand-Archive -LiteralPath $localArchive -DestinationPath $extractRoot -Force

        $extractedBackendDir = Join-Path $extractRoot $backend
        if (-not (Test-Path $extractedBackendDir -PathType Container)) {
            throw "Archive '$archiveName' did not contain expected '$backend' directory."
        }

        $modelTargetRoot = Join-Path $modelsRoot $modelId
        New-Item -ItemType Directory -Force -Path $modelTargetRoot | Out-Null
        if (Test-Path $targetDir) {
            Remove-Item -LiteralPath $targetDir -Recurse -Force
        }
        Copy-Item -LiteralPath $extractedBackendDir -Destination $targetDir -Recurse -Force

        Write-Host "  Installed to $targetDir"
    }

    Write-Host ""
    Write-Host "Model installation complete."
} finally {
    if (Test-Path $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force
    }
}
'@

    Set-Content -LiteralPath $Path -Value $content -Encoding UTF8
}

function New-InstallWrapperPowerShell {
    param([string]$Path)

    $content = @'
param(
    [switch]$ForceModels,
    [switch]$NoLaunchApp
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Find-Installer {
    param([string]$Root)

    return Get-ChildItem -LiteralPath $Root -File -Filter "*-setup.exe" |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
}

function Resolve-AppExecutable {
    $candidates = @(
        (Join-Path $env:LOCALAPPDATA "Programs\SmolPC Code Helper\SmolPC Code Helper.exe"),
        (Join-Path $env:LOCALAPPDATA "SmolPC Code Helper\SmolPC Code Helper.exe")
    )

    foreach ($candidate in $candidates) {
        if (Test-Path $candidate -PathType Leaf) {
            return $candidate
        }
    }

    return $null
}

$packageRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$installer = Find-Installer -Root $packageRoot
$modelsDir = Join-Path $packageRoot "models"
$modelInstallScript = Join-Path $modelsDir "Install-Models.ps1"
$manifestPath = Join-Path $modelsDir "model-archives.json"

if ($null -eq $installer) {
    throw "Could not find the packaged installer in '$packageRoot'. Expected a *-setup.exe file."
}

Write-Host "=========================================="
Write-Host "  SmolPC Code Helper Offline Installer"
Write-Host "=========================================="
Write-Host ""

Write-Host "Step 1: Installing application..."
$installerProcess = Start-Process -FilePath $installer.FullName -PassThru -Wait
if ($installerProcess.ExitCode -ne 0) {
    throw "Installer exited with code $($installerProcess.ExitCode)."
}
Write-Host "  Application installed successfully."

if (Test-Path $modelInstallScript -PathType Leaf) {
    Write-Host ""
    Write-Host "Step 2: Installing model files..."
    if ($ForceModels) {
        & powershell -NoProfile -ExecutionPolicy Bypass -File $modelInstallScript -ManifestPath $manifestPath -Force
    } else {
        & powershell -NoProfile -ExecutionPolicy Bypass -File $modelInstallScript -ManifestPath $manifestPath
    }
    Write-Host "  Models installed successfully."
} else {
    Write-Host ""
    Write-Host "Step 2: No model installer found. Models may need manual setup."
}

Write-Host ""
$appExecutable = Resolve-AppExecutable
if (-not $NoLaunchApp -and $appExecutable) {
    Write-Host "Step 3: Launching SmolPC Code Helper..."
    Start-Process -FilePath $appExecutable | Out-Null
} elseif (-not $NoLaunchApp) {
    Write-Host "Install complete. Launch SmolPC Code Helper from the Start menu."
}

Write-Host ""
Write-Host "=========================================="
Write-Host "  Installation complete!"
Write-Host ""
Write-Host "  The engine will auto-detect your best"
Write-Host "  available backend (DirectML > NPU > CPU)"
Write-Host "  on first launch."
Write-Host "=========================================="
'@

    Set-Content -LiteralPath $Path -Value $content -Encoding UTF8
}

function New-InstallWrapperCmd {
    param([string]$Path)

    $content = @'
@echo off
setlocal
echo SmolPC Code Helper - Offline Installer
echo.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0Install-CodeHelper.ps1" %*
if errorlevel 1 (
  echo.
  echo Installation failed. Check the error above.
  pause
  exit /b %errorlevel%
)
echo.
pause
endlocal
'@

    Set-Content -LiteralPath $Path -Value $content -Encoding ASCII
}

$repoRoot = Resolve-RepoRoot
$workspaceRoot = Resolve-WorkspaceRoot
$modelsRoot = Resolve-ModelsRoot -Path $ModelsRoot
$outputDir = Resolve-OutputDir -Path $OutputDir

$dmlArchiveScript = Join-Path $repoRoot "scripts\build-dml-model-archives.ps1"
$ovinoArchiveScript = Join-Path $repoRoot "scripts\build-openvino-model-archives.ps1"
$sidecarScript = Join-Path $repoRoot "scripts\stage-engine-sidecar.ps1"

Write-Host "Preparing all-backend offline bundle"
Write-Host "  Repo root:    $repoRoot"
Write-Host "  Models root:  $modelsRoot"
Write-Host "  Output dir:   $outputDir"
Write-Host "  Target:       $Target"

$tempRoot = Join-Path $env:TEMP ("smolpc-local-bundle-" + [Guid]::NewGuid().ToString("N"))

try {
    New-Item -ItemType Directory -Force -Path $tempRoot | Out-Null
    $archiveOutputDir = Join-Path $tempRoot "model-archives"
    New-Item -ItemType Directory -Force -Path $archiveOutputDir | Out-Null

    Write-Host ""
    Write-Host "=== Phase 1: Building engine sidecar ==="
    & powershell -NoProfile -ExecutionPolicy Bypass -File $sidecarScript -Target $Target
    if ($LASTEXITCODE -ne 0) {
        throw "Engine sidecar build failed."
    }

    Write-Host ""
    Write-Host "=== Phase 2: Archiving DirectML models ==="
    & powershell -NoProfile -ExecutionPolicy Bypass -File $dmlArchiveScript `
        -ModelsRoot $modelsRoot `
        -OutputDir $archiveOutputDir `
        -OnnxExternalDataChunkBytes $OnnxExternalDataChunkBytes
    if ($LASTEXITCODE -ne 0) {
        throw "DirectML model archive build failed."
    }

    Write-Host ""
    Write-Host "=== Phase 3: Archiving OpenVINO models ==="
    & powershell -NoProfile -ExecutionPolicy Bypass -File $ovinoArchiveScript `
        -ModelsRoot $modelsRoot `
        -OutputDir $archiveOutputDir
    if ($LASTEXITCODE -ne 0) {
        throw "OpenVINO model archive build failed."
    }

    Write-Host ""
    Write-Host "=== Phase 4: Building Tauri NSIS installer ==="
    Set-Location $repoRoot
    npm exec --package @tauri-apps/cli -- tauri build --bundles nsis --target $Target
    if ($LASTEXITCODE -ne 0) {
        throw "Tauri build failed."
    }

    $installer = Find-NsisInstaller -SearchRoots @(
        (Join-Path $workspaceRoot "target"),
        (Join-Path $repoRoot "src-tauri\target"),
        (Join-Path $repoRoot "target")
    )
    if ($null -eq $installer) {
        throw "Failed to locate the generated NSIS installer."
    }

    Write-Host ""
    Write-Host "=== Phase 5: Assembling offline bundle ==="
    if (Test-Path $outputDir) {
        Remove-Item -LiteralPath $outputDir -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $outputDir | Out-Null

    Copy-Item -LiteralPath $installer.FullName -Destination (Join-Path $outputDir $installer.Name) -Force

    $outputModelsDir = Join-Path $outputDir "models"
    New-Item -ItemType Directory -Force -Path $outputModelsDir | Out-Null

    $allModels = @()
    $allChecksums = [System.Collections.Generic.List[string]]::new()

    foreach ($manifestFile in @("dml-model-archives.json", "openvino-model-archives.json")) {
        $mPath = Join-Path $archiveOutputDir $manifestFile
        if (Test-Path $mPath -PathType Leaf) {
            $m = Get-Content -LiteralPath $mPath -Raw | ConvertFrom-Json
            $allModels += $m.models
        }
    }

    Get-ChildItem -LiteralPath $archiveOutputDir -File -Filter "*.zip" | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $outputModelsDir $_.Name) -Force
    }

    foreach ($checksumFile in @("DML-SHA256SUMS.txt", "OPENVINO-SHA256SUMS.txt")) {
        $cPath = Join-Path $archiveOutputDir $checksumFile
        if (Test-Path $cPath -PathType Leaf) {
            Get-Content -LiteralPath $cPath | ForEach-Object { $allChecksums.Add($_) }
        }
    }

    $combinedManifest = [PSCustomObject]@{
        version = 1
        models = $allModels
    }
    $combinedManifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $outputModelsDir "model-archives.json") -Encoding UTF8
    $allChecksums | Set-Content -LiteralPath (Join-Path $outputModelsDir "SHA256SUMS.txt") -Encoding ASCII

    New-InstallModelsPowerShell -Path (Join-Path $outputModelsDir "Install-Models.ps1")
    New-InstallWrapperPowerShell -Path (Join-Path $outputDir "Install-CodeHelper.ps1")
    New-InstallWrapperCmd -Path (Join-Path $outputDir "Install-CodeHelper.cmd")

    Write-Host ""
    Write-Host "=========================================="
    Write-Host "  Offline bundle ready!"
    Write-Host "  Path: $(Resolve-Path $outputDir)"
    Write-Host ""
    Write-Host "  Contents:"
    Get-ChildItem -LiteralPath $outputDir -Recurse -File | ForEach-Object {
        $rel = $_.FullName.Substring((Resolve-Path $outputDir).Path.Length + 1)
        $sizeMB = [math]::Round($_.Length / 1MB, 1)
        Write-Host "    $rel ($sizeMB MB)"
    }
    Write-Host "=========================================="
} finally {
    if (Test-Path $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force
    }
}
