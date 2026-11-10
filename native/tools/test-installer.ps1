param([Parameter(Mandatory)][string]$Binary)
$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) ('aurora-native9-installer-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$id = [guid]::NewGuid().ToString('N')
$source = (Resolve-Path -LiteralPath $Binary).Path
$payload = Join-Path $root 'accepted-payload.exe'
Copy-Item -LiteralPath $source -Destination $payload
& (Join-Path $PSScriptRoot 'package-windows.ps1') -Binary $payload -OutputDirectory (Join-Path $root 'artifacts') -AcceptanceRoot $root -AcceptanceId $id > (Join-Path $root 'packaging.txt') 2>&1
if (!$?) { throw 'Acceptance packaging failed' }
$installer = (Get-ChildItem -LiteralPath (Join-Path $root 'artifacts') -Filter '*setup.exe').FullName
$installation = Join-Path $root 'installation'
$exe = Join-Path $installation 'aurora-launcher.exe'
$marker = Join-Path $installation 'native-package.ini'
$link = Join-Path $root 'start.lnk'
$registry = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\com.aurora.launcher.native-preview.acceptance.' + $id
$results = [ordered]@{ sourceSha256=(Get-FileHash -LiteralPath $payload).Hash; packageSha256=(Get-FileHash -LiteralPath $installer).Hash }
function Run-Setup([string[]]$Arguments = @('/S')) {
    $process=Start-Process -FilePath $installer -ArgumentList $Arguments -WindowStyle Hidden -PassThru -Wait
    return $process.ExitCode
}
function Uninstall-Owned {
    $uninstaller = Join-Path $installation 'uninstall.exe'
    $process=Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
    for($attempt=0; $attempt -lt 50 -and (Test-Path -LiteralPath $exe); $attempt++) { Start-Sleep -Milliseconds 100 }
    if ((Test-Path -LiteralPath $exe) -or (Test-Path -LiteralPath $registry)) { throw 'Uninstall did not remove its exact package' }
    return $process.ExitCode
}
$results.installExit=Run-Setup
if ($results.installExit -ne 0 -or !(Test-Path -LiteralPath $exe)) { throw 'Install failed' }
$sentinel=Join-Path $installation 'user-sentinel.txt'
[IO.File]::WriteAllText($sentinel,'NATIVE9 user content must survive')
$results.reinstallExit=Run-Setup
if ($results.reinstallExit -ne 0) { throw 'Reinstall failed' }
$expectedHash=(Get-FileHash -LiteralPath $exe).Hash
# Exercise the real filename transition inside this unique acceptance identity.
$legacyExe=Join-Path $installation 'aurora-native.exe'
$shell=New-Object -ComObject WScript.Shell
Move-Item -LiteralPath $exe -Destination $legacyExe
$results.unknownLegacyVersionExit=Run-Setup
if ($results.unknownLegacyVersionExit -eq 0 -or (Test-Path -LiteralPath $exe) -or (Get-FileHash -LiteralPath $legacyExe).Hash -ne $expectedHash) { throw 'Unknown legacy executable version was not preserved/refused' }
$legacyMarker=[IO.File]::ReadAllText($marker) -replace 'Version=[^\r\n]+','Version=2.1.0'
[IO.File]::WriteAllText($marker,$legacyMarker)
Copy-Item -LiteralPath $legacyExe -Destination $exe
$results.ambiguousNamesExit=Run-Setup
if ($results.ambiguousNamesExit -eq 0 -or (Get-FileHash -LiteralPath $legacyExe).Hash -ne $expectedHash -or (Get-FileHash -LiteralPath $exe).Hash -ne $expectedHash) { throw 'Ambiguous executable names were not preserved/refused' }
# Remove only the exact duplicate this fixture just created.
Remove-Item -LiteralPath $exe
$legacyLock=[IO.File]::Open($legacyExe,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
try { $results.legacyInUseExit=Run-Setup } finally { $legacyLock.Dispose() }
if ($results.legacyInUseExit -eq 0 -or (Test-Path -LiteralPath $exe) -or (Get-FileHash -LiteralPath $legacyExe).Hash -ne $expectedHash) { throw 'In-use legacy executable was changed' }
$badMarker=$legacyMarker.Replace(('com.aurora.launcher.native-preview.acceptance.'+$id),'foreign.application')
[IO.File]::WriteAllText($marker,$badMarker)
$results.foreignLegacyPackageExit=Run-Setup
if ($results.foreignLegacyPackageExit -eq 0 -or (Test-Path -LiteralPath $exe) -or (Get-FileHash -LiteralPath $legacyExe).Hash -ne $expectedHash) { throw 'Unknown legacy package was changed' }
[IO.File]::WriteAllText($marker,$legacyMarker)
Move-Item -LiteralPath $link -Destination (Join-Path $root 'legacy-start.lnk')
$legacyLink=$shell.CreateShortcut((Join-Path $root 'legacy-start.lnk'));$legacyLink.TargetPath=$legacyExe;$legacyLink.Save()
$foreignLegacyDesktop=Join-Path $root 'legacy-desktop.lnk'
$foreign=$shell.CreateShortcut($foreignLegacyDesktop);$foreign.TargetPath=Join-Path $env:WINDIR 'notepad.exe';$foreign.Save()
$foreignLegacyHash=(Get-FileHash -LiteralPath $foreignLegacyDesktop).Hash
$results.legacyMigrationExit=Run-Setup
if ($results.legacyMigrationExit -ne 0 -or !(Test-Path -LiteralPath $exe) -or (Test-Path -LiteralPath $legacyExe) -or (Get-FileHash -LiteralPath $exe).Hash -ne $expectedHash -or (Test-Path -LiteralPath (Join-Path $root 'legacy-start.lnk')) -or (Get-FileHash -LiteralPath $foreignLegacyDesktop).Hash -ne $foreignLegacyHash -or $shell.CreateShortcut($link).TargetPath -ne $exe) { throw 'Legacy filename/shortcut migration failed' }
$results.legacyMigrationPreservedSentinel=[IO.File]::ReadAllText($sentinel) -eq 'NATIVE9 user content must survive'
if (!$results.legacyMigrationPreservedSentinel) { throw 'Migration changed user content' }
# A desktop entry belonging to the archived application must not block upgrade.
$foreignDesktop=Join-Path $root 'desktop.lnk'
$foreign=$shell.CreateShortcut($foreignDesktop);$foreign.TargetPath=Join-Path $env:WINDIR 'notepad.exe';$foreign.Save()
$foreignDesktopHash=(Get-FileHash -LiteralPath $foreignDesktop).Hash
$results.foreignDesktopPreservedExit=Run-Setup
if ($results.foreignDesktopPreservedExit -ne 0 -or (Get-FileHash -LiteralPath $foreignDesktop).Hash -ne $foreignDesktopHash) { throw 'Foreign desktop shortcut was changed' }
$createdMutex=$false
$installerMutex=[Threading.Mutex]::new($true,('Local\AuroraNativeInstaller.com.aurora.launcher.native-preview.acceptance.'+$id),[ref]$createdMutex)
try {
    if (!$createdMutex) { throw 'Acceptance mutex unexpectedly exists' }
    $results.concurrentInstallExit=Run-Setup
    # Run this blocked probe in place: the normal NSIS self-copy wrapper's
    # exit code does not report the copied uninstaller's preflight refusal.
    $blockedUninstall=Start-Process -FilePath (Join-Path $installation 'uninstall.exe') -ArgumentList @('/S',('_?='+$installation)) -WindowStyle Hidden -PassThru -Wait
    $results.concurrentUninstallExit=$blockedUninstall.ExitCode
    if ($results.concurrentInstallExit -eq 0 -or $results.concurrentUninstallExit -eq 0 -or (Get-FileHash -LiteralPath $exe).Hash -ne $expectedHash) { $results | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'failed-results.json'); throw 'Concurrent package mutation was not refused' }
} finally {
    if ($createdMutex) { $installerMutex.ReleaseMutex() }
    $installerMutex.Dispose()
}
$shell=New-Object -ComObject WScript.Shell
$results.startShortcutTarget=$shell.CreateShortcut($link).TargetPath
if ($results.startShortcutTarget -ne $exe) { throw 'Start shortcut target mismatch' }
$linkBytes=[IO.File]::ReadAllBytes($link)
$foreign=$shell.CreateShortcut($link);$foreign.TargetPath=Join-Path $env:WINDIR 'notepad.exe';$foreign.Save()
$foreignHash=(Get-FileHash -LiteralPath $link).Hash
$results.foreignShortcutExit=Run-Setup
if ($results.foreignShortcutExit -eq 0 -or (Get-FileHash -LiteralPath $link).Hash -ne $foreignHash) { throw 'Foreign shortcut was not preserved/refused' }
[IO.File]::WriteAllBytes($link,$linkBytes)
$markerBytes=[IO.File]::ReadAllBytes($marker)
$text=[IO.File]::ReadAllText($marker) -replace 'Version=[^\r\n]+','Version=9.0.0'
[IO.File]::WriteAllText($marker,$text)
$results.downgradeExit=Run-Setup
if ($results.downgradeExit -eq 0 -or (Get-FileHash -LiteralPath $exe).Hash -ne $expectedHash) { throw 'Downgrade was not refused safely' }
[IO.File]::WriteAllBytes($marker,$markerBytes)
$lock=[IO.File]::Open($exe,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
try { $results.inUseExit=Run-Setup } finally { $lock.Dispose() }
if ($results.inUseExit -eq 0 -or (Get-FileHash -LiteralPath $exe).Hash -ne $expectedHash) { throw 'In-use executable was changed' }
$helper=Join-Path $root 'wait-fixture.exe'
& rustc (Join-Path $PSScriptRoot 'update-wait-fixture.rs') -o $helper
if ($LASTEXITCODE) { throw 'Wait fixture build failed' }
$owned=Start-Process -FilePath $helper -WindowStyle Hidden -PassThru
$clock=[Diagnostics.Stopwatch]::StartNew()
$results.handoffExit=Run-Setup @('/S',("/UPDATEPID="+$owned.Id))
$clock.Stop();$results.handoffMilliseconds=$clock.Elapsed.TotalMilliseconds
if ($results.handoffExit -ne 0 -or !$owned.HasExited -or $clock.Elapsed.TotalSeconds -lt 3) { throw 'Installer did not wait for its exact owned process' }
$results.malformedPidExit=Run-Setup @('/S','/UPDATEPID=12x')
if ($results.malformedPidExit -eq 0) { throw 'A malformed handoff PID was accepted' }
# The real launcher exits right after spawning the installer, and nothing else
# holds its handle. Start the mirror fixture through WMI so this script holds no
# handle either; the PID is then gone before NSIS opens it. A completed install
# restores the removed notice file.
$exitFixture=Join-Path $root 'exit-fixture.exe'
& rustc (Join-Path $PSScriptRoot 'update-exit-fixture.rs') -o $exitFixture
if ($LASTEXITCODE) { throw 'Exit fixture build failed' }
$notice=Join-Path $installation 'THIRD_PARTY_NOTICES.txt'
Remove-Item -LiteralPath $notice
$created=Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{ CommandLine=('"'+$exitFixture+'" "'+$installer+'"') }
if ($created.ReturnValue -ne 0) { throw 'Exit fixture could not start' }
$installerName=[IO.Path]::GetFileName($installer).Replace("'","''")
$deadline=[DateTime]::UtcNow.AddSeconds(60)
do { Start-Sleep -Milliseconds 250; $active=Get-CimInstance Win32_Process -Filter ("Name='"+$installerName+"' or ProcessId="+$created.ProcessId) } while ($active -and [DateTime]::UtcNow -lt $deadline)
if ($active) { throw 'Fast-exit handoff installer did not finish' }
$results.fastExitHandoffCompleted=(Test-Path -LiteralPath $notice) -and (Get-FileHash -LiteralPath $exe).Hash -eq $expectedHash
if (!$results.fastExitHandoffCompleted) { throw 'Installer aborted when the launcher exited before it could be opened' }
$results.uninstallExit=Uninstall-Owned
$results.startShortcutRemoved=!(Test-Path -LiteralPath $link)
if (!$results.startShortcutRemoved -or [IO.File]::ReadAllText($sentinel) -ne 'NATIVE9 user content must survive') { throw 'Uninstall preservation failed' }
$results.reinstallAfterUninstallExit=Run-Setup
if ($results.reinstallAfterUninstallExit -ne 0) { throw 'Reinstall into a sentinel-preserving directory failed' }
$results.finalUninstallExit=Uninstall-Owned
$results.sentinelPreserved=[IO.File]::ReadAllText($sentinel) -eq 'NATIVE9 user content must survive'
$results.authenticode=(Get-AuthenticodeSignature -LiteralPath $installer).Status.ToString()
$results | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'results.json')
Write-Output $root
$results | ConvertTo-Json
