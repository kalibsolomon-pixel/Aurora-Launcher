# Starts the native launcher against a disposable data root and prints its PID.
param(
    [Parameter(Mandatory)] [string] $Root,
    [string] $Exe = "$PSScriptRoot\..\target\debug\aurora-native.exe"
)
New-Item -ItemType Directory -Force $Root | Out-Null
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = (Resolve-Path $Exe).Path
$psi.UseShellExecute = $false
$psi.Environment["AURORA_DIAGNOSTIC_DATA_ROOT"] = $Root
$p = [System.Diagnostics.Process]::Start($psi)
$p.Id
