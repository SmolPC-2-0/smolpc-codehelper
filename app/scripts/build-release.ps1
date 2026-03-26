[CmdletBinding()]
param(
    [ValidateSet('Online', 'Lite', 'Standard', 'Full', 'Portable')]
    [string]$Variant = 'Online'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# ---------------------------------------------------------------------------
# Path resolution
# ---------------------------------------------------------------------------

$ScriptsDir    = $PSScriptRoot
$AppRoot       = (Resolve-Path (Join-Path $ScriptsDir "..")).Path
$WorkspaceRoot = (Resolve-Path (Join-Path $ScriptsDir "..\..\..")).Path

$DistRoot         = Join-Path $WorkspaceRoot "dist"
$ModelArchivesDir = Join-Path $DistRoot "model-archives"
$NsisSearchRoot   = Join-Path $AppRoot "src-tauri\target\release\bundle\nsis"

$MaxInstallerBytes = 1610612736  # 1.5 GB in bytes

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Write-Success {
    param([string]$Message)
    Write-Host $Message -ForegroundColor Green
}

function Find-NsisInstaller {
    if (-not (Test-Path $NsisSearchRoot -PathType Container)) {
        throw "NSIS bundle directory does not exist: $NsisSearchRoot`nRun 'npm run tauri build' first."
    }

    $candidates = Get-ChildItem -LiteralPath $NsisSearchRoot -Filter "*.exe" -File |
        Sort-Object LastWriteTime -Descending

    if ($candidates.Count -eq 0) {
        throw "No .exe installer found in: $NsisSearchRoot"
    }

    return $candidates[0]
}

function Read-ModelManifest {
    param([string]$ManifestPath)

    if (-not (Test-Path $ManifestPath -PathType Leaf)) {
        throw "Model manifest not found: $ManifestPath"
    }

    $raw = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
    return $raw.models
}

function Get-ModelsForVariant {
    param(
        [string]$VariantName,
        [array]$AllModels
    )

    if ($VariantName -eq 'Lite') {
        return @($AllModels | Where-Object { $_.id -eq 'qwen2.5-1.5b-instruct' })
    }
    if ($VariantName -eq 'Standard') {
        return @($AllModels | Where-Object { $_.id -eq 'qwen3-4b' })
    }
    if ($VariantName -eq 'Full') {
        return @($AllModels)
    }

    throw "Unexpected variant: $VariantName"
}

function New-InstallModelsPowerShell {
    param([string]$Path)

    $content = @'
param(
    [string]$ManifestPath = "",
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$scriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { (Get-Location).Path }

if ([string]::IsNullOrWhiteSpace($ManifestPath)) {
    $ManifestPath = Join-Path $scriptDir "model-archives.json"
} elseif (-not [System.IO.Path]::IsPathRooted($ManifestPath)) {
    $ManifestPath = Join-Path (Get-Location).Path $ManifestPath
}

$manifestDirectory = Split-Path -Parent $ManifestPath
$modelsRoot = Join-Path $env:LOCALAPPDATA "SmolPC\models"

if (-not (Test-Path $ManifestPath -PathType Leaf)) {
    throw "Missing model manifest at '$ManifestPath'."
}

$manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
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
            $fileCount = @(Get-ChildItem -LiteralPath $targetDir -File -ErrorAction SilentlyContinue).Count
            if ($fileCount -gt 0) {
                Write-Host "  Skipping $modelId/$backend (already installed, $fileCount files)."
                continue
            }
        }

        Write-Host "  Installing $modelId ($backend)..."

        $localArchive = Join-Path $manifestDirectory $archivePath
        if (-not (Test-Path $localArchive -PathType Leaf)) {
            throw "Archive file not found: $localArchive"
        }

        if (-not [string]::IsNullOrWhiteSpace($expectedSha256)) {
            Write-Host "    Verifying checksum..."
            $actualSha256 = (Get-FileHash -Path $localArchive -Algorithm SHA256).Hash.ToLowerInvariant()
            if ($actualSha256 -ne $expectedSha256.ToLowerInvariant()) {
                throw "Checksum mismatch for '$archiveName'. Expected $expectedSha256, got $actualSha256."
            }
        }

        Write-Host "    Extracting (this may take a minute for large models)..."
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

        Write-Host "    Verifying extracted artifacts..."
        $verifyErrors = @()

        if ($backend -eq "openvino") {
            $modelManifestPath = Join-Path $targetDir "manifest.json"
            if (-not (Test-Path $modelManifestPath -PathType Leaf)) {
                $verifyErrors += "manifest.json is missing from $modelId/$backend"
            } else {
                try {
                    $modelManifest = Get-Content -LiteralPath $modelManifestPath -Raw | ConvertFrom-Json
                } catch {
                    $verifyErrors += "manifest.json for $modelId/$backend is not valid JSON: $_"
                }

                if ($null -ne $modelManifest) {
                    if ($null -ne $modelManifest.entrypoint) {
                        $entryPath = Join-Path $targetDir ([string]$modelManifest.entrypoint)
                        if (-not (Test-Path $entryPath -PathType Leaf)) {
                            $verifyErrors += "entrypoint '$($modelManifest.entrypoint)' is missing"
                        } elseif ((Get-Item -LiteralPath $entryPath).Length -eq 0) {
                            $verifyErrors += "entrypoint '$($modelManifest.entrypoint)' is zero bytes"
                        }
                    }

                    if ($null -ne $modelManifest.required_files -and $modelManifest.required_files.Count -gt 0) {
                        foreach ($reqFile in $modelManifest.required_files) {
                            $reqPath = Join-Path $targetDir ([string]$reqFile)
                            if (-not (Test-Path $reqPath -PathType Leaf)) {
                                $verifyErrors += "required file '$reqFile' is missing"
                            } elseif ((Get-Item -LiteralPath $reqPath).Length -eq 0) {
                                $verifyErrors += "required file '$reqFile' is zero bytes"
                            }
                        }
                    }
                }
            }
        } elseif ($backend -eq "dml") {
            $dmlRequired = @("model.onnx", "genai_config.json", "tokenizer.json")
            foreach ($reqFile in $dmlRequired) {
                $reqPath = Join-Path $targetDir $reqFile
                if (-not (Test-Path $reqPath -PathType Leaf)) {
                    $verifyErrors += "required file '$reqFile' is missing"
                } elseif ((Get-Item -LiteralPath $reqPath).Length -eq 0) {
                    $verifyErrors += "required file '$reqFile' is zero bytes"
                }
            }
        }

        $installedFiles = @(Get-ChildItem -LiteralPath $targetDir -File -ErrorAction SilentlyContinue)
        if ($installedFiles.Count -eq 0) {
            $verifyErrors += "no files found in $targetDir after extraction"
        }

        if ($verifyErrors.Count -gt 0) {
            $errorDetail = $verifyErrors -join "`n      "
            throw "Verification failed for $modelId ($backend):`n      $errorDetail"
        }

        Write-Host "    Verified: $($installedFiles.Count) files present, all non-zero."
    }

    Write-Host ""
    Write-Host "All models installed and verified."
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

$packageRoot = if ($PSScriptRoot) { $PSScriptRoot } else { (Get-Location).Path }

function Find-Installer {
    param([string]$Root)
    return Get-ChildItem -LiteralPath $Root -File -Filter "*-setup.exe" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
}

function Find-InstalledApp {
    $searchDirs = @(
        (Join-Path $env:LOCALAPPDATA "SmolPC Code Helper"),
        (Join-Path $env:LOCALAPPDATA "Programs\SmolPC Code Helper")
    )
    $exeNames = @("smolpc-desktop.exe", "SmolPC 2.0.exe")
    foreach ($dir in $searchDirs) {
        foreach ($name in $exeNames) {
            $exe = Join-Path $dir $name
            if (Test-Path $exe -PathType Leaf) {
                return $exe
            }
        }
    }
    return $null
}

$installer = Find-Installer -Root $packageRoot
$modelsDir = Join-Path $packageRoot "models"
$modelInstallScript = Join-Path $modelsDir "Install-Models.ps1"
$manifestPath = Join-Path $modelsDir "model-archives.json"

if ($null -eq $installer) {
    Write-Host "ERROR: No *-setup.exe found in '$packageRoot'." -ForegroundColor Red
    Write-Host "Make sure you are running this from the offline bundle folder."
    exit 1
}

Write-Host ""
Write-Host "  SmolPC Code Helper - Offline Installer" -ForegroundColor Cyan
Write-Host ""

Write-Host "[1/3] Installing application (silent)..."
$proc = Start-Process -FilePath $installer.FullName -ArgumentList "/S" -PassThru -Wait
if ($proc.ExitCode -ne 0) {
    Write-Host "ERROR: Installer failed with exit code $($proc.ExitCode)." -ForegroundColor Red
    exit 1
}
$appExe = Find-InstalledApp
if ($null -eq $appExe) {
    Write-Host "ERROR: App installed but executable not found. Check install path." -ForegroundColor Red
    exit 1
}
Write-Host "  Installed to: $(Split-Path -Parent $appExe)"

if (Test-Path $modelInstallScript -PathType Leaf) {
    Write-Host ""
    Write-Host "[2/3] Installing AI models (this takes several minutes)..."
    $modelArgs = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $modelInstallScript, "-ManifestPath", $manifestPath)
    if ($ForceModels) { $modelArgs += "-Force" }
    & powershell @modelArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Host "WARNING: Model installation had errors. App may not work until models are installed." -ForegroundColor Yellow
    }
} else {
    Write-Host ""
    Write-Host "[2/3] Skipped - no model installer found."
}

