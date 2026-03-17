param(
    [string]$ModelId = "qwen3-4b-int4-ov",
    [string]$RepoId = "OpenVINO/Qwen3-4B-int4-ov",
    [string]$ModelsRoot = "",
    [string]$HfToken = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Require-PythonModule {
    param([string]$ModuleName)

    python -c "import $ModuleName" 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "Required Python module '$ModuleName' is missing. Install it before running this script."
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
$laneRoot = Join-Path $modelRoot "openvino_npu"
$manifestPath = Join-Path $laneRoot "manifest.json"
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
    "generation_config.json",
    "config.json",
    "tokenizer.json",
    "tokenizer_config.json",
    "special_tokens_map.json",
    "added_tokens.json",
    "merges.txt",
    "vocab.json"
)

Require-PythonModule -ModuleName "huggingface_hub"

New-Item -ItemType Directory -Force -Path $laneRoot | Out-Null

$env:SMOLPC_HF_REPO_ID = $RepoId
$env:SMOLPC_HF_ALLOW_PATTERNS = ($allowPatterns | ConvertTo-Json -Compress)
if ([string]::IsNullOrWhiteSpace($resolvedToken)) {
    $env:SMOLPC_HF_TOKEN = ""
} else {
    $env:SMOLPC_HF_TOKEN = $resolvedToken
}

try {
    Write-Host "Downloading official OpenVINO Qwen3 model artifact..."
    $snapshot = (@'
import json
import os
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
        throw "Failed to resolve the Hugging Face snapshot for '$RepoId'."
    }

    foreach ($file in $allowPatterns) {
        $source = Join-Path $snapshot $file
        if (-not (Test-Path $source -PathType Leaf)) {
            throw "Snapshot for '$RepoId' is missing expected file '$file'."
        }

        Copy-Item -Force $source (Join-Path $laneRoot $file)
    }

    $manifest = [ordered]@{
        entrypoint = "openvino_model.xml"
        required_files = $manifestRequiredFiles
    }
    $manifestJson = $manifest | ConvertTo-Json -Depth 4
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($manifestPath, $manifestJson, $utf8NoBom)

    $manifestData = Get-Content $manifestPath -Raw | ConvertFrom-Json
    $expectedFiles = @($manifestData.entrypoint) + @($manifestData.required_files)
    $missingFiles = @(
        foreach ($file in $expectedFiles) {
            if (-not (Test-Path (Join-Path $laneRoot $file) -PathType Leaf)) {
                $file
            }
        }
    )

    if ($missingFiles.Count -gt 0) {
        throw "OpenVINO model lane staging was incomplete. Missing: $($missingFiles -join ', ')"
    }

    Write-Host ""
    Write-Host "OpenVINO Qwen3 model lane staged successfully."
    Write-Host "Model ID:    $ModelId"
    Write-Host "Lane root:   $laneRoot"
    Write-Host "Manifest:    $manifestPath"
} finally {
    Remove-Item Env:SMOLPC_HF_REPO_ID -ErrorAction SilentlyContinue
    Remove-Item Env:SMOLPC_HF_ALLOW_PATTERNS -ErrorAction SilentlyContinue
    Remove-Item Env:SMOLPC_HF_TOKEN -ErrorAction SilentlyContinue
}
