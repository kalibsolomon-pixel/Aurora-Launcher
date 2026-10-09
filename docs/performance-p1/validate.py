"""Read-only P1 evidence, privacy, artifact and source-contract checks."""
import hashlib
import json
from pathlib import Path, PureWindowsPath
import re
import sys
sys.dont_write_bytecode = True
from analyze import p02

HERE = Path(__file__).resolve().parent
EVIDENCE = HERE / 'evidence'

def privacy(value, location=''):
    if isinstance(value, dict):
        for key, item in value.items():
            assert key.lower() not in {'password', 'token', 'accesstoken', 'refreshtoken', 'accountid', 'username', 'email', 'processid', 'pid', 'arguments', 'stderr', 'rawerror'}
            privacy(item, location + '/' + key)
    elif isinstance(value, list):
        for i, item in enumerate(value): privacy(item, location + '/' + str(i))
    elif isinstance(value, str):
        # Tauri's checked-in HiDPI icon filename resembles an email address.
        if value == 'src-tauri/icons/128x128@2x.png' and location.startswith('source-snapshot.json/sourceFiles/') and location.endswith('/path'):
            return
        assert not re.search(r'(?i)[a-z]:[\\/]|/Users/|/home/|bearer\s+|[\w.+-]+@[\w.-]+\.[a-zA-Z]{2,}', value), location

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    for path in EVIDENCE.rglob('*.json'):
        privacy(json.loads(path.read_text(encoding='utf-8-sig')), path.name)
    rejected = EVIDENCE / 'pre-correction'
    for path in rejected.glob('trace-*.json'): p02.trace(path)
    summary = json.loads((EVIDENCE / 'summary.json').read_text())
    traces = {p.name: p02.trace(p) for p in sorted(EVIDENCE.glob('trace-*.json'))}
    assert summary['traces'] == traces
    assert summary['cohorts']['baseline']['n'] == summary['cohorts']['candidate']['n'] == 20
    assert summary['cohorts']['baseline']['countsPerRun']['readiness'] == [2]
    assert summary['cohorts']['baseline']['countsPerRun']['gameIntegrity'] == [4]
    assert summary['cohorts']['candidate']['countsPerRun']['readiness'] == [1]
    assert summary['cohorts']['candidate']['countsPerRun']['gameIntegrity'] == [3]
    assert summary['cohorts']['candidate']['readyCheckingReadyRuns'] == 0
    assert summary['gameLaunches'] == 0
    trials = json.loads((EVIDENCE / 'trials.json').read_text())
    assert len(trials) == 61 and len({(r['cohort'], r['trial']) for r in trials}) == 61
    assert all(r['success'] and r['closedNormally'] and r['competingLauncherOrJava'] == 0 and r['observedForMs'] >= 8000 for r in trials)
    primary = [r for r in trials if r['phase'] == 'matched']
    assert len(primary) == 40
    for n in range(1, 21):
        pair = sorted([r for r in primary if r['trial'] == n], key=lambda r: r['createdUnixMs'])
        assert [r['cohort'] for r in pair] == (['baseline', 'candidate'] if n % 2 else ['candidate', 'baseline'])
    assert len(traces) == 53
    assert len(list(EVIDENCE.glob('*-artifact.json'))) == 6
    snapshot = json.loads((EVIDENCE / 'source-snapshot.json').read_text())
    assert hashlib.sha256(json.dumps(snapshot['sourceFiles'], sort_keys=True, separators=(',', ':')).encode()).hexdigest() == snapshot['sourceSnapshotSha256']
    for path in EVIDENCE.glob('trace-*.json'):
        if '-ipc-' in path.name or path.stem.endswith('-00') or int(path.stem[-2:]) > 20: continue
        doc = json.loads(path.read_text())
        assert {r['hash_calls'] for r in doc['records'] if r['event'] == 20 and r['edge'] == 2} == {4677}
        assert {r['hash_calls'] for r in doc['records'] if r['event'] == 24 and r['edge'] == 2} == {402}
    artifacts = Path(sys.argv[1]) / 'artifacts' if len(sys.argv) > 1 else None
    for path in EVIDENCE.glob('*-artifact.json'):
        manifest = json.loads(path.read_text())
        rows = manifest['frontendFiles']
        assert rows == sorted(rows, key=lambda r: PureWindowsPath(r['path']))
        assert hashlib.sha256(json.dumps(rows, sort_keys=True, separators=(',', ':')).encode()).hexdigest() == manifest['frontendTreeSha256']
        assert manifest['sourceHead'] == ('6cbabd38bc1c5268b9f6c80034f18da3a26f1e21' if manifest['cohort'].startswith('baseline') else '0d68340f0e280978f675c6f83330a10fb97bf678')
        if manifest['cohort'].startswith('candidate'):
            assert manifest['sourceSnapshotSha256'] == snapshot['sourceSnapshotSha256']
        for row in rows:
            assert not Path(row['path']).is_absolute() and '..' not in Path(row['path']).parts
        if artifacts:
            root = artifacts / manifest['cohort']
            assert sha(root / 'aurora-launcher.exe') == manifest['executableSha256']
            for row in rows: assert sha(root / 'frontend' / row['path']) == row['sha256']
    for file in ['README.md', 'AGENTS.md', 'ARCHITECTURE.md', 'package.json', 'package-lock.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/tauri.conf.json', 'PERFORMANCE_OPTIMIZATION_P1.md']:
        assert (HERE.parents[1] / file).is_file()
    print(json.dumps({'numericTraces': len(traces), 'matchedPairs': 20, 'artifactManifests': 6, 'privacyAndCriticalFiles': 'passed'}))

if __name__ == '__main__': main()
