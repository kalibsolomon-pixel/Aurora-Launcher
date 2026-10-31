<#
.SYNOPSIS
  Posts a left click to the top-level window of one exact process id, at a
  point given in window-capture coordinates (as produced by capture-window.ps1).
  Used only against disposable acceptance instances.
#>
param([Parameter(Mandatory)] [int] $ProcessId, [Parameter(Mandatory)] [int] $X, [Parameter(Mandatory)] [int] $Y)
$ErrorActionPreference = 'Stop'
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class ClickWin {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  public static IntPtr Find(uint pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p);
      if (p == pid && IsWindowVisible(h) && GetWindow(h, 4) == IntPtr.Zero) { found = h; return false; } return true; }, IntPtr.Zero);
    return found;
  }
}
"@
[void][ClickWin]::SetProcessDpiAwarenessContext([IntPtr](-4))
$h = [ClickWin]::Find([uint32]$ProcessId)
if ($h -eq [IntPtr]::Zero) { throw "no window for pid $ProcessId" }
$r = New-Object ClickWin+RECT; [void][ClickWin]::GetWindowRect($h, [ref]$r)
$o = New-Object ClickWin+POINT; [void][ClickWin]::ClientToScreen($h, [ref]$o)
$cx = $X - ($o.X - $r.L); $cy = $Y - ($o.Y - $r.T)
$l = [IntPtr](($cy -shl 16) -bor ($cx -band 0xFFFF))
[void][ClickWin]::PostMessage($h, 0x0200, [IntPtr]0, $l)   # WM_MOUSEMOVE
Start-Sleep -Milliseconds 80
[void][ClickWin]::PostMessage($h, 0x0201, [IntPtr]1, $l)   # WM_LBUTTONDOWN
Start-Sleep -Milliseconds 60
[void][ClickWin]::PostMessage($h, 0x0202, [IntPtr]0, $l)   # WM_LBUTTONUP
"clicked client ($cx, $cy)"
