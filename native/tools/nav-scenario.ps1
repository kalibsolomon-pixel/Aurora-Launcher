<#
.SYNOPSIS
  Host-agnostic navigation scenario for either launcher build: idle sample,
  30 cycles through the given navigation destinations (UI Automation Invoke
  on the first invokable element with that name), then a 30 s settle and a
  20 s idle sample. Memory/CPU cover the exact spawned process tree.
#>
param(
    [Parameter(Mandatory)] [string] $Exe,
    [Parameter(Mandatory)] [string[]] $Pages,
    [Parameter(Mandatory)] [string] $Out,
    [hashtable] $Environment = @{}
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]
foreach ($k in $Environment.Keys) { Set-Item "Env:\$k" $Environment[$k] }
$p = Start-Process -FilePath $Exe -PassThru
foreach ($k in $Environment.Keys) { Remove-Item "Env:\$k" }
Start-Sleep -Seconds 10

function Tree {
    $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId
    $set = New-Object System.Collections.Generic.HashSet[int]; [void]$set.Add($p.Id)
    $changed = $true
    while ($changed) { $changed = $false; foreach ($x in $all) { if ($set.Contains([int]$x.ParentProcessId) -and -not $set.Contains([int]$x.ProcessId)) { [void]$set.Add([int]$x.ProcessId); $changed = $true } } }
    @($set)
}
function Sample([string] $step) {
    $ws = 0; $pv = 0; $cpu = 0.0
    foreach ($id in Tree) { try { $q = [System.Diagnostics.Process]::GetProcessById($id); $ws += $q.WorkingSet64; $pv += $q.PrivateMemorySize64; $cpu += $q.TotalProcessorTime.TotalSeconds } catch { } }
    [pscustomobject]@{ step = $step; workingSetMiB = [math]::Round($ws / 1MB, 1); privateMiB = [math]::Round($pv / 1MB, 1); cpuSeconds = [math]::Round($cpu, 2) }
}
$cond = New-Object System.Windows.Automation.PropertyCondition($A::ProcessIdProperty, $p.Id)
$window = $A::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
function Invoke([string] $name) {
    $c = New-Object System.Windows.Automation.PropertyCondition($A::NameProperty, $name)
    foreach ($e in $window.FindAll([System.Windows.Automation.TreeScope]::Descendants, $c)) {
        $pattern = $null
        if ($e.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) { $pattern.Invoke(); return }
    }
    throw "no invokable '$name'"
}
$samples = @(Sample 'idle-start')
for ($i = 0; $i -lt 30; $i++) { foreach ($page in $Pages) { Invoke $page; Start-Sleep -Milliseconds 150 } }
$samples += Sample 'after-navigation'
Invoke $Pages[0]
Start-Sleep -Seconds 30
$before = Sample 'settled'
Start-Sleep -Seconds 20
$after = Sample 'idle-end'
$samples += $before, $after
$result = [ordered]@{ exe = $Exe; pages = $Pages; capturedAt = (Get-Date).ToString('o'); processCount = @(Tree).Count; samples = $samples; finalIdleCpuPercent = [math]::Round(($after.cpuSeconds - $before.cpuSeconds) / 20 * 100, 3) }
$result | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8 $Out
& (Join-Path $PSScriptRoot 'close-window.ps1') -ProcessId $p.Id | Out-Null
$samples | Format-Table | Out-String
"final idle cpu $($result.finalIdleCpuPercent)% processes $($result.processCount)"
