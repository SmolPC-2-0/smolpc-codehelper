param(
    [string]$Target = "x86_64-pc-windows-msvc",
    [switch]$Debug
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Resolve-RepoRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Resolve-WorkspaceRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path
}

$repoRoot = Resolve-RepoRoot
$workspaceRoot = Resolve-WorkspaceRoot
$binariesDir = Join-Path $repoRoot "src-tauri\binaries"

$profile = if ($Debug) { "debug" } else { "release" }

Write-Host "Building smolpc-engine-host ($profile) for $Target..."
Set-Location $workspaceRoot
if ($Debug) {
    cargo build -p smolpc-engine-host --target $Target
} else {
    cargo build -p smolpc-engine-host --release --target $Target
}
if ($LASTEXITCODE -ne 0) {
    throw "smolpc-engine-host build failed with exit code $LASTEXITCODE."
}

$sidecarSource = Join-Path $workspaceRoot "target\$Target\$profile\smolpc-engine-host.exe"
if (-not (Test-Path $sidecarSource -PathType Leaf)) {
    throw "Expected engine binary missing: $sidecarSource"
}

New-Item -ItemType Directory -Force -Path $binariesDir | Out-Null

$plainName = Join-Path $binariesDir "smolpc-engine-host.exe"
Copy-Item -LiteralPath $sidecarSource -Destination $plainName -Force

Write-Host ""
Write-Host "Engine sidecar staged successfully."
Write-Host "  Source:  $sidecarSource"
Write-Host "  Staged:  $plainName"
