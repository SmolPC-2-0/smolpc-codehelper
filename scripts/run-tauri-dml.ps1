param(
    [int]$DeviceId = -1
)

$env:SMOLPC_FORCE_EP = "dml"

if ($DeviceId -ge 0) {
    $env:SMOLPC_DML_DEVICE_ID = "$DeviceId"
    Write-Host "Starting Tauri with forced DirectML (SMOLPC_DML_DEVICE_ID=$DeviceId)..."
} else {
    if (Test-Path Env:SMOLPC_DML_DEVICE_ID) {
        Remove-Item Env:SMOLPC_DML_DEVICE_ID
    }
    Write-Host "Starting Tauri with forced DirectML (auto device selection)..."
}

npx tauri dev
