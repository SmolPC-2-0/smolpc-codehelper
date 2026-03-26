param(
    [string]$BundleRoot = "",
    [switch]$Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$OrtPackageVersion = "1.23.0"
$DirectMlPackageVersion = "1.15.4"
$GenAiDirectMlPackageVersion = "0.12.1"

$ShaDmlOrtNupkg = "a33ec2382b3c440bab74042a135733bb6e5085f293b908d3997688a58fe307e7"
$ShaDirectMlNupkg = "4e7cb7ddce8cf837a7a75dc029209b520ca0101470fcdf275c1f49736a3615b9"
$ShaGenAiDirectMlNupkg = "dcc2adff3a0e7e3adb4e4d4cccce71d21a2acc86a78b20dadd60255cc7043b77"

function Resolve-RepoRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Resolve-TargetPath {
    param(
        [string]$Path,
        [string]$RepoRoot
    )

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return Join-Path $RepoRoot "src-tauri\libs"
    }

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }

    return Join-Path $RepoRoot $Path
}

function Get-RequiredFiles {
    return @(
        "onnxruntime.dll",
        "onnxruntime_providers_shared.dll",
        "DirectML.dll",
        "onnxruntime-genai.dll"
    )
}

function Test-AllRequiredFilesPresent {
    param([string]$Root)

    foreach ($file in Get-RequiredFiles) {
        if (-not (Test-Path (Join-Path $Root $file) -PathType Leaf)) {
            return $false
        }
    }

    return $true
}

function Download-WithChecksum {
    param(
        [string]$Url,
        [string]$OutFile,
        [string]$ExpectedSha256
    )

    Write-Host "Downloading $Url"
    Invoke-WebRequest -Uri $Url -OutFile $OutFile

    $actualSha256 = (Get-FileHash -Path $OutFile -Algorithm SHA256).Hash.ToLowerInvariant()
    $expected = $ExpectedSha256.ToLowerInvariant()
    if ($actualSha256 -ne $expected) {
        throw "Checksum mismatch for ${OutFile}. Expected $expected but got $actualSha256."
    }
}

function Copy-RequiredFile {
    param(
        [string]$Source,
        [string]$Destination
    )

    if (-not (Test-Path $Source -PathType Leaf)) {
        throw "Required runtime file missing: $Source"
    }

    Copy-Item -LiteralPath $Source -Destination $Destination -Force
}

$repoRoot = Resolve-RepoRoot
$bundleRoot = Resolve-TargetPath -Path $BundleRoot -RepoRoot $repoRoot

New-Item -ItemType Directory -Force -Path $bundleRoot | Out-Null

if (-not $Force -and (Test-AllRequiredFilesPresent -Root $bundleRoot)) {
    Write-Host "DirectML runtime bundle already staged at $bundleRoot"
    exit 0
}

$tempRoot = Join-Path $env:TEMP ("smolpc-directml-runtime-" + [Guid]::NewGuid().ToString("N"))

try {
    New-Item -ItemType Directory -Force -Path $tempRoot | Out-Null

    $ortUrl = "https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime.directml/$OrtPackageVersion/microsoft.ml.onnxruntime.directml.$OrtPackageVersion.nupkg"
    $directMlUrl = "https://api.nuget.org/v3-flatcontainer/microsoft.ai.directml/$DirectMlPackageVersion/microsoft.ai.directml.$DirectMlPackageVersion.nupkg"
    $genAiUrl = "https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntimegenai.directml/$GenAiDirectMlPackageVersion/microsoft.ml.onnxruntimegenai.directml.$GenAiDirectMlPackageVersion.nupkg"

    $ortPackage = Join-Path $tempRoot "ort.zip"
    $directMlPackage = Join-Path $tempRoot "directml.zip"
    $genAiPackage = Join-Path $tempRoot "genai.zip"

    Download-WithChecksum -Url $ortUrl -OutFile $ortPackage -ExpectedSha256 $ShaDmlOrtNupkg
    Download-WithChecksum -Url $directMlUrl -OutFile $directMlPackage -ExpectedSha256 $ShaDirectMlNupkg
    Download-WithChecksum -Url $genAiUrl -OutFile $genAiPackage -ExpectedSha256 $ShaGenAiDirectMlNupkg

    $ortExtract = Join-Path $tempRoot "ort"
    $directMlExtract = Join-Path $tempRoot "directml"
    $genAiExtract = Join-Path $tempRoot "genai"

    Expand-Archive -LiteralPath $ortPackage -DestinationPath $ortExtract -Force
    Expand-Archive -LiteralPath $directMlPackage -DestinationPath $directMlExtract -Force
    Expand-Archive -LiteralPath $genAiPackage -DestinationPath $genAiExtract -Force

    Copy-RequiredFile `
        -Source (Join-Path $ortExtract "runtimes\win-x64\native\onnxruntime.dll") `
        -Destination (Join-Path $bundleRoot "onnxruntime.dll")
    Copy-RequiredFile `
        -Source (Join-Path $ortExtract "runtimes\win-x64\native\onnxruntime_providers_shared.dll") `
        -Destination (Join-Path $bundleRoot "onnxruntime_providers_shared.dll")
    Copy-RequiredFile `
        -Source (Join-Path $directMlExtract "bin\x64-win\DirectML.dll") `
        -Destination (Join-Path $bundleRoot "DirectML.dll")
    Copy-RequiredFile `
        -Source (Join-Path $genAiExtract "runtimes\win-x64\native\onnxruntime-genai.dll") `
        -Destination (Join-Path $bundleRoot "onnxruntime-genai.dll")

    Write-Host ""
    Write-Host "DirectML runtime bundle staged successfully."
    Write-Host "Bundle root: $bundleRoot"
} finally {
    if (Test-Path $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force
    }
}
