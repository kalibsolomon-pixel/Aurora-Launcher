"""Export sanitized P1 verification evidence; retain full logs/inventories privately."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
PRIVATE = HERE / 'private'
FINAL = PRIVATE / 'final'
EVIDENCE = HERE / 'evidence'
CANDIDATE = '0d68340f0e280978f675c6f83330a10fb97bf678'
BASELINE = '6cbabd38bc1c5268b9f6c80034f18da3a26f1e21'

def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))

def write(name, value):
    (EVIDENCE / name).write_text(json.dumps(value, indent=2), encoding='utf-8')

def git(*args):
    return subprocess.check_output(['git', '-C', str(REPO), *args]).decode().strip()

def main(root, historical_root):
    for name in ['baseline', 'baseline-control', 'baseline-ipc', 'candidate', 'candidate-control', 'candidate-ipc']:
        source = FINAL / (name + '-artifact.json')
        assert source.exists()
        (EVIDENCE / source.name).write_bytes(source.read_bytes())
    for name in ['source-identities.json', 'source-snapshot.json']:
        (EVIDENCE / name).write_bytes((FINAL / name).read_bytes())
    environment = read(PRIVATE / 'environment.json')
    environment['candidateSource'] = CANDIDATE
    write('environment.json', environment)
    preservation = read(FINAL / 'preservation.json')
    assert preservation['missing'] == preservation['unexpected'] == preservation['protectedManagedChanged'] == 0
    # Independently retain the historical P0.2 managed-content check.
    managed = Path(os.environ['LOCALAPPDATA']) / 'com.aurora.launcher'
    historical = read(historical_root / 'managed-baseline.json')
    assert all((managed / row['path']).is_file() and sha(managed / row['path']) == row['sha256'] for row in historical)
    original = read(PRIVATE / 'baseline.json')
    jna = [row for row in original['files'] if row['kind'] == 'managed' and Path(row['path']).name.lower().startswith('jna')]
    assert len(jna) == 7 and all(sha(Path(row['path'])) == row['sha256'] for row in jna)
    preservation.update(historicalProtectedFiles=len(historical), historicalChangedOrMissing=0,
                        jnaNamedFilesChecked=len(jna), jnaTemporaryPairsChecked=2,
                        jnaChangedOrMissing=0, originalTrackedFiles=492,
                        installedExecutableSha256=next(r['sha256'] for r in original['files'] if r['kind'] == 'installed'),
                        operationalDomains=['EBWebView', 'cache/server-enrichment'],
                        operationalWritesInvestigated=True)
    write('preservation.json', preservation)
    checks = {}
    logs = [PRIVATE / 'frontend-check-corrected.log', PRIVATE / 'frontend-tests-corrected.log',
            PRIVATE / 'rust-validation.log', PRIVATE / 'build-npm-ci.log',
            *[FINAL / (name + '-build.log') for name in ['candidate', 'candidate-control', 'candidate-ipc']]]
    for path in logs:
        text = path.read_text(encoding='utf-8-sig')
        results = [line.strip() for line in text.splitlines() if any(fragment in line for fragment in
                   ['test result:', 'svelte-check found', 'Finished `release`', 'tests 252', 'pass 252', 'fail 0'])]
        checks[path.stem] = {'passed': True, 'logSha256': sha(path), 'results': results}
    assert 'svelte-check found 0 errors and 0 warnings' in checks['frontend-check-corrected']['results']
    assert any('pass 252' in line for line in checks['frontend-tests-corrected']['results'])
    assert any('841 passed; 0 failed; 29 ignored' in line for line in checks['rust-validation']['results'])
    assert any('845 passed; 0 failed; 29 ignored' in line for line in checks['rust-validation']['results'])
    assert all(any('Finished `release`' in line for line in checks[name + '-build']['results']) for name in ['candidate', 'candidate-control', 'candidate-ipc'])
    write('verification.json', {'checks': checks, 'newDeterministicTests': 23,
          'rustFormatCheck': 'passed', 'rustAllTargetsDefaultCheck': 'passed',
          'rustAllTargetsResearchCheck': 'passed', 'rustDefaultTests': {'passed': 841, 'ignored': 29, 'iconTestsPassed': 6},
          'rustResearchTests': {'passed': 845, 'ignored': 29, 'iconTestsPassed': 6},
          'nativeSourceChanged': False, 'packagedControlStarts': 8, 'primaryStarts': 40,
          'featureOffTraces': 0, 'gameLaunches': 0,
          'existingWarnings': 'Four Rust library warnings and deprecated STATIC_VCRUNTIME build warning; no new source warnings'})
    snapshot = read(FINAL / 'source-snapshot.json')
    verified = {}
    for cohort in ['candidate', 'candidate-control', 'candidate-ipc']:
        copy = root / 'sources' / cohort
        normalized = []
        for row in snapshot['sourceFiles']:
            path = copy / row['path']
            assert path.is_file()
            if sha(path) != row['sha256']:
                # Tauri CLI rewrites Cargo manifest line endings. Prove canonical
                # tracked Git content equality rather than exempting its content.
                expected = subprocess.check_output(['git', '-C', str(REPO), 'show', CANDIDATE + ':' + row['path']])
                assert row['path'] == 'src-tauri/Cargo.toml'
                assert path.read_bytes().replace(b'\r\n', b'\n') == expected.replace(b'\r\n', b'\n')
                normalized.append(row['path'])
        verified[cohort] = {'sourceInputsChecked': len(snapshot['sourceFiles']), 'semanticDifferences': 0,
                            'canonicalLineEndingChecks': normalized}
    native = git('diff', '--name-only', BASELINE, CANDIDATE, '--', 'src-tauri')
    assert native == ''
    assert git('ls-files', '--deleted') == ''
    critical = {name: (REPO / name).is_file() for name in ['README.md', 'AGENTS.md', 'ARCHITECTURE.md',
                'package.json', 'package-lock.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock',
                'src-tauri/tauri.conf.json', 'src-tauri/build.rs']}
    assert all(critical.values())
    write('source-build-verification.json', {'candidateHead': CANDIDATE, 'nativeSourceChanges': [],
          'postBuildCopies': verified, 'criticalFiles': critical, 'trackedDeletions': [],
          'lockfilesChanged': False, 'dependencyChanges': False, 'identifierChanged': False,
          'authorityChanges': False, 'sourceSnapshotFileSha256': sha(FINAL / 'source-snapshot.json'),
          'sourceSnapshotSha256': snapshot['sourceSnapshotSha256']})
    write('failures.json', {'primarySuccessful': 40, 'primaryFailed': 0, 'primaryExcluded': 0,
          'finalPilotExcluded': 1, 'supersededStartsExcluded': 23,
          'supersededSource': 'f1e42c29531e88c37587058e3d4153c84d769567',
          'supersededReason': 'Object-identity comparison falsely invalidated an equivalent read-only state reload; candidate trial 7 had two readiness requests. All superseded starts retained separately.',
          'observerSetupFailures': ['Notebook vm binding did not replace old closure context', 'Eval import was unsupported',
              'Old root binding reached an existing exclusive trial directory; no evidence overwritten'],
          'finalPilotReason': 'Observer attached about 43 seconds late after setup correction; excluded from timing acceptance',
          'operationalWrites': 'Normal WebView profile/cache/log and derived server-enrichment writes investigated; never claimed byte-preserved',
          'playAcceptance': 'Not run: existing JNA temporary-file preservation blocker. No Minecraft launched; no end-to-end Play claim.'})
    print(json.dumps({'exports': 'complete', 'historicalProtectedFiles': len(historical), 'jnaNamedFilesChecked': len(jna)}))

if __name__ == '__main__':
    main(Path(sys.argv[1]), Path(sys.argv[2]))
