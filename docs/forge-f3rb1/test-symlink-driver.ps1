# Wrapper regression only. Mock Cargo failure/empty success is not acceptance.
$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) ('aurora-f3rb2a-driver-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$fixture = Join-Path $root 'unrelated-owner-fixture'
New-Item -ItemType Directory -Path $fixture | Out-Null
[IO.File]::WriteAllBytes((Join-Path $fixture 'sentinel.bin'), [byte[]](0,1,2,255,42))
$driver = Join-Path $PSScriptRoot 'run-symlink-gate.ps1'
$bootstrap = Join-Path $root 'mock-cargo.ps1'
@'
param($Driver, $Fixture, $Phase, [int]$CargoCode)
$ErrorActionPreference = 'Stop'
function global:cargo {
    Write-Output 'test result: ok. 0 passed; 0 failed; 0 ignored'
    $global:LASTEXITCODE = $CargoCode
}
& $Driver -Phase $Phase -Root $Fixture
exit $LASTEXITCODE
'@ | Set-Content -LiteralPath $bootstrap
function Snapshot {
    @((Get-Item -LiteralPath $fixture).FullName) + @(Get-ChildItem -LiteralPath $fixture -Recurse -Force | ForEach-Object {
        $_.FullName + ':' + $(if ($_.PSIsContainer) { 'directory' } else { [Convert]::ToBase64String([IO.File]::ReadAllBytes($_.FullName)) })
    })
}
$before = Snapshot
$passed = 0
foreach ($case in @(@('verify',1), @('verify',0), @('prepare',0))) {
    $ErrorActionPreference = 'Continue'
    $output = & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $bootstrap -Driver $driver -Fixture $fixture -Phase $case[0] -CargoCode $case[1] 2>&1
    $code = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($code -eq 0) { throw "Driver accepted Cargo=$($case[1]) without execution evidence" }
    if (($output -join "`n") -match 'SYMLINK-GATE: PASS') { throw 'False PASS claim' }
    if (Compare-Object $before (Snapshot)) { throw 'Sentinel bytes or directory inventory changed' }
    Write-Host "PASS phase=$($case[0]) mockCargo=$($case[1]) exit=$code sentinelBytesAndInventoryUnchanged=true"
    $passed++
}
Write-Host "Driver regressions: $passed passed; 0 failed; fixtures preserved: $root"
