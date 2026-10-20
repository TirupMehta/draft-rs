# Draft v4 Windows development setup.
#
# Installs the native toolchain needed to build Draft:
#   - Rust stable (rustup, per user)
#   - Visual Studio C++ Build Tools + Windows SDK
#   - CMake
#   - Vulkan SDK (GPU inference)
#
# Run from an elevated PowerShell for the Visual Studio and Vulkan installs:
#   powershell -ExecutionPolicy Bypass -File .\scripts\setup-windows.ps1

#Requires -Version 5.1

$ErrorActionPreference = "Stop"

$transcript = Join-Path $PSScriptRoot "..\setup-windows.log"
try { Start-Transcript -Path $transcript -Append | Out-Null } catch { }

function Write-Step($message) {
    Write-Host ""
    Write-Host "==> $message" -ForegroundColor Cyan
}

function Test-Command($name) {
    return [bool](Get-Command $name -ErrorAction SilentlyContinue)
}

function Install-Package($id, $override) {
    if (Get-Command winget -ErrorAction SilentlyContinue) {
        $args = @(
            "install", "--id", $id, "-e",
            "--accept-package-agreements", "--accept-source-agreements",
            "--silent"
        )
        if ($override) {
            $args += @("--override", $override)
        }
        & winget @args
        if ($LASTEXITCODE -ne 0) {
            throw "winget failed to install $id (exit $LASTEXITCODE)"
        }
    } else {
        throw "winget is required. Install Microsoft App Installer first."
    }
}

if (-not (Test-Command winget)) {
    throw "winget is required. Install Microsoft App Installer first."
}

Write-Step "Rust"
if (Test-Command rustc) {
    rustc --version
} else {
    Install-Package "Rustlang.Rustup"
    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    if (Test-Path $cargoBin) {
        $env:Path = "$cargoBin;$env:Path"
    }
    & rustup default stable
    if ($LASTEXITCODE -ne 0) { throw "rustup default stable failed" }
}

& rustup component add rustfmt clippy
if ($LASTEXITCODE -ne 0) { throw "could not add rustfmt and clippy" }

Write-Step "Visual Studio C++ Build Tools"
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$hasMsvc = $false
if (Test-Path $vswhere) {
    $installPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    $hasMsvc = [bool]$installPath
}

if ($hasMsvc) {
    Write-Host "Visual Studio C++ tools already installed."
} else {
    if (-not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
            [Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw "Visual Studio Build Tools need administrator rights. Re-run this script from an elevated PowerShell."
    }
    Install-Package "Microsoft.VisualStudio.2022.BuildTools" `
        "--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
}

Write-Step "CMake"
if (Test-Command cmake) {
    cmake --version | Select-Object -First 1
} else {
    Install-Package "Kitware.CMake"
}

Write-Step "Vulkan SDK"
if ($env:VULKAN_SDK -and (Test-Path $env:VULKAN_SDK)) {
    Write-Host "Vulkan SDK found at $env:VULKAN_SDK"
} else {
    if (-not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
            [Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw "The Vulkan SDK installer needs administrator rights. Re-run from an elevated PowerShell."
    }
    Install-Package "KhronosGroup.VulkanSDK"
    Write-Host "Reopen PowerShell after the installer so VULKAN_SDK is visible."
}

Write-Step "GPUI shader compiler"
$fxc = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter fxc.exe -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match '\\x64\\' } |
    Sort-Object FullName -Descending |
    Select-Object -First 1

if ($fxc) {
    [Environment]::SetEnvironmentVariable("GPUI_FXC_PATH", $fxc.FullName, "User")
    $env:GPUI_FXC_PATH = $fxc.FullName
    Write-Host "GPUI_FXC_PATH=$($fxc.FullName)"
} else {
    Write-Warning "fxc.exe was not found. Install the Windows 10/11 SDK and re-run this step."
}

Write-Step "Verify"
& rustc --version
& cargo --version
& rustup component list --installed

Write-Host ""
Write-Host "Done. Open a new terminal, then run: cargo run -p draft-desktop" -ForegroundColor Green
try { Stop-Transcript | Out-Null } catch { }
