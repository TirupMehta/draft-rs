# Builds the Draft per-user Windows installer.
#
#   .\scripts\build-installer.ps1
#   .\scripts\build-installer.ps1 -IncludeModel
#
# The release binary must exist at dist\draft.exe first:
#   cargo build --release -p draft-desktop

[CmdletBinding()]
param(
    [switch]$IncludeModel,
    [string]$ModelPath = "",
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $repo "dist\draft.exe"
if (-not (Test-Path $exe)) {
    throw "dist\draft.exe is missing. Run: cargo build --release -p draft-desktop"
}

if (-not $Version) {
    $cargo = Get-Content (Join-Path $repo "Cargo.toml") -Raw
    $match = [regex]::Match($cargo, '(?m)^version\s*=\s*"([^"]+)"')
    if (-not $match.Success) {
        throw "Could not read the workspace version from Cargo.toml"
    }
    $Version = $match.Groups[1].Value
}

$iscc = @(
    (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe"),
    (Join-Path ${env:ProgramFiles(x86)} "Inno Setup 6\ISCC.exe"),
    (Join-Path $env:ProgramFiles "Inno Setup 6\ISCC.exe")
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $iscc) {
    throw "Inno Setup 6 was not found. Install it with: winget install JRSoftware.InnoSetup"
}

$defines = @(
    "/DAppVersion=$Version",
    "/DSourceExe=$exe",
    "/DRepoRoot=$repo"
)

if ($IncludeModel) {
    if (-not $ModelPath) {
        $ModelPath = Join-Path $env:LOCALAPPDATA "Draft4\models\whisper-large-v3-turbo-F16.gguf"
    }
    if (-not (Test-Path $ModelPath)) {
        throw "Model not found at $ModelPath"
    }
    $ModelPath = (Resolve-Path -LiteralPath $ModelPath).Path
    $defines += "/DIncludeModel=1"
    $defines += "/DModelPath=$ModelPath"
}

& $iscc /Qp @defines (Join-Path $repo "packaging\windows\draft.iss")
if ($LASTEXITCODE -ne 0) {
    throw "Inno Setup failed with exit code $LASTEXITCODE"
}

$installer = Join-Path $repo "dist\Draft-Setup-$Version.exe"
Get-Item $installer | Select-Object FullName, Length, LastWriteTime
