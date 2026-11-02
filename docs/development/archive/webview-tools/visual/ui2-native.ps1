param([string]$IsolatedRoot)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$evidence = Join-Path $repo 'docs/ui-2/native'
New-Item -ItemType Directory -Force -Path $evidence | Out-Null
if (!$IsolatedRoot) {
  $IsolatedRoot = Join-Path ([IO.Path]::GetTempPath()) ('aurora-ui2-' + [guid]::NewGuid().ToString())
  New-Item -ItemType Directory -Path $IsolatedRoot -ErrorAction Stop | Out-Null
  Set-Content -LiteralPath (Join-Path $IsolatedRoot 'UI2-ISOLATED.txt') -Value 'UI2 synthetic presentation acceptance'
  $launcherDir = Join-Path $IsolatedRoot 'managed/launcher'
  New-Item -ItemType Directory -Path $launcherDir -Force | Out-Null
  $id = '1234567890abcdef1234567890abcdef'
  @{schemaVersion=1;selectedInstanceId=$id} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $launcherDir 'config.json')
  $configuration = @{minecraftVersion='1.21.11';loader=@{kind='vanilla'};auroraEnabled=$false;memoryMib=2048;additionalJvmArguments='';window=$null}
  $record = @{id=$id;displayName='UI-2 Vanilla presentation fixture';state='installing';installed=@{minecraftVersion='1.21.11';platform=@{kind='vanilla'};aurora=$null};configuration=$configuration;pack=$null}
  @{schemaVersion=5;instances=@($record)} | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $launcherDir 'instances.json')
}
$resolved = (Resolve-Path -LiteralPath $IsolatedRoot).Path
if ((Split-Path $resolved -Parent) -ne ([IO.Path]::GetTempPath().TrimEnd('\')) -or (Split-Path $resolved -Leaf) -notlike 'aurora-ui2-*' -or (Get-Content -LiteralPath (Join-Path $resolved 'UI2-ISOLATED.txt') -Raw).Trim() -ne 'UI2 synthetic presentation acceptance') { throw 'Expected exact marked direct temporary root' }
$env:AURORA_DIAGNOSTIC_DATA_ROOT = Join-Path $resolved 'managed'
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $resolved 'webview'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9432'
$exe = Join-Path $repo 'src-tauri/target/debug/aurora-launcher.exe'
$run = Get-Date -Format 'HHmmss'
$process = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $evidence "$run-stdout.txt") -RedirectStandardError (Join-Path $evidence "$run-stderr.txt")
@{pid=$process.Id;exe=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;isolatedRoot=$resolved;managed=$env:AURORA_DIAGNOSTIC_DATA_ROOT;webview=$env:WEBVIEW2_USER_DATA_FOLDER;run=$run} | ConvertTo-Json | Tee-Object -FilePath (Join-Path $evidence "$run-start.json")
$resolved | Set-Content -LiteralPath (Join-Path $evidence 'isolated-root.txt')
$process.WaitForExit()
@{pid=$process.Id;exitCode=$process.ExitCode;run=$run} | ConvertTo-Json | Tee-Object -FilePath (Join-Path $evidence "$run-exit.json")
exit $process.ExitCode
