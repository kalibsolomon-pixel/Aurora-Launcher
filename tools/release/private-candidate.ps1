param([string]$OutputDirectory = '')
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
Set-Location -LiteralPath $repo
if (!$OutputDirectory) { $OutputDirectory = Join-Path $repo 'private-test-builds/20261019-3' }
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Choose a new output directory; existing candidates are preserved' }
$trackedChanges = @(git status --porcelain --untracked-files=no)
if ($trackedChanges.Count) { throw 'Commit the reviewed tracked source before packaging' }
$source = (git rev-parse HEAD).Trim()
$config = Get-Content -LiteralPath (Join-Path $repo 'src-tauri/tauri.private.conf.json') -Raw | ConvertFrom-Json
if ($config.identifier -cne 'com.aurora.launcher.private-candidate' -or $config.mainBinaryName -cne 'aurora-launcher-private-candidate' -or $config.bundle.createUpdaterArtifacts -ne $false) { throw 'Unexpected candidate identity/update configuration' }
# A diagnostic Client endpoint must never leak from the invoking environment.
if (Test-Path Env:AURORA_CLIENT_MANIFEST_URL) { throw 'Unset AURORA_CLIENT_MANIFEST_URL before packaging' }
& npm run tauri build -- --config src-tauri/tauri.private.conf.json --features private-candidate --bundles nsis
if ($LASTEXITCODE -ne 0) { throw 'Private Tauri packaging failed' }
$installer = Join-Path $repo "src-tauri/target/release/bundle/nsis/$($config.productName)_$($config.version)_x64-setup.exe"
$executable = Join-Path $repo "src-tauri/target/release/$($config.mainBinaryName).exe"
$nsis = Get-Content -LiteralPath (Join-Path $repo 'src-tauri/target/release/nsis/x64/installer.nsi') -Raw
if ($nsis -notmatch '!define MAINBINARYNAME "aurora-launcher-private-candidate"' -or
    $nsis -notmatch '!define BUNDLEID "com.aurora.launcher.private-candidate"' -or
    $nsis -notmatch '!define PRODUCTNAME "Aurora Launcher Private Test Candidate"') { throw 'Generated installer does not preserve candidate isolation' }
foreach ($path in @($installer, $executable)) {
    if (!(Test-Path -LiteralPath $path -PathType Leaf)) { throw "Required artifact missing: $path" }
    if ((Get-Item -LiteralPath $path).VersionInfo.ProductName -cne $config.productName) { throw 'Candidate product name mismatch' }
}
New-Item -ItemType Directory -Path $OutputDirectory -ErrorAction Stop | Out-Null
Copy-Item -LiteralPath $installer -Destination $OutputDirectory
Copy-Item -LiteralPath $executable -Destination (Join-Path $OutputDirectory 'Aurora-Private-Candidate.exe')
$artifacts = @(Get-ChildItem -LiteralPath $OutputDirectory -File | ForEach-Object {
    @{ name=$_.Name; sizeBytes=$_.Length; sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
})
@{ version=$config.version; sourceCommit=$source; identifier=$config.identifier; productName=$config.productName; artifacts=$artifacts; publicPublication=$false } |
    ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'candidate-artifacts.json') -Encoding utf8
Write-Output "Private candidate prepared at $OutputDirectory from $source"
