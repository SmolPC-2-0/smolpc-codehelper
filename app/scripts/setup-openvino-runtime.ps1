param(
    [string]$BundleRoot = "",
    [string]$ArchiveUrl = "https://storage.openvinotoolkit.org/repositories/openvino_genai/packages/2026.0/windows/openvino_genai_windows_2026.0.0.0_x86_64.zip",
    [string]$ArchiveSha256 = "8a6a75eb1ebc81bf82cbe8018e4390b40487ac2e8293d7a449f2415bc2beb1fb"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Resolve-TargetPath {
    param(
        [string]$Path,
        [string]$RepoRoot
    )

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return Join-Path $RepoRoot "src-tauri\libs\openvino"
    }

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }

    return Join-Path $RepoRoot $Path
}

function Resolve-ArchiveRoot {
    param([string]$ExtractRoot)

    $directRoot = Join-Path $ExtractRoot "runtime\bin\intel64\Release"
    if (Test-Path $directRoot -PathType Container) {
        return $ExtractRoot
    }

    foreach ($candidate in Get-ChildItem -Path $ExtractRoot -Directory) {
        $candidateRuntime = Join-Path $candidate.FullName "runtime\bin\intel64\Release"
        if (Test-Path $candidateRuntime -PathType Container) {
            return $candidate.FullName
        }
    }

    throw "Failed to locate the extracted OpenVINO runtime tree under $ExtractRoot"
}

function Copy-RequiredFile {
    param(
        [string]$Source,
        [string]$Destination
    )

    if (-not (Test-Path $Source -PathType Leaf)) {
        throw "Missing staged OpenVINO runtime file: $Source"
    }

    Copy-Item -LiteralPath $Source -Destination $Destination -Force
}

