param(
  [ValidateSet("idle", "settings", "listening")]
  [string]$Mode = "idle",
  [string]$Out = "$env:TEMP\draft-capture.png",
  [int]$CellW = 4,
  [int]$CellH = 6
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
Add-Type -Path "$env:TEMP\opencode\Inspect.cs" -ReferencedAssemblies System.Drawing, System.Drawing.Primitives
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class Win {
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT { public int Left, Top, Right, Bottom; }
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
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

function Capture([IntPtr]$handle, [string]$path, [int]$cw, [int]$ch) {
  $rect = New-Object Win+RECT
  [Win]::GetWindowRect($handle, [ref]$rect) | Out-Null
  $w = $rect.Right - $rect.Left
  $h = $rect.Bottom - $rect.Top
  if ($w -le 0 -or $h -le 0) { return }
  $bitmap = New-Object System.Drawing.Bitmap($w, $h)
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  $hdc = $graphics.GetHdc()
  [Inspect]::PrintWindow($handle, $hdc, 2) | Out-Null
  $graphics.ReleaseHdc($hdc)
  $graphics.Dispose()
  $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $bitmap.Dispose()
  "captured ${w}x${h} -> $path"
  [Inspect]::Run($path, $cw, $ch)
}

if (-not (Get-Process draft -ErrorAction SilentlyContinue)) {
  $exe = "$env:LOCALAPPDATA\Programs\Draft\draft.exe"
  Start-Process -FilePath $exe
  Start-Sleep -Seconds 6
}

$proc = Get-Process draft | Select-Object -First 1
$main = [IntPtr]::Zero
$hud = [IntPtr]::Zero
foreach ($handle in [Win]::TopLevel([uint32]$proc.Id)) {
  $r = New-Object Win+RECT
  [Win]::GetWindowRect($handle, [ref]$r) | Out-Null
  $w = $r.Right - $r.Left
  $h = $r.Bottom - $r.Top
  $sb = New-Object System.Text.StringBuilder 256
  [Win]::GetWindowTextW($handle, $sb, 256) | Out-Null
  $title = $sb.ToString()
  $cb = New-Object System.Text.StringBuilder 256
  [Win]::GetClassNameW($handle, $cb, 256) | Out-Null
  $class = $cb.ToString()
  if ($class -eq "Zed::Window") {
    if ($title -ne "") { $main = $handle } else { $hud = $handle }
  }
}
if ($main -eq [IntPtr]::Zero) { throw "Draft has no main window" }
$rect = New-Object Win+RECT
[Win]::GetWindowRect($main, [ref]$rect) | Out-Null
"main window handle: $main  rect $($rect.Right - $rect.Left)x$($rect.Bottom - $rect.Top)"

if ($Mode -eq "settings") {
  # The Settings tab sits at the far right of the header row. The window
  # rect includes the native frame, so the client area starts 8px in and
  # 30px down.
  [Win]::SetForegroundWindow($main) | Out-Null
  Start-Sleep -Milliseconds 400
  $x = $rect.Left + 8 + 540
  $y = $rect.Top + 30 + 30
  [Win]::SetCursorPos($x, $y) | Out-Null
  Start-Sleep -Milliseconds 200
  [Win]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 80
  [Win]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
  Start-Sleep -Seconds 2
}

if ($Mode -eq "listening") {
  [Win]::keybd_event(0xA3, 0, 0, [IntPtr]::Zero)
  Start-Sleep -Seconds 2
}

if ($Mode -eq "listening") {
  "--- main window (listening) ---"
  Capture $main $Out $CellW $CellH
  "--- hud ---"
  if ($hud -ne [IntPtr]::Zero) {
    Capture $hud "$env:TEMP\draft-hud.png" $CellW $CellH
  }
  [Win]::keybd_event(0x1B, 0, 0, [IntPtr]::Zero)
  [Win]::keybd_event(0x1B, 0, 2, [IntPtr]::Zero)
  [Win]::keybd_event(0xA3, 0, 2, [IntPtr]::Zero)
} else {
  Capture $main $Out $CellW $CellH
}
