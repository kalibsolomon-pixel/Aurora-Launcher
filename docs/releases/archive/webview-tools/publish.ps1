param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][ValidateSet('nsis', 'msi', 'both')][string]$Format,
    [Parameter(Mandatory = $true)][string]$SourceSha,
    [Parameter(Mandatory = $true)][string]$AssetsDirectory,
    [long]$ResumeReleaseId = 0
)

$ErrorActionPreference = 'Stop'
if (-not $env:GH_TOKEN -or -not $env:GITHUB_REPOSITORY) { throw 'GitHub release environment is incomplete' }
& (Join-Path $PSScriptRoot 'windows-artifacts.ps1') -Version $Version -Format $Format -Mode Verify -SourceSha $SourceSha -OutputDirectory $AssetsDirectory
if (-not $?) { throw 'Transferred release artifacts failed verification' }
$arguments = @((Join-Path $PSScriptRoot 'publish.mjs'), $Version, $SourceSha, $AssetsDirectory)
if ($ResumeReleaseId -gt 0) { $arguments += [string]$ResumeReleaseId }
& node @arguments
if ($LASTEXITCODE -ne 0) { throw 'Release publication or public-byte verification failed' }
