<# NATIVE-8 exact-process benchmark. Requires an acceptance release build and
   an explicitly disposable populated root. Changes only that root's config,
   restoring its original bytes in finally. No account or media interactions.
   CPU is percent of ONE logical processor. Captures are lossless PrintWindow.
   Run after verification work settles. Requires PowerShell 7 on Windows. #>
param([Parameter(Mandatory)][string]$Exe,[Parameter(Mandatory)][string]$DataRoot,
 [Parameter(Mandatory)][string]$Out,[Parameter(Mandatory)][string]$FocusSink,[int]$Runs=3,[int]$Seconds=10)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Collections.Generic; using System.Runtime.InteropServices;
public static class N8Window {
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc cb, IntPtr l);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h,uint cmd);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int c);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out RECT r);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint f);
 [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr c);
 [StructLayout(LayoutKind.Sequential)] public struct RECT {public int L,T,R,B;}
 public static IntPtr Find(uint process) {return FindSized(process,720,520);}
 public static IntPtr FindSized(uint process,int minWidth,int minHeight) {
  var all=new List<IntPtr>();
  EnumWindows((h,l)=>{uint p;GetWindowThreadProcessId(h,out p);
   if(p==process && IsWindowVisible(h) && GetWindow(h,4)==IntPtr.Zero) {
    RECT r;GetWindowRect(h,out r); if(r.R-r.L>=minWidth && r.B-r.T>=minHeight) all.Add(h);
   } return true;},IntPtr.Zero);
  if(all.Count>1) throw new Exception("Ambiguous application HWND");
  return all.Count==1 ? all[0] : IntPtr.Zero;
 }
}
"@
[void][N8Window]::SetProcessDpiAwarenessContext([IntPtr](-4))
$exePath=(Resolve-Path $Exe).Path
$config=Join-Path $DataRoot 'launcher/config.json'
if(!(Test-Path (Join-Path (Split-Path $DataRoot) 'NATIVE8-DISPOSABLE.txt'))){throw 'Disposable marker required'}
$original=[IO.File]::ReadAllBytes($config)
$doc=[Text.Encoding]::UTF8.GetString($original) | ConvertFrom-Json
New-Item -ItemType Directory -Force $Out | Out-Null
$rows=[Collections.Generic.List[object]]::new()
$logicalProcessors=[int](Get-CimInstance Win32_ComputerSystem).NumberOfLogicalProcessors
if($logicalProcessors -lt 1){throw 'Host logical processor count unavailable'}
function Require-Foreground($h,$p,$phase,$title='Aurora Launcher') {
 [void][N8Window]::SetForegroundWindow($h)
 if([N8Window]::GetForegroundWindow() -eq $h){return}
 # Windows correctly refuses foreground stealing by a background script.
 # The acceptance operator/helper activates this exact returned app window.
 $request=Join-Path $Out 'activation-request.json'
 if(Test-Path -LiteralPath $request){throw 'A previous activation request already exists'}
 [pscustomobject]@{pid=$p.Id;createdUtc=$p.StartTime.ToUniversalTime().ToString('O');hwnd=$h.ToInt64();phase=$phase;title=$title} | ConvertTo-Json | Set-Content -Encoding utf8 $request
 Write-Output "Activation required: PID $($p.Id), $phase"
 $wait=[Diagnostics.Stopwatch]::StartNew()
 while([N8Window]::GetForegroundWindow() -ne $h){
  if($wait.Elapsed.TotalSeconds -gt 120 -or $p.HasExited){throw 'Foreground activation was not established'}
  Start-Sleep -Milliseconds 100
 }
 # Exact transient file created by this probe, beneath its output root.
 Remove-Item -LiteralPath $request
}
function State($h,$p) {
 [uint32]$owner=0;[void][N8Window]::GetWindowThreadProcessId($h,[ref]$owner)
 if($owner -ne $p.Id -or ![N8Window]::IsWindow($h)){throw 'Stale HWND'}
 [pscustomobject]@{hwnd=$h.ToInt64();owner=$owner;visible=[N8Window]::IsWindowVisible($h);minimized=[N8Window]::IsIconic($h);foreground=([N8Window]::GetForegroundWindow() -eq $h)}
}
function Capture($h,$name) {
 $r=[N8Window+RECT]::new();[void][N8Window]::GetWindowRect($h,[ref]$r)
 $bmp=[Drawing.Bitmap]::new($r.R-$r.L,$r.B-$r.T);$g=[Drawing.Graphics]::FromImage($bmp);$dc=$g.GetHdc()
 try {if(![N8Window]::PrintWindow($h,$dc,2)){throw 'PrintWindow failed'}} finally {$g.ReleaseHdc($dc);$g.Dispose()}
 $bmp.Save((Join-Path $Out "$name.png"),[Drawing.Imaging.ImageFormat]::Png);$bmp.Dispose()
}
function Sample($p,$h,$label,$clock) {
 $identity=$p.StartTime.ToUniversalTime().ToString('O');$p.Refresh();$cpu=$p.TotalProcessorTime.TotalSeconds
 $begin=$clock.Elapsed.TotalMilliseconds;$watch=[Diagnostics.Stopwatch]::StartNew();$samples=@()
 while($watch.Elapsed.TotalSeconds -lt $Seconds){
  $p.Refresh(); if($p.HasExited -or $p.StartTime.ToUniversalTime().ToString('O') -ne $identity){throw 'Process identity changed'}
  $samples += [pscustomobject]@{ms=$clock.Elapsed.TotalMilliseconds;ws=$p.WorkingSet64;private=$p.PrivateMemorySize64;state=(State $h $p)}
  Start-Sleep -Milliseconds 250
 }
 $p.Refresh();$duration=$watch.Elapsed.TotalSeconds;$delta=$p.TotalProcessorTime.TotalSeconds-$cpu
 $children=@(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($p.Id)" | Select-Object ProcessId,Name)
 if($children.Count){throw 'Unexpected child processes: inspect and account before accepting'}
 [pscustomobject]@{scenario=$label;pid=$p.Id;createdUtc=$identity;beginMs=$begin;endMs=$clock.Elapsed.TotalMilliseconds;seconds=$duration;cpuSeconds=$delta;cpuPercentOneCore=100*$delta/$duration;cpuPercentMachine=100*$delta/$duration/$logicalProcessors;logicalProcessors=$logicalProcessors;workingSetMiB=($samples|Measure-Object ws -Average).Average/1MB;privateMiB=($samples|Measure-Object private -Average).Average/1MB;children=$children;samples=$samples}
}
try {
 foreach($run in 1..$Runs){
  foreach($mode in @('simple','oled','borealis','reduced','forced')){
   $doc.appearance.background=if($mode -in @('simple','oled')){'simple'}else{'borealis'}
   $doc.appearance.theme=if($mode -eq 'oled'){'oled'}else{'aurora-dark'}
   $doc.appearance.auroraMotionSpeed=if($mode -eq 'reduced'){0}else{50}
   foreach($w in $doc.homeWidgets.widgets){if($w.id -eq 'media'){$w.enabled=$false}}
   [IO.File]::WriteAllText($config,($doc|ConvertTo-Json -Depth 20)+"`n",[Text.UTF8Encoding]::new($false))
   $psi=[Diagnostics.ProcessStartInfo]::new($exePath);$psi.UseShellExecute=$false;$psi.RedirectStandardError=$true
   $psi.ArgumentList.Add('--diagnostic-size=1120x760')
   $psi.Environment['AURORA_DIAGNOSTIC_DATA_ROOT']=$DataRoot
   $psi.Environment['AURORA_DIAGNOSTIC_BOREALIS_TRACE']='1'
   $psi.Environment.Remove('AURORA_DIAGNOSTIC_BOREALIS_ACTIVE')|Out-Null
   if($mode -eq 'forced'){$psi.Environment['AURORA_DIAGNOSTIC_BOREALIS_ACTIVE']='1'}
   $clock=[Diagnostics.Stopwatch]::StartNew();$p=[Diagnostics.Process]::Start($psi);$log=$p.StandardError.ReadToEndAsync()
   try {
    $h=[IntPtr]::Zero
    while($clock.Elapsed.TotalSeconds -lt 20 -and $h -eq [IntPtr]::Zero){$h=[N8Window]::Find([uint32]$p.Id);Start-Sleep -Milliseconds 10}
    if($h -eq [IntPtr]::Zero){throw 'No application window'}
    $visibleMs=$clock.Elapsed.TotalMilliseconds
    if($mode -ne 'forced'){Require-Foreground $h $p "$mode-$run-active"}
    Start-Sleep -Seconds 6
    $label="$mode-$run"
    if($run -eq 1){Capture $h "$mode-home"}
    $row=Sample $p $h "$mode-active" $clock;$row|Add-Member -NotePropertyName windowCreatedVisibleMs -NotePropertyValue $visibleMs
    if($mode -eq 'borealis' -and @($row.samples|Where-Object {!$_.state.foreground}).Count){$rows.Add($row);throw 'Active window lost foreground'}
    $rows.Add($row)
    if($run -eq 1 -and $mode -eq 'borealis'){Capture $h 'borealis-active-after'}
    if($mode -in @('borealis','forced')){
     [void][N8Window]::ShowWindow($h,6);Start-Sleep -Seconds 1
     $rows.Add((Sample $p $h "$mode-minimized" $clock))
     [void][N8Window]::ShowWindow($h,9)
     if($mode -ne 'forced'){Require-Foreground $h $p "$mode-$run-restored"}
     Start-Sleep -Seconds 1
     $rows.Add((Sample $p $h "$mode-restored" $clock))
     if($mode -eq 'borealis'){
      # A neutral test-owned window receives focus. The Launcher remains
      # restored/visible; minimizing it would not establish unfocused behavior.
      $sinkInfo=[Diagnostics.ProcessStartInfo]::new((Resolve-Path $FocusSink).Path);$sinkInfo.UseShellExecute=$false
      $sink=[Diagnostics.Process]::Start($sinkInfo)
      $sinkWindow=[IntPtr]::Zero
      try {
       $limit=[Diagnostics.Stopwatch]::StartNew()
       while($sinkWindow -eq [IntPtr]::Zero -and $limit.Elapsed.TotalSeconds -lt 10){$sinkWindow=[N8Window]::FindSized([uint32]$sink.Id,200,50);Start-Sleep -Milliseconds 20}
       if($sinkWindow -eq [IntPtr]::Zero){throw 'Focus fixture window missing'}
       Require-Foreground $sinkWindow $sink "borealis-$run-unfocused" 'Aurora benchmark focus sink'
       Start-Sleep -Seconds 1
       $row=Sample $p $h 'borealis-unfocused' $clock
       $rows.Add($row)
       if(@($row.samples|Where-Object {$_.state.foreground -or $_.state.minimized -or !$_.state.visible}).Count){throw 'Invalid unfocused window state'}
       if($run -eq 1){Capture $h 'borealis-paused-before';Start-Sleep -Seconds 3;Capture $h 'borealis-paused-after'}
      } finally {
       [void][N8Window]::PostMessage($sinkWindow,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
       if(!$sink.WaitForExit(3000)){throw 'Focus fixture did not close'}
       $sink.Dispose()
      }
     }else{
      [void][N8Window]::ShowWindow($h,0);Start-Sleep -Seconds 1
      $rows.Add((Sample $p $h 'forced-hidden' $clock))
      [void][N8Window]::ShowWindow($h,4)
     }
    }
   } finally {
    [void][N8Window]::PostMessage($h,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
    if(!$p.WaitForExit(7000)){throw 'Application did not close cleanly'}
    $trace=$log.GetAwaiter().GetResult()
    [IO.File]::WriteAllText((Join-Path $Out "$mode-$run.trace.txt"),(($trace -split "`n" | Where-Object { $_.StartsWith("[borealis]") }) -join "`n"))
    $rows | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 (Join-Path $Out 'resources.json')
    $p.Dispose()
   }
  }
 }
} finally {[IO.File]::WriteAllBytes($config,$original)}
$rows | Select-Object scenario,pid,cpuPercentOneCore,workingSetMiB,privateMiB | Format-Table