Write-Host ""
if (-not $NoLaunchApp) {
    Write-Host "[3/3] Launching SmolPC Code Helper..."
    Start-Process -FilePath $appExe | Out-Null
} else {
    Write-Host "[3/3] Skipped launch (-NoLaunchApp)."
}

Write-Host ""
Write-Host "  Done! The engine auto-detects your best backend on first launch." -ForegroundColor Green
Write-Host ""
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

function Build-OfflineBundle {
    param([string]$VariantName, [object]$InstallerItem)

    Write-Step "Building model archives for $VariantName"

    Write-Host "  Running build-dml-model-archives.ps1..."
    & "$ScriptsDir\build-dml-model-archives.ps1"
    if ($LASTEXITCODE -ne 0) { throw "build-dml-model-archives.ps1 failed." }

    Write-Host "  Running build-openvino-model-archives.ps1..."
    & "$ScriptsDir\build-openvino-model-archives.ps1"
    if ($LASTEXITCODE -ne 0) { throw "build-openvino-model-archives.ps1 failed." }

    Write-Success "  Model archives built."

    Write-Step "Assembling offline bundle for $VariantName"

    # Read manifests written by the archive scripts
    $dmlModels      = Read-ModelManifest (Join-Path $ModelArchivesDir "dml-model-archives.json")
    $openVinoModels = Read-ModelManifest (Join-Path $ModelArchivesDir "openvino-model-archives.json")
    $allModels      = @($dmlModels) + @($openVinoModels)

    # Filter by variant
    $selectedModels = Get-ModelsForVariant -VariantName $VariantName -AllModels $allModels

    if ($selectedModels.Count -eq 0) {
        throw "No model archives matched variant '$VariantName'. Verify that model archives were built successfully."
    }

    # Assemble bundle directory
    $bundleName   = "SmolPC-$VariantName"
    $bundleRoot   = Join-Path $DistRoot "offline\$bundleName"
    $bundleModels = Join-Path $bundleRoot "models"

    # Clean existing bundle directory so we don't accumulate stale files
    if (Test-Path $bundleRoot) {
        Remove-Item -LiteralPath $bundleRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $bundleModels | Out-Null

    # Copy installer
    Copy-Item -LiteralPath $InstallerItem.FullName -Destination (Join-Path $bundleRoot $InstallerItem.Name) -Force

    # Copy model archives and collect manifest entries
    $manifestModels = [System.Collections.Generic.List[object]]::new()

    foreach ($model in $selectedModels) {
        $archiveSrc = Join-Path $ModelArchivesDir $model.archive_name
        if (-not (Test-Path $archiveSrc -PathType Leaf)) {
            throw "Model archive not found: $archiveSrc"
        }
        $archiveSizeMB = [math]::Round((Get-Item $archiveSrc).Length / 1MB, 1)
        Copy-Item -LiteralPath $archiveSrc -Destination (Join-Path $bundleModels $model.archive_name) -Force
        Write-Host "  Included: $($model.archive_name) ($archiveSizeMB MB)"
        $manifestModels.Add($model)
    }

    # Write combined model-archives.json into the bundle
    $combinedManifest = [PSCustomObject]@{
        version = 1
        models  = $manifestModels
    }
    $combinedManifest | ConvertTo-Json -Depth 6 |
        Set-Content -LiteralPath (Join-Path $bundleModels "model-archives.json") -Encoding UTF8

    # Generate install scripts
    New-InstallModelsPowerShell -Path (Join-Path $bundleModels "Install-Models.ps1")
    New-InstallWrapperPowerShell -Path (Join-Path $bundleRoot "Install-CodeHelper.ps1")
    New-InstallWrapperCmd -Path (Join-Path $bundleRoot "Install-CodeHelper.cmd")

    # ZIP the bundle
    $offlineDir = Join-Path $DistRoot "offline"
    New-Item -ItemType Directory -Force -Path $offlineDir | Out-Null

    $zipPath = Join-Path $offlineDir "$bundleName.zip"
    if (Test-Path $zipPath) {
        Remove-Item -LiteralPath $zipPath -Force
    }

    $tarExe = Join-Path $env:SystemRoot "System32\tar.exe"
    & $tarExe -a -cf $zipPath -C $offlineDir $bundleName
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to create bundle archive: $zipPath"
    }

    # Remove staging directory (the ZIP is the artifact)
    Remove-Item -LiteralPath $bundleRoot -Recurse -Force

    $zipSizeMB = [math]::Round((Get-Item $zipPath).Length / 1MB, 1)
    Write-Success "  Offline bundle: $zipPath ($zipSizeMB MB)"
}

