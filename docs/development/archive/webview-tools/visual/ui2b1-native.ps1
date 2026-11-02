param([ValidateSet('native','native-final')][string]$EvidenceName = 'native')
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$evidence = Join-Path $repo "docs/ui-2b-1/native"
New-Item -ItemType Directory -Force -Path $evidence | Out-Null
$isolated = Join-Path ([IO.Path]::GetTempPath()) ('aurora-ui2b1-' + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $isolated -ErrorAction Stop | Out-Null
Set-Content -LiteralPath (Join-Path $isolated 'UI2B-ISOLATED.txt') -Value 'UI2B synthetic presentation acceptance; retain this root'
$launcherDir = Join-Path $isolated 'managed/launcher'
New-Item -ItemType Directory -Path $launcherDir -Force | Out-Null
$id = '1234567890abcdef1234567890abcdef'
@{schemaVersion=1;selectedInstanceId=$id} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $launcherDir 'config.json')
$configuration = @{minecraftVersion='1.21.11';loader=@{kind='vanilla'};auroraEnabled=$false;memoryMib=2048;additionalJvmArguments='';window=$null}
$record = @{id=$id;displayName='UI-2B Vanilla presentation fixture';state='installing';installed=@{minecraftVersion='1.21.11';platform=@{kind='vanilla'};aurora=$null};configuration=$configuration;pack=$null}
@{schemaVersion=5;instances=@($record)} | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $launcherDir 'instances.json')
$resolved = (Resolve-Path -LiteralPath $isolated).Path
if ((Split-Path $resolved -Parent) -ne ([IO.Path]::GetTempPath().TrimEnd('\')) -or (Split-Path $resolved -Leaf) -notlike 'aurora-ui2b1-*') { throw 'Expected fresh direct temporary root' }
$env:AURORA_DIAGNOSTIC_DATA_ROOT = Join-Path $resolved 'managed'
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $resolved 'webview'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9432'
$exe = Join-Path $repo 'src-tauri/target/debug/aurora-launcher.exe'
$process = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $evidence 'stdout.txt') -RedirectStandardError (Join-Path $evidence 'stderr.txt')
@{pid=$process.Id;exe=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;isolatedRoot=$resolved;managed=$env:AURORA_DIAGNOSTIC_DATA_ROOT;webview=$env:WEBVIEW2_USER_DATA_FOLDER} | ConvertTo-Json | Tee-Object -FilePath (Join-Path $evidence 'start.json')
$process.WaitForExit()
@{pid=$process.Id;exitCode=$process.ExitCode} | ConvertTo-Json | Tee-Object -FilePath (Join-Path $evidence 'exit.json')
exit $process.ExitCode
