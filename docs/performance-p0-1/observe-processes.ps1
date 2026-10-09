param([Parameter(Mandatory=$true)][string]$OutputFile,
      [Parameter(Mandatory=$true)][string]$InstalledExecutable,
      [int]$DurationSeconds=1800)
# Read process identity and parentage only: never arguments, environment or output.
# Private local observations correlate GUI clocks with OS creation timestamps.
$seen = @{}
$clock = [Diagnostics.Stopwatch]::StartNew()
while ($clock.Elapsed.TotalSeconds -lt $DurationSeconds) {
    $parents = @(Get-CimInstance Win32_Process -Filter "Name='aurora-launcher.exe'" |
        Where-Object { $_.ExecutablePath -eq $InstalledExecutable })
    $children = @(Get-CimInstance Win32_Process -Filter "Name='javaw.exe' OR Name='java.exe'" |
        Where-Object { $_.ParentProcessId -in $parents.ProcessId })
    foreach ($process in @($parents) + @($children)) {
        $created = [DateTimeOffset]$process.CreationDate
        $key = "$($process.ProcessId):$($created.UtcTicks)"
        if (!$seen.ContainsKey($key)) {
            $seen[$key] = $true
            [ordered]@{kind=if ($process.Name -eq 'aurora-launcher.exe') {'launcher'} else {'javaChild'};
                processId=$process.ProcessId; parentId=$process.ParentProcessId;
                createdWallMs=$created.ToUnixTimeMilliseconds();
                observedWallMs=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();
                watchElapsedMs=$clock.Elapsed.TotalMilliseconds} |
                ConvertTo-Json -Compress | Add-Content -LiteralPath $OutputFile -Encoding utf8
        }
    }
    Start-Sleep -Milliseconds 60
}
