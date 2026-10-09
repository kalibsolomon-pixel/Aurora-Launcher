"""Read-only P2 preservation; inventories and paths remain private."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
PRIVATE = HERE / 'private'
ALLOWED = {'src-tauri/src/performance.rs', 'src-tauri/src/application.rs',
           'src-tauri/src/updates/client.rs', 'src-tauri/src/instances/lifecycle.rs'}

def sha(p):
    with p.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()

def git(*args):
    return subprocess.check_output(['git', '-C', str(ROOT), *args]).decode().strip()

def operational(p):
    return p.startswith(('EBWebView/', 'cache/server-enrichment/'))

def capture():
    PRIVATE.mkdir(exist_ok=False)
    rows, seen = [], set()
    def add(p, kind, rel):
        if p in seen: return
        seen.add(p)
        assert p.is_file(), 'missing baseline file'
        rows.append(dict(path=str(p), relative=rel, kind=kind,
                         bytes=p.stat().st_size, sha256=sha(p)))
    for kind, args in [('tracked', ['ls-files', '-z']), ('untracked', ['ls-files', '--others', '--exclude-standard', '-z'])]:
        for name in git(*args).split('\0'):
            if name and not name.startswith('docs/performance-p2/'):
                add(ROOT / name, kind, name)
    # Include ignored prior evidence/logs and all existing diagnostics as well.
    for parent in [*ROOT.glob('.zcode*'), *ROOT.glob('docs/performance-*/private')]:
        if parent == PRIVATE: continue
        for p in ([parent] if parent.is_file() else parent.rglob('*')):
            if p.is_file(): add(p, 'diagnostic', p.relative_to(ROOT).as_posix())
    managed = Path(os.environ['LOCALAPPDATA']) / 'com.aurora.launcher'
    for p in sorted(managed.rglob('*')):
        if p.is_file(): add(p, 'operational' if operational(p.relative_to(managed).as_posix()) else 'managed', p.relative_to(managed).as_posix())
    installed = Path(os.environ['LOCALAPPDATA']) / 'Aurora Launcher/aurora-launcher.exe'
    add(installed, 'installed', 'installed-executable')
    data = dict(head=git('rev-parse', 'HEAD'), history=git('log', '-8', '--oneline'),
                status=git('status', '--short', '--untracked-files=all'), files=rows)
    with (PRIVATE / 'baseline.json').open('x') as f: json.dump(data, f, indent=2)
    summary = dict(head=data['head'], p1Ancestor=subprocess.run(['git', 'merge-base', '--is-ancestor', '0d68340f0e280978f675c6f83330a10fb97bf678', 'HEAD']).returncode == 0,
                   counts={kind: sum(r['kind'] == kind for r in rows) for kind in sorted({r['kind'] for r in rows})})
    with (HERE / 'evidence/preflight.json').open('x') as f: json.dump(summary, f, indent=2)
    print(json.dumps(summary))

def verify():
    data = json.loads((PRIVATE / 'baseline.json').read_text())
    changed = [r for r in data['files'] if not Path(r['path']).is_file() or sha(Path(r['path'])) != r['sha256']]
    missing = [r for r in data['files'] if not Path(r['path']).is_file()]
    unexpected = [r for r in changed if r['kind'] != 'operational' and not (r['kind'] == 'tracked' and r['relative'] in ALLOWED)]
    result = dict(checked=len(data['files']), missing=len(missing), unexpected=len(unexpected),
                  kinds={k: dict(checked=sum(r['kind'] == k for r in data['files']), changed=sum(r['kind'] == k for r in changed)) for k in sorted({r['kind'] for r in data['files']})})
    managed = Path(os.environ['LOCALAPPDATA']) / 'com.aurora.launcher'
    original_managed = {r['relative'] for r in data['files'] if r['kind'] in {'managed', 'operational'}}
    new_managed = [p.relative_to(managed).as_posix() for p in managed.rglob('*')
                   if p.is_file() and p.relative_to(managed).as_posix() not in original_managed]
    jna = [r for r in data['files'] if 'jna' in r['relative'].lower()]
    installed = next(r for r in data['files'] if r['kind'] == 'installed')
    result.update(jnaNamedFilesChecked=len(jna),
                  jnaChangedOrMissing=sum(r in changed for r in jna),
                  installedExecutableSha256=sha(Path(installed['path'])),
                  operationalDomains=['EBWebView', 'cache/server-enrichment'],
                  newOperationalFiles=sum(operational(p) for p in new_managed),
                  newProtectedManagedFiles=sum(not operational(p) for p in new_managed))
    (PRIVATE / 'preservation-differences.json').write_text(json.dumps(changed, indent=2))
    print(json.dumps(result))
    assert not missing and not unexpected and not result['newProtectedManagedFiles'], 'STOP: unexpected protected-file difference'

if __name__ == '__main__':
    capture() if sys.argv[1:] == ['capture'] else verify()
