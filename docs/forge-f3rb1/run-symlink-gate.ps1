# F3R-B1/B2 Windows file-symlink security gate driver.
#
# Runs the real-retention refusal test against REAL file symbolic links
# placed inside a candidate backup, with a sentinel OUTSIDE the managed
# tree. The fixture lives only in a fresh, marked, direct child of $env:TEMP.
#
# This script changes no Windows security setting, modifies no registry
# value, requests no elevation itself, and launches no production
# application. It removes only its own marked temporary root.
#
# HOW TO RUN (owner instruction, one-time authorization)
# -----------------------------------------------------
# Creating a file symbolic link on Windows needs Developer Mode or
# SeCreateSymbolicLinkPrivilege. Two supported executions:
#
# PREFERRED (link creation elevated, retention verification UNELEVATED):
#   1. In an elevated PowerShell (the only privileged step):
#        powershell -ExecutionPolicy Bypass -File docs\forge-f3rb1\run-symlink-gate.ps1 -Phase prepare
#      This builds the disposable fixture and creates the two real links,
#      then exits. It runs no retention decision.
#   2. In an ordinary (unelevated) PowerShell:
#        powershell -ExecutionPolicy Bypass -File docs\forge-f3rb1\run-symlink-gate.ps1 -Phase verify -Root <printed-root>
#      This executes the real retention refusal assertions unelevated and is
#      the result that counts as acceptance evidence.
#
# SINGLE-SHOT (everything in one elevated process; simplest, but the
# retention assertions then also run elevated — recorded as that limitation):
#   In an elevated PowerShell:
#     powershell -ExecutionPolicy Bypass -File docs\forge-f3rb1\run-symlink-gate.ps1
#
# With Developer Mode enabled, either flow runs completely unelevated.
#
# OUTCOME SEMANTICS
#   PASS    - the real links existed and retention refused the reparse-point
#             content, preserved the sentinel byte-for-byte, reclaimed
#             nothing, and changed no authority.
#   BLOCKED - Windows refused unprivileged file-symlink creation. A creation
#             refusal is never counted as a passing security assertion.

param(
    [ValidateSet("", "prepare", "verify")]
    [string]$Phase = "",
    [string]$Root = ""
)

$ErrorActionPreference = 'Stop'

$id = [Security.Principal.WindowsIdentity]::GetCurrent()
Write-Host ("User: " + $id.Name)
Write-Host ("ElevatedAdmin: " + (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator))
try {
  $dev = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock' -ErrorAction Stop).AllowDevelopmentWithoutDevLicense
  Write-Host ("DeveloperMode: " + $(if ($dev -eq 1) { 'ENABLED' } else { "OFF ($dev)" }))
} catch { Write-Host "DeveloperMode: OFF (key absent)" }
$priv = whoami.exe /priv 2>&1 | Select-String 'SeCreateSymbolicLink'
if ($priv) { Write-Host ("Privilege: " + $priv.Line.Trim()) } else { Write-Host "Privilege: SeCreateSymbolicLink NOT held in this token" }

if ($Phase -eq "" -or $Phase -eq "prepare") {
    if (-not $Root) { $Root = Join-Path $env:TEMP ("aurora-f3rb1-symlink-" + [guid]::NewGuid().ToString('N')) }
}
if (-not $Root) { throw "-Root is required for the verify phase (use the path printed by prepare)" }
Write-Host ("Acceptance root: " + $Root)

$env:AURORA_F3RB1_SYMLINK_ROOT = $Root
if ($Phase) { $env:AURORA_F3RB1_SYMLINK_PHASE = $Phase } else { Remove-Item env:AURORA_F3RB1_SYMLINK_PHASE -ErrorAction SilentlyContinue }

Push-Location (Join-Path $PSScriptRoot '..\..\src-tauri')
try {
  cargo test --release --lib install::retention::tests::windows_file_symlink_refusal_helper -- --exact --nocapture
  if ($LASTEXITCODE -eq 0) {
    if ($Phase -eq "verify") { Write-Host 'SYMLINK-GATE: PASS (real links existed; retention refused them UNELEVATED)' }
    elseif ($Phase -eq "prepare") { Write-Host ('SYMLINK-GATE PREPARED: now run the verify phase unelevated with -Root ' + $Root) }
    else { Write-Host 'SYMLINK-GATE: PASS (single-shot; retention ran in THIS process privilege level)' }
  } else {
    Write-Host 'SYMLINK-GATE: see test output above (BLOCKED means creation was refused; never counted as pass)'
  }
  exit $LASTEXITCODE
} finally {
  Pop-Location
  if ($Phase -ne "prepare") { Remove-Item -Recurse -Force $Root -ErrorAction SilentlyContinue }
}
