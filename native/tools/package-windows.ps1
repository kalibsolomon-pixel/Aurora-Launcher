param(
    [string]$Binary,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$Makensis = "$env:LOCALAPPDATA\tauri\NSIS\makensis.exe",
    [string]$AcceptanceRoot,
    [string]$AcceptanceId
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$version = (Get-Content (Join-Path $repo 'package.json') -Raw | ConvertFrom-Json).version
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'A numeric release version is required' }
if (!$Binary) { $Binary = Join-Path $repo 'native\target\release\aurora-native.exe' }
$Binary = (Resolve-Path -LiteralPath $Binary).Path
$output = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $output -Force | Out-Null
$name = "Aurora.Native.Preview_${version}_x64-setup.exe"
if ($AcceptanceRoot) {
    $AcceptanceRoot = [IO.Path]::GetFullPath($AcceptanceRoot)
    $tempPrefix = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\aurora-native9-'
    if (!$AcceptanceRoot.StartsWith($tempPrefix, [StringComparison]::OrdinalIgnoreCase) -or $AcceptanceId -notmatch '^[a-f0-9]{32}$') { throw 'Acceptance requires an explicit native9 disposable temp root and UUID identity' }
    if (!(Test-Path -LiteralPath $AcceptanceRoot -PathType Container)) { throw 'Create the disposable acceptance root first' }
    $name = "Acceptance-$AcceptanceId-$name"
} else {
    $binaryText = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($Binary))
    if (!$binaryText.Contains('com.aurora.launcher.native-preview')) { throw 'The native preview identity is required; do not package a shared-production-data executable' }
    if ($binaryText.Contains('AURORA_DIAGNOSTIC_DATA_ROOT') -or $binaryText.Contains('--preview-probe=')) { throw 'Distribution cannot contain acceptance features' }
    $metadata = (Get-Item -LiteralPath $Binary).VersionInfo
    if ($metadata.ProductName -ne 'Aurora Launcher (native preview)' -or $metadata.ProductVersion -ne $version) { throw 'Native executable metadata does not match the package identity/version' }
}
$destination = Join-Path $output $name
if (Test-Path -LiteralPath $destination) { throw 'Refusing to overwrite an existing release artifact' }
$arguments = @('/V2', "/DVERSION=$version", "/DBINARY=$Binary", "/DOUTPUT=$destination")
if ($AcceptanceRoot) { $arguments += @("/DACCEPTANCE_ROOT=$AcceptanceRoot", "/DACCEPTANCE_ID=$AcceptanceId") }
Push-Location (Join-Path $repo 'native\installer')
try {
    & $Makensis @arguments 'native-preview.nsi'
    if ($LASTEXITCODE -ne 0) { throw 'Native NSIS packaging failed' }
} finally { Pop-Location }
Get-FileHash -Algorithm SHA256 -LiteralPath $destination
Get-AuthenticodeSignature -LiteralPath $destination | Select-Object Path,Status