function Build-PortableBundle {
    param([object]$InstallerItem)

    Write-Step "Assembling Portable variant"

    Write-Warning "Portable variant requires manual verification -- layout extraction from Tauri NSIS installs is not fully automated."

    $portableDir  = Join-Path $DistRoot "portable"
    $portableRoot = Join-Path $portableDir "SmolPC-Portable"

    if (Test-Path $portableRoot) {
        Remove-Item -LiteralPath $portableRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $portableRoot | Out-Null

    # Sentinel file — signals is_portable() to use flat layout resolution
    Set-Content -LiteralPath (Join-Path $portableRoot ".portable") -Value "SmolPC portable deployment" -Encoding UTF8
    Write-Host "  Created .portable sentinel."

    # Engine binary
    $engineSrc = Join-Path $AppRoot "src-tauri\binaries\smolpc-engine-host-x86_64-pc-windows-msvc.exe"
    if (Test-Path $engineSrc -PathType Leaf) {
        Copy-Item -LiteralPath $engineSrc -Destination $portableRoot -Force
        Write-Host "  Copied engine binary."
    } else {
        Write-Warning "  Engine binary not found at expected path: $engineSrc"
        Write-Warning "  Run stage-engine-sidecar.ps1 first."
    }

    # Runtime libs -- DirectML and OpenVINO
    $libsDir = Join-Path $AppRoot "src-tauri\libs"
    if (Test-Path $libsDir -PathType Container) {
        $portableLibs = Join-Path $portableRoot "libs"
        New-Item -ItemType Directory -Force -Path $portableLibs | Out-Null
        Copy-Item -Path (Join-Path $libsDir "*") -Destination $portableLibs -Recurse -Force
        Write-Host "  Copied libs."
    } else {
        Write-Warning "  Libs directory not found: $libsDir"
    }

    # Bundled Python runtime
    $pythonSrc = Join-Path $AppRoot "src-tauri\resources\python\payload"
    if (Test-Path $pythonSrc -PathType Container) {
        $portablePython = Join-Path $portableRoot "resources\python\payload"
        New-Item -ItemType Directory -Force -Path $portablePython | Out-Null
        Copy-Item -Path (Join-Path $pythonSrc "*") -Destination $portablePython -Recurse -Force
        Write-Host "  Copied bundled Python runtime."
    } else {
        Write-Warning "  Bundled Python runtime not found: $pythonSrc"
    }

    # Lite model archives (qwen2.5-1.5b-instruct only)
    $dmlManifestPath   = Join-Path $ModelArchivesDir "dml-model-archives.json"
    $ovinoManifestPath = Join-Path $ModelArchivesDir "openvino-model-archives.json"

    $portableModelsDir = Join-Path $portableRoot "models"
    New-Item -ItemType Directory -Force -Path $portableModelsDir | Out-Null

    $portableModels = [System.Collections.Generic.List[object]]::new()

    foreach ($manifestPath in @($dmlManifestPath, $ovinoManifestPath)) {
        if (Test-Path $manifestPath -PathType Leaf) {
            $models     = Read-ModelManifest $manifestPath
            $liteModels = @($models | Where-Object { $_.id -eq 'qwen2.5-1.5b-instruct' })
            foreach ($model in $liteModels) {
                $archiveSrc = Join-Path $ModelArchivesDir $model.archive_name
                if (Test-Path $archiveSrc -PathType Leaf) {
                    Copy-Item -LiteralPath $archiveSrc -Destination $portableModelsDir -Force
                    Write-Host "  Copied model archive: $($model.archive_name)"
                    $portableModels.Add($model)
                } else {
                    Write-Warning "  Model archive not found (skipped): $archiveSrc"
                    Write-Warning "  Run build-dml-model-archives.ps1 / build-openvino-model-archives.ps1 first."
                }
            }
        } else {
            Write-Warning "  Model manifest not found (skipped): $manifestPath"
        }
    }

    if ($portableModels.Count -gt 0) {
        $portableManifest = [PSCustomObject]@{
            version = 1
            models  = $portableModels
        }
        $portableManifest | ConvertTo-Json -Depth 6 |
            Set-Content -LiteralPath (Join-Path $portableModelsDir "model-archives.json") -Encoding UTF8
    }

    # ZIP the portable folder
    New-Item -ItemType Directory -Force -Path $portableDir | Out-Null

    $zipPath = Join-Path $portableDir "SmolPC-Portable.zip"
    if (Test-Path $zipPath) {
        Remove-Item -LiteralPath $zipPath -Force
    }

    $tarExe = Join-Path $env:SystemRoot "System32\tar.exe"
    & $tarExe -a -cf $zipPath -C $portableDir "SmolPC-Portable"
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to create portable archive: $zipPath"
    }

    Remove-Item -LiteralPath $portableRoot -Recurse -Force

    $zipSizeMB = [math]::Round((Get-Item $zipPath).Length / 1MB, 1)
    Write-Success "  Portable bundle: $zipPath ($zipSizeMB MB)"
    Write-Warning "  Portable variant requires manual verification -- confirm app binary is included and the install layout is correct before distributing."
}

