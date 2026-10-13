# F3R-B1 Windows file-symlink security gate driver.
#
# Runs the real-retention refusal test against a REAL file symbolic link
# placed inside a candidate backup, with a sentinel OUTSIDE the managed tree.
# The fixture lives only in a fresh, marked, direct child of $env:TEMP.
#
# Outcome semantics:
#   PASS    - Windows allowed real link creation and retention refused the
#             reparse-point content, preserved the sentinel byte-for-byte,
#             reclaimed nothing, and changed no authority.
#   BLOCKED - Windows refused unprivileged file-symlink creation (requires
#             Developer Mode or SeCreateSymbolicLinkPrivilege). A creation
#             refusal is never counted as a passing security assertion.
#
# No Windows security setting is changed by this script.

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

$root = Join-Path $env:TEMP ("aurora-f3rb1-symlink-" + [guid]::NewGuid().ToString('N'))
Write-Host ("Acceptance root: " + $root)

$env:AURORA_F3RB1_SYMLINK_ROOT = $root
Push-Location (Join-Path $PSScriptRoot '..\..\src-tauri')
try {
  cargo test --release --lib install::retention::tests::windows_file_symlink_refusal_helper -- --exact --nocapture
  if ($LASTEXITCODE -eq 0) {
    Write-Host 'SYMLINK-GATE: PASS (real file symlink existed; retention refused it)'
  } else {
    Write-Host 'SYMLINK-GATE: see test output above (BLOCKED means creation was refused; never counted as pass)'
  }
  exit $LASTEXITCODE
} finally {
  Pop-Location
  Remove-Item -Recurse -Force $root -ErrorAction SilentlyContinue
}
