$ErrorActionPreference='Stop'
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$isolated=Join-Path ([IO.Path]::GetTempPath()) ('aurora-p3a-'+[guid]::NewGuid())
New-Item -ItemType Directory -Path $isolated -ErrorAction Stop | Out-Null
if((Split-Path $isolated -Parent) -ne ([IO.Path]::GetTempPath().TrimEnd('\'))){throw 'Expected direct temporary root'}
Set-Content -LiteralPath (Join-Path $isolated 'P3A-ISOLATED.txt') -Value 'P3A isolated preparation; no copied owner data; retain'
$exe=Join-Path $isolated 'aurora-launcher.exe'
Copy-Item -LiteralPath (Join-Path $repo 'src-tauri/target/debug/aurora-launcher.exe') -Destination $exe
$env:AURORA_DIAGNOSTIC_DATA_ROOT=Join-Path $isolated 'managed'
$env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $isolated 'webview'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9443'
$record=Join-Path $repo '.git/performance-p3a-native-session.json'
if(Test-Path -LiteralPath $record){throw 'Preserve existing session record'}
$process=Start-Process -FilePath $exe -WorkingDirectory $isolated -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $isolated 'stdout.txt') -RedirectStandardError (Join-Path $isolated 'stderr.txt')
$retained=$process.Handle
@{pid=$process.Id;isolatedRoot=$isolated;managed=$env:AURORA_DIAGNOSTIC_DATA_ROOT;webview=$env:WEBVIEW2_USER_DATA_FOLDER;exe=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash} | ConvertTo-Json | Set-Content -LiteralPath $record
$process.WaitForExit();$process.Refresh()
@{pid=$process.Id;exitCode=$process.ExitCode;handleRetained=($retained-ne[IntPtr]::Zero)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $repo '.git/performance-p3a-native-exit.json')
exit $process.ExitCode
