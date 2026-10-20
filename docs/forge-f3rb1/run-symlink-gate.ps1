# F3R-B2a disposable REAL file-symlink gate. All fixtures are preserved.
# No elevation or Windows settings change is performed by this script.
#
# OWNER PROCEDURE (elevation awaits separate authorization):
# prepare invokes Cargo: it may compile/link repository code and run build
# scripts, initializes and promotes synthetic game fixtures, writes evidence,
# and creates two real file links. It performs no retention decision.
#   powershell -NoProfile -ExecutionPolicy Bypass -File docs\forge-f3rb1\run-symlink-gate.ps1 -Phase prepare
# Then run verify in an ordinary process with the printed root:
#   powershell -NoProfile -ExecutionPolicy Bypass -File docs\forge-f3rb1\run-symlink-gate.ps1 -Phase verify -Root <printed-root>
# Review the source/build chain before authorizing privileged prepare. No
# privileged execution is authorized by the F3R-B2a correction request.
# Single-shot reports its actual mode; ELEVATED does not close the ordinary
# process gate. PREPARED and failed link creation never constitute PASS.
param(
    [ValidateSet("", "prepare", "verify")]
    [string]$Phase = "",
    [string]$Root = ""
)
$ErrorActionPreference = 'Stop'

# Actual process-token elevation, not Administrators group membership.
Add-Type -TypeDefinition @"
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class AuroraSymlinkToken {
    [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr token);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool OpenProcessToken(IntPtr process, uint access, out IntPtr token);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool GetTokenInformation(IntPtr token, int kind, out int info, int size, out int returned);
    public static bool Elevated() {
        IntPtr token;
        if (!OpenProcessToken(GetCurrentProcess(), 8, out token)) throw new Win32Exception(Marshal.GetLastWin32Error());
        try {
            int info, returned;
            if (!GetTokenInformation(token, 20, out info, 4, out returned)) throw new Win32Exception(Marshal.GetLastWin32Error());
            if (returned != 4) throw new InvalidOperationException("Unexpected token elevation result");
            return info != 0;
        } finally { CloseHandle(token); }
    }
}
"@
$mode = if ([AuroraSymlinkToken]::Elevated()) { 'ELEVATED' } else { 'UNELEVATED' }
Write-Host "SYMLINK-GATE DRIVER: mode=$mode phase=$Phase"
if ($Phase -eq 'verify' -and $mode -eq 'ELEVATED') { throw 'Elevated verify rejected; fixture preserved.' }
if (-not $Root -and $Phase -ne 'verify') {
    $Root = Join-Path ([IO.Path]::GetTempPath()) ('aurora-f3rb1-symlink-' + [guid]::NewGuid().ToString('N'))
}
if (-not $Root) { throw '-Root is required for verify.' }
Write-Host "Acceptance root: $Root"

$oldRoot = $env:AURORA_F3RB1_SYMLINK_ROOT
$oldPhase = $env:AURORA_F3RB1_SYMLINK_PHASE
$env:AURORA_F3RB1_SYMLINK_ROOT = $Root
$env:AURORA_F3RB1_SYMLINK_PHASE = $Phase
$code = 1
Push-Location (Join-Path $PSScriptRoot '..\..\src-tauri')
try {
    # Rust validates caller roots, marker CONTENTS, strict manifest, slot and
    # inventory before retention. It independently checks its own token.
    # No PowerShell cleanup exists, including when Cargo/validation fails.
    $priorErrorPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue' # Cargo writes test evidence to stderr.
    $output = @(cargo test --release --lib install::retention::tests::windows_file_symlink_refusal_helper -- --exact --ignored --nocapture 2>&1)
    $code = $LASTEXITCODE
    $ErrorActionPreference = $priorErrorPreference
    $output | ForEach-Object { Write-Host $_ }
    if ($code -eq 0) {
        $text = $output -join "`n"
        if ($Phase -eq 'prepare') {
            if ($text -notmatch '(?m)^SYMLINK-GATE-HELPER: PREPARED links=real root=') { throw 'Cargo succeeded without expected PREPARED evidence.' }
            Write-Host "SYMLINK-GATE: PREPARED mode=$mode; verify unelevated with -Root $Root"
        } else {
            $expected = 'SYMLINK-GATE-HELPER: VERIFIED mode=' + $mode + ' links=real dangling=real attempts=2 reclaimed=0 currentValid=true'
            if ($text -notmatch ('(?m)^' + [regex]::Escape($expected) + '$')) { throw 'Cargo succeeded without expected real-link verification evidence.' }
            if ($mode -eq 'UNELEVATED') { Write-Host 'SYMLINK-GATE: PASS mode=UNELEVATED (real assertions executed)' }
            else { Write-Host 'SYMLINK-GATE: VERIFIED mode=ELEVATED (ordinary-process acceptance remains open)' }
        }
    } else { Write-Host 'SYMLINK-GATE: FAILED/BLOCKED; no acceptance claimed. See helper output.' }
} finally {
    Pop-Location
    $env:AURORA_F3RB1_SYMLINK_ROOT = $oldRoot
    $env:AURORA_F3RB1_SYMLINK_PHASE = $oldPhase
    Write-Host "Fixture preserved (no automatic deletion): $Root"
}
exit $code
