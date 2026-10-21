param(
  [string]$Out = "$env:TEMP\draft-window.png",
  [int]$CellW = 4,
  [int]$CellH = 6,
  [switch]$Start
)

# Capture the running Draft window and report it as text.
#
#   .\tools\inspect\capture.ps1            # capture what is running
#   .\tools\inspect\capture.ps1 -Start     # start Draft first
#
# The window is captured with PrintWindow, so the report is the real rendered
# surface, not a guess.

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -Path (Join-Path $PSScriptRoot 'Inspect.cs') -ReferencedAssemblies System.Drawing, System.Drawing.Primitives
Add-Type -Path (Join-Path $PSScriptRoot 'Windows.cs') -ReferencedAssemblies System.Drawing, System.Drawing.Primitives

if ($Start -and -not (Get-Process draft -ErrorAction SilentlyContinue)) {
    Start-Process -FilePath "$env:LOCALAPPDATA\Programs\Draft\draft.exe"
    Start-Sleep -Seconds 6
}

$process = Get-Process draft -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $process) { throw "Draft is not running" }

$main = [DraftWindows]::MainWindow([uint32]$process.Id)
if ($main -eq [IntPtr]::Zero) { throw "Draft has no visible window" }

$rect = New-Object DraftWindows+RECT
[DraftWindows]::GetWindowRect($main, [ref]$rect) | Out-Null
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top

$bitmap = New-Object System.Drawing.Bitmap($width, $height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$hdc = $graphics.GetHdc()
[DraftWindows]::PrintWindow($main, $hdc, 2) | Out-Null
$graphics.ReleaseHdc($hdc)
$graphics.Dispose()
$bitmap.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bitmap.Dispose()

"captured ${width}x${height} -> $Out"
[Inspect]::Run($Out, $CellW, $CellH)
"--- text ---"
& (Join-Path $PSScriptRoot 'ocr.ps1') -Path $Out
