#Requires -Version 5.1
# Repairs a canceled or partial Visual Studio Build Tools instance, then installs
# the C++ workload. Safe to rerun.

$ErrorActionPreference = "Continue"

$transcript = Join-Path $PSScriptRoot "..\fix-vs-buildtools.log"
try { Start-Transcript -Path $transcript -Append | Out-Null } catch { }

$installer = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vs_installer.exe"
$installPath = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools"
$instancesRoot = "C:\ProgramData\Microsoft\VisualStudio\Packages\_Instances"

function Test-VCTools {
    $msvc = Join-Path $installPath "VC\Tools\MSVC"
    if (-not (Test-Path $msvc)) { return $false }
    foreach ($version in Get-ChildItem $msvc -Directory -ErrorAction SilentlyContinue) {
        if (Test-Path (Join-Path $version.FullName "bin\Hostx64\x64\cl.exe")) { return $true }
    }
    return $false
}

function Remove-BrokenInstance {
    if (-not (Test-Path $instancesRoot)) { return }
    foreach ($instance in Get-ChildItem $instancesRoot -Directory -ErrorAction SilentlyContinue) {
        $statePath = Join-Path $instance.FullName "state.json"
        if (-not (Test-Path $statePath)) { continue }
        try {
            $state = Get-Content $statePath -Raw | ConvertFrom-Json
        } catch {
            continue
        }
        $path = $state.installationPath
        $canceled = $state.properties.canceled
        if ($path -eq $installPath -or $canceled -eq "1") {
            Write-Host "Removing stale instance $($instance.Name) ($path, canceled=$canceled)"
            Remove-Item $instance.FullName -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

Write-Host "==> Checking the current Build Tools state"
if (Test-VCTools) {
    Write-Host "Visual C++ tools are already installed." -ForegroundColor Green
    try { Stop-Transcript | Out-Null } catch { }
    exit 0
}

Write-Host "==> Repairing the existing instance"
& $installer repair --installPath $installPath --quiet --wait --norestart
Write-Host "repair exit code: $LASTEXITCODE"

if (-not (Test-VCTools)) {
    Write-Host "==> Repair did not produce the C++ tools; uninstalling the broken instance"
    & $installer uninstall --installPath $installPath --quiet --wait --norestart
    Write-Host "uninstall exit code: $LASTEXITCODE"
    Start-Sleep -Seconds 2

    if (Test-Path $installPath) {
        Write-Host "Removing leftover installation directory"
        Remove-Item $installPath -Recurse -Force -ErrorAction SilentlyContinue
    }
    Remove-BrokenInstance
}

if (-not (Test-VCTools)) {
    Write-Host "==> Installing Visual Studio Build Tools with the C++ workload"
    winget install --id Microsoft.VisualStudio.2022.BuildTools --exact --silent `
        --accept-package-agreements --accept-source-agreements `
        --override "--wait --quiet --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    Write-Host "winget exit code: $LASTEXITCODE"
}

Write-Host ""
if (Test-VCTools) {
    Write-Host "Visual C++ tools installed successfully." -ForegroundColor Green
} else {
    Write-Host "C++ tools are still missing. See fix-vs-buildtools.log and the Visual Studio installer logs." -ForegroundColor Red
}

try { Stop-Transcript | Out-Null } catch { }
