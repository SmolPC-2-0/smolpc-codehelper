param(
    [string]$BundleRoot = "",
    [string]$OpenVinoVersion = "2026.0.0",
    [string]$OpenVinoGenAiVersion = "2026.0.0.0",
    [string]$OpenVinoTokenizersVersion = "2026.0.0.0"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Require-Command {
    param([string]$Name)

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' is missing."
    }
}

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

function Copy-RequiredFile {
    param(
        [string]$Source,
        [string]$Destination
    )

    if (-not (Test-Path $Source -PathType Leaf)) {
        throw "Missing staged OpenVINO runtime file: $Source"
    }

    Copy-Item -Force $Source $Destination
}

function Test-OpenVinoGenAiExports {
    param([string]$PythonTarget)

    $validationScriptPath = Join-Path $PythonTarget "smolpc-openvino-export-check.py"
    $validationScript = @'
import ctypes
import os
import pathlib
import sys

python_target = pathlib.Path(sys.argv[1])
search_dirs = [
    python_target / "openvino" / "libs",
    python_target / "openvino_genai",
    python_target / "openvino_tokenizers" / "lib",
]
for directory in search_dirs:
    os.add_dll_directory(str(directory))

dll = ctypes.WinDLL(str(python_target / "openvino_genai" / "openvino_genai.dll"))
required = [
    "ov_genai_llm_pipeline_create",
    "ov_genai_llm_pipeline_free",
    "ov_genai_llm_pipeline_generate",
    "ov_genai_generation_config_create",
    "ov_genai_generation_config_set_max_new_tokens",
]
missing = []
for symbol in required:
    try:
        getattr(dll, symbol)
    except AttributeError:
        missing.append(symbol)

if missing:
    sys.stderr.write(
        "openvino_genai.dll is missing required C API exports: "
        + ", ".join(missing)
        + "\n"
    )
    sys.exit(3)
'@

    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($validationScriptPath, $validationScript, $utf8NoBom)

    try {
        & python $validationScriptPath $PythonTarget
        if ($LASTEXITCODE -ne 0) {
            throw (
                "Installed OpenVINO packages did not provide the required GenAI C API exports. " +
                "The current staged Windows PyPI bundle is not compatible with the native runtime adapter."
            )
        }
    } finally {
        if (Test-Path $validationScriptPath) {
            Remove-Item -LiteralPath $validationScriptPath -Force
        }
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$bundleRoot = Resolve-TargetPath -Path $BundleRoot -RepoRoot $repoRoot
$pythonTarget = Join-Path $env:TEMP ("smolpc-openvino-runtime-" + [Guid]::NewGuid().ToString("N"))

$requiredFiles = @(
    @{ Name = "openvino.dll"; Source = "openvino\libs\openvino.dll" },
    @{ Name = "openvino_c.dll"; Source = "openvino\libs\openvino_c.dll" },
    @{ Name = "openvino_intel_npu_plugin.dll"; Source = "openvino\libs\openvino_intel_npu_plugin.dll" },
    @{ Name = "openvino_intel_npu_compiler.dll"; Source = "openvino\libs\openvino_intel_npu_compiler.dll" },
    @{ Name = "openvino_intel_cpu_plugin.dll"; Source = "openvino\libs\openvino_intel_cpu_plugin.dll" },
    @{ Name = "openvino_ir_frontend.dll"; Source = "openvino\libs\openvino_ir_frontend.dll" },
    @{ Name = "openvino_genai.dll"; Source = "openvino_genai\openvino_genai.dll" },
    @{ Name = "openvino_tokenizers.dll"; Source = "openvino_tokenizers\lib\openvino_tokenizers.dll" },
    @{ Name = "tbb12.dll"; Source = "openvino\libs\tbb12.dll" },
    @{ Name = "tbbbind_2_5.dll"; Source = "openvino\libs\tbbbind_2_5.dll" },
    @{ Name = "tbbmalloc.dll"; Source = "openvino\libs\tbbmalloc.dll" },
    @{ Name = "tbbmalloc_proxy.dll"; Source = "openvino\libs\tbbmalloc_proxy.dll" },
    @{ Name = "icudt70.dll"; Source = "openvino_tokenizers\lib\icudt70.dll" },
    @{ Name = "icuuc70.dll"; Source = "openvino_tokenizers\lib\icuuc70.dll" }
)

Require-Command -Name "python"

try {
    New-Item -ItemType Directory -Force -Path $pythonTarget | Out-Null
    New-Item -ItemType Directory -Force -Path $bundleRoot | Out-Null

    Write-Host "Installing pinned OpenVINO Python packages into a temporary staging root..."
    & python -m pip install `
        --upgrade `
        --no-deps `
        --target $pythonTarget `
        "openvino==$OpenVinoVersion" `
        "openvino-genai==$OpenVinoGenAiVersion" `
        "openvino-tokenizers==$OpenVinoTokenizersVersion"

    if ($LASTEXITCODE -ne 0) {
        throw "Failed to install the pinned OpenVINO packages for bundle staging."
    }

    Test-OpenVinoGenAiExports -PythonTarget $pythonTarget

    foreach ($file in $requiredFiles) {
        $source = Join-Path $pythonTarget $file.Source
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
    Write-Host "Files:"
    foreach ($file in $requiredFiles) {
        Write-Host "  - $($file.Name)"
    }
} finally {
    if (Test-Path $pythonTarget) {
        Remove-Item -LiteralPath $pythonTarget -Recurse -Force
    }
}
