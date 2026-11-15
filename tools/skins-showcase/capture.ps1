param([string]$Exe, [string]$Out, [int]$Settle = 8, [string]$Pages = 'skins:7', [switch]$KeepOpen)
# Boots the showcase build once per page against a fresh fake data root on the
# portrait secondary display (never focused), captures the client area, closes.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class Cap {
 [StructLayout(LayoutKind.Sequential)] public struct RECT {public int L,T,R,B;}
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h,uint m,IntPtr w,IntPtr l,uint flags,uint timeout,out IntPtr result);
 public static void Send(IntPtr h,uint m,long w,long l){IntPtr r;SendMessageTimeout(h,m,new IntPtr(w),new IntPtr(l),2,2000,out r);}
 public static IntPtr Find(uint pid){IntPtr found=IntPtr.Zero;RECT best=new RECT();EnumWindows((h,l)=>{uint p;GetWindowThreadProcessId(h,out p);if(p==pid&&IsWindowVisible(h)){RECT r;GetClientRect(h,out r);if(r.R*r.B>best.R*best.B){best=r;found=h;}}return true;},IntPtr.Zero);return found;}
}
"@
[void][Cap]::SetProcessDpiAwarenessContext([IntPtr](-4))
if (Get-Process java, javaw -ErrorAction SilentlyContinue) { throw 'Java is running; refusing to open test windows.' }
$portrait = [System.Windows.Forms.Screen]::AllScreens | Where-Object { -not $_.Primary -and $_.Bounds.Height -gt $_.Bounds.Width } | Select-Object -First 1
if (-not $portrait) { throw 'A non-primary portrait display is required.' }
New-Item -ItemType Directory -Force -Path $Out | Out-Null
# Run a private copy so concurrent `cargo build`s can still replace the exe.
$runExe = Join-Path $env:TEMP ("aurora-cap-" + [guid]::NewGuid().ToString('N') + ".exe")
Copy-Item -LiteralPath $Exe -Destination $runExe
$seedScript = Join-Path $PSScriptRoot 'seed.py'
foreach ($entry in $Pages.Split(',')) {
  if (Get-Process java, javaw -ErrorAction SilentlyContinue) { throw 'Java is running; refusing to open test windows.' }
  # name:page[@scrollY]
  $name, $target = $entry.Split(':')
  $page, $scroll = $target.Split('@')
  if ($scroll) { $env:AURORA_SHOWCASE_SCROLL = $scroll } else { Remove-Item Env:AURORA_SHOWCASE_SCROLL -ErrorAction SilentlyContinue }
  $root = Join-Path $env:TEMP ("Aurora-" + [guid]::NewGuid().ToString("N").Substring(0,8))
  & python -I $seedScript $root | Out-Null
  $env:AURORA_SHOWCASE_PAGE = $page
  $p = Start-Process -FilePath $runExe -ArgumentList @('--showcase', $root) -WindowStyle Hidden -PassThru -RedirectStandardError "$Out\$name.stderr.txt" -RedirectStandardOutput "$Out\$name.stdout.txt"
  Start-Sleep -Seconds $Settle
  $h = [Cap]::Find([uint32]$p.Id)
  if ($h -eq [IntPtr]::Zero) { Stop-Process -Id $p.Id -Force; throw "no window for $name" }
  $wr = New-Object Cap+RECT; [void][Cap]::GetWindowRect($h, [ref]$wr)
  if ($wr.L -lt $portrait.Bounds.Left -or $wr.R -gt $portrait.Bounds.Right -or $wr.T -lt $portrait.Bounds.Top -or $wr.B -gt $portrait.Bounds.Bottom) { Stop-Process -Id $p.Id -Force; throw "window for $name is not on the portrait display ($($wr.L),$($wr.T))" }
  $r = New-Object Cap+RECT; [void][Cap]::GetClientRect($h, [ref]$r)
  $bmp = New-Object System.Drawing.Bitmap $r.R, $r.B
  $g = [System.Drawing.Graphics]::FromImage($bmp); $dc = $g.GetHdc()
  [void][Cap]::PrintWindow($h, $dc, 2); $g.ReleaseHdc($dc); $g.Dispose()
  $bmp.Save("$Out\$name.png"); $bmp.Dispose()
  if ($KeepOpen) { "PID=$($p.Id) EXE=$runExe ROOT=$root"; return }
  $dpi = [Cap]::GetDpiForWindow($h)
  [Cap]::Send($h, 0x10, 0, 0); if (-not $p.WaitForExit(6000)) { Stop-Process -Id $p.Id -Force }
  # Keep the small disposable fixture for reproducible review.
  "$name ok $($r.R)x$($r.B) dpi=$dpi"
}
Remove-Item -Force $runExe -ErrorAction SilentlyContinue
