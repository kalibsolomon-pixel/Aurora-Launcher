<#
.SYNOPSIS
  NATIVE initiative resource probe. Starts one executable, then measures the
  exact spawned process tree (never processes found by name): time to first
  visible window, time to CPU quiescence, idle CPU/memory/GPU, and minimized
  overhead. Writes one JSON document.

  The launched process tree is closed through its main window and only the
  recorded PIDs are ever terminated.
#>
param(
    [Parameter(Mandatory)] [string] $Exe,
    [string[]] $Arguments = @(),
    [Parameter(Mandatory)] [string] $Label,
    [Parameter(Mandatory)] [string] $Out,
    [int] $SettleSeconds = 8,
    [int] $IdleSeconds = 20,
    [int] $MinimizedSeconds = 10,
    [hashtable] $Environment = @{}
)

$ErrorActionPreference = 'Stop'
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class NativeWin {
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
  public static IntPtr FindTopLevel(uint pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p == pid && IsWindowVisible(h) && GetWindow(h, 4) == IntPtr.Zero) { found = h; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
"@

function Get-Tree([int] $RootPid) {
    $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name, CreationDate
    $set = New-Object System.Collections.Generic.HashSet[int]
    [void]$set.Add($RootPid)
    $changed = $true
    while ($changed) {
        $changed = $false
        foreach ($p in $all) {
            if ($set.Contains([int]$p.ParentProcessId) -and -not $set.Contains([int]$p.ProcessId)) {
                [void]$set.Add([int]$p.ProcessId); $changed = $true
            }
        }
    }
    return @($set)
}

function Get-Snapshot([int[]] $Pids) {
    $rows = @()
    foreach ($id in $Pids) {
        try {
            $p = [System.Diagnostics.Process]::GetProcessById($id)
            $p.Refresh()
            $rows += [pscustomobject]@{
                pid = $id; name = $p.ProcessName
                workingSet = $p.WorkingSet64; privateBytes = $p.PrivateMemorySize64
                cpu = $p.TotalProcessorTime.TotalSeconds; peakWorkingSet = $p.PeakWorkingSet64
                handles = $p.HandleCount; threads = $p.Threads.Count
            }
        } catch { }
    }
    return $rows
}

function Get-Gpu([int[]] $Pids) {
    $util = 0.0; $dedicated = 0.0; $shared = 0.0; $ok = $true
    try {
        $engine = (Get-Counter '\GPU Engine(*)\Utilization Percentage' -ErrorAction Stop).CounterSamples
        $mem = (Get-Counter @('\GPU Process Memory(*)\Dedicated Usage', '\GPU Process Memory(*)\Shared Usage') -ErrorAction Stop).CounterSamples
        foreach ($id in $Pids) {
            $tag = "pid_$($id)_"
            $util += ($engine | Where-Object { $_.InstanceName -like "$tag*" } | Measure-Object CookedValue -Sum).Sum
            $dedicated += ($mem | Where-Object { $_.InstanceName -like "$tag*" -and $_.Path -like '*dedicated usage' } | Measure-Object CookedValue -Sum).Sum
            $shared += ($mem | Where-Object { $_.InstanceName -like "$tag*" -and $_.Path -like '*shared usage' } | Measure-Object CookedValue -Sum).Sum
        }
    } catch { $ok = $false }
    return [pscustomobject]@{ available = $ok; utilizationPercent = $util; dedicatedBytes = $dedicated; sharedBytes = $shared }
}

function Sum($rows, $field) { ($rows | Measure-Object -Property $field -Sum).Sum }

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = (Resolve-Path $Exe).Path
# Windows PowerShell 5.1 lacks ArgumentList; probe arguments never contain spaces or quotes.
foreach ($a in $Arguments) { if ($a -match '[\s"]') { throw "unsupported probe argument: $a" } }
$psi.Arguments = ($Arguments -join ' ')
$psi.UseShellExecute = $false
$psi.WorkingDirectory = Split-Path $psi.FileName
foreach ($k in $Environment.Keys) { $psi.Environment[$k] = [string]$Environment[$k] }

$clock = [System.Diagnostics.Stopwatch]::StartNew()
$proc = [System.Diagnostics.Process]::Start($psi)
$rootPid = $proc.Id
$window = [IntPtr]::Zero
while ($clock.ElapsedMilliseconds -lt 30000) {
    $window = [NativeWin]::FindTopLevel([uint32]$rootPid)
    if ($window -ne [IntPtr]::Zero) { break }
    Start-Sleep -Milliseconds 10
}
$windowMs = $clock.Elapsed.TotalMilliseconds

# Quiescence: the whole tree used < 20 ms CPU over a 500 ms window.
$lastCpu = -1.0; $quietMs = $null
while ($clock.ElapsedMilliseconds -lt 30000) {
    $cpu = Sum (Get-Snapshot (Get-Tree $rootPid)) 'cpu'
    if ($lastCpu -ge 0 -and ($cpu - $lastCpu) -lt 0.020) { $quietMs = $clock.Elapsed.TotalMilliseconds - 500; break }
    $lastCpu = $cpu
    Start-Sleep -Milliseconds 500
}

Start-Sleep -Seconds $SettleSeconds
$tree = Get-Tree $rootPid
$before = Get-Snapshot $tree
$gpuSamples = @()
$t0 = [System.Diagnostics.Stopwatch]::StartNew()
while ($t0.Elapsed.TotalSeconds -lt $IdleSeconds) { $gpuSamples += Get-Gpu $tree; Start-Sleep -Milliseconds 1500 }
$idleElapsed = $t0.Elapsed.TotalSeconds
$tree = Get-Tree $rootPid
$after = Get-Snapshot $tree

[void][NativeWin]::ShowWindow($window, 6)  # SW_MINIMIZE
Start-Sleep -Seconds 2
$minBefore = Get-Snapshot $tree
$t1 = [System.Diagnostics.Stopwatch]::StartNew()
$minGpu = @()
while ($t1.Elapsed.TotalSeconds -lt $MinimizedSeconds) { $minGpu += Get-Gpu $tree; Start-Sleep -Milliseconds 1500 }
$minElapsed = $t1.Elapsed.TotalSeconds
$minAfter = Get-Snapshot $tree
[void][NativeWin]::ShowWindow($window, 9)  # SW_RESTORE

$result = [ordered]@{
    label = $Label; exe = $psi.FileName; arguments = $Arguments
    capturedAt = (Get-Date).ToString('o')
    processCount = @($after).Count
    processes = $after
    windowVisibleMs = [math]::Round($windowMs, 1)
    quiescentMs = if ($quietMs) { [math]::Round($quietMs, 1) } else { $null }
    idle = [ordered]@{
        seconds = [math]::Round($idleElapsed, 2)
        workingSetMiB = [math]::Round((Sum $after 'workingSet') / 1MB, 1)
        privateMiB = [math]::Round((Sum $after 'privateBytes') / 1MB, 1)
        cpuPercentOfOneCore = [math]::Round(((Sum $after 'cpu') - (Sum $before 'cpu')) / $idleElapsed * 100, 3)
        gpuUtilizationPercentMean = [math]::Round((($gpuSamples | Where-Object available | Measure-Object utilizationPercent -Average).Average), 3)
        gpuDedicatedMiB = [math]::Round((($gpuSamples | Measure-Object dedicatedBytes -Maximum).Maximum) / 1MB, 1)
        gpuSharedMiB = [math]::Round((($gpuSamples | Measure-Object sharedBytes -Maximum).Maximum) / 1MB, 1)
    }
    minimized = [ordered]@{
        seconds = [math]::Round($minElapsed, 2)
        workingSetMiB = [math]::Round((Sum $minAfter 'workingSet') / 1MB, 1)
        privateMiB = [math]::Round((Sum $minAfter 'privateBytes') / 1MB, 1)
        cpuPercentOfOneCore = [math]::Round(((Sum $minAfter 'cpu') - (Sum $minBefore 'cpu')) / $minElapsed * 100, 3)
        gpuUtilizationPercentMean = [math]::Round((($minGpu | Where-Object available | Measure-Object utilizationPercent -Average).Average), 3)
    }
}

# Graceful close of the measured window, then terminate only recorded PIDs.
[void][NativeWin]::PostMessage($window, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)  # WM_CLOSE
if (-not $proc.WaitForExit(8000)) {
    foreach ($id in $tree) { try { Stop-Process -Id $id -Force -ErrorAction Stop } catch { } }
}
$result | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $Out
"{0}: window {1} ms, quiescent {2} ms, idle WS {3} MiB, private {4} MiB, cpu {5}%, procs {6}, min cpu {7}%" -f $Label, $result.windowVisibleMs, $result.quiescentMs, $result.idle.workingSetMiB, $result.idle.privateMiB, $result.idle.cpuPercentOfOneCore, $result.processCount, $result.minimized.cpuPercentOfOneCore
