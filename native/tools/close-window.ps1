# Sends WM_CLOSE to the visible top-level window of one exact process id and
# waits for that process to exit. Never terminates by name.
param([Parameter(Mandatory)] [int] $ProcessId, [int] $TimeoutSeconds = 15)
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class CloseWin {
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
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
$p = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
if (-not $p) { "not running"; return }
$h = [CloseWin]::Find([uint32]$ProcessId)
if ($h -eq [IntPtr]::Zero) { "no visible window" } else { [void][CloseWin]::PostMessage($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) }
if ($p.WaitForExit($TimeoutSeconds * 1000)) { "exited ($($p.ExitCode))" } else { "still running after $TimeoutSeconds s" }
