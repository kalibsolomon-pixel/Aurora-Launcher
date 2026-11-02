param([ValidateSet('size','minimum','close')][string]$Mode='size',[int]$Width=1120,[int]$Height=760,[ValidateSet('first','restart')][string]$Phase='first')
$ErrorActionPreference='Stop'
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$start=Get-Content -LiteralPath (Join-Path $repo ".git/performance-p2-native-$Phase.json") | ConvertFrom-Json
$process=Get-Process -Id $start.pid
if($process.Path -ne $start.exe -or (Split-Path $start.isolatedRoot -Leaf) -notmatch '^aurora-p2-[a-f0-9-]+$' -or !(Test-Path -LiteralPath (Join-Path $start.isolatedRoot 'P2-ISOLATED.txt'))){throw 'Expected exact isolated process'}
Add-Type @'
using System;using System.Runtime.InteropServices;
public class P2Window {
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int L,T,R,B;}
 [StructLayout(LayoutKind.Sequential)] public struct Point {public int X,Y;}
 [StructLayout(LayoutKind.Sequential)] public struct MinMax {public Point Reserved,MaxSize,MaxPosition,MinTrack,MaxTrack;}
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int height,uint flags);
 [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,ref MinMax l);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
}
'@
[P2Window]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
$handle=$process.MainWindowHandle
if($handle -eq [IntPtr]::Zero){throw 'No window handle'}
$outer=New-Object P2Window+Rect;$client=New-Object P2Window+Rect;$limits=New-Object P2Window+MinMax
[P2Window]::GetWindowRect($handle,[ref]$outer) | Out-Null
[P2Window]::GetClientRect($handle,[ref]$client) | Out-Null
[P2Window]::SendMessage($handle,0x24,[IntPtr]::Zero,[ref]$limits) | Out-Null
$scale=[P2Window]::GetDpiForWindow($handle)/96
if($Mode -eq 'close'){ [P2Window]::PostMessage($handle,0x10,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null }
else {
 if($Mode -eq 'minimum'){
  if($limits.MinTrack.X -lt 720*$scale -or $limits.MinTrack.Y -lt 520*$scale){throw 'Native minimum missing'}
  $w=$limits.MinTrack.X;$h=$limits.MinTrack.Y
 } else {
  if($Width -lt 720 -or $Height -lt 520){throw 'Cannot bypass minimum'}
  $w=[int][Math]::Round($Width*$scale)+($outer.R-$outer.L)-($client.R-$client.L)
  $h=[int][Math]::Round($Height*$scale)+($outer.B-$outer.T)-($client.B-$client.T)
 }
 if(![P2Window]::SetWindowPos($handle,[IntPtr]::Zero,0,0,$w,$h,0x16)){throw 'Resize failed'}
}
[P2Window]::GetWindowRect($handle,[ref]$outer) | Out-Null
[P2Window]::GetClientRect($handle,[ref]$client) | Out-Null
@{mode=$Mode;dpiScale=$scale;outer=$outer;client=$client;nativeMinimum=$limits.MinTrack} | ConvertTo-Json -Depth 4
