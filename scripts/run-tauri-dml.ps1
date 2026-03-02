param(
    [int]$DeviceId = -1
)

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$launcher = Join-Path $scriptDir "run-tauri-dev.ps1"
& $launcher -ForceEp dml -DeviceId $DeviceId
exit $LASTEXITCODE
