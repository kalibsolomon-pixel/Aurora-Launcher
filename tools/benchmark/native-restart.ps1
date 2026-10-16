param([ValidateSet('restart','runtime','materials')][string]$EvidenceName='restart')
$ErrorActionPreference='Stop'
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$session=Get-Content -LiteralPath (Join-Path $repo '.git/performance-p3a-native-session.json') -Raw | ConvertFrom-Json
$isolated=(Resolve-Path -LiteralPath $session.isolatedRoot).Path
if((Split-Path $isolated -Parent) -ne ([IO.Path]::GetTempPath().TrimEnd('\')) -or (Split-Path $isolated -Leaf) -notmatch '^aurora-p3a-[a-f0-9-]+$'){throw 'Expected direct disposable root'}
if((Get-Item -LiteralPath $isolated).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Redirected root'}
if((Get-Content -LiteralPath (Join-Path $isolated 'P3A-ISOLATED.txt') -Raw).Trim() -ne 'P3A isolated preparation; no copied owner data; retain'){throw 'Marker mismatch'}
foreach($relative in @('managed','managed/launcher','webview')){
 $candidate=Join-Path $isolated $relative
 if((Get-Item -LiteralPath $candidate).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Redirected control directory'}
}
$accounts=Join-Path $isolated 'managed/launcher/accounts.json'
if(Test-Path -LiteralPath $accounts){
 $document=Get-Content -LiteralPath $accounts -Raw | ConvertFrom-Json
 if($document.schemaVersion -ne 1 -or $document.accounts.Count -ne 0){throw 'Expected no accounts; authentication gate remains closed'}
}
$exe=Join-Path $isolated 'aurora-launcher.exe'
if((Get-FileHash -LiteralPath $exe).Hash -ne $session.sha256){throw 'Executable changed'}
$env:AURORA_DIAGNOSTIC_DATA_ROOT=Join-Path $isolated 'managed'
$env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $isolated 'webview'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9443'
$record=Join-Path $repo ".git/performance-p3a-native-$EvidenceName-exit.json"
if(Test-Path -LiteralPath $record){throw 'Preserve restart evidence'}
$process=Start-Process -FilePath $exe -WorkingDirectory $isolated -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $isolated "$EvidenceName-stdout.txt") -RedirectStandardError (Join-Path $isolated "$EvidenceName-stderr.txt")
$retained=$process.Handle
$process.WaitForExit();$process.Refresh()
@{pid=$process.Id;exitCode=$process.ExitCode;handleRetained=($retained-ne[IntPtr]::Zero)} | ConvertTo-Json | Set-Content -LiteralPath $record
exit $process.ExitCode