# ---------------------------------------------------------------------------
# Step 1: Stage runtimes (idempotent)
# ---------------------------------------------------------------------------

Write-Step "Staging runtimes"

Write-Host "  Running setup-directml-runtime.ps1..."
& "$ScriptsDir\setup-directml-runtime.ps1"
if ($LASTEXITCODE -ne 0) { throw "setup-directml-runtime.ps1 failed." }

Write-Host "  Running setup-openvino-runtime.ps1..."
& "$ScriptsDir\setup-openvino-runtime.ps1"
if ($LASTEXITCODE -ne 0) { throw "setup-openvino-runtime.ps1 failed." }

Write-Host "  Running setup-bundled-python-runtime.ps1..."
& "$ScriptsDir\setup-bundled-python-runtime.ps1"
if ($LASTEXITCODE -ne 0) { throw "setup-bundled-python-runtime.ps1 failed." }

Write-Success "  Runtimes staged."

# ---------------------------------------------------------------------------
# Step 2: Stage engine sidecar
# ---------------------------------------------------------------------------

Write-Step "Staging engine sidecar"
& "$ScriptsDir\stage-engine-sidecar.ps1"
if ($LASTEXITCODE -ne 0) { throw "stage-engine-sidecar.ps1 failed." }
Write-Success "  Engine sidecar staged."

