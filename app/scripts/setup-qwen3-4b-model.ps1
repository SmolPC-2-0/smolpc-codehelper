param(
    [string]$ModelId = "qwen3-4b",
    [string]$OpenVinoRepoId = "OpenVINO/Qwen3-4B-int4-ov",
    [string]$DmlSourceModel = "Qwen/Qwen3-4B",
    [ValidateSet("self_build", "fallback_snapshot")]
    [string]$DmlSourceMode = "self_build",
    [string]$FallbackOnnxRepoId = "onnx-community/Qwen3-4B-ONNX",
    [ValidateSet("int4", "fp16")]
    [string]$DmlPrecision = "int4",
    [string]$ModelsRoot = "",
    [string]$HfToken = "",
    [switch]$SkipOpenVino,
    [switch]$SkipDml
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot "common-dml-builder.ps1")

function Require-PythonModule {
    param([string]$ModuleName)
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    python -c "import $ModuleName" 2>$null
    $ErrorActionPreference = $prev
    if ($LASTEXITCODE -ne 0) {
        throw "Required Python module '$ModuleName' is missing. Install it before running this script."
    }
}

function Invoke-BuilderSnapshotDownload {
    param(
        [Parameter(Mandatory = $true)]
        [string]$PythonPath,
        [Parameter(Mandatory = $true)]
        [string]$RepoId,
        [Parameter(Mandatory = $true)]
        [string[]]$AllowPatterns,
        [Parameter(Mandatory = $true)]
        [string]$LocalDir,
        [string]$Token = ""
    )

    $env:SMOLPC_HF_REPO_ID = $RepoId
    $env:SMOLPC_HF_ALLOW_PATTERNS = ($AllowPatterns | ConvertTo-Json -Compress)
    $env:SMOLPC_HF_TOKEN = if ([string]::IsNullOrWhiteSpace($Token)) { "" } else { $Token }
    $env:SMOLPC_HF_LOCAL_DIR = $LocalDir

    try {
        $snapshot = (@'
import json
import os

from huggingface_hub import snapshot_download

kwargs = {
    "repo_id": os.environ["SMOLPC_HF_REPO_ID"],
    "allow_patterns": json.loads(os.environ["SMOLPC_HF_ALLOW_PATTERNS"]),
    "local_dir": os.environ["SMOLPC_HF_LOCAL_DIR"],
}
token = os.environ.get("SMOLPC_HF_TOKEN", "").strip()
if token:
    kwargs["token"] = token

print(snapshot_download(**kwargs))
'@ | & $PythonPath -).Trim()

        if ([string]::IsNullOrWhiteSpace($snapshot) -or -not (Test-Path $snapshot)) {
            throw "Failed to resolve Hugging Face snapshot for '$RepoId'."
        }

        return $snapshot
    } finally {
        Remove-Item Env:SMOLPC_HF_REPO_ID -ErrorAction SilentlyContinue
        Remove-Item Env:SMOLPC_HF_ALLOW_PATTERNS -ErrorAction SilentlyContinue
        Remove-Item Env:SMOLPC_HF_TOKEN -ErrorAction SilentlyContinue
        Remove-Item Env:SMOLPC_HF_LOCAL_DIR -ErrorAction SilentlyContinue
    }
}

function Invoke-Qwen3FallbackSnapshotStage {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$BuilderEnvironment,
        [Parameter(Mandatory = $true)]
        [string]$RepoId,
        [Parameter(Mandatory = $true)]
        [string]$OutputDir,
        [Parameter(Mandatory = $true)]
        [ValidateSet("int4")]
        [string]$Precision,
        [Parameter(Mandatory = $true)]
        [string]$LogPath,
        [string]$Token = ""
    )

    $snapshotRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("smolpc-qwen3-4b-fallback-" + [Guid]::NewGuid().ToString("N"))
    $allowPatterns = @(
        "added_tokens.json",
        "chat_template.jinja",
        "config.json",
        "generation_config.json",
        "merges.txt",
        "onnx/model_q4f16.onnx",
        "onnx/model_q4f16.onnx_data",
        "onnx/model_q4f16.onnx_data_1",
        "special_tokens_map.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "vocab.json"
    )

    try {
        New-Item -ItemType Directory -Force -Path $snapshotRoot | Out-Null
        Write-Host "Downloading fallback ONNX snapshot from $RepoId ..."
        $snapshot = Invoke-BuilderSnapshotDownload `
            -PythonPath $BuilderEnvironment.PythonPath `
            -RepoId $RepoId `
            -AllowPatterns $allowPatterns `
            -LocalDir $snapshotRoot `
            -Token $Token

        Add-DmlLogLine -LogPath $LogPath -Message ("fallback_repo_id={0}" -f $RepoId)
        Add-DmlLogLine -LogPath $LogPath -Message ("fallback_snapshot_dir={0}" -f $snapshot)

        $null = Invoke-DmlModelBuilder `
            -InputPath $snapshot `
            -OutputDir $OutputDir `
            -Precision $Precision `
            -ExecutionProvider "dml" `
            -ExtraOptions @("config_only=true") `
            -LogPath $LogPath

        $modelSource = Join-Path $snapshot "onnx\model_q4f16.onnx"
        if (-not (Test-Path $modelSource -PathType Leaf)) {
            throw "Fallback ONNX snapshot is missing 'onnx/model_q4f16.onnx'."
        }

        Copy-Item -Force $modelSource (Join-Path $OutputDir "model.onnx")

        $externalDataFiles = @(Get-ChildItem -Path (Join-Path $snapshot "onnx") -Filter "model_q4f16.onnx_data*" -File)
        if ($externalDataFiles.Count -eq 0) {
            throw "Fallback ONNX snapshot did not include model_q4f16 external-data files."
        }

        foreach ($file in $externalDataFiles) {
            Copy-Item -Force $file.FullName (Join-Path $OutputDir $file.Name)
        }

        Add-DmlLogLine -LogPath $LogPath -Message ("fallback_model_source={0}" -f $modelSource)
        Add-DmlLogLine -LogPath $LogPath -Message ("fallback_external_data={0}" -f (($externalDataFiles | ForEach-Object { $_.Name }) -join ","))

        return $BuilderEnvironment
    } finally {
        if (Test-Path $snapshotRoot) {
            Remove-Item -LiteralPath $snapshotRoot -Recurse -Force
        }
    }
}

if ([string]::IsNullOrWhiteSpace($ModelsRoot)) {
    $ModelsRoot = Join-Path $env:LOCALAPPDATA "SmolPC\models"
}

$resolvedToken = if ([string]::IsNullOrWhiteSpace($HfToken)) {
    [Environment]::GetEnvironmentVariable("HF_TOKEN")
} else {
    $HfToken
}

$modelRoot = Join-Path $ModelsRoot $ModelId
$openvinoDir = Join-Path $modelRoot "openvino"
$dmlDir = Join-Path $modelRoot "dml"
$manifestPath = Join-Path $openvinoDir "manifest.json"

Write-Host "Setting up model: $ModelId"
Write-Host "  Models root:      $ModelsRoot"
Write-Host "  Model directory:  $modelRoot"
Write-Host "  OpenVINO repo:    $OpenVinoRepoId"
Write-Host "  DML mode:         $DmlSourceMode"
Write-Host "  DML source:       $DmlSourceModel"
Write-Host "  DML fallback:     $FallbackOnnxRepoId"
Write-Host ""

# ---------------------------------------------------------------------------
# OpenVINO IR artifacts (serves CPU + NPU)
# ---------------------------------------------------------------------------
if (-not $SkipOpenVino) {
    Require-PythonModule -ModuleName "huggingface_hub"

    New-Item -ItemType Directory -Force -Path $openvinoDir | Out-Null

    $allowPatterns = @(
        "added_tokens.json",
        "chat_template.jinja",
        "config.json",
        "generation_config.json",
        "merges.txt",
        "openvino_config.json",
        "openvino_detokenizer.bin",
        "openvino_detokenizer.xml",
        "openvino_model.bin",
        "openvino_model.xml",
        "openvino_tokenizer.bin",
        "openvino_tokenizer.xml",
        "special_tokens_map.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "vocab.json"
    )
    $manifestRequiredFiles = @(
        "openvino_model.bin",
        "openvino_tokenizer.xml",
        "openvino_tokenizer.bin",
        "openvino_detokenizer.xml",
        "openvino_detokenizer.bin",
        "openvino_config.json",
        "generation_config.json",
        "config.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "special_tokens_map.json",
        "chat_template.jinja",
        "added_tokens.json",
        "merges.txt",
        "vocab.json"
    )

    $env:SMOLPC_HF_REPO_ID = $OpenVinoRepoId
    $env:SMOLPC_HF_ALLOW_PATTERNS = ($allowPatterns | ConvertTo-Json -Compress)
    $env:SMOLPC_HF_TOKEN = if ([string]::IsNullOrWhiteSpace($resolvedToken)) { "" } else { $resolvedToken }

    try {
        Write-Host "Downloading OpenVINO IR artifacts from $OpenVinoRepoId ..."

        $snapshot = (@'
import json, os
from huggingface_hub import snapshot_download

kwargs = {
    "repo_id": os.environ["SMOLPC_HF_REPO_ID"],
    "allow_patterns": json.loads(os.environ["SMOLPC_HF_ALLOW_PATTERNS"]),
}
token = os.environ.get("SMOLPC_HF_TOKEN", "").strip()
if token:
    kwargs["token"] = token

print(snapshot_download(**kwargs))
'@ | python -).Trim()

        if ([string]::IsNullOrWhiteSpace($snapshot) -or -not (Test-Path $snapshot)) {
            throw "Failed to resolve Hugging Face snapshot for '$OpenVinoRepoId'."
        }

        foreach ($file in $allowPatterns) {
            $source = Join-Path $snapshot $file
            if (Test-Path $source -PathType Leaf) {
                Copy-Item -Force $source (Join-Path $openvinoDir $file)
            }
        }

        $manifest = [ordered]@{
            entrypoint     = "openvino_model.xml"
            required_files = $manifestRequiredFiles
        }
        $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
        [System.IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 4), $utf8NoBom)

        $manifestData = Get-Content $manifestPath -Raw | ConvertFrom-Json
        $expectedFiles = @($manifestData.entrypoint) + @($manifestData.required_files)
        $missingFiles = @(
            foreach ($file in $expectedFiles) {
                if (-not (Test-Path (Join-Path $openvinoDir $file) -PathType Leaf)) { $file }
            }
        )

        if ($missingFiles.Count -gt 0) {
            Write-Warning "OpenVINO lane incomplete. Missing: $($missingFiles -join ', ')"
        } else {
            Write-Host "OpenVINO lane staged successfully at: $openvinoDir"
        }
    } finally {
        Remove-Item Env:SMOLPC_HF_REPO_ID -ErrorAction SilentlyContinue
        Remove-Item Env:SMOLPC_HF_ALLOW_PATTERNS -ErrorAction SilentlyContinue
        Remove-Item Env:SMOLPC_HF_TOKEN -ErrorAction SilentlyContinue
    }
}

# ---------------------------------------------------------------------------
# DirectML ONNX GenAI artifact (self-build or maintained snapshot fallback)
# ---------------------------------------------------------------------------

if (-not $SkipDml) {
    Write-Host ""
    $dmlLogPath = Get-DmlExportLogPath -ModelId $ModelId -SourceLabel $DmlSourceMode
    $builderEnvironment = $null
    $dmlSourceSummary = ""

    switch ($DmlSourceMode) {
        "self_build" {
            Write-Host "Exporting DirectML GenAI artifact from $DmlSourceModel ..."

            $tokenArg = if ([string]::IsNullOrWhiteSpace($resolvedToken)) { "hf_token=false" } else { "hf_token=$resolvedToken" }
            $builderEnvironment = Invoke-DmlModelBuilder `
                -ModelName $DmlSourceModel `
                -OutputDir $dmlDir `
                -Precision $DmlPrecision `
                -ExecutionProvider "dml" `
                -ExtraOptions @($tokenArg) `
                -LogPath $dmlLogPath
            $dmlSourceSummary = $DmlSourceModel
        }
        "fallback_snapshot" {
            if ($DmlPrecision -ne "int4") {
                throw "fallback_snapshot currently supports only -DmlPrecision int4 because the maintained snapshot is q4f16."
            }

            Write-DmlLogHeader -LogPath $dmlLogPath -Metadata @{
                dml_source_mode     = $DmlSourceMode
                fallback_repo_id    = $FallbackOnnxRepoId
                output_dir          = $dmlDir
                precision           = $DmlPrecision
            }

            $builderEnvironment = Ensure-DmlBuilderEnvironment
            $builderEnvironment = Invoke-Qwen3FallbackSnapshotStage `
                -BuilderEnvironment $builderEnvironment `
                -RepoId $FallbackOnnxRepoId `
                -OutputDir $dmlDir `
                -Precision $DmlPrecision `
                -LogPath $dmlLogPath `
                -Token $resolvedToken
            $dmlSourceSummary = $FallbackOnnxRepoId
        }
    }

    $dmlValidation = Assert-DmlArtifactReady -ArtifactDir $dmlDir
    Write-Host "DirectML source mode: $DmlSourceMode"
    Write-Host "DirectML source:      $dmlSourceSummary"
    Write-Host "DirectML builder env: $($builderEnvironment.Root)"
    Write-Host "DirectML export log:  $dmlLogPath"
    if ((Get-DmlStringArray -Value $dmlValidation.external_refs).Count -gt 0) {
        Write-Host "DirectML external data: $($dmlValidation.external_refs -join ', ')"
    }
    Write-Host "DirectML lane staged successfully at: $dmlDir"
}

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

Write-Host ""
Write-Host "Setup complete for $ModelId"
Write-Host "  OpenVINO (CPU+NPU): $openvinoDir"
Write-Host "  DirectML (GPU):     $dmlDir"
Write-Host ""
Write-Host "Set SMOLPC_MODELS_DIR=$ModelsRoot to use with the engine."
