param(
    [string]$ModelId = "qwen2.5-1.5b-instruct",
    [string]$HuggingFaceModel = "Qwen/Qwen2.5-1.5B-Instruct",
    [ValidateSet("int4", "fp16", "fp32", "bf16")]
    [string]$Precision = "int4"
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$outputDir = Join-Path $repoRoot "src-tauri/models/$ModelId/dml"

Write-Host "Exporting DirectML artifact"
Write-Host "  Model ID:  $ModelId"
Write-Host "  HF model:  $HuggingFaceModel"
Write-Host "  Precision: $Precision"
Write-Host "  Output:    $outputDir"

New-Item -ItemType Directory -Force -Path $outputDir | Out-Null

python -m onnxruntime_genai.models.builder `
  -m $HuggingFaceModel `
  -o $outputDir `
  -p $Precision `
  -e dml

if ($LASTEXITCODE -ne 0) {
    throw "onnxruntime_genai.models.builder failed with exit code $LASTEXITCODE"
}

Write-Host ""
Write-Host "DirectML artifact export complete."
Write-Host "Expected model file: $outputDir/model.onnx"
