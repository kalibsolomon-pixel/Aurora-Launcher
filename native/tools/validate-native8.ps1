<# Validate numeric lifecycle traces and lossless contour pixels from native8-benchmark.ps1. #>
param([Parameter(Mandatory)][string]$Evidence)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
$drawingReferences=@([Drawing.Bitmap].Assembly.Location, [Drawing.Size].Assembly.Location)
# PowerShell on .NET 10 splits these Drawing contracts into private assemblies.
$drawingReferences += @(Get-ChildItem $PSHOME -Filter 'System.Private.Windows*.dll' | ForEach-Object FullName)
Add-Type -ReferencedAssemblies $drawingReferences -TypeDefinition @"
using System; using System.Drawing;
public static class N8Pixels {
 public static double[] Compare(string a,string b) {
  using(var x=new Bitmap(a)) using(var y=new Bitmap(b)) {
   if(x.Size!=y.Size) throw new Exception("Capture geometry changed");
   long changed=0,count=0,sum=0; int maximum=0;
   for(int j=x.Height-100;j<x.Height-20;j++) for(int i=150;i<x.Width-70;i++) {
    var p=x.GetPixel(i,j); var q=y.GetPixel(i,j);
    int d=Math.Abs(p.R-q.R)+Math.Abs(p.G-q.G)+Math.Abs(p.B-q.B);
    count++;sum+=d;if(d>0)changed++;maximum=Math.Max(maximum,d);
   }
   return new double[]{count,changed,(double)sum/count/3,maximum};
  }
 }
}
"@
$traces=@{}
Get-ChildItem $Evidence -Filter '*.trace.txt' | ForEach-Object {
 $events=@(Get-Content $_.FullName | Where-Object {$_ -like '[[]borealis[]]*'} | ForEach-Object {($_ -replace '^\[borealis\] ','') | ConvertFrom-Json})
 $traces[$_.BaseName.Replace('.trace','')]=$events
}
$rows=Get-Content (Join-Path $Evidence 'resources.json') -Raw | ConvertFrom-Json
$result=@()
$modeRuns=@{}
foreach($r in $rows){
 $mode=($r.scenario -split '-')[0]
 if($r.scenario -eq "$mode-active"){$modeRuns[$mode]=1+$modeRuns[$mode]}
 $index=$modeRuns[$mode]
 $events=$traces["$mode-$index"]
 if(!$events){throw "No trace for $mode-$index"}
 if(@($events|Where-Object {$_.event -eq 'home-state-loaded'}).Count -ne 1){throw 'Loaded Home state not established'}
 foreach($sample in $r.samples){
  $state=$sample.state
  if($r.scenario -like '*-minimized' -and !$state.minimized){throw 'Minimized sample was not minimized'}
  if($r.scenario -eq 'forced-hidden' -and $state.visible){throw 'Hidden sample was visible'}
  if($r.scenario -match '(borealis|forced)-(active|restored)$' -and (!$state.visible -or $state.minimized)){throw 'Moving sample was hidden/minimized'}
  if($r.scenario -match '^borealis-(active|restored)$' -and !$state.foreground){throw 'Ordinary active/restored sample lacked foreground'}
 }
 $ticks=@($events|Where-Object {$_.event -eq 'tick' -and $_.ms -gt ($r.beginMs+500) -and $_.ms -lt ($r.endMs-500)})
 $moving=$r.scenario -match '(borealis|forced)-(active|restored)$'
 if($moving -and $ticks.Count -lt [Math]::Max(2,[Math]::Floor(($r.seconds-2)*2))){throw "Animation missing: $($r.scenario)"}
 if(!$moving -and $ticks.Count -ne 0){throw "Unexpected animation: $($r.scenario)"}
 $intervals=@();for($i=1;$i -lt $ticks.Count;$i++){$intervals+=($ticks[$i].ms-$ticks[$i-1].ms)}
 if(@($intervals|Where-Object {$_ -lt 300}).Count){throw 'Duplicate/burst timer cadence'}
 $result += [pscustomobject]@{scenario=$r.scenario;pid=$r.pid;observedTicks=$ticks.Count;meanTickMs=if($intervals.Count){($intervals|Measure-Object -Average).Average}else{$null};cpuPercentOneCore=$r.cpuPercentOneCore;workingSetMiB=$r.workingSetMiB;privateMiB=$r.privateMiB}
}
foreach($key in $traces.Keys){
 $paused=$null
 foreach($e in $traces[$key]){
  if($e.event -eq 'paused'){$paused=$e}
  if($e.event -eq 'running' -and $paused){
   if($e.phaseMs -ne $paused.phaseMs -or $e.updates -ne $paused.updates){throw 'Phase advanced while paused'}
   $paused=$null
  }
 }
}
$active=[N8Pixels]::Compare((Join-Path $Evidence 'borealis-home.png'),(Join-Path $Evidence 'borealis-active-after.png'))
$pausedPixels=[N8Pixels]::Compare((Join-Path $Evidence 'borealis-paused-before.png'),(Join-Path $Evidence 'borealis-paused-after.png'))
if($active[1] -lt 100){throw 'Contour movement not proven'}
if($pausedPixels[1] -ne 0){throw 'Paused pixels changed'}
$output=[pscustomobject]@{region='x=150..width-70, y=height-100..height-20; contour only';active=[pscustomobject]@{pixels=$active[0];changed=$active[1];meanAbsoluteChannelDelta=$active[2];maxSummedRgbDelta=$active[3]};paused=[pscustomobject]@{pixels=$pausedPixels[0];changed=$pausedPixels[1]};scenarios=$result}
$output | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 (Join-Path $Evidence 'validated.json')
$output | ConvertTo-Json -Depth 6
