# NATIVE-0 comparison matrix: interleaved runs of each candidate under the
# same probe. Results land in -OutDir as one JSON per run.
param([Parameter(Mandatory)] [string] $OutDir, [int] $Runs = 3)
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$probe = Join-Path $PSScriptRoot 'measure-process.ps1'
New-Item -ItemType Directory -Force $OutDir | Out-Null
$candidates = @(
    @{ label = 'tauri-release'; exe = "$root\src-tauri\target\release\aurora-launcher.exe"; env = @{} },
    @{ label = 'slint-femtovg'; exe = "$root\native\prototypes\slint-proto\target\release\aurora-slint-proto.exe"; env = @{ SLINT_BACKEND = 'winit-femtovg' } },
    @{ label = 'slint-software'; exe = "$root\native\prototypes\slint-proto\target\release\aurora-slint-proto.exe"; env = @{ SLINT_BACKEND = 'winit-software' } },
    @{ label = 'gpui'; exe = "$root\native\prototypes\gpui-proto\target\release\aurora-gpui-proto.exe"; env = @{} }
)
for ($run = 1; $run -le $Runs; $run++) {
    foreach ($c in $candidates) {
        $out = Join-Path $OutDir ("{0}-run{1}.json" -f $c.label, $run)
        & $probe -Exe $c.exe -Label $c.label -Out $out -Environment $c.env
        Start-Sleep -Seconds 3
    }
}
