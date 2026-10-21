param(
  [string]$Out = "$env:TEMP\draft-hud.png",
  [int]$CellW = 2,
  [int]$CellH = 3
)

# Capture the recording state.
#
#   .\tools\inspect\listen.ps1
#
# The dictation key is injected, so the app must be started with
# DRAFT_ALLOW_INJECTED_KEYS=1 (draft-hotkey ignores synthetic input by
# default, so Draft's own paste can never start a capture).

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -Path (Join-Path $PSScriptRoot 'Inspect.cs') -ReferencedAssemblies System.Drawing, System.Drawing.Primitives
Add-Type -Path (Join-Path $PSScriptRoot 'Windows.cs') -ReferencedAssemblies System.Drawing, System.Drawing.Primitives

if (-not (Get-Process draft -ErrorAction SilentlyContinue)) {
    $env:DRAFT_ALLOW_INJECTED_KEYS = '1'
    Start-Process -FilePath "$env:LOCALAPPDATA\Programs\Draft\draft.exe"
    Start-Sleep -Seconds 6
} else {
    "Draft is already running. Synthetic keys are ignored unless it was started"
    "with DRAFT_ALLOW_INJECTED_KEYS=1, so restart it with that variable set."
}

$process = Get-Process draft -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $process) { throw "Draft is not running" }

# Right Ctrl is the default dictation key (VK_RCONTROL).
[DraftWindows]::keybd_event(0xA3, 0, 0, [IntPtr]::Zero)
Start-Sleep -Seconds 2

try {
    $hud = [DraftWindows]::HudWindow([uint32]$process.Id)
    if ($hud -ne [IntPtr]::Zero) {
        $rect = New-Object DraftWindows+RECT
        [DraftWindows]::GetWindowRect($hud, [ref]$rect) | Out-Null
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
        $bitmap = New-Object System.Drawing.Bitmap($width, $height)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $hdc = $graphics.GetHdc()
        [DraftWindows]::PrintWindow($hud, $hdc, 2) | Out-Null
        $graphics.ReleaseHdc($hdc)
        $graphics.Dispose()
        $bitmap.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
        $bitmap.Dispose()
        "captured hud ${width}x${height} -> $Out"
        [Inspect]::Run($Out, $CellW, $CellH)
    } else {
        "the HUD window was not found"
    }
} finally {
    # Escape cancels the capture, then release the key.
    [DraftWindows]::keybd_event(0x1B, 0, 0, [IntPtr]::Zero)
    [DraftWindows]::keybd_event(0x1B, 0, 2, [IntPtr]::Zero)
    [DraftWindows]::keybd_event(0xA3, 0, 2, [IntPtr]::Zero)
}
