param(
    [string]$ModelId = "qwen2.5-coder-1.5b",
    [string]$RepoId = "onnx-community/Qwen2.5-Coder-1.5B-Instruct",
    [ValidateSet("q4", "bnb4", "int8", "uint8", "fp16", "fp32")]
    [string]$CpuVariant = "q4",
    [string]$ModelsRoot = "",
    [string]$HfToken = "",
    [switch]$NoUserEnv
)

$ErrorActionPreference = "Stop"

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

$modelRoot = Join-Path $ModelsRoot $ModelId
$cpuDir = Join-Path $modelRoot "cpu"

Write-Host "Preparing shared Qwen2.5 model bundle"
Write-Host "  Model ID:        $ModelId"
Write-Host "  Models root:     $ModelsRoot"
Write-Host "  Model directory: $modelRoot"
Write-Host "  Source repo:     $RepoId"
Write-Host "  CPU variant:     $CpuVariant"

New-Item -ItemType Directory -Force -Path $cpuDir | Out-Null

Require-PythonModule -ModuleName "huggingface_hub"
Require-PythonModule -ModuleName "onnx"

$variantCandidates = @{
    "q4"   = @(
        "onnx\model_q4.onnx",
        "onnx\model_q4f16.onnx",
        "onnx\model_quantized.onnx",
        "onnx\decoder_model_merged_q4.onnx",
        "onnx\decoder_model_merged_q4f16.onnx",
        "onnx\decoder_model_merged_quantized.onnx"
    )
    "bnb4" = @(
        "onnx\model_bnb4.onnx",
        "onnx\model_q4.onnx",
        "onnx\decoder_model_merged_q4.onnx"
    )
    "int8" = @(
        "onnx\model_int8.onnx",
        "onnx\model_quantized.onnx",
        "onnx\decoder_model_merged_quantized.onnx"
    )
    "uint8"= @(
        "onnx\model_uint8.onnx",
        "onnx\model_quantized.onnx",
        "onnx\decoder_model_merged_quantized.onnx"
    )
    "fp16" = @(
        "onnx\model_fp16.onnx",
        "onnx\model.onnx",
        "onnx\decoder_model_merged_fp16.onnx",
        "onnx\decoder_model_merged.onnx"
    )
    "fp32" = @(
        "onnx\model.onnx",
        "onnx\decoder_model_merged.onnx"
    )
}

$candidateRelPaths = @()
$candidateRelPaths += $variantCandidates[$CpuVariant]
# Add common quantized fallback sequence for resilience if a repo variant is missing.
# Keep this list quantized-focused to avoid pulling very large fp16/fp32 artifacts unless requested.
$candidateRelPaths += @(
    "onnx\model_q4.onnx",
    "onnx\model_q4f16.onnx",
    "onnx\model_quantized.onnx",
    "onnx\model_bnb4.onnx",
    "onnx\model_int8.onnx",
    "onnx\model_uint8.onnx",
    "onnx\decoder_model_merged_q4.onnx",
    "onnx\decoder_model_merged_q4f16.onnx",
    "onnx\decoder_model_merged_quantized.onnx",
    "model.onnx"
)
$candidateRelPaths = $candidateRelPaths | Select-Object -Unique

