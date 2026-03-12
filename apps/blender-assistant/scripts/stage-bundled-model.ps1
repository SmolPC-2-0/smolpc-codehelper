param(
    [string]$ModelId = "qwen2.5-coder-1.5b",
    [string]$SourceRoot = "",
    [string]$DestinationRoot = "src-tauri/resources/models",
    [switch]$CopyFiles,
    [switch]$ForceRestage
)

$ErrorActionPreference = "Stop"

function Resolve-AbsolutePath {
    param(
        [string]$Path,
        [string]$Base
    )

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return $Base
    }

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }

    return (Join-Path $Base $Path)
}

function Assert-RequiredFiles {
    param(
        [string]$RootPath
    )

    $required = @(
        "tokenizer.json",
        "cpu/model.onnx"
    )

    $dmlDir = Join-Path $RootPath "dml"
    if (Test-Path -LiteralPath $dmlDir -PathType Container) {
        $required += @(
            "dml/model.onnx",
            "dml/genai_config.json"
        )
    }

    $missing = @()
    foreach ($relative in $required) {
        $candidate = Join-Path $RootPath $relative
        if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
            $missing += $candidate
        }
    }

    if ($missing.Count -gt 0) {
        throw "Model artifact is incomplete. Missing files:`n$($missing -join "`n")"
    }
}

function Stage-WithHardLinks {
    param(
        [string]$SourcePath,
        [string]$DestinationPath
    )

    New-Item -ItemType Directory -Force -Path $DestinationPath | Out-Null

    $files = Get-ChildItem -LiteralPath $SourcePath -Recurse -File -Force
    foreach ($file in $files) {
        $relativePath = $file.FullName.Substring($SourcePath.Length).TrimStart('\', '/')
        $targetPath = Join-Path $DestinationPath $relativePath
        $targetDir = Split-Path -Parent $targetPath

        if (-not (Test-Path -LiteralPath $targetDir -PathType Container)) {
            New-Item -ItemType Directory -Force -Path $targetDir | Out-Null
        }

        New-Item -ItemType HardLink -Path $targetPath -Target $file.FullName -Force | Out-Null
    }
}

function Stage-WithCopy {
    param(
        [string]$SourcePath,
        [string]$DestinationPath
    )

    New-Item -ItemType Directory -Force -Path $DestinationPath | Out-Null
    $args = @(
        $SourcePath,
        $DestinationPath,
        "/E",
        "/XO",
        "/R:1",
        "/W:1",
        "/NFL",
        "/NDL",
        "/NJH",
        "/NJS",
        "/NP"
    )

    & robocopy @args | Out-Null
    if ($LASTEXITCODE -ge 8) {
        throw "robocopy failed with exit code $LASTEXITCODE"
    }
}

function Write-StageSummary {
    param(
        [string]$ModelId,
        [string]$SourcePath,
        [string]$DestinationPath,
        [string]$Method
    )

    $files = Get-ChildItem -LiteralPath $DestinationPath -Recurse -File
    $fileCount = ($files | Measure-Object).Count
    $totalBytes = ($files | Measure-Object -Property Length -Sum).Sum
    $totalGb = [Math]::Round(($totalBytes / 1GB), 2)

    Write-Host "Bundled model staged successfully."
    Write-Host "  Model ID: $ModelId"
    Write-Host "  Source:   $SourcePath"
    Write-Host "  Target:   $DestinationPath"
    Write-Host "  Method:   $Method"
    Write-Host "  Files:    $fileCount"
    Write-Host "  Size:     $totalGb GB"
}

$repoRoot = Resolve-AbsolutePath -Path ".." -Base $PSScriptRoot

if ([string]::IsNullOrWhiteSpace($SourceRoot)) {
    if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
        throw "LOCALAPPDATA is not set. Provide -SourceRoot explicitly."
    }
    $SourceRoot = Join-Path $env:LOCALAPPDATA "SmolPC\models"
}

$sourceRootAbs = Resolve-AbsolutePath -Path $SourceRoot -Base $repoRoot
$destRootAbs = Resolve-AbsolutePath -Path $DestinationRoot -Base $repoRoot

$sourceModelDir = Join-Path $sourceRootAbs $ModelId
$destModelDir = Join-Path $destRootAbs $ModelId

if (-not (Test-Path -LiteralPath $sourceModelDir -PathType Container)) {
    throw "Model directory not found: $sourceModelDir"
}

Assert-RequiredFiles -RootPath $sourceModelDir

if (Test-Path -LiteralPath $destModelDir) {
    $destinationReady = $false
    try {
        Assert-RequiredFiles -RootPath $destModelDir
        $destinationReady = $true
    } catch {
        $destinationReady = $false
    }

    if ($destinationReady -and -not $ForceRestage) {
        Write-Host "Bundled model already staged and valid. Skipping restage."
        Write-StageSummary `
            -ModelId $ModelId `
            -SourcePath $sourceModelDir `
            -DestinationPath $destModelDir `
            -Method "existing"
        exit 0
    }

    if (-not $ForceRestage) {
        Write-Warning "Bundled model exists but is incomplete. Attempting incremental repair copy."
        Stage-WithCopy -SourcePath $sourceModelDir -DestinationPath $destModelDir
        Assert-RequiredFiles -RootPath $destModelDir
        Write-StageSummary `
            -ModelId $ModelId `
            -SourcePath $sourceModelDir `
            -DestinationPath $destModelDir `
            -Method "copied files (incremental)"
        exit 0
    }

    Remove-Item -LiteralPath $destModelDir -Recurse -Force
}

$stageMethod = "hard links"
if ($CopyFiles) {
    Stage-WithCopy -SourcePath $sourceModelDir -DestinationPath $destModelDir
    $stageMethod = "copied files"
} else {
    try {
        Stage-WithHardLinks -SourcePath $sourceModelDir -DestinationPath $destModelDir
    } catch {
        Write-Warning "Hard-link staging failed ($($_.Exception.Message)); falling back to file copy."
        if (Test-Path -LiteralPath $destModelDir) {
            Remove-Item -LiteralPath $destModelDir -Recurse -Force
        }
        Stage-WithCopy -SourcePath $sourceModelDir -DestinationPath $destModelDir
        $stageMethod = "copied files"
    }
}

Write-StageSummary `
    -ModelId $ModelId `
    -SourcePath $sourceModelDir `
    -DestinationPath $destModelDir `
    -Method $stageMethod
