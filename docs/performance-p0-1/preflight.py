"""Read-only identity/precondition checks; no app launch, tokens or command lines."""
import hashlib
import json
import os
import pathlib
import subprocess

base = pathlib.Path(__file__).resolve().parent
root = base.parents[1]
evidence = base / 'evidence'
evidence.mkdir(exist_ok=True)

def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()

def git(*args):
    return subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()

identities = []
for alias, path in [
    ('owner-installed-launcher', pathlib.Path(os.environ['LOCALAPPDATA']) / 'Aurora Launcher/aurora-launcher.exe'),
    ('existing-local-release-launcher', root / 'src-tauri/target/release/aurora-launcher.exe'),
]:
    identities.append({'alias': alias, 'present': path.is_file(),
                       'sha256': sha(path) if path.is_file() else None,
                       'bytes': path.stat().st_size if path.is_file() else None})

# Parent/name facts only. Never request Win32_Process.CommandLine or Java arguments.
code = "Get-CimInstance Win32_Process | Where-Object {$_.Name -in @('aurora-launcher.exe','java.exe','javaw.exe')} | Select-Object Name,ProcessId,ParentProcessId | ConvertTo-Json -Compress"
process_output = subprocess.check_output(['powershell', '-NoProfile', '-Command', code], text=True).strip()
processes = json.loads(process_output) if process_output else []
if isinstance(processes, dict):
    processes = [processes]
launchers = {p['ProcessId'] for p in processes if p['Name'] == 'aurora-launcher.exe'}
children = [p for p in processes if p['Name'] in ('java.exe', 'javaw.exe') and p['ParentProcessId'] in launchers]

baseline = json.loads((base / 'private/launcher-baseline.json').read_text())
client_baseline = json.loads((base / 'private/client-baseline.json').read_text())
reconciliation = git('diff', '--name-only', 'fad40125f8fd87b8fa0893c97827e8e4a890081b',
                     baseline['head']['stdout'].strip()).splitlines()
assert all(p == 'PERFORMANCE_RESEARCH_P0.md' or p.startswith('docs/performance-p0/') for p in reconciliation)
result = {
    'investigatedLauncherHead': baseline['head']['stdout'].strip(),
    'investigatedClientHead': client_baseline['head']['stdout'].strip(),
    'launcherBranch': baseline['branch']['stdout'].strip(),
    'clientBranch': client_baseline['branch']['stdout'].strip(),
    'productionSourceChangedSinceP0': False,
    'committedChangesSinceProductionCodeHead': reconciliation,
    'launcherAheadBehindCachedOriginMain': git('rev-list', '--left-right', '--count', 'HEAD...origin/main').split(),
    'buildIdentities': identities,
    'runningLauncherCount': len(launchers),
    'runningJavaCount': sum(p['Name'] in ('java.exe', 'javaw.exe') for p in processes),
    'launcherHasRunningJavaChild': bool(children),
    'productionIdentifier': json.loads((root / 'src-tauri/tauri.conf.json').read_text())['identifier'],
    'launcherDeclaredVersion': json.loads((root / 'package.json').read_text())['version'],
    # These are source-reviewed preconditions, not results of a packaged isolation test.
    'packagedReleaseDataRootOverride': False,
    'debugOnlyDataRootOverride': True,
    'credentialScopeFollowsDataRoot': False,
    'packagedLifecycleTrialsStarted': 0,
    'ownerBoundary': ('active supervised Java child; isolated production test session not prepared'
                      if children else 'isolated production test session not prepared'),
}
(evidence / 'preflight.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
print(json.dumps(result, indent=2))
