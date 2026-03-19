$script:DmlBuilderPythonVersions = @("3.14", "3.13", "3.12", "3.11")
$script:DmlBuilderPinnedPackages = [ordered]@{
    "onnxruntime"                  = "1.24.2"
    "onnxruntime-directml"         = "1.24.2"
    "onnxruntime-genai"            = "0.12.2"
    "onnxruntime-genai-directml"   = "0.12.2"
    "torch"                        = "2.10.0"
    "transformers"                 = "5.2.0"
    "tokenizers"                   = "0.22.2"
    "onnx"                         = "1.20.1"
    "onnx-ir"                      = "0.2.0"
    "huggingface_hub"              = "1.5.0"
    "safetensors"                  = "0.7.0"
    "sentencepiece"                = "0.2.1"
    "tqdm"                         = "4.67.1"
}
$script:DmlRequiredArtifactFiles = @("model.onnx", "genai_config.json", "tokenizer.json")

function Get-DmlBuilderPackageSpecs {
    return @(
        foreach ($entry in $script:DmlBuilderPinnedPackages.GetEnumerator()) {
            "{0}=={1}" -f $entry.Key, $entry.Value
        }
    )
}

function Get-DmlStringArray {
    param($Value)

    if ($null -eq $Value) {
        return @()
    }

    return @($Value | ForEach-Object { "$_".Trim() } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
}

function Get-DmlExportLogPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ModelId,
        [string]$SourceLabel = "selfbuild"
    )

    $logsRoot = Join-Path $env:LOCALAPPDATA "SmolPC\logs\dml-export"
    New-Item -ItemType Directory -Force -Path $logsRoot | Out-Null

    $safeModelId = (($ModelId -replace "[^A-Za-z0-9._-]", "-").Trim("-"))
    if ([string]::IsNullOrWhiteSpace($safeModelId)) {
        $safeModelId = "model"
    }

    $safeSourceLabel = (($SourceLabel -replace "[^A-Za-z0-9._-]", "-").Trim("-"))
    if ([string]::IsNullOrWhiteSpace($safeSourceLabel)) {
        $safeSourceLabel = "export"
    }

    $timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
    return Join-Path $logsRoot "$safeModelId-$safeSourceLabel-$timestamp.log"
}

function Write-DmlLogHeader {
    param(
        [Parameter(Mandatory = $true)]
        [string]$LogPath,
        [Parameter(Mandatory = $true)]
        [hashtable]$Metadata
    )

    $lines = @(
        "# SmolPC DirectML export log"
        ("timestamp={0}" -f (Get-Date -Format "o"))
    )

    foreach ($entry in $Metadata.GetEnumerator()) {
        $lines += ("{0}={1}" -f $entry.Key, $entry.Value)
    }

    $lines += ""

    $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::WriteAllLines($LogPath, $lines, $utf8NoBom)
}

function Add-DmlLogLine {
    param(
        [Parameter(Mandatory = $true)]
        [string]$LogPath,
        [Parameter(Mandatory = $true)]
        [string]$Message
    )

    $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::AppendAllText($LogPath, ("{0}{1}" -f $Message, [Environment]::NewLine), $utf8NoBom)
}

function Test-PythonCandidate {
    param(
        [string]$Executable,
        [string[]]$PrefixArgs = @()
    )

    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $version = (& $Executable @($PrefixArgs + @("-c", "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')")) 2>$null | Select-Object -Last 1)
    $exitCode = $LASTEXITCODE
    $ErrorActionPreference = $previous

    if ($exitCode -ne 0) {
        return $null
    }

    $resolvedVersion = "$version".Trim()
    if ([string]::IsNullOrWhiteSpace($resolvedVersion)) {
        return $null
    }

    return $resolvedVersion
}

function Resolve-DmlBuilderPython {
    $override = [Environment]::GetEnvironmentVariable("SMOLPC_DML_PYTHON")
    if (-not [string]::IsNullOrWhiteSpace($override)) {
        $version = Test-PythonCandidate -Executable $override
        if ($null -eq $version) {
            throw "SMOLPC_DML_PYTHON='$override' could not be executed."
        }
        if ($script:DmlBuilderPythonVersions -notcontains $version) {
            throw "SMOLPC_DML_PYTHON='$override' resolved to Python $version. Python 3.11-3.14 is required for DirectML model exports."
        }
        return [pscustomobject]@{
            Executable = $override
            PrefixArgs = @()
            Version    = $version
            Display    = "$override ($version)"
        }
    }

    if (Get-Command py -ErrorAction SilentlyContinue) {
        foreach ($candidate in $script:DmlBuilderPythonVersions) {
            $version = Test-PythonCandidate -Executable "py" -PrefixArgs @("-$candidate")
            if ($version -eq $candidate) {
                return [pscustomobject]@{
                    Executable = "py"
                    PrefixArgs = @("-$candidate")
                    Version    = $version
                    Display    = "py -$candidate"
                }
            }
        }
    }

    if (Get-Command python -ErrorAction SilentlyContinue) {
        $version = Test-PythonCandidate -Executable "python"
        if ($script:DmlBuilderPythonVersions -contains $version) {
            return [pscustomobject]@{
                Executable = "python"
                PrefixArgs = @()
                Version    = $version
                Display    = "python ($version)"
            }
        }
    }

    throw "Python 3.11-3.14 is required for DirectML model exports. Install one of those versions or set SMOLPC_DML_PYTHON."
}

