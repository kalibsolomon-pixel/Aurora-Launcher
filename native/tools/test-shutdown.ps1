<#
  Bounded exact-process shutdown test (NATIVE-9 lingering-close observation).

  Each run copies a small disposable root, starts the acceptance executable
  through acceptance-session.ps1, closes only its exact application HWND with
  WM_CLOSE, and requires both process exit and the "exited cleanly" log line
  within 15 seconds. A lingering process is recorded (threads, visible
  windows, CPU time, log tail) before it is ended by its exact PID. Unrelated
  processes are never enumerated for termination.

  Windows PowerShell 5.1 compatible.
#>
param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$TemplateManaged,
    [int]$Runs = 5
)
$ErrorActionPreference = 'Stop'
$session = Join-Path $PSScriptRoot 'acceptance-session.ps1'
Add-Type -Namespace SD -Name W -MemberDefinition '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool PostMessage(System.IntPtr h, uint m, System.IntPtr w, System.IntPtr l);'
$results = @()
for ($run = 1; $run -le $Runs; $run++) {
    $root = Join-Path $env:TEMP ('aurora-native10-shutdown-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory $root | Out-Null
    Set-Content -LiteralPath (Join-Path $root 'NATIVE10-DISPOSABLE.txt') -Value 'shutdown test' -Encoding utf8
    Copy-Item -Recurse -LiteralPath $TemplateManaged -Destination (Join-Path $root 'managed')
    # No captured output pipe: the launched app would inherit it and block the read.
    # Start-Process -Wait would also wait for the launched app (the whole tree).
    $starter = Start-Process -FilePath powershell -WindowStyle Hidden -PassThru -ArgumentList @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', ('"' + $session + '"'), '-Exe', ('"' + $Exe + '"'), '-Root', ('"' + $root + '"'))
    if (-not $starter.WaitForExit(60000) -or $starter.ExitCode -ne 0) { throw "Session start failed in $root" }
    $info = Get-Content -LiteralPath (Join-Path $root 'session.json') -Raw | ConvertFrom-Json
    Start-Sleep -Seconds 4
    $process = Get-Process -Id $info.pid
    if ($process.StartTime.ToString('o') -ne $info.created) { throw 'Process identity changed before close.' }
    [void][SD.W]::PostMessage([IntPtr][Convert]::ToInt64($info.hwnd, 16), 0x10, [IntPtr]::Zero, [IntPtr]::Zero)
    $exited = $process.WaitForExit(15000)
    $log = Get-Content -LiteralPath (Join-Path $root 'session-stderr.txt') -Raw
    $entry = [ordered]@{ run = $run; pid = $info.pid; exited = $exited; runReturned = ($log -match 'window closed'); exitedCleanly = ($log -match 'exited cleanly') }
    if (-not $exited) {
        $process.Refresh()
        $entry.threads = $process.Threads.Count
        $entry.cpuSeconds = $process.TotalProcessorTime.TotalSeconds
        $entry.visibleWindows = @(Get-Process -Id $info.pid | Where-Object { $_.MainWindowHandle -ne 0 }).Count
        $entry.logTail = ($log -split "`n" | Select-Object -Last 5) -join ' | '
        Stop-Process -Id $info.pid -Force
    }
    $results += [pscustomobject]$entry
}
$results | ConvertTo-Json
if ($results | Where-Object { -not $_.exited -or -not $_.exitedCleanly }) { exit 1 }
