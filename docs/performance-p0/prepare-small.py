"""Prepare a minimal disposable benchmark; never write in the owner's data root."""
import hashlib, json, os, pathlib, re, shutil, tempfile
evidence = pathlib.Path(__file__).resolve().parent / 'evidence'
source = pathlib.Path(os.environ['LOCALAPPDATA']) / 'com.aurora.launcher'
registry = json.loads((source / 'launcher/instances.json').read_text(encoding='utf-8'))
assert len(registry['instances']) == 1, 'select a source explicitly if multiple instances exist'
record = registry['instances'][0]
identifier = record['id']
assert re.fullmatch('[0-9a-f]{32}', identifier)
instance = source / 'instances' / identifier
def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda:f.read(1024*1024),b''): h.update(block)
    return h.hexdigest()
protected = []
for name in ['game','mods','resourcepacks','shaderpacks']:
    for path in (instance/name).rglob('*'):
        if path.is_file() and not path.is_symlink():
            assert not (getattr(path.stat(),'st_file_attributes',0) & 0x400), 'reparse point'
            protected.append({'path':path.relative_to(source).as_posix(),'sha256':sha(path)})
for path in [source/'launcher/instances.json', source/'launcher/config.json',
             instance/'aurora-installed.json', instance/'aurora-release.json', instance/'content-managed.json']:
    if path.is_file(): protected.append({'path':path.relative_to(source).as_posix(),'sha256':sha(path)})
(evidence/'instance-protection.json').write_text(json.dumps(protected),encoding='utf-8')
assert shutil.disk_usage(tempfile.gettempdir()).free > 2_000_000_000
target = pathlib.Path(tempfile.mkdtemp(prefix='aurora-p0-small-'))
destination = target/'instances'/identifier
destination.mkdir(parents=True)
shutil.copytree(instance/'game',destination/'game',symlinks=True)
state = json.loads((instance/'aurora-installed.json').read_text(encoding='utf-8'))
for artifact in [state['artifact'],state['fabricApi']['artifact']]:
    relative = artifact['relativePath']
    assert relative.startswith('mods/') and '\\' not in relative
    assert all(p not in ['', '.', '..'] for p in relative.split('/'))
    path = instance/relative
    assert sha(path) == artifact['sha256']
    dest = destination/relative
    dest.parent.mkdir(exist_ok=True)
    shutil.copy2(path,dest)
for name in ['aurora-installed.json','aurora-release.json']:
    if (instance/name).is_file(): shutil.copy2(instance/name,destination/name)
for name in ['resourcepacks','shaderpacks']: (destination/name).mkdir()
record['displayName'] = 'P0 minimal disposable'
record['pack'] = None
(target/'launcher').mkdir()
(target/'launcher/instances.json').write_text(json.dumps(registry),encoding='utf-8')
# No accounts, credentials, config, worlds, logs, options or user packs are copied.
(evidence/'small-root.local.txt').write_text(str(target),encoding='utf-8')
print(json.dumps({'protectedFiles':len(protected),'disposableMinimalCreated':True,'requiredJars':2}))