function Get-DmlBuilderSignature {
    param([pscustomobject]$PythonCommand)

    $signature = [ordered]@{
        python_version = $PythonCommand.Version
        packages       = $script:DmlBuilderPinnedPackages
    }

    return ($signature | ConvertTo-Json -Depth 5 -Compress)
}

function Get-DmlBuilderVenvRoot {
    param([pscustomobject]$PythonCommand)

    $toolingRoot = Join-Path $env:LOCALAPPDATA "SmolPC\tooling"
    $pythonTag = $PythonCommand.Version.Replace(".", "")
    $genAiVersion = $script:DmlBuilderPinnedPackages["onnxruntime-genai-directml"]
    return Join-Path $toolingRoot "ort-genai-dml-py$pythonTag-v$genAiVersion"
}

function Test-DmlBuilderEnvironment {
    param(
        [string]$VenvPython,
        [string]$Signature,
        [string]$SignaturePath
    )

    if (-not (Test-Path $VenvPython -PathType Leaf)) {
        return $false
    }

    if (-not (Test-Path $SignaturePath -PathType Leaf)) {
        return $false
    }

    $existingSignature = (Get-Content $SignaturePath -Raw).Trim()
    if ($existingSignature -ne $Signature) {
        return $false
    }

    $validationScript = @'
import importlib.metadata as metadata
import sys

required = {
    "onnxruntime": "1.24.2",
    "onnxruntime-directml": "1.24.2",
    "onnxruntime-genai": "0.12.2",
    "onnxruntime-genai-directml": "0.12.2",
    "torch": "2.10.0",
    "transformers": "5.2.0",
    "tokenizers": "0.22.2",
    "onnx": "1.20.1",
    "onnx-ir": "0.2.0",
    "huggingface_hub": "1.5.0",
    "safetensors": "0.7.0",
    "sentencepiece": "0.2.1",
    "tqdm": "4.67.1",
}

modules = (
    "onnxruntime_genai",
    "torch",
    "transformers",
    "onnx",
    "onnx_ir",
    "huggingface_hub",
)

for dist_name, expected_version in required.items():
    if metadata.version(dist_name) != expected_version:
        raise SystemExit(1)

for module_name in modules:
    __import__(module_name)
'@

    $null = ($validationScript | & $VenvPython - 2>$null)
    return ($LASTEXITCODE -eq 0)
}

function Ensure-DmlBuilderEnvironment {
    $pythonCommand = Resolve-DmlBuilderPython
    $venvRoot = Get-DmlBuilderVenvRoot -PythonCommand $pythonCommand
    $venvPython = Join-Path $venvRoot "Scripts\python.exe"
    $signaturePath = Join-Path $venvRoot "builder-env.json"
    $signature = Get-DmlBuilderSignature -PythonCommand $pythonCommand

    if (-not (Test-Path $venvPython -PathType Leaf)) {
        New-Item -ItemType Directory -Force -Path (Split-Path $venvRoot -Parent) | Out-Null
        Write-Host "Creating DirectML builder venv with $($pythonCommand.Display) ..."
        & $pythonCommand.Executable @($pythonCommand.PrefixArgs + @("-m", "venv", $venvRoot))
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to create DirectML builder venv at '$venvRoot'."
        }
    }

    if (-not (Test-DmlBuilderEnvironment -VenvPython $venvPython -Signature $signature -SignaturePath $signaturePath)) {
        $packageSpecs = Get-DmlBuilderPackageSpecs

        Write-Host "Installing pinned DirectML builder packages into $venvRoot ..."
        & $venvPython -m pip install --upgrade pip
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to upgrade pip in '$venvRoot'."
        }

        & $venvPython -m pip install --upgrade @packageSpecs
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to install pinned DirectML builder packages into '$venvRoot'."
        }

        [System.IO.File]::WriteAllText($signaturePath, $signature, [System.Text.UTF8Encoding]::new($false))

        if (-not (Test-DmlBuilderEnvironment -VenvPython $venvPython -Signature $signature -SignaturePath $signaturePath)) {
            throw "DirectML builder environment validation failed after package installation."
        }
    }

    return [pscustomobject]@{
        Root             = $venvRoot
        PythonPath       = $venvPython
        PythonVersion    = $pythonCommand.Version
        SignaturePath    = $signaturePath
        PinnedPackages   = $script:DmlBuilderPinnedPackages
    }
}

