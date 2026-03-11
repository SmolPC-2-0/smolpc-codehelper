param(
    [string]$ModelId = "qwen3-4b-instruct-2507",
    [string]$CpuRepoId = "onnx-community/Qwen3-4B-Instruct-2507-ONNX",
    [string]$DmlSourceModel = "Qwen/Qwen3-4B-Instruct-2507",
    [ValidateSet("int4", "fp16", "fp32", "bf16")]
    [string]$DmlPrecision = "int4",
    [string]$ModelsRoot = "",
    [string]$HfToken = "",
    [switch]$SkipCpuDownload,
    [switch]$SkipDmlExport,
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
$dmlDir = Join-Path $modelRoot "dml"

Write-Host "Preparing shared Qwen3 model bundle"
Write-Host "  Model ID:         $ModelId"
Write-Host "  Models root:      $ModelsRoot"
Write-Host "  Model directory:  $modelRoot"
Write-Host "  CPU source repo:  $CpuRepoId"
Write-Host "  DML source model: $DmlSourceModel"

New-Item -ItemType Directory -Force -Path $cpuDir | Out-Null
New-Item -ItemType Directory -Force -Path $dmlDir | Out-Null

Require-PythonModule -ModuleName "huggingface_hub"
Require-PythonModule -ModuleName "onnx"
if (-not $SkipDmlExport) {
    Require-PythonModule -ModuleName "onnxruntime_genai"
}

if (-not $SkipCpuDownload) {
    Write-Host ""
    Write-Host "Downloading CPU ONNX artifact from Hugging Face..."

    $cpuDownloadDir = Join-Path $modelRoot "_hf_snapshot_cpu"
    if (Test-Path $cpuDownloadDir) {
        Remove-Item -Recurse -Force $cpuDownloadDir
    }
    New-Item -ItemType Directory -Force -Path $cpuDownloadDir | Out-Null
    $cpuDownloadDirPython = $cpuDownloadDir -replace "\\", "/"

    $cpuSnapshot = @'
from huggingface_hub import snapshot_download
snapshot = snapshot_download(
    repo_id="__CPU_REPO__",
    token=__TOKEN__,
    local_dir=r"__LOCAL_DIR__",
    allow_patterns=[
        "onnx/model_q4.onnx",
        "onnx/model_q4.onnx_data",
        "onnx/model_q4.onnx_data_1",
        "tokenizer.json",
    ],
)
print(snapshot)
'@

    $cpuSnapshot = $cpuSnapshot.Replace("__CPU_REPO__", $CpuRepoId)
    $tokenArg = if ([string]::IsNullOrWhiteSpace($HfToken)) { "None" } else { "'$HfToken'" }
    $cpuSnapshot = $cpuSnapshot.Replace("__TOKEN__", $tokenArg)
    $cpuSnapshot = $cpuSnapshot.Replace("__LOCAL_DIR__", $cpuDownloadDirPython)
    $snapshotPath = ($cpuSnapshot | python -).Trim()

    if ([string]::IsNullOrWhiteSpace($snapshotPath) -or !(Test-Path $snapshotPath)) {
        throw "Failed to resolve Hugging Face snapshot for CPU artifact."
    }

    Copy-Item -Force (Join-Path $snapshotPath "onnx\model_q4.onnx") (Join-Path $cpuDir "model.onnx")
    Copy-Item -Force (Join-Path $snapshotPath "onnx\model_q4.onnx_data") (Join-Path $cpuDir "model_q4.onnx_data")
    if (Test-Path (Join-Path $snapshotPath "onnx\model_q4.onnx_data_1")) {
        Copy-Item -Force (Join-Path $snapshotPath "onnx\model_q4.onnx_data_1") (Join-Path $cpuDir "model_q4.onnx_data_1")
    }
    Copy-Item -Force (Join-Path $snapshotPath "tokenizer.json") (Join-Path $modelRoot "tokenizer.json")

    if (Test-Path $cpuDownloadDir) {
        Remove-Item -Recurse -Force $cpuDownloadDir
    }
}

if (-not $SkipDmlExport) {
    Write-Host ""
    Write-Host "Exporting DirectML GenAI artifact..."

    $hfTokenExtra = if ([string]::IsNullOrWhiteSpace($HfToken)) {
        "hf_token=false"
    } else {
        "hf_token=$HfToken"
    }

    python -m onnxruntime_genai.models.builder `
      -m $DmlSourceModel `
      -o $dmlDir `
      -p $DmlPrecision `
      -e dml `
      --extra_options $hfTokenExtra

    if ($LASTEXITCODE -ne 0) {
        throw "DirectML export failed with exit code $LASTEXITCODE"
    }
}

Write-Host ""
Write-Host "Validating artifact layout..."

$validate = @'
import os
import onnx

model_root = r"__MODEL_ROOT__"
cpu_model = os.path.join(model_root, "cpu", "model.onnx")
cpu_tokenizer = os.path.join(model_root, "tokenizer.json")
dml_model = os.path.join(model_root, "dml", "model.onnx")
dml_config = os.path.join(model_root, "dml", "genai_config.json")
dml_tokenizer = os.path.join(model_root, "dml", "tokenizer.json")

required = [cpu_model, cpu_tokenizer, dml_model, dml_config, dml_tokenizer]
missing = [path for path in required if not os.path.exists(path)]

def missing_external_data(model_path):
    model = onnx.load(model_path, load_external_data=False)
    missing_files = set()
    model_dir = os.path.dirname(model_path)
    for tensor in model.graph.initializer:
        if tensor.data_location != onnx.TensorProto.EXTERNAL:
            continue
        entries = {entry.key: entry.value for entry in tensor.external_data}
        location = entries.get("location")
        if location and not os.path.exists(os.path.join(model_dir, location)):
            missing_files.add(os.path.join(model_dir, location))
    return sorted(missing_files)

if not missing:
    missing.extend(missing_external_data(cpu_model))
    missing.extend(missing_external_data(dml_model))

if missing:
    print("MISSING")
    for path in missing:
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
Write-Host "Qwen3 shared model setup complete."
Write-Host "Validation result: $validationResult"
Write-Host "Runtime model path: $modelRoot"
Write-Host "Next step: stage the model with 'npm run bundle:stage:model' before packaging."