# ---------------------------------------------------------------------------
# Step 3: Build NSIS installer
# ---------------------------------------------------------------------------

Write-Step "Building NSIS installer (npm run tauri build)"

Push-Location $AppRoot
try {
    npm run tauri build
    if ($LASTEXITCODE -ne 0) { throw "'npm run tauri build' failed with exit code $LASTEXITCODE." }
} finally {
    Pop-Location
}

Write-Success "  Tauri build complete."

# ---------------------------------------------------------------------------
# Step 4: Find installer
# ---------------------------------------------------------------------------

Write-Step "Locating NSIS installer"

$installer = Find-NsisInstaller
Write-Host "  Found: $($installer.FullName)"
Write-Host "  Size:  $([math]::Round($installer.Length / 1MB, 1)) MB"

# ---------------------------------------------------------------------------
# Step 5: Size guard (must be < 1.5 GB)
# ---------------------------------------------------------------------------

if ($installer.Length -gt $MaxInstallerBytes) {
    $sizeMB  = [math]::Round($installer.Length / 1MB, 1)
    $limitMB = [math]::Round($MaxInstallerBytes / 1MB, 1)
    Write-Host "  ERROR: Installer is $sizeMB MB -- exceeds the $limitMB MB limit." -ForegroundColor Red
    throw "Installer size guard failed."
}

