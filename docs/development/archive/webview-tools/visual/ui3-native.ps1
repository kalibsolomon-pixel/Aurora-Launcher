param([string]$ReuseIsolatedRoot)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$evidence = Join-Path $repo $(if ($ReuseIsolatedRoot) {'docs/ui-3/native/restart'} else {'docs/ui-3/native'})
New-Item -ItemType Directory -Force -Path $evidence | Out-Null
if ($ReuseIsolatedRoot) {
  $isolated = (Resolve-Path -LiteralPath $ReuseIsolatedRoot).Path
  if ((Split-Path $isolated -Parent) -ne ([IO.Path]::GetTempPath().TrimEnd('\')) -or (Split-Path $isolated -Leaf) -notlike 'aurora-ui3-*' -or (Get-Content -LiteralPath (Join-Path $isolated 'UI3-ISOLATED.txt') -Raw).Trim() -ne 'UI3 isolated acceptance; no owner data; retain this root') { throw 'Expected this phase marked direct temporary root' }
} else {
$isolated = Join-Path ([IO.Path]::GetTempPath()) ('aurora-ui3-' + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $isolated -ErrorAction Stop | Out-Null
Set-Content -LiteralPath (Join-Path $isolated 'UI3-ISOLATED.txt') -Value 'UI3 isolated acceptance; no owner data; retain this root'
$launcherDir = Join-Path $isolated 'managed/launcher'
New-Item -ItemType Directory -Path $launcherDir -Force | Out-Null
$id = '1234567890abcdef1234567890abcdef'
@{schemaVersion=1;selectedInstanceId=$id} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $launcherDir 'config.json')
$configuration = @{minecraftVersion='1.21.11';loader=@{kind='fabric';policy=@{type='pinned';version='0.19.5'}};auroraEnabled=$false;memoryMib=2048;additionalJvmArguments='';window=$null}
$record = @{id=$id;displayName='UI-3 isolated Fabric fixture';state='installing';installed=@{minecraftVersion='1.21.11';platform=@{kind='fabric';version='0.19.5'};aurora=$null};configuration=$configuration;pack=$null}
@{schemaVersion=5;instances=@($record)} | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $launcherDir 'instances.json')
foreach ($folder in @('mods','resourcepacks')) { New-Item -ItemType Directory -Path (Join-Path $isolated "managed/instances/$id/$folder") -Force | Out-Null }
}
$resolved = (Resolve-Path -LiteralPath $isolated).Path
if ((Split-Path $resolved -Parent) -ne ([IO.Path]::GetTempPath().TrimEnd('\')) -or (Split-Path $resolved -Leaf) -notlike 'aurora-ui3-*') { throw 'Expected fresh direct temporary root' }
$env:AURORA_DIAGNOSTIC_DATA_ROOT = Join-Path $resolved 'managed'
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $resolved 'webview'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9433'
$exe = Join-Path $repo 'src-tauri/target/debug/aurora-launcher.exe'
$process = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $evidence 'stdout.txt') -RedirectStandardError (Join-Path $evidence 'stderr.txt')
# Windows PowerShell may dispose its unobserved process handle before ExitCode
# is read. Retain it before waiting so the actual native result is available.
$nativeProcessHandle = $process.Handle
@{pid=$process.Id;exe=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;isolatedRoot=$resolved;managed=$env:AURORA_DIAGNOSTIC_DATA_ROOT;webview=$env:WEBVIEW2_USER_DATA_FOLDER;nativeData=$(if ($ReuseIsolatedRoot) {'Restart of the same isolated root after genuine Fabric installation; no credentials or owner content'} else {'Empty Installing Fabric fixture; no credentials or owner content'})} | ConvertTo-Json | Tee-Object -FilePath (Join-Path $evidence 'start.json')
$process.WaitForExit()
$process.Refresh()
$nativeExitCode = $process.ExitCode
@{pid=$process.Id;exitCode=$nativeExitCode;handleRetained=($nativeProcessHandle -ne [IntPtr]::Zero)} | ConvertTo-Json | Tee-Object -FilePath (Join-Path $evidence 'exit.json')
if ($null -eq $nativeExitCode) { throw 'Native exit code was unavailable; do not report a successful process result' }
exit $nativeExitCode
