$ErrorActionPreference = "Stop"
$hasViolation = $false

function Report-Violation {
    param([string]$Message)
    Write-Host "[boundary] $Message" -ForegroundColor Red
    $script:hasViolation = $true
}

function Assert-PathAbsent {
    param(
        [string]$Path,
        [string]$Reason
    )

    if (Test-Path $Path) {
        Report-Violation ("{0}: {1}" -f $Reason, $Path)
    }
}

Assert-PathAbsent "apps/codehelper/src-tauri/src/inference" "Legacy app-owned inference module must remain removed"
Assert-PathAbsent "apps/codehelper/src-tauri/src/models" "Legacy app-owned model module must remain removed"
Assert-PathAbsent "apps/codehelper/src-tauri/src/commands/ollama.rs" "Legacy Ollama command path must remain removed"
Assert-PathAbsent "apps/codehelper/src/lib/stores/ollama.svelte.ts" "Legacy Ollama frontend store must remain removed"
Assert-PathAbsent "apps/codehelper/src/lib/types/ollama.ts" "Legacy Ollama frontend types must remain removed"

$commandsModPath = "apps/codehelper/src-tauri/src/commands/mod.rs"
if (Test-Path $commandsModPath) {
    $commandsMod = Get-Content $commandsModPath -Raw
    if ($commandsMod -match "\\bollama\\b") {
        Report-Violation "Ollama module references are not allowed in commands/mod.rs"
    }
}

$appCargoPath = "apps/codehelper/src-tauri/Cargo.toml"
if (Test-Path $appCargoPath) {
    $appCargo = Get-Content $appCargoPath -Raw
    if ($appCargo -match "smolpc-engine-host") {
        Report-Violation "CodeHelper app must not depend directly on smolpc-engine-host"
    }
}

$hostImportMatches = @()
if (Get-Command rg -ErrorAction SilentlyContinue) {
    $rgMatches = rg --line-number --glob "*.rs" "smolpc_engine_host" "apps/codehelper/src-tauri/src"
    if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($rgMatches)) {
        $hostImportMatches = @($rgMatches)
    }
} else {
    $hostImportMatches = @(
        Get-ChildItem -Path "apps/codehelper/src-tauri/src" -Recurse -Filter "*.rs" |
            Select-String -Pattern "smolpc_engine_host" |
            ForEach-Object { "{0}:{1}:{2}" -f $_.Path, $_.LineNumber, $_.Line.Trim() }
    )
}

if ($hostImportMatches.Count -gt 0) {
    Report-Violation "Direct smolpc_engine_host imports are not allowed in apps/codehelper/src-tauri/src"
    Write-Host $hostImportMatches
}

if ($hasViolation) {
    exit 1
}

Write-Host "Boundary checks passed." -ForegroundColor Green
