# NATIVE-6 comparison: the production-profile Tauri build and the native
# release build, interleaved, each on an empty data root of its own.
param([Parameter(Mandatory)] [string] $OutDir, [Parameter(Mandatory)] [string] $NativeRoot, [int] $Runs = 3)
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$probe = Join-Path $PSScriptRoot 'measure-process.ps1'
New-Item -ItemType Directory -Force $OutDir, $NativeRoot | Out-Null
$candidates = @(
    @{ label = 'tauri-release'; exe = "$root\src-tauri\target\release\aurora-launcher.exe"; env = @{} },
    @{ label = 'native-release'; exe = "$root\native\target\release\aurora-native.exe"; env = @{ AURORA_DIAGNOSTIC_DATA_ROOT = $NativeRoot } }
)
for ($run = 1; $run -le $Runs; $run++) {
    foreach ($c in $candidates) {
        & $probe -Exe $c.exe -Label $c.label -Out (Join-Path $OutDir ("{0}-run{1}.json" -f $c.label, $run)) -Environment $c.env
        Start-Sleep -Seconds 3
    }
}
