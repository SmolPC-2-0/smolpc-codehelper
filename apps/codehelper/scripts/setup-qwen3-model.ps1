param(
    [string]$ModelId = "qwen3-4b-instruct",
    [string]$OpenVinoRepoId = "FluidInference/qwen3-4b-int4-ov-npu",
    [string]$DmlSourceModel = "Qwen/Qwen3-4B-Instruct-2507",
    [ValidateSet("int4", "fp16")]
    [string]$DmlPrecision = "int4",
    [string]$ModelsRoot = "",
    [string]$HfToken = "",
    [switch]$SkipOpenVino,
    [switch]$SkipDml
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

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
Write-Host "  DML source:       $DmlSourceModel"
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
        "generation_config.json",
        "config.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "special_tokens_map.json",
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
# DirectML ONNX GenAI artifact (self-conversion via builder.py)
# ---------------------------------------------------------------------------

if (-not $SkipDml) {
    Write-Host ""
    Write-Host "Checking onnxruntime-genai-directml..."

    $ErrorActionPreference = "Continue"
    python -c "import onnxruntime_genai_directml" 2>$null
    $ErrorActionPreference = "Stop"
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Installing onnxruntime-genai-directml..."
        pip install onnxruntime-genai-directml --quiet
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to install onnxruntime-genai-directml."
        }
    }

    New-Item -ItemType Directory -Force -Path $dmlDir | Out-Null

    Write-Host "Exporting DirectML GenAI artifact from $DmlSourceModel ..."

    $tokenArg = if ([string]::IsNullOrWhiteSpace($resolvedToken)) { "hf_token=false" } else { "hf_token=$resolvedToken" }

    python -m onnxruntime_genai.models.builder `
        -m $DmlSourceModel `
        -o $dmlDir `
        -p $DmlPrecision `
        -e dml `
        --extra_options $tokenArg

    if ($LASTEXITCODE -ne 0) {
        throw "DirectML export failed with exit code $LASTEXITCODE."
    }

    $requiredDml = @("model.onnx", "genai_config.json", "tokenizer.json")
    $missingDml = @(
        foreach ($file in $requiredDml) {
            if (-not (Test-Path (Join-Path $dmlDir $file) -PathType Leaf)) { $file }
        }
    )

    if ($missingDml.Count -gt 0) {
        Write-Warning "DirectML lane incomplete. Missing: $($missingDml -join ', ')"
    } else {
        Write-Host "DirectML lane staged successfully at: $dmlDir"
    }
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
