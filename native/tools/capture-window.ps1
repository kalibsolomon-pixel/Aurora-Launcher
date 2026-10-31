<#
.SYNOPSIS
  Captures the top-level window of one exact process id to a PNG using
  PrintWindow (PW_RENDERFULLCONTENT). Optionally resizes the window first.
#>
param(
    [Parameter(Mandatory)] [int] $ProcessId,
    [Parameter(Mandatory)] [string] $Out,
    [int] $Width = 0,
    [int] $Height = 0
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class CaptureWin {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
  public static IntPtr Find(uint pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p);
      if (p == pid && IsWindowVisible(h) && GetWindow(h, 4) == IntPtr.Zero) { found = h; return false; } return true; }, IntPtr.Zero);
    return found;
  }
}
"@
# Per-monitor-v2 awareness so window rectangles are physical pixels.
[void][CaptureWin]::SetProcessDpiAwarenessContext([IntPtr](-4))
$h = [CaptureWin]::Find([uint32]$ProcessId)
if ($h -eq [IntPtr]::Zero) { throw "no visible window for pid $ProcessId" }
if ($Width -gt 0 -and $Height -gt 0) {
    [void][CaptureWin]::SetWindowPos($h, [IntPtr]::Zero, 0, 0, $Width, $Height, 0x0002 -bor 0x0004)  # NOMOVE|NOZORDER
    Start-Sleep -Milliseconds 700
}
$r = New-Object CaptureWin+RECT
[void][CaptureWin]::GetWindowRect($h, [ref]$r)
$bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
[void][CaptureWin]::PrintWindow($h, $hdc, 2)
$g.ReleaseHdc($hdc); $g.Dispose()
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
"captured $Out ($($r.R - $r.L)x$($r.B - $r.T))"
