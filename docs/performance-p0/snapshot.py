"""Read-only repository protection snapshot; never reads credentials/process arguments."""
import hashlib, json, pathlib, subprocess, sys, time

out = pathlib.Path(sys.argv[1]).resolve()
out.mkdir(parents=True, exist_ok=True)
def git(root, *args):
    try:
        p = subprocess.run(['git', '-C', str(root), *args], capture_output=True, text=True, timeout=25)
    except subprocess.TimeoutExpired:
        return {'exit': None, 'stdout': '', 'stderr': 'read-only query timed out after 25 seconds'}
    return {'exit': p.returncode, 'stdout': p.stdout, 'stderr': p.stderr}
def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda: f.read(1024 * 1024), b''): h.update(block)
    return h.hexdigest()
for label, location in [('launcher', r'C:\Dev\aurora-launcher'), ('client', r'C:\Dev\Aurora-Client')]:
    root = pathlib.Path(location)
    tracked = git(root, 'ls-files', '-z')['stdout'].split('\0')
    untracked = git(root, 'ls-files', '--others', '--exclude-standard', '-z')['stdout'].split('\0')
    state = {'capturedUnix': time.time(), 'repository': label,
             'status': git(root, 'status', '--short', '--untracked-files=all'),
             'branch': git(root, 'branch', '--show-current'), 'head': git(root, 'rev-parse', 'HEAD'),
             'history': git(root, 'log', '-5', '--oneline'),
             'remoteHeadCached': git(root, 'symbolic-ref', '--quiet', 'refs/remotes/origin/HEAD'),
             'remoteHeadLive': git(root, 'ls-remote', '--symref', 'origin', 'HEAD'),
             'upstream': git(root, 'rev-parse', '--abbrev-ref', '@{upstream}'),
             'aheadBehind': git(root, 'rev-list', '--left-right', '--count', 'HEAD...@{upstream}'),
             'files': []}
    for kind, names in [('tracked', tracked), ('untracked', untracked)]:
        for name in names:
            if not name or name.startswith('docs/performance-p0/'): continue
            path = root / name
            if path.is_file():
                state['files'].append({'kind': kind, 'path': name, 'bytes': path.stat().st_size, 'sha256': digest(path)})
            else: state['files'].append({'kind': kind, 'path': name, 'missing': True})
    (out / (label + '-baseline.json')).write_text(json.dumps(state, indent=2), encoding='utf-8')
    print(label, state['head']['stdout'].strip(), 'tracked', sum(x['kind']=='tracked' for x in state['files']),
          'untracked', sum(x['kind']=='untracked' for x in state['files']), 'ahead/behind', state['aheadBehind']['stdout'].strip())
