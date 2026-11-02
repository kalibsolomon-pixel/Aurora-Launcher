param([string]$Root)
$ErrorActionPreference='Stop'
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if (!$Root) { $Root=Join-Path ([IO.Path]::GetTempPath()) ('aurora-native9-signed-e2e-'+[guid]::NewGuid().ToString('N')) }
$Root=[IO.Path]::GetFullPath($Root)
$prefix=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')+'\aurora-native9-'
if (!$Root.StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $Root)) { throw 'A new disposable native9 root is required' }
New-Item -ItemType Directory -Path $Root | Out-Null
Set-Content -LiteralPath (Join-Path $Root 'NATIVE-RELEASE-DISPOSABLE.txt') -Value 'Local signed updater acceptance. No production authority or credentials.'
$id=[guid]::NewGuid().ToString('N')
git -C $repo rev-parse HEAD | Set-Content -LiteralPath (Join-Path $Root 'source-sha.txt')
$server=Start-Process -FilePath (Get-Command node).Source -ArgumentList @((Join-Path $PSScriptRoot 'signed-update-server.mjs'),$Root,$id) -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $Root 'server-stderr.txt') -RedirectStandardOutput (Join-Path $Root 'server-stdout.txt')
$publicPath=Join-Path $Root 'local-signing-public.json'
try {
    for($i=0;$i -lt 100 -and !(Test-Path -LiteralPath $publicPath);$i++){Start-Sleep -Milliseconds 100}
    $public=Get-Content -LiteralPath $publicPath -Raw | ConvertFrom-Json
    $env:AURORA_NATIVE_ACCEPTANCE_ROOT=$Root
    $env:AURORA_NATIVE_ACCEPTANCE_ID=$id
    $env:AURORA_NATIVE_ACCEPTANCE_ORIGIN=$public.origin
    $env:AURORA_NATIVE_ACCEPTANCE_KEY=$public.key
    Push-Location $repo
    try {
        foreach($version in @('1.4.2','1.4.3')) {
            $env:AURORA_NATIVE_ACCEPTANCE_VERSION=$version
            cargo build --manifest-path native/Cargo.toml --locked --release --features native-update-acceptance *> (Join-Path $Root "build-$version.txt")
            if($LASTEXITCODE){throw "Acceptance build $version failed; inspect $Root"}
            $binary=Join-Path $Root "aurora-native-$version.exe"
            Copy-Item -LiteralPath (Join-Path $repo 'native/target/release/aurora-native.exe') -Destination $binary
            & (Join-Path $PSScriptRoot 'package-windows.ps1') -Binary $binary -OutputDirectory (Join-Path $Root 'artifacts') -AcceptanceRoot $Root -AcceptanceId $id -AcceptanceVersion $version *> (Join-Path $Root "package-$version.txt")
            if(!$?){throw 'Acceptance packaging failed'}
            $installer=Join-Path $Root "artifacts/Acceptance-$id-Aurora.Native.Preview_${version}_x64-setup.exe"
            if($version -eq '1.4.2') {
                $setup=Start-Process -FilePath $installer -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
                if($setup.ExitCode){throw 'Source installation failed'}
            } else { Copy-Item -LiteralPath $installer -Destination (Join-Path $Root 'signed-installer.exe') }
        }
        # Distribution guard must refuse the exact acceptance executable.
        $rejected=$false
        try { & (Join-Path $PSScriptRoot 'package-windows.ps1') -Binary $binary -OutputDirectory (Join-Path $Root 'must-not-distribute') *> (Join-Path $Root 'distribution-refusal.txt') } catch { $rejected=$true }
        if(!$rejected){throw 'Acceptance executable was admitted to distribution'}
        $sentinel=Join-Path $Root 'managed/instances/user-owned/saves/World/sentinel.dat'
        New-Item -ItemType Directory -Path (Split-Path $sentinel) -Force | Out-Null
        [IO.File]::WriteAllBytes($sentinel,[Text.Encoding]::UTF8.GetBytes('User world must survive signed native update.'))
        $before=(Get-FileHash -LiteralPath $sentinel).Hash
        $installed=Join-Path $Root 'installation/aurora-native.exe'
        $sourceHash=(Get-FileHash -LiteralPath $installed).Hash
        $source=Start-Process -FilePath $installed -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $Root 'source-stderr.txt') -RedirectStandardOutput (Join-Path $Root 'source-stdout.txt')
        $deadline=[DateTime]::UtcNow.AddSeconds(120)
        $ready=Join-Path $Root 'restarted-ready.json'
        while(!(Test-Path -LiteralPath $ready) -and [DateTime]::UtcNow -lt $deadline) {
            if(Test-Path -LiteralPath (Join-Path $Root 'failed.json')){throw 'Updater driver failed; inspect failed.json'}
            Start-Sleep -Milliseconds 250
        }
        if(!(Test-Path -LiteralPath $ready)){throw 'Updated application did not restart ready'}
        $receipt=Get-Content -LiteralPath $ready -Raw | ConvertFrom-Json
        $newHash=(Get-FileHash -LiteralPath $installed).Hash
        $expected=(Get-FileHash -LiteralPath $binary).Hash
        if($receipt.version -ne '1.4.3' -or !$receipt.stateLoaded -or $newHash -ne $expected -or $newHash -eq $sourceHash -or (Get-FileHash -LiteralPath $sentinel).Hash -ne $before -or !(Test-Path -LiteralPath (Join-Path $Root 'handoff.json'))){throw 'Updated identity or data preservation failed'}
        $progress=@(Get-Content -LiteralPath (Join-Path $Root 'progress.jsonl') | ForEach-Object {$_ | ConvertFrom-Json})
        $last=$progress[-1]
        if(!$progress.Count -or $last.downloadedBytes -ne $last.totalBytes -or $last.totalBytes -ne (Get-Item -LiteralPath (Join-Path $Root 'signed-installer.exe')).Length){throw 'Actual transfer progress differs'}
        for($i=1;$i -lt $progress.Count;$i++){if($progress[$i].downloadedBytes -lt $progress[$i-1].downloadedBytes){throw 'Progress moved backward'}}
        [ordered]@{passed=$true;sourceVersion='1.4.2';updatedVersion=$receipt.version;sourceHash=$sourceHash;updatedHash=$newHash;installerHash=(Get-FileHash -LiteralPath (Join-Path $Root 'signed-installer.exe')).Hash;sentinelHash=$before;downloadedBytes=$last.downloadedBytes;progressEvents=$progress.Count;sourcePid=$source.Id;restartPid=$receipt.pid;packageId="com.aurora.launcher.native-preview.acceptance.$id";distributionRejected=$rejected} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Root 'results.json')
        Get-Content -LiteralPath (Join-Path $Root 'results.json')
    } finally {Pop-Location}
} finally {
    [IO.File]::WriteAllText((Join-Path $Root 'stop-server'),'stop')
    foreach($name in @('ROOT','ID','ORIGIN','KEY','VERSION')) { [Environment]::SetEnvironmentVariable("AURORA_NATIVE_ACCEPTANCE_$name",$null,'Process') }
    Write-Output "Evidence: $Root"
}