function Get-DmlArtifactValidation {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ArtifactDir
    )

    $environment = Ensure-DmlBuilderEnvironment
    $artifactPath = Resolve-Path -LiteralPath $ArtifactDir -ErrorAction SilentlyContinue
    if ($null -eq $artifactPath) {
        $artifactPath = $ArtifactDir
    } else {
        $artifactPath = $artifactPath.ProviderPath
    }

    $validationScript = @'
import json
import os
import sys
from pathlib import Path

import onnx

artifact_dir = Path(sys.argv[1])
required = ["model.onnx", "genai_config.json", "tokenizer.json"]
required_missing = [name for name in required if not (artifact_dir / name).is_file()]

external_refs = []
missing_external = []
if not required_missing:
    model = onnx.load(str(artifact_dir / "model.onnx"), load_external_data=False)
    refs = set()
    missing = set()
    for tensor in model.graph.initializer:
        if tensor.data_location != onnx.TensorProto.EXTERNAL:
            continue
        entries = {entry.key: entry.value for entry in tensor.external_data}
        location = entries.get("location")
        if location:
            refs.add(location)
            if not (artifact_dir / location).exists():
                missing.add(location)
    external_refs = sorted(refs)
    missing_external = sorted(missing)

print(
    json.dumps(
        {
            "artifact_dir": str(artifact_dir),
            "required_missing": required_missing,
            "external_refs": external_refs,
            "missing_external": missing_external,
        }
    )
)
'@

    $json = & $environment.PythonPath -c $validationScript $artifactPath
    if ($LASTEXITCODE -ne 0) {
        throw "DirectML artifact validation failed for '$artifactPath'."
    }

    return $json | ConvertFrom-Json
}

function Assert-DmlArtifactReady {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ArtifactDir
    )

    $validation = Get-DmlArtifactValidation -ArtifactDir $ArtifactDir
    $requiredMissing = Get-DmlStringArray -Value $validation.required_missing
    $missingExternal = Get-DmlStringArray -Value $validation.missing_external
    $missing = @($requiredMissing + $missingExternal)

    if ($missing.Count -gt 0) {
        throw "DirectML lane incomplete. Missing: $($missing -join ', ')"
    }

    return $validation
}

function Invoke-DmlModelBuilder {
    param(
        [string]$ModelName = "",
        [string]$InputPath = "",
        [Parameter(Mandatory = $true)]
        [string]$OutputDir,
        [Parameter(Mandatory = $true)]
        [string]$Precision,
        [ValidateSet("cpu", "cuda", "dml", "webgpu", "NvTensorRtRtx")]
        [string]$ExecutionProvider = "dml",
        [string[]]$ExtraOptions = @(),
        [string]$LogPath = ""
    )

    if ([string]::IsNullOrWhiteSpace($ModelName) -and [string]::IsNullOrWhiteSpace($InputPath)) {
        throw "Invoke-DmlModelBuilder requires either -ModelName or -InputPath."
    }

    $environment = Ensure-DmlBuilderEnvironment
    $builderPython = $environment.PythonPath

    if (Test-Path $OutputDir) {
        Remove-Item -LiteralPath $OutputDir -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

    $builderArgs = @(
        "-m",
        "onnxruntime_genai.models.builder",
        "-o",
        $OutputDir,
        "-p",
        $Precision,
        "-e",
        $ExecutionProvider
    )

    if (-not [string]::IsNullOrWhiteSpace($ModelName)) {
        $builderArgs += @("-m", $ModelName)
    }

    if (-not [string]::IsNullOrWhiteSpace($InputPath)) {
        $builderArgs += @("-i", $InputPath)
    }

    if ($ExtraOptions.Count -gt 0) {
        $builderArgs += "--extra_options"
        $builderArgs += $ExtraOptions
    }

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"

    try {
        if (-not [string]::IsNullOrWhiteSpace($LogPath)) {
        $logDir = Split-Path -Parent $LogPath
        if (-not [string]::IsNullOrWhiteSpace($logDir)) {
            New-Item -ItemType Directory -Force -Path $logDir | Out-Null
        }

        if (-not (Test-Path $LogPath -PathType Leaf) -or (Get-Item $LogPath).Length -eq 0) {
            Write-DmlLogHeader -LogPath $LogPath -Metadata @{
                model_name         = $ModelName
                input_path         = $InputPath
                output_dir         = $OutputDir
                precision          = $Precision
                execution_provider = $ExecutionProvider
                python_path        = $builderPython
            }
        }

            & $builderPython @builderArgs 2>&1 `
                | ForEach-Object {
                    if ($_ -is [System.Management.Automation.ErrorRecord]) {
                        $_.ToString()
                    } else {
                        $_
                    }
                } `
                | Tee-Object -FilePath $LogPath -Append `
                | Out-Host
        } else {
            & $builderPython @builderArgs 2>&1 `
                | ForEach-Object {
                    if ($_ -is [System.Management.Automation.ErrorRecord]) {
                        $_.ToString()
                    } else {
                        $_
                    }
                } `
                | Out-Host
        }
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }

    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) {
        throw "DirectML model builder failed with exit code $exitCode."
    }

    return $environment
}
