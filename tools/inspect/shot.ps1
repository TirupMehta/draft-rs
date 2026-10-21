param(
  [string]$View = "dictate",
  [string]$Section = "dictation",
  [int]$CellW = 4,
  [int]$CellH = 6
)

$ErrorActionPreference = "Stop"
Get-Process draft -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 900
$env:DRAFT_VIEW = $View
$env:DRAFT_SECTION = $Section
Start-Process -FilePath "$env:LOCALAPPDATA\Programs\Draft\draft.exe"
Start-Sleep -Seconds 6

Add-Type -AssemblyName System.Drawing
Add-Type -Path "$env:TEMP\opencode\Inspect.cs" -ReferencedAssemblies System.Drawing, System.Drawing.Primitives
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class Win2 {
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT { public int Left, Top, Right, Bottom; }
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder t, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder t, int n);
  public static List<IntPtr> TopLevel(uint wanted) {
    var found = new List<IntPtr>();
    EnumWindows((h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      if (pid == wanted) found.Add(h);
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
"@

$proc = Get-Process draft | Select-Object -First 1
$main = [IntPtr]::Zero
foreach ($handle in [Win2]::TopLevel([uint32]$proc.Id)) {
  $cb = New-Object System.Text.StringBuilder 256
  [Win2]::GetClassNameW($handle, $cb, 256) | Out-Null
  $sb = New-Object System.Text.StringBuilder 256
  [Win2]::GetWindowTextW($handle, $sb, 256) | Out-Null
  if ($cb.ToString() -eq "Zed::Window" -and $sb.ToString() -ne "") { $main = $handle }
}
if ($main -eq [IntPtr]::Zero) { throw "no main window" }

$rect = New-Object Win2+RECT
[Win2]::GetWindowRect($main, [ref]$rect) | Out-Null
$w = $rect.Right - $rect.Left
$h = $rect.Bottom - $rect.Top
$bitmap = New-Object System.Drawing.Bitmap($w, $h)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$hdc = $graphics.GetHdc()
[Inspect]::PrintWindow($main, $hdc, 2) | Out-Null
$graphics.ReleaseHdc($hdc)
$graphics.Dispose()
$path = "$env:TEMP\draft-$View-$Section.png"
$bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
$bitmap.Dispose()
"captured ${w}x${h} -> $path"
[Inspect]::Run($path, $CellW, $CellH)
