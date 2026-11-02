param([string]$ReuseRoot='',[string]$EvidenceName='native')
$ErrorActionPreference='Stop'
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if($EvidenceName -notmatch '^native(-[a-z0-9-]+)?$'){throw 'Unexpected evidence name'}
$evidence=Join-Path $repo "docs/ui-5/$EvidenceName"
New-Item -ItemType Directory -Path $evidence -Force | Out-Null
if(Test-Path -LiteralPath (Join-Path $evidence 'start.json')){throw 'Evidence already exists; choose a new evidence name'}
if(!$ReuseRoot){
 $isolated=Join-Path ([IO.Path]::GetTempPath()) ('aurora-ui5-'+[guid]::NewGuid().ToString())
 New-Item -ItemType Directory -Path $isolated -ErrorAction Stop | Out-Null
 Set-Content -LiteralPath (Join-Path $isolated 'UI5-ISOLATED.txt') -Value 'UI5 isolated settings acceptance; retain; synthetic skins only; no owner accounts/data'
}else{$isolated=$ReuseRoot}
$resolved=(Resolve-Path -LiteralPath $isolated).Path
if((Split-Path $resolved -Parent) -ne ([IO.Path]::GetTempPath().TrimEnd('\')) -or (Split-Path $resolved -Leaf) -notlike 'aurora-ui5-*' -or !(Test-Path -LiteralPath (Join-Path $resolved 'UI5-ISOLATED.txt'))){throw 'Expected marked direct-temp root'}
$env:AURORA_DIAGNOSTIC_DATA_ROOT=Join-Path $resolved 'managed'
$env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $resolved 'webview'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9437'
$exe=Join-Path $repo 'src-tauri/target/debug/aurora-launcher.exe'
$process=Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $evidence 'stdout.txt') -RedirectStandardError (Join-Path $evidence 'stderr.txt')
$retainedHandle=$process.Handle
@{pid=$process.Id;exe=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;isolatedRoot=$resolved;managed=$env:AURORA_DIAGNOSTIC_DATA_ROOT;webview=$env:WEBVIEW2_USER_DATA_FOLDER;nativeData='Actual compiled frontend; marked isolated root; no accounts/credentials/instances copied'} | ConvertTo-Json | Tee-Object -FilePath (Join-Path $evidence 'start.json')
$process.WaitForExit();$process.Refresh()
@{pid=$process.Id;exitCode=$process.ExitCode;handleRetained=($retainedHandle -ne [IntPtr]::Zero)} | ConvertTo-Json | Tee-Object -FilePath (Join-Path $evidence 'exit.json')
if($null -eq $process.ExitCode){throw 'Exit unavailable'}
exit $process.ExitCode
