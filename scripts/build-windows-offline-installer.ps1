[CmdletBinding()]
param(
    [string]$ModelId = "qwen2.5-coder-1.5b",
    [switch]$SkipNpmInstall,
    [switch]$SkipCargoCheck,
    [switch]$SkipOrtDownload,
    [switch]$PreflightOnly
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Format-Bytes {
    param([long]$Bytes)
    if ($Bytes -ge 1GB) { return "{0:N2} GB" -f ($Bytes / 1GB) }
    if ($Bytes -ge 1MB) { return "{0:N2} MB" -f ($Bytes / 1MB) }
    if ($Bytes -ge 1KB) { return "{0:N2} KB" -f ($Bytes / 1KB) }
    return "$Bytes B"
}

function Ensure-OnnxRuntimeDll {
    param([string]$DllPath)

    if (Test-Path -LiteralPath $DllPath) {
        return
    }

    $version = "1.22.1"
    $archiveName = "onnxruntime-win-x64-$version.zip"
    $expectedArchiveSha256 = "855276cd4be3cda14fe636c69eb038d75bf5bcd552bda1193a5d79c51f436dfe"
    $url = "https://github.com/microsoft/onnxruntime/releases/download/v$version/$archiveName"

    $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("smolpc-ort-" + [Guid]::NewGuid().ToString("N"))
    $zipPath = Join-Path $tempRoot $archiveName
    $extractedRoot = Join-Path $tempRoot "extracted"
    $sourceDll = Join-Path $extractedRoot "onnxruntime-win-x64-$version\lib\onnxruntime.dll"

    New-Item -ItemType Directory -Path $tempRoot | Out-Null
    New-Item -ItemType Directory -Path $extractedRoot | Out-Null

    try {
        Write-Host "Downloading ONNX Runtime from $url"
        Invoke-WebRequest -Uri $url -OutFile $zipPath

        $actualArchiveSha256 = (Get-FileHash -Path $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actualArchiveSha256 -ne $expectedArchiveSha256) {
            throw "ONNX Runtime archive checksum mismatch. expected=$expectedArchiveSha256 actual=$actualArchiveSha256"
        }
        Write-Host "Verified ONNX Runtime archive SHA256: $actualArchiveSha256"

        Expand-Archive -LiteralPath $zipPath -DestinationPath $extractedRoot -Force

        if (-not (Test-Path -LiteralPath $sourceDll)) {
            throw "onnxruntime.dll not found in downloaded archive: $sourceDll"
        }

        $dllDir = Split-Path -Path $DllPath -Parent
        New-Item -ItemType Directory -Path $dllDir -Force | Out-Null
        Copy-Item -LiteralPath $sourceDll -Destination $DllPath -Force
        Write-Host "Installed onnxruntime.dll at $DllPath"
    }
    finally {
        if (Test-Path -LiteralPath $tempRoot) {
            Remove-Item -LiteralPath $tempRoot -Recurse -Force
        }
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $repoRoot

$modelOnnx = Join-Path $repoRoot "src-tauri\models\$ModelId\model.onnx"
$tokenizer = Join-Path $repoRoot "src-tauri\models\$ModelId\tokenizer.json"
$ortDll = Join-Path $repoRoot "src-tauri\libs\onnxruntime.dll"

if (-not $SkipOrtDownload) {
    Ensure-OnnxRuntimeDll -DllPath $ortDll
}

$requiredFiles = @($modelOnnx, $tokenizer, $ortDll)
foreach ($file in $requiredFiles) {
    if (-not (Test-Path -LiteralPath $file)) {
        throw "Required file missing: $file"
    }
}

$payloadBytes = ($requiredFiles | ForEach-Object { (Get-Item -LiteralPath $_).Length } | Measure-Object -Sum).Sum
$nsisLimitBytes = 2147483648L
$headroomBytes = $nsisLimitBytes - $payloadBytes

Write-Host ""
Write-Host "Offline payload files:"
Write-Host "  model.onnx      $(Format-Bytes ((Get-Item -LiteralPath $modelOnnx).Length))"
Write-Host "  tokenizer.json  $(Format-Bytes ((Get-Item -LiteralPath $tokenizer).Length))"
Write-Host "  onnxruntime.dll $(Format-Bytes ((Get-Item -LiteralPath $ortDll).Length))"
Write-Host "  Total payload   $(Format-Bytes $payloadBytes)"
Write-Host "  NSIS headroom   $(Format-Bytes $headroomBytes)"
Write-Host ""

if ($payloadBytes -ge $nsisLimitBytes) {
    throw "Payload exceeds NSIS 2GB limit before app binaries are added. Reduce payload size."
}

if ($headroomBytes -lt 150MB) {
    Write-Warning "Payload is close to NSIS size limits. Keep additional bundled assets minimal."
}

if ($PreflightOnly) {
    Write-Host "Preflight complete. Skipping build steps because -PreflightOnly was provided."
    exit 0
}

if (-not $SkipNpmInstall) {
    Write-Host "Running npm ci"
    npm ci
    if ($LASTEXITCODE -ne 0) { throw "npm ci failed with exit code $LASTEXITCODE" }
}

if (-not $SkipCargoCheck) {
    Write-Host "Running cargo check"
    cargo check --manifest-path src-tauri/Cargo.toml
    if ($LASTEXITCODE -ne 0) { throw "cargo check failed with exit code $LASTEXITCODE" }
}

Write-Host "Building Windows NSIS installer"
npm run tauri build -- --bundles nsis
if ($LASTEXITCODE -ne 0) { throw "tauri build failed with exit code $LASTEXITCODE" }

$bundleDir = Join-Path $repoRoot "src-tauri\target\release\bundle\nsis"
$installer = Get-ChildItem -Path $bundleDir -Filter "*.exe" -File | Sort-Object LastWriteTime -Descending | Select-Object -First 1

if (-not $installer) {
    throw "Build completed but no NSIS installer EXE found in $bundleDir"
}

Write-Host ""
Write-Host "Installer ready:"
Write-Host "  $($installer.FullName)"
Write-Host "  Size: $(Format-Bytes $installer.Length)"
