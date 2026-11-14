param(
    [string]$Binary,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$Makensis = "$env:LOCALAPPDATA\tauri\NSIS\makensis.exe",
    [string]$AcceptanceRoot,
    [string]$AcceptanceId,
    [string]$AcceptanceVersion
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$version = [regex]::Match([IO.File]::ReadAllText((Join-Path $repo 'native-gpui/Cargo.toml')), '(?ms)^\[package\]\s*\n.*?^version\s*=\s*"([^"]+)"').Groups[1].Value
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'A numeric release version is required' }
if (!$Binary) { $Binary = Join-Path $repo 'native-gpui\target\release\aurora-launcher.exe' }
$Binary = (Resolve-Path -LiteralPath $Binary).Path
$image=[IO.File]::ReadAllBytes($Binary)
if ($image.Length -lt 64 -or $image[0] -ne 0x4d -or $image[1] -ne 0x5a) { throw 'Native package requires a Windows PE executable' }
$pe=[BitConverter]::ToInt32($image,0x3c)
if ($pe -lt 64 -or $pe -gt $image.Length-6 -or [BitConverter]::ToUInt32($image,$pe) -ne 0x4550 -or [BitConverter]::ToUInt16($image,$pe+4) -ne 0x8664) { throw 'Native package requires Windows x86_64 machine identity' }
$output = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $output -Force | Out-Null
$name = "Aurora.Launcher_${version}_x64-setup.exe"
if ($AcceptanceRoot) {
    if ($AcceptanceVersion) {
        if ($AcceptanceVersion -notmatch '^\d+\.\d+\.\d+$') { throw 'Invalid acceptance version' }
        $version=$AcceptanceVersion
        $name="Aurora.Launcher_${version}_x64-setup.exe"
    }
    $AcceptanceRoot = [IO.Path]::GetFullPath($AcceptanceRoot)
    $tempPrefix = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\aurora-native9-'
    if (!$AcceptanceRoot.StartsWith($tempPrefix, [StringComparison]::OrdinalIgnoreCase) -or $AcceptanceId -notmatch '^[a-f0-9]{32}$') { throw 'Acceptance requires an explicit native9 disposable temp root and UUID identity' }
    if (!(Test-Path -LiteralPath $AcceptanceRoot -PathType Container)) { throw 'Create the disposable acceptance root first' }
    $name = "Acceptance-$AcceptanceId-$name"
} else {
    if ($AcceptanceVersion) { throw 'Version overrides are acceptance-only' }
    $binaryText = [Text.Encoding]::ASCII.GetString($image)
    if (!$binaryText.Contains('com.aurora.launcher.native-preview')) { throw 'The native preview identity is required; do not package a shared-production-data executable' }
    if ($binaryText.Contains('AURORA_DIAGNOSTIC_DATA_ROOT') -or $binaryText.Contains('--preview-probe=') -or $binaryText.Contains('AURORA_DISPOSABLE_DIAGNOSTIC')) { throw 'Distribution cannot contain acceptance features' }
    $metadata = (Get-Item -LiteralPath $Binary).VersionInfo
    if ([IO.Path]::GetFileName($Binary) -ne 'aurora-launcher.exe' -or $metadata.ProductName -ne 'Aurora Launcher' -or $metadata.ProductVersion -ne $version -or $metadata.FileDescription -ne 'Aurora Launcher' -or $metadata.OriginalFilename -ne 'aurora-launcher.exe') { throw 'Aurora Launcher executable name or metadata does not match the package identity/version' }
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
