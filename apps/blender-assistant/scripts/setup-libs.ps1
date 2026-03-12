param(
    [ValidateSet("x64", "arm64")]
    [string]$Arch = "",
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$ortVersion = "1.23.0"
$dmlVersion = "1.15.4"
$genaiVersion = "0.12.1"

if ([string]::IsNullOrWhiteSpace($Arch)) {
    $Arch = if ([Environment]::Is64BitOperatingSystem) { "x64" } else { "arm64" }
}

$runtimeArch = if ($Arch -eq "arm64") { "win-arm64" } else { "win-x64" }
$dmlArch = if ($Arch -eq "arm64") { "arm64-win" } else { "x64-win" }

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$appRoot = Split-Path -Parent $scriptRoot
$libsDir = Join-Path $appRoot "src-tauri\libs"
New-Item -ItemType Directory -Force -Path $libsDir | Out-Null

$requiredFiles = @(
    "onnxruntime.dll",
    "onnxruntime_providers_shared.dll",
    "DirectML.dll",
    "onnxruntime-genai.dll"
)

if (-not $Force) {
    $allPresent = $true
    foreach ($file in $requiredFiles) {
        if (-not (Test-Path (Join-Path $libsDir $file))) {
            $allPresent = $false
            break
        }
    }
    if ($allPresent) {
        Write-Host "Runtime libs already present in $libsDir. Use -Force to reinstall."
        exit 0
    }
}

$tempDir = Join-Path $env:TEMP ("blender-assistant-libs-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tempDir | Out-Null

function Download-File {
    param(
        [string]$Url,
        [string]$OutPath
    )
    $maxAttempts = 3
    for ($attempt = 1; $attempt -le $maxAttempts; $attempt++) {
        try {
            Write-Host "Downloading $Url (attempt $attempt/$maxAttempts)"
            Invoke-WebRequest -Uri $Url -OutFile $OutPath
            return
        } catch {
            if ($attempt -ge $maxAttempts) {
                throw
            }
            Start-Sleep -Seconds (2 * $attempt)
        }
    }
}

try {
    $ortPkgName = "microsoft.ml.onnxruntime.directml.$ortVersion.nupkg"
    $dmlPkgName = "microsoft.ai.directml.$dmlVersion.nupkg"
    $genaiPkgName = "microsoft.ml.onnxruntimegenai.directml.$genaiVersion.nupkg"

    $ortPkgPath = Join-Path $tempDir $ortPkgName
    $dmlPkgPath = Join-Path $tempDir $dmlPkgName
    $genaiPkgPath = Join-Path $tempDir $genaiPkgName

    Download-File -Url "https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime.directml/$ortVersion/$ortPkgName" -OutPath $ortPkgPath
    Download-File -Url "https://api.nuget.org/v3-flatcontainer/microsoft.ai.directml/$dmlVersion/$dmlPkgName" -OutPath $dmlPkgPath
    Download-File -Url "https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntimegenai.directml/$genaiVersion/$genaiPkgName" -OutPath $genaiPkgPath

    $ortExtract = Join-Path $tempDir "ort"
    $dmlExtract = Join-Path $tempDir "dml"
    $genaiExtract = Join-Path $tempDir "genai"
    Expand-Archive -Path $ortPkgPath -DestinationPath $ortExtract -Force
    Expand-Archive -Path $dmlPkgPath -DestinationPath $dmlExtract -Force
    Expand-Archive -Path $genaiPkgPath -DestinationPath $genaiExtract -Force

    Copy-Item -Force (Join-Path $ortExtract "runtimes\$runtimeArch\native\onnxruntime.dll") (Join-Path $libsDir "onnxruntime.dll")
    Copy-Item -Force (Join-Path $ortExtract "runtimes\$runtimeArch\native\onnxruntime_providers_shared.dll") (Join-Path $libsDir "onnxruntime_providers_shared.dll")
    Copy-Item -Force (Join-Path $dmlExtract "bin\$dmlArch\DirectML.dll") (Join-Path $libsDir "DirectML.dll")
    Copy-Item -Force (Join-Path $genaiExtract "runtimes\$runtimeArch\native\onnxruntime-genai.dll") (Join-Path $libsDir "onnxruntime-genai.dll")

    $ortFile = Join-Path $libsDir "onnxruntime.dll"
    $fileVersion = (Get-Item $ortFile).VersionInfo.FileVersion
    Write-Host "Installed runtime libs into $libsDir"
    Write-Host "onnxruntime.dll version: $fileVersion"
} finally {
    if (Test-Path $tempDir) {
        Remove-Item -Recurse -Force $tempDir -ErrorAction SilentlyContinue
    }
}

exit 0
