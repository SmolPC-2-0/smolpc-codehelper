param(
    [string]$ModelsRoot = "",
    [string]$OutputDir = "",
    [long]$OnnxExternalDataChunkBytes = 1073741824
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

$RequiredDmlFiles = @(
    "model.onnx",
    "genai_config.json",
    "tokenizer.json"
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

function Assert-DmlArtifactReady {
    param(
        [string]$ModelRoot,
        [string]$ModelId,
        [string]$SetupHint
    )

    $dmlRoot = Join-Path $ModelRoot "dml"
    if (-not (Test-Path $dmlRoot -PathType Container)) {
        throw "Missing DirectML directory for '$ModelId' at '$dmlRoot'. Prepare it with '$SetupHint'."
    }

    $missing = @(
        foreach ($file in $RequiredDmlFiles) {
            $candidate = Join-Path $dmlRoot $file
            if (-not (Test-Path $candidate -PathType Leaf)) {
                $file
            }
        }
    )

    if ($missing.Count -gt 0) {
        throw "DirectML artifact for '$ModelId' is incomplete. Missing: $($missing -join ', ')."
    }
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
$reshardScript = Join-Path $repoRoot "scripts\reshard-onnx-external-data.py"

if (-not (Test-Path $modelsRoot -PathType Container)) {
    throw "Models root does not exist: $modelsRoot"
}

Write-Host "Preparing DirectML model archives"
Write-Host "  Models root:  $modelsRoot"
Write-Host "  Output dir:   $outputDir"
Write-Host "  ONNX chunks:  $OnnxExternalDataChunkBytes bytes max"

foreach ($model in $ModelsToArchive) {
    $sourceModelRoot = Join-Path $modelsRoot $model.Id
    Assert-DmlArtifactReady -ModelRoot $sourceModelRoot -ModelId $model.Id -SetupHint $model.SetupHint
}

New-Item -ItemType Directory -Force -Path $outputDir | Out-Null

$tempRoot = Join-Path $env:TEMP ("smolpc-model-archive-" + [Guid]::NewGuid().ToString("N"))
$manifestModels = @()
$checksums = [System.Collections.Generic.List[string]]::new()

try {
    New-Item -ItemType Directory -Force -Path $tempRoot | Out-Null

    foreach ($model in $ModelsToArchive) {
        Write-Host ""
        Write-Host "Archiving $($model.Id) (DirectML)..."

        $stageRoot = Join-Path $tempRoot $model.Id
        $stageDmlDir = Join-Path $stageRoot "dml"
        $sourceDmlDir = Join-Path (Join-Path $modelsRoot $model.Id) "dml"
        Copy-DirectoryContents -Source $sourceDmlDir -Destination $stageDmlDir

        $stagedModelPath = Join-Path $stageDmlDir "model.onnx"
        if (Test-Path $reshardScript -PathType Leaf) {
            python $reshardScript --model $stagedModelPath --max-chunk-bytes $OnnxExternalDataChunkBytes
            if ($LASTEXITCODE -ne 0) {
                throw "Failed to normalize ONNX external data for '$($model.Id)'."
            }
        }

        $archiveName = "$($model.Id)-dml.zip"
        $archivePath = Join-Path $outputDir $archiveName
        if (Test-Path $archivePath) {
            Remove-Item -LiteralPath $archivePath -Force
        }

        $tarExe = Join-Path $env:SystemRoot "System32\tar.exe"
        & $tarExe -a -cf $archivePath -C $stageRoot "dml"
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to create archive for '$($model.Id)'."
        }

        $archiveItem = Get-Item -LiteralPath $archivePath
        $sha256 = (Get-FileHash -Path $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
        $checksums.Add("$sha256  $archiveName")

        $manifestModels += [PSCustomObject]@{
            id = $model.Id
            backend = "dml"
            archive_name = $archiveName
            archive_path = $archiveName
            sha256 = $sha256
            archive_size_bytes = $archiveItem.Length
        }
    }

    $manifestPath = Join-Path $outputDir "dml-model-archives.json"
    $manifest = [PSCustomObject]@{
        version = 1
        models = $manifestModels
    }
    $manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
    $checksums | Set-Content -LiteralPath (Join-Path $outputDir "DML-SHA256SUMS.txt") -Encoding ASCII

    Write-Host ""
    Write-Host "DirectML model archives ready."
    Write-Host "Output dir: $outputDir"
    Write-Host "Manifest:   $manifestPath"
} finally {
    if (Test-Path $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force
    }
}
