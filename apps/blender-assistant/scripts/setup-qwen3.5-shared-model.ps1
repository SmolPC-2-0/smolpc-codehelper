param(
    [string]$ModelId = "qwen3.5-2b",
    [string]$RepoId = "onnx-community/Qwen3.5-2B-ONNX",
    [ValidateSet("q4", "bnb4", "int8", "uint8", "fp16", "fp32")]
    [string]$CpuVariant = "q4",
    [string]$ModelsRoot = "",
    [string]$HfToken = "",
    [switch]$NoUserEnv
)

$ErrorActionPreference = "Stop"

$delegate = Join-Path $PSScriptRoot "setup-qwen2.5-shared-model.ps1"
if (!(Test-Path $delegate)) {
    throw "Missing delegate script: $delegate"
}

$delegateArgs = @{
    ModelId = $ModelId
    RepoId = $RepoId
    CpuVariant = $CpuVariant
    ModelsRoot = $ModelsRoot
    HfToken = $HfToken
}

if ($NoUserEnv) {
    $delegateArgs["NoUserEnv"] = $true
}

& $delegate @delegateArgs
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
