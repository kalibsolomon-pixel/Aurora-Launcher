param([string]$ReuseRoot='', [ValidateSet('first','restart')][string]$Phase='first')
$ErrorActionPreference='Stop'
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if(!$ReuseRoot){
 $isolated=Join-Path ([IO.Path]::GetTempPath()) ('aurora-p2-'+[guid]::NewGuid().ToString())
 New-Item -ItemType Directory -Path $isolated -ErrorAction Stop | Out-Null
 Set-Content -LiteralPath (Join-Path $isolated 'P2-ISOLATED.txt') -Value 'P2 isolated configuration fixture; no game, accounts or owner data; retain'
 $launcher=Join-Path $isolated 'managed/launcher'
 New-Item -ItemType Directory -Path $launcher -ErrorAction Stop | Out-Null
 $id='a'*32
 $configuration=@{minecraftVersion='1.21.11';auroraEnabled=$false;loader=@{kind='fabric';policy=@{type='pinned';version='0.19.5'}};memoryMib=3217;additionalJvmArguments='-Dlabel="two words" -Dlabel=last';window=$null}
 $registry=@{schemaVersion=5;instances=@(@{id=$id;displayName='P2 isolated configuration';state='ready';installed=@{minecraftVersion='1.21.11';platform=@{kind='fabric';version='0.19.5'};aurora=$null};configuration=$configuration;pack=$null})}
 $registry | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $launcher 'instances.json') -Encoding utf8
 Copy-Item -LiteralPath (Join-Path $launcher 'instances.json') -Destination (Join-Path $isolated 'before-registry.json')
 @{schemaVersion=1;selectedInstanceId=$id} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $launcher 'config.json') -Encoding utf8
 Copy-Item -LiteralPath (Join-Path $repo 'src-tauri/target/debug/aurora-launcher.exe') -Destination (Join-Path $isolated 'aurora-launcher.exe')
} else { $isolated=$ReuseRoot }
$resolved=(Resolve-Path -LiteralPath $isolated).Path
if((Split-Path $resolved -Parent) -ne ([IO.Path]::GetTempPath().TrimEnd('\')) -or (Split-Path $resolved -Leaf) -notmatch '^aurora-p2-[a-f0-9-]+$'){throw 'Expected direct disposable root'}
if((Get-Content -LiteralPath (Join-Path $resolved 'P2-ISOLATED.txt') -Raw).Trim() -ne 'P2 isolated configuration fixture; no game, accounts or owner data; retain'){throw 'Marker mismatch'}
if((Get-Item -LiteralPath $resolved).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Redirected root'}
$env:AURORA_DIAGNOSTIC_DATA_ROOT=Join-Path $resolved 'managed'
$env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $resolved 'webview'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9442'
$exe=Join-Path $resolved 'aurora-launcher.exe'
$record=Join-Path $repo ".git/performance-p2-native-$Phase.json"
if(Test-Path -LiteralPath $record){throw 'Session record exists; preserve it'}
$process=Start-Process -FilePath $exe -WorkingDirectory $resolved -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $resolved "$Phase-stdout.txt") -RedirectStandardError (Join-Path $resolved "$Phase-stderr.txt")
$retained=$process.Handle
@{pid=$process.Id;exe=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;isolatedRoot=$resolved;managed=$env:AURORA_DIAGNOSTIC_DATA_ROOT;webview=$env:WEBVIEW2_USER_DATA_FOLDER} | ConvertTo-Json | Set-Content -LiteralPath $record
$process.WaitForExit();$process.Refresh()
@{pid=$process.Id;exitCode=$process.ExitCode;handleRetained=($retained -ne [IntPtr]::Zero)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $repo ".git/performance-p2-native-$Phase-exit.json")
if($null -eq $process.ExitCode){throw 'Exit unavailable'}
exit $process.ExitCode
