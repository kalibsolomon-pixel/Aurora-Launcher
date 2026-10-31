<#
.SYNOPSIS
  Memory-regression and interaction scenario for the native launcher, driven
  through UI Automation: idle sample, 30 navigation cycles, 20 dialog
  open/close cycles, a Discover search, then a final idle sample. Reports the
  exact process's working set / private bytes / CPU at each step and the
  wall time of each UIA-driven navigation (an upper bound on UI latency that
  includes cross-process UIA overhead).
#>
param([Parameter(Mandatory)] [string] $Exe, [Parameter(Mandatory)] [string] $Root, [Parameter(Mandatory)] [string] $Out)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]
$env:AURORA_DIAGNOSTIC_DATA_ROOT = $Root
$p = Start-Process -FilePath $Exe -PassThru
Remove-Item Env:\AURORA_DIAGNOSTIC_DATA_ROOT
Start-Sleep -Seconds 6
$cond = New-Object System.Windows.Automation.PropertyCondition($A::ProcessIdProperty, $p.Id)
$window = $A::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
function Find([string] $name) {
    $c = New-Object System.Windows.Automation.PropertyCondition($A::NameProperty, $name)
    $window.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $c)
}
function Invoke([string] $name) {
    $e = Find $name
    if (-not $e) { throw "missing $name" }
    $e.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
}
function Sample([string] $step) {
    $p.Refresh()
    [pscustomobject]@{ step = $step; workingSetMiB = [math]::Round($p.WorkingSet64 / 1MB, 1); privateMiB = [math]::Round($p.PrivateMemorySize64 / 1MB, 1); cpuSeconds = [math]::Round($p.TotalProcessorTime.TotalSeconds, 2); handles = $p.HandleCount }
}
$samples = @(Sample 'idle-start')
$nav = @()
for ($i = 0; $i -lt 30; $i++) {
    foreach ($page in 'Instances', 'Discover', 'Settings', 'Home') {
        $sw = [System.Diagnostics.Stopwatch]::StartNew(); Invoke $page; $sw.Stop(); $nav += $sw.Elapsed.TotalMilliseconds
        Start-Sleep -Milliseconds 60
    }
}
$samples += Sample 'after-120-navigations'
Invoke 'Settings'; Start-Sleep -Milliseconds 300
for ($i = 0; $i -lt 20; $i++) {
    Invoke 'Manage accounts'; Start-Sleep -Milliseconds 150
    Invoke 'Close'; Start-Sleep -Milliseconds 150
}
$samples += Sample 'after-20-dialogs'
Invoke 'Discover'; Start-Sleep -Seconds 1
$search = Find 'Search Modrinth'
# Discover runs its first (default, most-downloaded) search when shown.
Start-Sleep -Seconds 6
$samples += Sample 'discover-results'
try {
    if ($search) { $search.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('sodium') }
} catch { }
Invoke 'Home'; Start-Sleep -Seconds 10
$before = Sample 'idle-end-start'
Start-Sleep -Seconds 20
$after = Sample 'idle-end'
$samples += $before, $after
$sorted = $nav | Sort-Object
$result = [ordered]@{
    exe = $Exe; capturedAt = (Get-Date).ToString('o'); samples = $samples
    navigationMs = [ordered]@{ count = $nav.Count; p50 = [math]::Round($sorted[[int]($nav.Count * 0.5)], 1); p95 = [math]::Round($sorted[[int]($nav.Count * 0.95)], 1); max = [math]::Round($sorted[-1], 1) }
    finalIdleCpuPercent = [math]::Round(($after.cpuSeconds - $before.cpuSeconds) / 20 * 100, 3)
}
$result | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8 $Out
& (Join-Path $PSScriptRoot 'close-window.ps1') -ProcessId $p.Id
$samples | Format-Table | Out-String
"navigation p50 $($result.navigationMs.p50) ms p95 $($result.navigationMs.p95) ms; final idle cpu $($result.finalIdleCpuPercent)%"
