$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$evidence = Join-Path $repo 'docs/ui-3-1/native'
New-Item -ItemType Directory -Path $evidence -Force | Out-Null
$isolated = Join-Path ([IO.Path]::GetTempPath()) ('aurora-ui31-' + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $isolated -ErrorAction Stop | Out-Null
Set-Content -LiteralPath (Join-Path $isolated 'UI31-ISOLATED.txt') -Value 'UI3.1 isolated empty acceptance root; retain; no owner data'
$resolved = (Resolve-Path -LiteralPath $isolated).Path
if ((Split-Path $resolved -Parent) -ne ([IO.Path]::GetTempPath().TrimEnd('\')) -or (Split-Path $resolved -Leaf) -notlike 'aurora-ui31-*') { throw 'Expected new direct temporary root' }
$env:AURORA_DIAGNOSTIC_DATA_ROOT = Join-Path $resolved 'managed'
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $resolved 'webview'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9434'
$exe = Join-Path $repo 'src-tauri/target/debug/aurora-launcher.exe'
$process = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $evidence 'stdout.txt') -RedirectStandardError (Join-Path $evidence 'stderr.txt')
$retainedHandle = $process.Handle
@{pid=$process.Id;exe=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;isolatedRoot=$resolved;managed=$env:AURORA_DIAGNOSTIC_DATA_ROOT;webview=$env:WEBVIEW2_USER_DATA_FOLDER;nativeData='New empty root; no accounts, instances, game or modpack installations; actual compiled frontend'} | ConvertTo-Json | Tee-Object -FilePath (Join-Path $evidence 'start.json')
$process.WaitForExit()
$process.Refresh()
@{pid=$process.Id;exitCode=$process.ExitCode;handleRetained=($retainedHandle -ne [IntPtr]::Zero)} | ConvertTo-Json | Tee-Object -FilePath (Join-Path $evidence 'exit.json')
if ($null -eq $process.ExitCode) { throw 'Exit code unavailable; do not claim a successful exit' }
exit $process.ExitCode
