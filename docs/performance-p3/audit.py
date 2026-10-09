"""Verify preservation and publish only allowlisted receipts and hashes."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

root=Path(sys.argv[1]).resolve(strict=True)
here=Path(__file__).resolve().parent
repo=here.parents[1]
baseline=json.loads((root/'baseline.json').read_text())
out=here/'evidence'
def sha(p):
    with p.open('rb') as f: return hashlib.file_digest(f,'sha256').hexdigest()
def git(*a): return subprocess.check_output(['git',*a],cwd=repo).decode()
tracked=set(filter(None,git('ls-files','-z').split('\0')))
original={r['relative'] for r in baseline['files'] if r['kind']=='launcher-tracked'}
assert original<=tracked
# No original untracked file may accidentally enter a research commit.
staged=set(filter(None,git('diff','--cached','--name-only','-z').split('\0')))
protected={r['relative'] for r in baseline['files'] if r['kind']=='launcher-untracked'}
assert not staged & protected
assert not git('diff','--cached','--name-only','--diff-filter=D').strip()
assert not git('diff','--name-only','--diff-filter=D').strip()
for r in baseline['files']:
    if r['kind'] in ['launcher-tracked','launcher-untracked','client-tracked','client-untracked']:
        assert Path(r['path']).is_file() and sha(Path(r['path']))==r['sha256']
for critical in ['AGENTS.md','ARCHITECTURE.md','README.md','package.json','package-lock.json',
    'src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/tauri.conf.json','src-tauri/build.rs']:
    assert (repo/critical).is_file()
for name in ['jna-isolation','agent-tests','mapping-provenance']:
    (out/(name+'.json')).write_text(json.dumps(json.loads((root/(name+'.json')).read_text()),indent=2)+'\n')
pres=json.loads((root/'preservation-final.json').read_text())
assert pres['protectedMissing']==pres['protectedChanged']==pres['protectedMtimeChanged']==0
(out/'preservation.json').write_text(json.dumps(pres,indent=2)+'\n')
javaw=next(r for r in baseline['files'] if r['kind']=='managed' and r['relative'].endswith('/bin/javaw.exe'))
checks=json.loads((root/'checks.json').read_text())
verification=dict(checks=checks,originalTrackedCount=len(original),originalUntrackedCount=len(protected),
    clientTrackedCount=sum(r['kind']=='client-tracked' for r in baseline['files']),
    clientUntrackedCount=sum(r['kind']=='client-untracked' for r in baseline['files']),
    originalRepositoryFilesUnchanged=True,originalUntrackedFilesStaged=0,stagedDeletions=0,
    javawSha256=javaw['sha256'],jmc=json.loads((root/'jmc-tool.json').read_text()),
    rawIndexSha256=sha(root/'raw-index.json'),rawRecordingCount=len(json.loads((root/'raw-index.json').read_text())))
(out/'verification.json').write_text(json.dumps(verification,indent=2)+'\n')
# Private values are used as deny terms; never print them on failure.
deny=[str(root),str(Path.home()),Path.home().name,
    *[p.name for p in (root/'managed/instances').iterdir() if p.is_dir()]]
scanned=0
for p in [*out.glob('*.json'),repo/'PERFORMANCE_RESEARCH_P3.md',here/'README.md']:
    if not p.exists(): continue
    data=p.read_text()
    assert all(x.lower() not in data.lower() for x in deny if len(x)>3), 'private identifier in export'
    assert not re.search(r'eyJ[A-Za-z0-9_-]{25,}\.',data), 'JWT-like content in export'
    assert not re.search(r'[A-Za-z]:[\\/](?:Users|Dev|Program Files)[\\/]',data), 'absolute private path in export'
    scanned+=1
(out/'privacy.json').write_text(json.dumps(dict(filesScanned=scanned,privateIdentifiersFound=0,
    jwtLikeValuesFound=0,privateAbsolutePathsFound=0,rawJfrCommitted=False,
    exportedFields='code symbols, numeric aggregates, public identities and hashes only'),indent=2)+'\n')
print('repository membership, preservation receipts and privacy checks passed')