function Test-OpenVinoGenAiCExports {
    param(
        [string]$BundleRoot,
        [string]$TempRoot
    )

    $validationScriptPath = Join-Path $TempRoot "validate-openvino-genai-c.ps1"
    $validationScript = @'
param([string]$BundleRoot)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

Add-Type @"
using System;
using System.Runtime.InteropServices;

namespace SmolPc.OpenVino {
    public static class Win32 {
        [DllImport("kernel32", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern IntPtr LoadLibraryExW(string lpFileName, IntPtr hFile, uint dwFlags);

        [DllImport("kernel32", SetLastError = true, CharSet = CharSet.Ansi, ExactSpelling = true)]
        public static extern IntPtr GetProcAddress(IntPtr hModule, string procName);

        [DllImport("kernel32", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool FreeLibrary(IntPtr hModule);
    }
}
"@

$dllPath = Join-Path $BundleRoot "openvino_genai_c.dll"
$flags = 0x00000100 -bor 0x00000800
$module = [SmolPc.OpenVino.Win32]::LoadLibraryExW($dllPath, [IntPtr]::Zero, $flags)
if ($module -eq [IntPtr]::Zero) {
    $errorCode = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
    throw "Failed to load openvino_genai_c.dll for export validation (Win32 error $errorCode)."
}

try {
    $requiredExports = @(
        "ov_genai_llm_pipeline_create",
        "ov_genai_llm_pipeline_free",
        "ov_genai_llm_pipeline_generate",
        "ov_genai_generation_config_create",
        "ov_genai_generation_config_set_max_new_tokens"
    )

    $missing = @(
        foreach ($symbol in $requiredExports) {
            $address = [SmolPc.OpenVino.Win32]::GetProcAddress($module, $symbol)
            if ($address -eq [IntPtr]::Zero) {
                $symbol
            }
        }
    )

    if ($missing.Count -gt 0) {
        throw "openvino_genai_c.dll is missing required C API exports: $($missing -join ', ')"
    }
} finally {
    [SmolPc.OpenVino.Win32]::FreeLibrary($module) | Out-Null
}
'@

    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($validationScriptPath, $validationScript, $utf8NoBom)

    try {
        & powershell -NoProfile -ExecutionPolicy Bypass -File $validationScriptPath -BundleRoot $BundleRoot
        if ($LASTEXITCODE -ne 0) {
            throw "openvino_genai_c.dll export validation failed."
        }
    } finally {
        if (Test-Path $validationScriptPath) {
            Remove-Item -LiteralPath $validationScriptPath -Force
        }
    }
}

function Remove-TempTree {
    param([string]$Path)

    if (-not (Test-Path $Path)) {
        return
    }

    for ($attempt = 1; $attempt -le 5; $attempt++) {
        try {
            Remove-Item -LiteralPath $Path -Recurse -Force
            return
        } catch {
            if ($attempt -eq 5) {
                Write-Warning "Failed to clean up temporary OpenVINO staging directory ${Path}: $($_.Exception.Message)"
                return
            }
            Start-Sleep -Milliseconds 500
        }
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$bundleRoot = Resolve-TargetPath -Path $BundleRoot -RepoRoot $repoRoot
$tempRoot = Join-Path $env:TEMP ("smolpc-openvino-runtime-" + [Guid]::NewGuid().ToString("N"))
$archivePath = Join-Path $tempRoot "openvino_genai_windows_2026.0.0.0_x86_64.zip"
$extractRoot = Join-Path $tempRoot "extract"
$stagingRoot = Join-Path $tempRoot "bundle"

$requiredFiles = @(
    @{ Name = "openvino.dll"; SourceGroup = "runtime" },
    @{ Name = "openvino_c.dll"; SourceGroup = "runtime" },
    @{ Name = "openvino_genai.dll"; SourceGroup = "runtime" },
    @{ Name = "openvino_genai_c.dll"; SourceGroup = "runtime" },
    @{ Name = "openvino_tokenizers.dll"; SourceGroup = "runtime" },
    @{ Name = "openvino_intel_npu_plugin.dll"; SourceGroup = "runtime" },
    @{ Name = "openvino_intel_npu_compiler.dll"; SourceGroup = "runtime" },
    @{ Name = "openvino_intel_cpu_plugin.dll"; SourceGroup = "runtime" },
    @{ Name = "openvino_ir_frontend.dll"; SourceGroup = "runtime" },
    @{ Name = "icudt70.dll"; SourceGroup = "runtime" },
    @{ Name = "icuuc70.dll"; SourceGroup = "runtime" },
    @{ Name = "tbb12.dll"; SourceGroup = "tbb" },
    @{ Name = "tbbbind_2_5.dll"; SourceGroup = "tbb" },
    @{ Name = "tbbmalloc.dll"; SourceGroup = "tbb" },
    @{ Name = "tbbmalloc_proxy.dll"; SourceGroup = "tbb" }
)

try {
    New-Item -ItemType Directory -Force -Path $tempRoot, $extractRoot, $stagingRoot | Out-Null
    New-Item -ItemType Directory -Force -Path $bundleRoot | Out-Null

    Write-Host "Downloading OpenVINO GenAI 2026 Windows archive..."
    Invoke-WebRequest -Uri $ArchiveUrl -OutFile $archivePath

    $actualSha256 = (Get-FileHash -Path $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    $expectedSha256 = $ArchiveSha256.ToLowerInvariant()
    if ($actualSha256 -ne $expectedSha256) {
        throw "OpenVINO archive hash mismatch. Expected $expectedSha256 but got $actualSha256."
    }

    Write-Host "Extracting OpenVINO archive..."
    Expand-Archive -LiteralPath $archivePath -DestinationPath $extractRoot -Force

    $archiveRoot = Resolve-ArchiveRoot -ExtractRoot $extractRoot
    $runtimeRoot = Join-Path $archiveRoot "runtime\bin\intel64\Release"
    $tbbRoot = Join-Path $archiveRoot "runtime\3rdparty\tbb\bin"

    if (-not (Test-Path $runtimeRoot -PathType Container)) {
        throw "Archive runtime bin directory is missing: $runtimeRoot"
    }
    if (-not (Test-Path $tbbRoot -PathType Container)) {
        throw "Archive TBB bin directory is missing: $tbbRoot"
    }

    foreach ($file in $requiredFiles) {
        $sourceRoot = if ($file.SourceGroup -eq "tbb") { $tbbRoot } else { $runtimeRoot }
        $source = Join-Path $sourceRoot $file.Name
        $destination = Join-Path $stagingRoot $file.Name
        Copy-RequiredFile -Source $source -Destination $destination
    }

    Test-OpenVinoGenAiCExports -BundleRoot $stagingRoot -TempRoot $tempRoot

    foreach ($file in $requiredFiles) {
        $source = Join-Path $stagingRoot $file.Name
        $destination = Join-Path $bundleRoot $file.Name
        Copy-RequiredFile -Source $source -Destination $destination
    }

    $missing = @(
        foreach ($file in $requiredFiles) {
            $destination = Join-Path $bundleRoot $file.Name
            if (-not (Test-Path $destination -PathType Leaf)) {
                $file.Name
            }
        }
    )

    if ($missing.Count -gt 0) {
        throw "OpenVINO runtime bundle staging was incomplete. Missing: $($missing -join ', ')"
    }

    Write-Host ""
    Write-Host "OpenVINO runtime bundle staged successfully."
    Write-Host "Bundle root: $bundleRoot"
    Write-Host "Archive: $ArchiveUrl"
    Write-Host "Files:"
    foreach ($file in $requiredFiles) {
        Write-Host "  - $($file.Name)"
    }
} finally {
    Remove-TempTree -Path $tempRoot
}
