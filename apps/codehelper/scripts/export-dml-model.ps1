param(
    [string]$ModelId = "qwen2.5-1.5b-instruct",
    [string]$HuggingFaceModel = "Qwen/Qwen2.5-1.5B-Instruct",
    [ValidateSet("int4", "fp16", "fp32", "bf16")]
    [string]$Precision = "int4"
)

$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot "common-dml-builder.ps1")

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$outputDir = Join-Path $repoRoot "src-tauri/models/$ModelId/dml"

Write-Host "Exporting DirectML artifact"
Write-Host "  Model ID:  $ModelId"
Write-Host "  HF model:  $HuggingFaceModel"
Write-Host "  Precision: $Precision"
Write-Host "  Output:    $outputDir"

$dmlLogPath = Get-DmlExportLogPath -ModelId $ModelId -SourceLabel "manual-export"
$builderEnvironment = Invoke-DmlModelBuilder `
    -ModelName $HuggingFaceModel `
    -OutputDir $outputDir `
    -Precision $Precision `
    -ExecutionProvider "dml" `
    -LogPath $dmlLogPath

$dmlValidation = Assert-DmlArtifactReady -ArtifactDir $outputDir

Write-Host ""
Write-Host "DirectML artifact export complete."
Write-Host "Expected model file: $outputDir/model.onnx"
Write-Host "DirectML builder env: $($builderEnvironment.Root)"
Write-Host "DirectML export log: $dmlLogPath"
if ((Get-DmlStringArray -Value $dmlValidation.external_refs).Count -gt 0) {
    Write-Host "DirectML external data: $($dmlValidation.external_refs -join ', ')"
}
