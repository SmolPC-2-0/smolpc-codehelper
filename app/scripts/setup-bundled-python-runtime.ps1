param(
    [string]$PayloadRoot = "",
    [string]$PythonArchivePath = "",
    [string]$UvArchivePath = "",
    [string]$PythonArchiveUrl = "https://github.com/indygreg/python-build-standalone/releases/download/20250317/cpython-3.13.2+20250317-x86_64-pc-windows-msvc-install_only_stripped.tar.gz",
    [string]$PythonArchiveSha256 = "",
    [string]$UvArchiveUrl = "https://releases.astral.sh/github/uv/releases/download/0.10.12/uv-x86_64-pc-windows-msvc.zip",
    [string]$UvArchiveSha256 = "4c1d55501869b3330d4aabf45ad6024ce2367e0f3af83344395702d272c22e88"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw "Bundled Python dev staging is currently supported only on Windows."
}

function Resolve-TargetPath {
    param(
        [string]$Path,
        [string]$RepoRoot
    )

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return Join-Path $RepoRoot "src-tauri\resources\python\payload"
    }

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }

    return Join-Path $RepoRoot $Path
}

function Resolve-ArchivePath {
    param(
        [string]$ArchivePath,
        [string]$DownloadPath
    )

    if (-not [string]::IsNullOrWhiteSpace($ArchivePath)) {
        if (-not (Test-Path $ArchivePath -PathType Leaf)) {
            throw "Archive file not found: $ArchivePath"
        }
        return (Resolve-Path $ArchivePath).Path
    }

    return $DownloadPath
}

function Download-ArchiveIfNeeded {
    param(
        [string]$ArchivePath,
        [string]$ArchiveUrl,
        [string]$Label
    )

    if (Test-Path $ArchivePath -PathType Leaf) {
        Write-Host "Using existing $Label archive: $ArchivePath"
        return
    }

    Write-Host "Downloading $Label archive..."
    Invoke-WebRequest -Uri $ArchiveUrl -OutFile $ArchivePath
}

function Assert-FileHash {
    param(
        [string]$Path,
        [string]$Algorithm,
        [string]$Expected,
        [string]$Label
    )

    $actual = (Get-FileHash -Path $Path -Algorithm $Algorithm).Hash.ToLowerInvariant()
    $expectedNormalized = $Expected.ToLowerInvariant()
    if ($actual -ne $expectedNormalized) {
        throw "$Label archive hash mismatch. Expected $expectedNormalized but got $actual."
    }
}

function Resolve-ExtractedRoot {
    param(
        [string]$ExtractRoot,
        [string]$Sentinel
    )

    $directSentinel = Join-Path $ExtractRoot $Sentinel
    if (Test-Path $directSentinel -PathType Leaf) {
        return $ExtractRoot
    }

    foreach ($candidate in Get-ChildItem -Path $ExtractRoot -Directory) {
        $candidateSentinel = Join-Path $candidate.FullName $Sentinel
        if (Test-Path $candidateSentinel -PathType Leaf) {
            return $candidate.FullName
        }
    }

    throw "Failed to locate extracted archive contents containing $Sentinel under $ExtractRoot"
}

function Copy-DirectoryContents {
    param(
        [string]$SourceRoot,
        [string]$DestinationRoot
    )

    foreach ($entry in Get-ChildItem -LiteralPath $SourceRoot -Force) {
        Copy-Item -LiteralPath $entry.FullName -Destination $DestinationRoot -Recurse -Force
    }
}

function Copy-RequiredFile {
    param(
        [string]$Source,
        [string]$Destination
    )

    if (-not (Test-Path $Source -PathType Leaf)) {
        throw "Missing staged runtime file: $Source"
    }

    Copy-Item -LiteralPath $Source -Destination $Destination -Force
}

