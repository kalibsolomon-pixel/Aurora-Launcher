"""P1 read-only preservation inventory. Private paths/hashes never enter public evidence."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
PRIVATE = Path(__file__).resolve().parent / 'private'

def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def git(*args):
    return subprocess.check_output(['git', '-C', str(ROOT), *args]).decode('utf-8').strip()

def capture():
    PRIVATE.mkdir(exist_ok=True)
    assert not (PRIVATE / 'baseline.json').exists(), 'never replace a baseline'
    files = []
    for kind, args in [('tracked', ['ls-files', '-z']), ('untracked', ['ls-files', '--others', '--exclude-standard', '-z'])]:
        for name in git(*args).split('\0'):
            if not name or name.startswith('docs/performance-p1/'):
                continue
            path = ROOT / name
            files.append(dict(kind=kind, path=str(path), bytes=path.stat().st_size, sha256=sha(path)))
    # Preserve every existing managed file, including the known JNA temporaries.
    managed = Path(os.environ['LOCALAPPDATA']) / 'com.aurora.launcher'
    for path in sorted(managed.rglob('*')):
        if path.is_file():
            files.append(dict(kind='managed', path=str(path), bytes=path.stat().st_size, sha256=sha(path)))
    installed = Path(os.environ['LOCALAPPDATA']) / 'Aurora Launcher' / 'aurora-launcher.exe'
    if installed.exists():
        files.append(dict(kind='installed', path=str(installed), bytes=installed.stat().st_size, sha256=sha(installed)))
    data = dict(head=git('rev-parse', 'HEAD'), branch=git('branch', '--show-current'),
                remotes=git('remote', '-v'), status=git('status', '--short', '--untracked-files=all'),
                history=git('log', '-5', '--oneline'), files=files)
    with (PRIVATE / 'baseline.json').open('x', encoding='utf-8') as stream:
        json.dump(data, stream, indent=2)
    print(json.dumps({'head': data['head'], 'counts': {kind: sum(r['kind'] == kind for r in files) for kind in ['tracked', 'untracked', 'managed', 'installed']}}))

def verify():
    data = json.loads((PRIVATE / 'baseline.json').read_text(encoding='utf-8'))
    changed = [r for r in data['files'] if not Path(r['path']).is_file() or sha(Path(r['path'])) != r['sha256']]
    allowed = {str(ROOT / 'src/lib/launcher/store.svelte.ts'), str(ROOT / 'src/lib/launcher/updates.svelte.ts'), str(ROOT / 'ARCHITECTURE.md')}
    managed = Path(os.environ['LOCALAPPDATA']) / 'com.aurora.launcher'
    def operational(row):
        if row['kind'] != 'managed': return False
        relative = Path(row['path']).relative_to(managed).as_posix()
        # Investigated after the first packaged pilot: WebView owns its live
        # profile/log/cache files; normal Home refreshes this derived server cache.
        # Never exempt artifacts, runtimes, instances, accounts or launcher config.
        return relative.startswith(('EBWebView/', 'cache/server-enrichment/'))
    expected_operational = [r for r in changed if operational(r)]
    unexpected = [r for r in changed if r['path'] not in allowed and not operational(r)]
    result = {'checked': len(data['files']), 'changed': len(changed), 'unexpected': len(unexpected),
              'missing': sum(not Path(r['path']).exists() for r in data['files']),
              'normalOperationalWrites': len(expected_operational),
              'protectedManagedChecked': sum(r['kind'] == 'managed' and not operational(r) for r in data['files']),
              'protectedManagedChanged': sum(r['kind'] == 'managed' and not operational(r) for r in changed),
              'protectedKinds': {kind: {'checked': sum(r['kind'] == kind for r in data['files']), 'changed': sum(r['kind'] == kind for r in changed)} for kind in ['untracked', 'managed', 'installed']}}
    (PRIVATE / 'verification-private.json').write_text(json.dumps(changed, indent=2), encoding='utf-8')
    print(json.dumps(result))
    assert not unexpected and not result['missing'], 'investigate unexpected preservation differences'

if __name__ == '__main__':
    capture() if sys.argv[1:] == ['capture'] else verify()
