"""Create an immutable, private preservation baseline in a fresh disposable root.

Review repository guidance and prior reports before invoking this tool. The tool
records document hashes; it does not substitute for reading the documents.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

repo=Path(__file__).resolve().parents[2]
client=Path(sys.argv[1]).resolve(strict=True)
root=Path(tempfile.mkdtemp(prefix='aurora-p0-2-p3-'))
files=[]; repos={}; seen=set()

def git(path,*args): return subprocess.check_output(['git',*args],cwd=path).decode('utf-8')

def add(path,kind,relative):
    if not path.is_file() or str(path) in seen: return
    assert not path.is_symlink() and not path.is_junction(), 'linked protected input needs explicit review'
    seen.add(str(path)); info=path.stat()
    with path.open('rb') as f: digest=hashlib.file_digest(f,'sha256').hexdigest()
    assert path.stat().st_mtime_ns==info.st_mtime_ns, 'input changed during preflight'
    files.append(dict(path=str(path),kind=kind,relative=str(relative).replace('\\','/'),
        bytes=info.st_size,mtimeNs=info.st_mtime_ns,sha256=digest))

for key,path in [('launcher',repo),('client',client)]:
    tracked=list(filter(None,git(path,'ls-files','-z').split('\0')))
    untracked=list(filter(None,git(path,'ls-files','--others','--exclude-standard','-z').split('\0')))
    docs={}
    for name in ['AGENTS.md','ARCHITECTURE.md','README.md',*sorted(p.name for p in path.glob('PERFORMANCE_*.md'))]:
        p=path/name
        if p.is_file(): docs[name]=hashlib.sha256(p.read_bytes()).hexdigest()
    assert 'AGENTS.md' in docs and 'ARCHITECTURE.md' in docs
    repos[key]=dict(head=git(path,'rev-parse','HEAD').strip(),branch=git(path,'branch','--show-current').strip(),
        remotes=git(path,'remote','-v'),status=git(path,'status','--short','--untracked-files=all'),
        history=git(path,'log','-8','--oneline'),tracked=tracked,documents=docs)
    for name in tracked: add(path/name,key+'-tracked',name)
    for name in untracked: add(path/name,key+'-untracked',name)
    ignored=list(filter(None,git(path,'ls-files','--others','--ignored','--exclude-standard','-z').split('\0')))
    for name in ignored:
        if name.startswith(('.zcode','docs/performance-')): add(path/name,key+'-diagnostic',name)
managed=Path(os.environ['LOCALAPPDATA'])/'com.aurora.launcher'
for p in managed.rglob('*'):
    rel=p.relative_to(managed).as_posix()
    kind='operational' if rel.startswith(('EBWebView/','cache/server-enrichment/')) else 'managed'
    add(p,kind,rel)
installed=Path(os.environ['LOCALAPPDATA'])/'Aurora Launcher/aurora-launcher.exe'
assert installed.is_file();add(installed,'installed',installed.name)
for p in Path(tempfile.gettempdir()).glob('jna*'):
    if p.is_file(): add(p,'native-temp',p.name)
    elif p.is_dir():
        for f in p.rglob('*'): add(f,'native-temp',f.relative_to(p.parent))
with (root/'baseline.json').open('x',encoding='utf-8') as f: json.dump(dict(repositories=repos,files=files),f,indent=2)
print(root)