function Remove-TreeIfPresent {
    param([string]$Path)

    if (-not (Test-Path $Path)) {
        return
    }

    Remove-Item -LiteralPath $Path -Recurse -Force
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$payloadRoot = Resolve-TargetPath -Path $PayloadRoot -RepoRoot $repoRoot
$tempRoot = Join-Path $env:TEMP ("smolpc-python-runtime-" + [Guid]::NewGuid().ToString("N"))
$pythonDownloadPath = Join-Path $tempRoot "cpython-standalone.tar.gz"
$uvDownloadPath = Join-Path $tempRoot "uv.zip"
$pythonExtractRoot = Join-Path $tempRoot "python-extract"
$uvExtractRoot = Join-Path $tempRoot "uv-extract"
$stagingPayloadRoot = Join-Path $tempRoot "payload"

$pythonArchive = Resolve-ArchivePath -ArchivePath $PythonArchivePath -DownloadPath $pythonDownloadPath
$uvArchive = Resolve-ArchivePath -ArchivePath $UvArchivePath -DownloadPath $uvDownloadPath

$requiredPayloadFiles = @(
    "python.exe",
    "python313.dll",
    "python3.dll",
    "vcruntime140.dll",
    "uv.exe",
    "uvx.exe"
)

# Additional sentinel files that prove we have a full CPython (not embed zip)
$requiredSentinelFiles = @(
    "DLLs\_ssl.pyd",
    "Lib\ssl.py",
    "Lib\site.py"
)

try {
    New-Item -ItemType Directory -Force -Path $tempRoot, $pythonExtractRoot, $uvExtractRoot, $stagingPayloadRoot | Out-Null

    Download-ArchiveIfNeeded -ArchivePath $pythonArchive -ArchiveUrl $PythonArchiveUrl -Label "CPython standalone"
    Download-ArchiveIfNeeded -ArchivePath $uvArchive -ArchiveUrl $UvArchiveUrl -Label "uv"

    if (-not [string]::IsNullOrWhiteSpace($PythonArchiveSha256)) {
        Assert-FileHash -Path $pythonArchive -Algorithm SHA256 -Expected $PythonArchiveSha256 -Label "CPython standalone"
    }
    Assert-FileHash -Path $uvArchive -Algorithm SHA256 -Expected $UvArchiveSha256 -Label "uv"

    Write-Host "Extracting CPython standalone runtime..."
    # python-build-standalone ships as .tar.gz — use system tar to avoid Git Bash path issues
    & "$env:SystemRoot\System32\tar.exe" -xzf $pythonArchive -C $pythonExtractRoot 2>&1 | ForEach-Object { "$_" }
    $pythonRoot = Resolve-ExtractedRoot -ExtractRoot $pythonExtractRoot -Sentinel "python.exe"
    Copy-DirectoryContents -SourceRoot $pythonRoot -DestinationRoot $stagingPayloadRoot

    Write-Host "Extracting uv runtime..."
    Expand-Archive -LiteralPath $uvArchive -DestinationPath $uvExtractRoot -Force
    $uvRoot = Resolve-ExtractedRoot -ExtractRoot $uvExtractRoot -Sentinel "uv.exe"
    foreach ($fileName in @("uv.exe", "uvx.exe")) {
        $source = Join-Path $uvRoot $fileName
        $destination = Join-Path $stagingPayloadRoot $fileName
        Copy-RequiredFile -Source $source -Destination $destination
    }

    $missing = @(
        foreach ($fileName in $requiredPayloadFiles) {
            $candidate = Join-Path $stagingPayloadRoot $fileName
            if (-not (Test-Path $candidate -PathType Leaf)) {
                $fileName
            }
        }
        foreach ($fileName in $requiredSentinelFiles) {
            $candidate = Join-Path $stagingPayloadRoot $fileName
            if (-not (Test-Path $candidate -PathType Leaf)) {
                $fileName
            }
        }
    )

    if ($missing.Count -gt 0) {
        throw "Bundled Python payload staging was incomplete. Missing: $($missing -join ', ')"
    }

    if (Test-Path $payloadRoot) {
        Remove-TreeIfPresent -Path $payloadRoot
    }
    if (-not (Test-Path (Split-Path -Parent $payloadRoot))) {
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $payloadRoot) | Out-Null
    }
    Move-Item -LiteralPath $stagingPayloadRoot -Destination $payloadRoot -Force

    Write-Host ""
    Write-Host "Bundled Python payload staged successfully."
    Write-Host "Payload root: $payloadRoot"
    Write-Host "Python archive: $PythonArchiveUrl"
    Write-Host "uv archive: $UvArchiveUrl"
    Write-Host "Files:"
    foreach ($fileName in $requiredPayloadFiles) {
        Write-Host "  - $fileName"
    }
} finally {
    Remove-TreeIfPresent -Path $tempRoot
}
