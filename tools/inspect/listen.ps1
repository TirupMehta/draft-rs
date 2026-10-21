param(
  [string]$Out = "$env:TEMP\draft-listening.png",
  [int]$CellW = 4,
  [int]$CellH = 6
)

$ErrorActionPreference = "Stop"
Get-Process draft -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 900
Remove-Item Env:\DRAFT_VIEW -ErrorAction SilentlyContinue
Remove-Item Env:\DRAFT_SECTION -ErrorAction SilentlyContinue
$env:DRAFT_ALLOW_INJECTED_KEYS = "1"
Start-Process -FilePath "$env:LOCALAPPDATA\Programs\Draft\draft.exe"
Start-Sleep -Seconds 6

Add-Type -AssemblyName System.Drawing
Add-Type -Path "$env:TEMP\opencode\Inspect.cs" -ReferencedAssemblies System.Drawing, System.Drawing.Primitives
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class Win3 {
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT { public int Left, Top, Right, Bottom; }
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder t, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder t, int n);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
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
$hud = [IntPtr]::Zero
foreach ($handle in [Win3]::TopLevel([uint32]$proc.Id)) {
  $cb = New-Object System.Text.StringBuilder 256
  [Win3]::GetClassNameW($handle, $cb, 256) | Out-Null
  $sb = New-Object System.Text.StringBuilder 256
  [Win3]::GetWindowTextW($handle, $sb, 256) | Out-Null
  if ($cb.ToString() -eq "Zed::Window") {
    if ($sb.ToString() -ne "") { $main = $handle } else { $hud = $handle }
  }
}

# Hold the dictation key so the recording state is real.
[Win3]::keybd_event(0xA3, 0, 0, [IntPtr]::Zero)
Start-Sleep -Seconds 2

$rect = New-Object Win3+RECT
[Win3]::GetWindowRect($main, [ref]$rect) | Out-Null
$w = $rect.Right - $rect.Left
$h = $rect.Bottom - $rect.Top
$bitmap = New-Object System.Drawing.Bitmap($w, $h)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$hdc = $graphics.GetHdc()
[Inspect]::PrintWindow($main, $hdc, 2) | Out-Null
$graphics.ReleaseHdc($hdc)
$graphics.Dispose()
$bitmap.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bitmap.Dispose()
"captured main ${w}x${h} -> $Out"
[Inspect]::Run($Out, $CellW, $CellH)

if ($hud -ne [IntPtr]::Zero) {
  $hr = New-Object Win3+RECT
  [Win3]::GetWindowRect($hud, [ref]$hr) | Out-Null
  "hud visible: $([Win3]::IsWindowVisible($hud)) at $($hr.Left),$($hr.Top) $($hr.Right - $hr.Left)x$($hr.Bottom - $hr.Top)"
  $hw = $hr.Right - $hr.Left
  $hh = $hr.Bottom - $hr.Top
  $screen = New-Object System.Drawing.Bitmap($hw, $hh)
  $sg = [System.Drawing.Graphics]::FromImage($screen)
  $sg.CopyFromScreen($hr.Left, $hr.Top, 0, 0, $screen.Size)
  $sg.Dispose()
  $hudPath = "$env:TEMP\draft-hud.png"
  $screen.Save($hudPath, [System.Drawing.Imaging.ImageFormat]::Png)
  $screen.Dispose()
  "captured hud -> $hudPath"
  [Inspect]::Run($hudPath, $CellW, $CellH)
}

[Win3]::keybd_event(0x1B, 0, 0, [IntPtr]::Zero)
[Win3]::keybd_event(0x1B, 0, 2, [IntPtr]::Zero)
[Win3]::keybd_event(0xA3, 0, 2, [IntPtr]::Zero)
