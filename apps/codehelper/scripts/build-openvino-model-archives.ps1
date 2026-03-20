param(
    [string]$ModelsRoot = "",
    [string]$OutputDir = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ModelsToArchive = @(
    @{
        Id = "qwen2.5-1.5b-instruct"
        SetupHint = "npm run model:setup:qwen25-instruct"
    },
    @{
        Id = "qwen3-4b"
        SetupHint = "npm run model:setup:qwen3-4b"
    }
)

$RequiredOpenVinoFiles = @(
    "manifest.json"
)

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
        return Join-Path (Resolve-WorkspaceRoot) "dist\model-archives"
    }

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }

    return Join-Path (Resolve-WorkspaceRoot) $Path
}

function Assert-OpenVinoArtifactReady {
    param(
        [string]$ModelRoot,
        [string]$ModelId,
        [string]$SetupHint
    )

    $ovinoRoot = Join-Path $ModelRoot "openvino"
    if (-not (Test-Path $ovinoRoot -PathType Container)) {
        throw "Missing OpenVINO directory for '$ModelId' at '$ovinoRoot'. Prepare it with '$SetupHint'."
    }

    foreach ($file in $RequiredOpenVinoFiles) {
        $candidate = Join-Path $ovinoRoot $file
        if (-not (Test-Path $candidate -PathType Leaf)) {
            throw "OpenVINO artifact for '$ModelId' is missing required file: $file"
        }
    }

    $manifestPath = Join-Path $ovinoRoot "manifest.json"
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json

    if ($null -eq $manifest.required_files -or $manifest.required_files.Count -eq 0) {
        throw "OpenVINO manifest.json for '$ModelId' has no required_files list."
    }

    $missingArtifacts = @()
    foreach ($requiredFile in $manifest.required_files) {
        $artifactPath = Join-Path $ovinoRoot $requiredFile
        if (-not (Test-Path $artifactPath -PathType Leaf)) {
            $missingArtifacts += $requiredFile
        }
    }

    if ($missingArtifacts.Count -gt 0) {
        throw "OpenVINO artifact for '$ModelId' is incomplete. Missing: $($missingArtifacts -join ', ')"
    }

    $templatePath = Join-Path $ovinoRoot "chat_template.jinja"
    if (-not (Test-Path $templatePath -PathType Leaf)) {
        Write-Host "  WARNING: chat_template.jinja not found for '$ModelId' — template may be embedded in tokenizer_config.json"
    }

    if ($ModelId -like "qwen3*") {
        if (Test-Path $templatePath -PathType Leaf) {
            $templateContent = Get-Content -LiteralPath $templatePath -Raw
            if ($templateContent -match "enable_thinking.*true") {
                Write-Host "  WARNING: Qwen3 template may default to thinking mode — verify non-thinking patch is applied"
            }
        }
    }

    Write-Host "  Validated OpenVINO artifact for '$ModelId' ($($manifest.required_files.Count) required files)"
}

function Copy-DirectoryContents {
    param(
        [string]$Source,
        [string]$Destination
    )

    if (-not (Test-Path $Source -PathType Container)) {
        throw "Source directory does not exist: $Source"
    }

    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    Copy-Item -Path (Join-Path $Source "*") -Destination $Destination -Recurse -Force
}

$repoRoot = Resolve-RepoRoot
$workspaceRoot = Resolve-WorkspaceRoot
$modelsRoot = Resolve-ModelsRoot -Path $ModelsRoot
$outputDir = Resolve-OutputDir -Path $OutputDir

if (-not (Test-Path $modelsRoot -PathType Container)) {
    throw "Models root does not exist: $modelsRoot"
}

Write-Host "Preparing OpenVINO model archives"
Write-Host "  Models root:  $modelsRoot"
Write-Host "  Output dir:   $outputDir"

foreach ($model in $ModelsToArchive) {
    $sourceModelRoot = Join-Path $modelsRoot $model.Id
    Assert-OpenVinoArtifactReady -ModelRoot $sourceModelRoot -ModelId $model.Id -SetupHint $model.SetupHint
}

New-Item -ItemType Directory -Force -Path $outputDir | Out-Null

$tempRoot = Join-Path $env:TEMP ("smolpc-openvino-archive-" + [Guid]::NewGuid().ToString("N"))
$manifestModels = @()
$checksums = [System.Collections.Generic.List[string]]::new()

try {
    New-Item -ItemType Directory -Force -Path $tempRoot | Out-Null

    foreach ($model in $ModelsToArchive) {
        Write-Host ""
        Write-Host "Archiving $($model.Id) (OpenVINO)..."

        $stageRoot = Join-Path $tempRoot $model.Id
        $stageOvinoDir = Join-Path $stageRoot "openvino"
        $sourceOvinoDir = Join-Path (Join-Path $modelsRoot $model.Id) "openvino"
        Copy-DirectoryContents -Source $sourceOvinoDir -Destination $stageOvinoDir

        $archiveName = "$($model.Id)-openvino.zip"
        $archivePath = Join-Path $outputDir $archiveName
        if (Test-Path $archivePath) {
            Remove-Item -LiteralPath $archivePath -Force
        }

        & tar -a -cf $archivePath -C $stageRoot "openvino"
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to create archive for '$($model.Id)'."
        }

        $archiveItem = Get-Item -LiteralPath $archivePath
        $sha256 = (Get-FileHash -Path $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
        $checksums.Add("$sha256  $archiveName")

        $manifestModels += [PSCustomObject]@{
            id = $model.Id
            backend = "openvino"
            archive_name = $archiveName
            archive_path = $archiveName
            sha256 = $sha256
            archive_size_bytes = $archiveItem.Length
        }
    }

    $manifestPath = Join-Path $outputDir "openvino-model-archives.json"
    $manifest = [PSCustomObject]@{
        version = 1
        models = $manifestModels
    }
    $manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
    $checksums | Set-Content -LiteralPath (Join-Path $outputDir "OPENVINO-SHA256SUMS.txt") -Encoding ASCII

    Write-Host ""
    Write-Host "OpenVINO model archives ready."
    Write-Host "Output dir: $outputDir"
    Write-Host "Manifest:   $manifestPath"
} finally {
    if (Test-Path $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force
    }
}
