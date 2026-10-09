"""Strict read-only verification against the immutable private preflight."""
from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess
import sys

def sha(p):
    with p.open('rb') as f: return hashlib.file_digest(f, 'sha256').hexdigest()

root = Path(sys.argv[1]).resolve(strict=True)
label = sys.argv[2]
assert label.replace('-', '').isalnum()
data = json.loads((root / 'baseline.json').read_text())
missing, changed, metadata = [], [], []
for row in data['files']:
    p = Path(row['path'])
    if not p.is_file(): missing.append(row)
    elif sha(p) != row['sha256']: changed.append(row)
    elif p.stat().st_mtime_ns != row['mtimeNs']: metadata.append(row)
receipt = dict(checked=len(data['files']), missing=len(missing), changed=len(changed),
    mtimeChanged=len(metadata), kinds=dict(Counter(r['kind'] for r in data['files'])),
    protectedJnaChecked=sum('jna' in r['relative'].lower() for r in data['files']),
    installedExecutableSha256=next(r['sha256'] for r in data['files'] if r['kind']=='installed'))
repo = Path(__file__).resolve().parents[2]
tracked = set(filter(None, subprocess.check_output(['git','ls-files','-z'],cwd=repo).decode().split('\0')))
original = {r['relative'] for r in data['files'] if r['kind']=='launcher-tracked'}
receipt['originalTrackedMissing'] = len(original - tracked)
details = dict(missing=missing, changed=changed, metadata=metadata)
allowed = lambda row: row['kind'] == 'operational' and row['relative'].startswith(('EBWebView/', 'cache/server-enrichment/'))
receipt['protectedMissing'] = sum(not allowed(r) for r in missing)
receipt['protectedChanged'] = sum(not allowed(r) for r in changed)
receipt['protectedMtimeChanged'] = sum(not allowed(r) for r in metadata)
receipt['operationalMissing'] = sum(allowed(r) for r in missing)
receipt['operationalChanged'] = sum(allowed(r) for r in changed)
receipt['operationalMtimeChanged'] = sum(allowed(r) for r in metadata)
(root / ('preservation-' + label + '-private.json')).write_text(json.dumps(details, indent=2))
(root / ('preservation-' + label + '.json')).write_text(json.dumps(receipt, indent=2))
print(json.dumps(receipt))
assert not receipt['protectedMissing'] and not receipt['protectedChanged'] and not receipt['protectedMtimeChanged'] and not (original-tracked), 'STOP: protected pre-existing data changed'
