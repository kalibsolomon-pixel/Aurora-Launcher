param(
  [Parameter(Mandatory=$true)][string]$ResearchRoot,
  [ValidateSet('baseline','candidate','baseline-control','candidate-control','baseline-ipc','candidate-ipc')][string]$Cohort,
  [int]$Trial
)
$ErrorActionPreference = 'Stop'
$competing = @(Get-Process -Name 'aurora-launcher','java','javaw' -ErrorAction SilentlyContinue)
if ($competing.Count -ne 0) { throw 'STOP: competing Launcher or Java process; leave it untouched' }
$resolvedResearch = (Resolve-Path -LiteralPath $ResearchRoot).Path
if (-not ((Split-Path -Leaf $resolvedResearch).StartsWith('aurora-p0-2-p1-'))) { throw 'not a P1 capture root' }
python (Join-Path $PSScriptRoot 'start-trial.py') $resolvedResearch $Cohort $Trial
if ($LASTEXITCODE -ne 0) { throw 'trial startup failed' }
