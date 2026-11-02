param([Parameter(Mandatory)][string]$Root)
$ErrorActionPreference='Stop'
$Root=[IO.Path]::GetFullPath($Root)
$prefix=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')+'\aurora-native9-'
if(!$Root.StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase) -or !(Test-Path -LiteralPath (Join-Path $Root 'NATIVE-RELEASE-DISPOSABLE.txt'))){throw 'Marked disposable signed-update root required'}
$accepted=Get-Content -LiteralPath (Join-Path $Root 'results.json') -Raw | ConvertFrom-Json
if(!$accepted.passed -or $accepted.packageId -notmatch '^com\.aurora\.launcher\.native-preview\.acceptance\.[a-f0-9]{32}$'){throw 'Successful isolated signed acceptance required first'}
$exe=Join-Path $Root 'installation/aurora-native.exe'
$installer=Join-Path $Root 'signed-installer.exe'
$sentinel=Join-Path $Root 'managed/instances/user-owned/saves/World/sentinel.dat'
$expected=(Get-FileHash -LiteralPath $exe).Hash
if($expected -ne $accepted.updatedHash -or (Get-FileHash -LiteralPath $installer).Hash -ne $accepted.installerHash){throw 'Accepted package bytes changed'}
$deadline=[DateTime]::UtcNow.AddSeconds(20)
while((Get-Process -Id $accepted.restartPid -ErrorAction SilentlyContinue) -and [DateTime]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 100}
if(Get-Process -Id $accepted.restartPid -ErrorAction SilentlyContinue){throw 'Exact restarted acceptance process is still running'}
function Invoke-RefusedSetup {
    $setup=Start-Process -FilePath $installer -ArgumentList '/S' -WindowStyle Hidden -Wait -PassThru
    if($setup.ExitCode -eq 0 -or (Get-FileHash -LiteralPath $exe).Hash -ne $expected -or (Get-FileHash -LiteralPath $sentinel).Hash -ne $accepted.sentinelHash){throw 'Refused update changed accepted installation or user world'}
    return $setup.ExitCode
}
$results=[ordered]@{}
$lock=[IO.File]::Open($exe,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
try{$results.executableInUseExit=Invoke-RefusedSetup}finally{$lock.Dispose()}
$created=$false
$mutex=[Threading.Mutex]::new($true,('Local\AuroraNativeInstaller.'+$accepted.packageId),[ref]$created)
try{if(!$created){throw 'Another exact acceptance installer owns the mutex'};$results.packageLockExit=Invoke-RefusedSetup}finally{if($created){$mutex.ReleaseMutex()};$mutex.Dispose()}
$staging=Join-Path $Root 'installation/native-new.exe'
if(Test-Path -LiteralPath $staging){throw 'Pre-existing staging evidence must not be overwritten'}
[IO.File]::WriteAllText($staging,'Interrupted test staging: preserve until verified cleanup')
try{
    $results.interruptedStagingExit=Invoke-RefusedSetup
    if([IO.File]::ReadAllText($staging) -ne 'Interrupted test staging: preserve until verified cleanup'){throw 'Interrupted staging evidence changed'}
}finally{
    # Exact file positively created by this test under the checked disposable root.
    if([IO.Path]::GetFullPath($staging).StartsWith($Root+'\',[StringComparison]::OrdinalIgnoreCase) -and (Test-Path -LiteralPath $staging) -and [IO.File]::ReadAllText($staging) -eq 'Interrupted test staging: preserve until verified cleanup'){Remove-Item -LiteralPath $staging}
}
$marker=[IO.File]::ReadAllText((Join-Path $Root 'installation/native-package.ini'))
if(!$marker.Contains('Identity='+$accepted.packageId) -or !$marker.Contains('Version=1.4.3')){throw 'Installed native package identity/version differs'}
$entry=Get-ItemProperty -LiteralPath ('HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\'+$accepted.packageId)
if($entry.DisplayVersion -ne '1.4.3' -or $entry.InstallLocation -ne (Join-Path $Root 'installation')){throw 'Exact acceptance uninstall registration differs'}
$results.packageIdentityVerified=$true
$results.userWorldPreserved=(Get-FileHash -LiteralPath $sentinel).Hash -eq $accepted.sentinelHash
$results.executablePreserved=(Get-FileHash -LiteralPath $exe).Hash -eq $expected
$results | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Root 'recovery-results.json')
$results | ConvertTo-Json
