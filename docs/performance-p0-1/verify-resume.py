"""Verify preserved data without restoring, cleaning, or inspecting credentials."""
import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys

base = pathlib.Path(__file__).resolve().parent
private = base / 'private/resume-2026-10-09'
label = sys.argv[1]
assert re.fullmatch('[a-z0-9-]+', label)
destination = private / (label + '-verification-details.json')
assert not destination.exists(), 'use a fresh verification label'
authorized = {'PERFORMANCE_RESEARCH_P0_1.md', 'docs/performance-p0-1/README.md'}

def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

details = []
for name, root in [('launcher', pathlib.Path(r'C:\Dev\aurora-launcher')),
                   ('client', pathlib.Path(r'C:\Dev\Aurora-Client'))]:
    baseline = json.loads((private / (name + '-baseline.json')).read_text())
    changed = []
    for entry in baseline['files']:
        if entry.get('missing'):
            continue
        path = root / entry['path']
        if not path.is_file() or sha(path) != entry['sha256']:
            changed.append(entry['path'])
    tracked = set(subprocess.check_output(['git', '-C', str(root), 'ls-files', '-z']).decode().split('\0'))
    removed = [r['path'] for r in baseline['files'] if r['kind'] == 'tracked' and r['path'] not in tracked]
    details.append({'domain': name, 'checkedFiles': len(baseline['files']), 'changed': changed,
                    'unexpected': [p for p in changed if name != 'launcher' or p not in authorized], 'trackedRemoved': removed})

managed = pathlib.Path(os.environ['LOCALAPPDATA']) / 'com.aurora.launcher'
protected = json.loads((private / 'managed-baseline.json').read_text()) + json.loads((private / 'additional-content-baseline.json').read_text())
changed = [r['path'] for r in protected if not (managed / r['path']).is_file() or sha(managed / r['path']) != r['sha256']]
details.append({'domain': 'managed-protected-content-and-state', 'checkedFiles': len(protected),
                'changed': changed, 'unexpected': changed, 'trackedRemoved': []})
payload = pathlib.Path(os.environ['LOCALAPPDATA']) / 'Aurora Launcher/aurora-launcher.exe'
details.append({'domain': 'owner-installed-executable', 'checkedFiles': 1, 'changed': [] if sha(payload) ==
    '15bfd5d71bbe73c9ad7f3010c052f3f1669d81abfa4790b8c48efb8b169fd868' else ['installed-payload'],
    'unexpected': [] if sha(payload) == '15bfd5d71bbe73c9ad7f3010c052f3f1669d81abfa4790b8c48efb8b169fd868' else ['installed-payload'], 'trackedRemoved': []})
destination.write_text(json.dumps(details, indent=2), encoding='utf-8')
public = [{'domain': r['domain'], 'checkedFiles': r['checkedFiles'], 'changedCount': len(r['changed']),
           'unexpectedChangedCount': len(r['unexpected']), 'trackedRemovedCount': len(r['trackedRemoved'])} for r in details]
out = base / ('evidence/as-is-' + label + '-protection.json')
assert not out.exists()
out.write_text(json.dumps(public, indent=2), encoding='utf-8')
print(json.dumps(public, indent=2))
assert not any(r['unexpected'] or r['trackedRemoved'] for r in details), 'inspect private details; never restore blindly'
