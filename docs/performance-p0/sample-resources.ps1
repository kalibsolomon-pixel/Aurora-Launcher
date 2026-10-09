param([Parameter(Mandatory=$true)][int]$LauncherPid,
      [Parameter(Mandatory=$true)][string]$OutputFile,
      [int]$Samples=6,[int]$IntervalSeconds=10)
# Enumerate descendants by parent identity only. Never read command lines/window titles.
function Tree {
    $rows = Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name
    $ids = @($LauncherPid)
    do {
        $next = @($rows | Where-Object { $ids -contains $_.ParentProcessId -and $ids -notcontains $_.ProcessId } | ForEach-Object { [int]$_.ProcessId })
        $ids += $next
    } while ($next.Count)
    return ,$ids
}
function Snapshot {
    $snap = @{}
    foreach ($p in Get-Process -Id (Tree) -ErrorAction SilentlyContinue) {
        # Minecraft is reported separately; it is never called launcher idle cost.
        if ($p.ProcessName -notin @('aurora-launcher','msedgewebview2')) { continue }
        $snap[$p.Id] = @{ cpu=$p.TotalProcessorTime.TotalSeconds; ws=$p.WorkingSet64;
            private=$p.PrivateMemorySize64; handles=$p.HandleCount; threads=$p.Threads.Count;
            kind=$p.ProcessName; start=$p.StartTime.ToUniversalTime().Ticks }
    }
    return $snap
}
$machine = Get-CimInstance Win32_ComputerSystem
$cpu = (Get-CimInstance Win32_Processor).Name
$first = Snapshot
for ($trial=0; $trial -lt $Samples; $trial++) {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    Start-Sleep -Seconds $IntervalSeconds
    $last = Snapshot
    $watch.Stop()
    $stable = $first.Count -eq $last.Count
    $cpuSeconds=0; $ws=0; $private=0; $handles=0; $threads=0
    foreach ($id in $last.Keys) {
        $now = $last[$id]
        if ($first.ContainsKey($id) -and $first[$id].start -eq $now.start) { $cpuSeconds += $now.cpu-$first[$id].cpu }
        else { $stable=$false }
        $ws+=$now.ws; $private+=$now.private; $handles+=$now.handles; $threads+=$now.threads
    }
    $row = [ordered]@{ sample=$trial; seconds=$watch.Elapsed.TotalSeconds;
        cpuOneCorePercent=100*$cpuSeconds/$watch.Elapsed.TotalSeconds;
        stableProcessTree=$stable; processes=$last.Count; workingSetMiB=$ws/1MB;
        privateMiB=$private/1MB; handles=$handles; threads=$threads;
        logicalProcessors=$machine.NumberOfLogicalProcessors; ramGiB=$machine.TotalPhysicalMemory/1GB;
        cpu=$cpu; backgroundMinecraftProcesses=@(Get-Process javaw,java -ErrorAction SilentlyContinue).Count }
    $row | ConvertTo-Json -Compress | Add-Content -LiteralPath $OutputFile -Encoding utf8
    $first=$last
}