Write-Success "  Size guard passed."

# ---------------------------------------------------------------------------
# Step 6: Variant-specific packaging
# ---------------------------------------------------------------------------

if ($Variant -eq 'Online') {
    Write-Step "Packaging Online variant"

    $onlineDir = Join-Path $DistRoot "online"
    New-Item -ItemType Directory -Force -Path $onlineDir | Out-Null

    $dest = Join-Path $onlineDir $installer.Name
    Copy-Item -LiteralPath $installer.FullName -Destination $dest -Force
    Write-Success "  Online installer: $dest"
}
elseif ($Variant -eq 'Lite' -or $Variant -eq 'Standard' -or $Variant -eq 'Full') {
    Build-OfflineBundle -VariantName $Variant -InstallerItem $installer
}
elseif ($Variant -eq 'Portable') {
    Build-PortableBundle -InstallerItem $installer
}
else {
    throw "Unknown variant: $Variant"
}

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------

Write-Host ""
Write-Host "==========================================" -ForegroundColor Green
Write-Host "  build-release.ps1 complete!" -ForegroundColor Green
Write-Host "  Variant: $Variant" -ForegroundColor Green
Write-Host "  Dist:    $DistRoot" -ForegroundColor Green
Write-Host "==========================================" -ForegroundColor Green
