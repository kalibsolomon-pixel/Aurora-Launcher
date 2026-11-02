param([ValidateSet('size','maximize','restore','close')][string]$Mode='size',[int]$Width=1024,[int]$Height=768)
$ErrorActionPreference='Stop'
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$start=Get-Content -LiteralPath (Join-Path $repo 'docs/ui-2b-1/native/start.json') | ConvertFrom-Json
$expected=Join-Path $repo 'src-tauri/target/debug/aurora-launcher.exe'
$process=Get-Process -Id $start.pid
if($process.Path -ne $expected -or $start.exe -ne $expected -or !(Test-Path -LiteralPath (Join-Path $start.isolatedRoot 'UI2B-ISOLATED.txt')) -or (Split-Path $start.isolatedRoot -Leaf) -notlike 'aurora-ui2b1-*'){throw 'Expected exact isolated acceptance process'}
Add-Type @'
using System;using System.Runtime.InteropServices;
public class UI2B1Window {
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int L,T,R,B;}
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int height,uint flags);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int command);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
}
'@
[UI2B1Window]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
$handle=$process.MainWindowHandle
if($handle -eq [IntPtr]::Zero){throw 'No native window handle'}
$outer=New-Object UI2B1Window+Rect;$client=New-Object UI2B1Window+Rect
[UI2B1Window]::GetWindowRect($handle,[ref]$outer) | Out-Null
[UI2B1Window]::GetClientRect($handle,[ref]$client) | Out-Null
$scale=[UI2B1Window]::GetDpiForWindow($handle)/96
switch($Mode){
 'maximize' {[UI2B1Window]::ShowWindow($handle,3) | Out-Null}
 'restore' {[UI2B1Window]::ShowWindow($handle,9) | Out-Null}
 'close' {[UI2B1Window]::PostMessage($handle,0x10,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null}
 'size' {
  if($Width -lt 720 -or $Height -lt 520){throw 'Do not bypass configured minimum'}
  $w=[int][Math]::Round($Width*$scale)+($outer.R-$outer.L)-($client.R-$client.L)
  $h=[int][Math]::Round($Height*$scale)+($outer.B-$outer.T)-($client.B-$client.T)
  if(![UI2B1Window]::SetWindowPos($handle,[IntPtr]::Zero,0,0,$w,$h,0x16)){throw 'SetWindowPos failed'}
 }
}
@{pid=$process.Id;mode=$Mode;requestedCss=@($Width,$Height);dpiScale=$scale;windowBefore=$outer;clientBefore=$client} | ConvertTo-Json
