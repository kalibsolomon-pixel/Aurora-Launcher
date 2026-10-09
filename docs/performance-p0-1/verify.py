"""Compare the P0.1 inventories without cleanup or attempted restoration."""
import hashlib
import json
import os
import pathlib
import subprocess

base = pathlib.Path(__file__).resolve().parent

def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()

results = []
for label, root in [('launcher', pathlib.Path(r'C:\Dev\aurora-launcher')),
                    ('client', pathlib.Path(r'C:\Dev\Aurora-Client'))]:
    baseline = json.loads((base / 'private' / (label + '-baseline.json')).read_text())
    differences = []
    for entry in baseline['files']:
        path = root / entry['path']
        if entry.get('missing'):
            continue
        if not path.is_file() or sha(path) != entry['sha256']:
            differences.append(entry['path'])
    current = set(subprocess.check_output(['git', '-C', str(root), 'ls-files', '-z']).decode().split('\0'))
    removed = [entry['path'] for entry in baseline['files']
               if entry['kind'] == 'tracked' and entry['path'] not in current]
    results.append({'repository': label, 'checkedFiles': len(baseline['files']),
                    'changedOrMissing': differences, 'trackedRemoved': removed})

# Reuse the preserved P0 owner-content inventory as an additional cross-phase check.
managed = pathlib.Path(os.environ['LOCALAPPDATA']) / 'com.aurora.launcher'
prior = base.parent / 'performance-p0/evidence/instance-protection.json'
if prior.is_file():
    protected = json.loads(prior.read_text())
    changed = [item['path'] for item in protected if not (managed / item['path']).is_file()
               or sha(managed / item['path']) != item['sha256']]
    results.append({'repository': 'P0-protected-managed-content', 'checkedFiles': len(protected),
                    'changedOrMissing': changed, 'comparison': 'preserved P0 baseline'})

# Path-level failures remain local; public output never reveals instance paths.
(base / 'private/verification-details.json').write_text(json.dumps(results, indent=2), encoding='utf-8')
public = [{'repository': item['repository'], 'checkedFiles': item['checkedFiles'],
           'changedOrMissingCount': len(item['changedOrMissing']),
           'trackedRemovedCount': len(item.get('trackedRemoved', []))} for item in results]
(base / 'evidence/protection-verification.json').write_text(json.dumps(public, indent=2), encoding='utf-8')
print(json.dumps(public, indent=2))
assert not any(item['changedOrMissing'] or item.get('trackedRemoved') for item in results), 'inspect local details; do not restore blindly'