# Build Hugging Face allow-patterns dynamically so only relevant artifacts are fetched.
$allowPatterns = @("tokenizer.json")
$allowPatterns += $candidateRelPaths | ForEach-Object { $_ -replace "\\", "/" }
foreach ($candidate in $candidateRelPaths) {
    if ($candidate -match "\.onnx$") {
        $onnxStem = ($candidate -replace "\\", "/") -replace "\.onnx$", ""
        $allowPatterns += "$onnxStem.onnx_data*"
        $allowPatterns += "$onnxStem.onnx.data*"
    }
}
$allowPatterns = $allowPatterns | Select-Object -Unique
$allowPatternsLiteral = ($allowPatterns | ForEach-Object { "        `"$($_)`"" }) -join ",`n"

Write-Host ""
Write-Host "Downloading model artifact snapshot from Hugging Face..."

$downloadDir = Join-Path $modelRoot "_hf_snapshot"
if (Test-Path $downloadDir) {
    Remove-Item -Recurse -Force $downloadDir
}
New-Item -ItemType Directory -Force -Path $downloadDir | Out-Null
$downloadDirPython = $downloadDir -replace "\\", "/"

$snapshotScript = @'
from huggingface_hub import snapshot_download
snapshot = snapshot_download(
    repo_id="__REPO_ID__",
    token=__TOKEN__,
    local_dir=r"__LOCAL_DIR__",
    allow_patterns=[
__ALLOW_PATTERNS__
    ],
)
print(snapshot)
'@

$snapshotScript = $snapshotScript.Replace("__REPO_ID__", $RepoId)
$tokenArg = if ([string]::IsNullOrWhiteSpace($HfToken)) { "None" } else { "'$HfToken'" }
$snapshotScript = $snapshotScript.Replace("__TOKEN__", $tokenArg)
$snapshotScript = $snapshotScript.Replace("__LOCAL_DIR__", $downloadDirPython)
$snapshotScript = $snapshotScript.Replace("__ALLOW_PATTERNS__", $allowPatternsLiteral)
try {
    $snapshotOutput = ($snapshotScript | python -)
    if ($LASTEXITCODE -ne 0) {
        throw "Hugging Face snapshot download failed for repo '$RepoId'."
    }
    $snapshotPath = ($snapshotOutput | Out-String).Trim()
} catch {
    throw "Failed to fetch snapshot from Hugging Face repo '$RepoId'. $_"
}

if ([string]::IsNullOrWhiteSpace($snapshotPath) -or !(Test-Path $snapshotPath)) {
    throw "Failed to resolve Hugging Face snapshot."
}

$modelCandidates = $candidateRelPaths |
    ForEach-Object { Join-Path $snapshotPath $_ }

$quantModelPath = $modelCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if ([string]::IsNullOrWhiteSpace($quantModelPath)) {
    throw "Model ONNX file was not found. Checked: $($modelCandidates -join ', ')"
}

# Clear stale artifacts from previous runs before writing the selected variant.
Get-ChildItem -Path $cpuDir -Filter "*.onnx*" -File -ErrorAction SilentlyContinue | Remove-Item -Force

Copy-Item -Force $quantModelPath (Join-Path $cpuDir "model.onnx")
Write-Host "Selected ONNX artifact: $quantModelPath"

$quantModelDir = Split-Path -Parent $quantModelPath
$quantModelBase = [System.IO.Path]::GetFileNameWithoutExtension($quantModelPath)
$dataFiles = @()
$dataFiles += Get-ChildItem -Path $quantModelDir -Filter "$quantModelBase.onnx_data*" -File -ErrorAction SilentlyContinue
$dataFiles += Get-ChildItem -Path $quantModelDir -Filter "$quantModelBase.onnx.data*" -File -ErrorAction SilentlyContinue

if ($dataFiles) {
    foreach ($file in ($dataFiles | Sort-Object -Property FullName -Unique)) {
        # Preserve original external-data filenames because ONNX location metadata
        # can reference the source basename (for example: decoder_model_merged_q4.onnx_data).
        Copy-Item -Force $file.FullName (Join-Path $cpuDir $file.Name)
        Write-Host "Copied external data: $($file.Name)"

        # Keep a "model*" alias for compatibility with older layouts.
        $aliasName = $file.Name -replace [Regex]::Escape($quantModelBase), "model"
        if ($aliasName -ne $file.Name) {
            $sourceInCpu = Join-Path $cpuDir $file.Name
            $aliasPath = Join-Path $cpuDir $aliasName
            try {
                New-Item -ItemType HardLink -Path $aliasPath -Target $sourceInCpu -Force | Out-Null
                Write-Host "Created external data alias (hard link): $($file.Name) -> $aliasName"
            } catch {
                Copy-Item -Force $file.FullName $aliasPath
                Write-Host "Copied external data alias: $($file.Name) -> $aliasName"
            }
        }
    }
}

Copy-Item -Force (Join-Path $snapshotPath "tokenizer.json") (Join-Path $modelRoot "tokenizer.json")

$modelPath = Join-Path $cpuDir "model.onnx"
$tokenizerPath = Join-Path $modelRoot "tokenizer.json"
if (!(Test-Path $modelPath) -or !(Test-Path $tokenizerPath)) {
    throw "Model setup validation failed: required files are missing."
}

Write-Host ""
Write-Host "Validating artifact layout..."

$validate = @'
import os
import onnx

model_root = r"__MODEL_ROOT__"
model_path = os.path.join(model_root, "cpu", "model.onnx")
tokenizer_path = os.path.join(model_root, "tokenizer.json")

missing = []
for required in (model_path, tokenizer_path):
    if not os.path.exists(required):
        missing.append(required)

def missing_external_data(path):
    model = onnx.load(path, load_external_data=False)
    model_dir = os.path.dirname(path)
    missing_refs = []
    for tensor in model.graph.initializer:
        if tensor.data_location != onnx.TensorProto.EXTERNAL:
            continue
        entries = {entry.key: entry.value for entry in tensor.external_data}
        location = entries.get("location")
        if location:
            target = os.path.join(model_dir, location)
            if not os.path.exists(target):
                missing_refs.append(target)
    return missing_refs

if not missing and os.path.exists(model_path):
    missing.extend(missing_external_data(model_path))

if missing:
    print("MISSING")
    for path in sorted(set(missing)):
        print(path)
    raise SystemExit(1)

print("VALID")
'@

$validate = $validate.Replace("__MODEL_ROOT__", $modelRoot)
$validationResult = ($validate | python -).Trim()
if ($LASTEXITCODE -ne 0) {
    throw "Model validation failed. See missing file list above."
}

$env:SMOLPC_MODELS_DIR = $ModelsRoot
if (-not $NoUserEnv) {
    [Environment]::SetEnvironmentVariable("SMOLPC_MODELS_DIR", $ModelsRoot, "User")
    Write-Host "Set user environment variable: SMOLPC_MODELS_DIR=$ModelsRoot"
}

Write-Host ""
Write-Host "Qwen2.5 shared model setup complete."
Write-Host "Validation result: $validationResult"
Write-Host "Runtime model path: $modelRoot"

if (Test-Path $downloadDir) {
    Remove-Item -Recurse -Force $downloadDir
}
