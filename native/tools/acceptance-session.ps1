<#
  Starts one isolated native acceptance session and proves its identity.

  - Refuses any data root without a NATIVE*-DISPOSABLE.txt marker beside it,
    and refuses the real production / preview data roots outright.
  - Records every Aurora process that already exists. They are never touched:
    another window may be under the owner's control.
  - Records the exact executable hash, the PID and its creation time, the one
    owned application HWND ("Window Class", unowned, visible), and the data
    root the process itself reports on stderr ("managed data root: ...").
  - Requires an executable that honors AURORA_DIAGNOSTIC_DATA_ROOT (an
    acceptance build); a distribution build would open real preview data.

  Windows PowerShell 5.1 compatible. Writes session.json into -Root.
#>
param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Root,
    [int]$TimeoutSeconds = 20
)
$ErrorActionPreference = 'Stop'
$Root = [IO.Path]::GetFullPath($Root)
$managed = Join-Path $Root 'managed'
if (-not (Get-ChildItem -LiteralPath $Root -Filter 'NATIVE*-DISPOSABLE.txt' -ErrorAction SilentlyContinue)) {
    throw "Refusing: $Root has no NATIVE*-DISPOSABLE.txt marker."
}
foreach ($forbidden in @('com.aurora.launcher', 'com.aurora.launcher.native-preview', 'Aurora Launcher', 'Aurora Launcher Native Preview')) {
    $real = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA $forbidden))
    if ($managed.StartsWith($real, [StringComparison]::OrdinalIgnoreCase)) { throw "Refusing a real data root: $real" }
}
$Exe = (Resolve-Path -LiteralPath $Exe).Path
$text = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($Exe))
if (-not $text.Contains('AURORA_DIAGNOSTIC_DATA_ROOT')) { throw 'Refusing: not an acceptance build (no disposable data root support).' }

Add-Type -Namespace AS -Name W -MemberDefinition @'
public delegate bool EnumProc(System.IntPtr h, System.IntPtr l);
[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, System.IntPtr l);
[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(System.IntPtr h, out uint p);
[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool IsWindowVisible(System.IntPtr h);
[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern System.IntPtr GetWindow(System.IntPtr h, uint c);
[System.Runtime.InteropServices.DllImport("user32.dll", CharSet=System.Runtime.InteropServices.CharSet.Unicode)] public static extern int GetClassName(System.IntPtr h, System.Text.StringBuilder s, int n);
'@
function Find-AppWindow([int]$TargetPid) {
    $found = New-Object System.Collections.ArrayList
    $callback = [AS.W+EnumProc]{
        param($h, $l)
        $p = 0; [void][AS.W]::GetWindowThreadProcessId($h, [ref]$p)
        if ($p -eq $TargetPid -and [AS.W]::IsWindowVisible($h) -and [AS.W]::GetWindow($h, 4) -eq [IntPtr]::Zero) {
            $c = New-Object Text.StringBuilder 64; [void][AS.W]::GetClassName($h, $c, 64)
            if ($c.ToString() -eq 'Window Class') { [void]$found.Add($h) }
        }
        $true
    }
    [void][AS.W]::EnumWindows($callback, [IntPtr]::Zero)
    if ($found.Count -gt 1) { throw 'Ambiguous application window' }
    if ($found.Count -eq 1) { return $found[0] } else { return [IntPtr]::Zero }
}

$preexisting = @(Get-CimInstance Win32_Process | Where-Object { $_.Name -like 'aurora*' } |
    ForEach-Object { [ordered]@{ pid = $_.ProcessId; path = $_.ExecutablePath; created = $_.CreationDate.ToString('o') } })

$env:AURORA_DIAGNOSTIC_DATA_ROOT = $managed
$stderr = Join-Path $Root 'session-stderr.txt'
$process = Start-Process -FilePath $Exe -PassThru -RedirectStandardError $stderr -RedirectStandardOutput (Join-Path $Root 'session-stdout.txt')
$clock = [Diagnostics.Stopwatch]::StartNew()
$hwnd = [IntPtr]::Zero
while ($hwnd -eq [IntPtr]::Zero -and $clock.Elapsed.TotalSeconds -lt $TimeoutSeconds) { Start-Sleep -Milliseconds 50; $hwnd = Find-AppWindow $process.Id }
if ($hwnd -eq [IntPtr]::Zero) { throw 'The session window did not appear.' }
$reported = $null
while (-not $reported -and $clock.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
    Start-Sleep -Milliseconds 100
    $line = Get-Content -LiteralPath $stderr -ErrorAction SilentlyContinue | Where-Object { $_ -match 'managed data root: (.+)$' } | Select-Object -First 1
    if ($line -match 'managed data root: (.+)$') { $reported = $Matches[1].Trim() }
}
if (-not $reported -or -not [IO.Path]::GetFullPath($reported).Equals($managed, [StringComparison]::OrdinalIgnoreCase)) {
    throw "The process reported data root '$reported', not $managed."
}
$session = [ordered]@{
    exe = $Exe; exeSha256 = (Get-FileHash -LiteralPath $Exe).Hash
    pid = $process.Id; created = $process.StartTime.ToString('o')
    hwnd = ('0x{0:X}' -f [int64]$hwnd); windowMs = [math]::Round($clock.Elapsed.TotalMilliseconds)
    dataRoot = $managed; reportedDataRoot = $reported
    preexistingAuroraProcesses = $preexisting
}
$session | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $Root 'session.json') -Encoding utf8
$session | ConvertTo-Json -Depth 4
