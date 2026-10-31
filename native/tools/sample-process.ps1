<# Read-only resource sample for an exact acceptance process. No window input. #>
param([Parameter(Mandatory)][int]$ProcessId,[Parameter(Mandatory)][string]$Label,[Parameter(Mandatory)][string]$Out,[int]$Seconds=12)
$ErrorActionPreference='Stop'
$process=[System.Diagnostics.Process]::GetProcessById($ProcessId)
$identity=$process.StartTime
$start=$process.TotalProcessorTime.TotalSeconds
$clock=[System.Diagnostics.Stopwatch]::StartNew()
$samples=@()
while($clock.Elapsed.TotalSeconds -lt $Seconds){
    $process.Refresh()
    if($process.HasExited -or $process.StartTime -ne $identity){throw 'Acceptance process changed'}
    $samples += [pscustomobject]@{workingSetMiB=$process.WorkingSet64/1MB;privateMiB=$process.PrivateMemorySize64/1MB;handles=$process.HandleCount;cpuSeconds=$process.TotalProcessorTime.TotalSeconds}
    Start-Sleep -Milliseconds 250
}
$process.Refresh()
$elapsed=$clock.Elapsed.TotalSeconds
$endCpu=$process.TotalProcessorTime.TotalSeconds
$gpu=[pscustomobject]@{available=$false;dedicatedBytes=$null;sharedBytes=$null;utilizationPercent=$null}
try {
    $memory=(Get-Counter '\GPU Process Memory(*)\Dedicated Usage','\GPU Process Memory(*)\Shared Usage' -ErrorAction Stop).CounterSamples | Where-Object {$_.InstanceName -like "pid_$($ProcessId)_*"}
    $engines=(Get-Counter '\GPU Engine(*)\Utilization Percentage' -ErrorAction Stop).CounterSamples | Where-Object {$_.InstanceName -like "pid_$($ProcessId)_*"}
    $gpu=[pscustomobject]@{available=$true;dedicatedBytes=($memory|Where-Object {$_.Path -like '*dedicated usage'}|Measure-Object CookedValue -Sum).Sum;sharedBytes=($memory|Where-Object {$_.Path -like '*shared usage'}|Measure-Object CookedValue -Sum).Sum;utilizationPercent=($engines|Measure-Object CookedValue -Sum).Sum}
}catch{}
$result=[pscustomobject]@{label=$Label;pid=$ProcessId;seconds=$elapsed;cpuPercentOneCore=100*($endCpu-$start)/$elapsed;workingSetMiB=($samples|Measure-Object workingSetMiB -Average).Average;privateMiB=($samples|Measure-Object privateMiB -Average).Average;handles=$process.HandleCount;gpu=$gpu;samples=$samples}
$result|ConvertTo-Json -Depth 5|Set-Content -LiteralPath $Out
$result|Select-Object label,pid,cpuPercentOneCore,workingSetMiB,privateMiB,gpu|ConvertTo-Json -Depth 3
