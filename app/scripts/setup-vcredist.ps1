param(
    [switch]$Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$VcRedistUrl = "https://aka.ms/vs/17/release/vc_redist.x64.exe"
$ExpectedSha256 = "cc0ff0eb1dc3f5188ae6300faef32bf5beeba4bdd6e8e445a9184072096b713b"

function Resolve-RepoRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

$repoRoot = Resolve-RepoRoot
$prereqsDir = Join-Path $repoRoot "src-tauri\prereqs"
$targetPath = Join-Path $prereqsDir "vc_redist.x64.exe"

New-Item -ItemType Directory -Force -Path $prereqsDir | Out-Null

if (-not $Force -and (Test-Path $targetPath -PathType Leaf)) {
    $actualSha = (Get-FileHash -Path $targetPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualSha -eq $ExpectedSha256) {
        Write-Host "VC++ Redistributable already staged at $targetPath"
        exit 0
    }
    Write-Host "Existing file has wrong checksum ($actualSha), re-downloading..."
}

$tempPath = Join-Path $env:TEMP ("vc_redist-" + [Guid]::NewGuid().ToString("N") + ".exe")

try {
    Write-Host "Downloading VC++ Redistributable..."
    Invoke-WebRequest -Uri $VcRedistUrl -OutFile $tempPath -UseBasicParsing

    $actualSha = (Get-FileHash -Path $tempPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualSha -ne $ExpectedSha256) {
        throw "Checksum mismatch. Expected $ExpectedSha256 but got $actualSha. Microsoft may have updated the redistributable -- verify the new hash and update this script."
    }

    Move-Item -LiteralPath $tempPath -Destination $targetPath -Force

    $sizeMB = [math]::Round((Get-Item $targetPath).Length / 1MB, 1)
    Write-Host "VC++ Redistributable staged: $targetPath ($sizeMB MB)"
} finally {
    if (Test-Path $tempPath) {
        Remove-Item -LiteralPath $tempPath -Force
    }
}
