"""Build source-proven P2 artifacts in new disposable copies; install nothing."""
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
PRIVATE = HERE / 'private'
EVIDENCE = HERE / 'evidence'
PREVIOUS = Path(os.environ['TEMP']) / 'aurora-p0-2-p1-final-wRvJU6'

def sha(p):
    with p.open('rb') as f: return hashlib.file_digest(f, 'sha256').hexdigest()

def write(name, data):
    with (EVIDENCE / name).open('x') as f: json.dump(data, f, indent=2)

head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip()
assert subprocess.run(['git', 'diff', '--quiet'], cwd=REPO).returncode == 0
assert subprocess.run(['git', 'diff', '--cached', '--quiet'], cwd=REPO).returncode == 0
root = Path(tempfile.mkdtemp(prefix='aurora-p0-2-p2-'))
with (PRIVATE / 'research-root.txt').open('x') as f: f.write(str(root))
tracked = [p for p in subprocess.check_output(['git', 'ls-files', '-z'], cwd=REPO).decode().split('\0') if p]
rows = [dict(path=p, bytes=(REPO / p).stat().st_size, sha256=sha(REPO / p)) for p in tracked]
snapshot = hashlib.sha256(json.dumps(rows, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
write('source-snapshot.json', dict(sourceHead=head, sourceSnapshotSha256=snapshot, files=rows))
config = json.loads((PREVIOUS / 'candidate-config.json').read_text())
key = config['plugins']['updater']['pubkey']
key_sha = hashlib.sha256(key.encode()).hexdigest()
assert key_sha == 'bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc'
# Copy and re-hash the P1 optimized research artifact, not its pre-P1 baseline.
baseline = json.loads((REPO / 'docs/performance-p1/evidence/candidate-artifact.json').read_text())
old = PREVIOUS / 'artifacts/candidate'
assert sha(old / 'aurora-launcher.exe') == baseline['executableSha256']
for row in baseline['frontendFiles']: assert sha(old / 'frontend' / row['path']) == row['sha256']
shutil.copytree(old, root / 'artifacts/baseline')
baseline['cohort'] = 'baseline'
write('baseline-artifact.json', baseline)

def build(label):
    stage = root / 'sources' / label
    for row in rows:
        dest = stage / row['path']
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO / row['path'], dest)
        assert sha(dest) == row['sha256']
    shutil.copytree(REPO / 'node_modules', stage / 'node_modules')
    target = root / 'targets' / label
    shutil.copytree(PREVIOUS / ('targets/candidate' if label == 'attribution' else 'targets/candidate-control'), target)
    mode = 'performance-p0-2' if label == 'attribution' else 'production'
    build_config = dict(config, build={'beforeBuildCommand': 'npm exec vite build -- --mode ' + mode})
    path = root / (label + '-config.json')
    path.write_text(json.dumps(build_config))
    env = dict(os.environ, CARGO_TARGET_DIR=str(target), AURORA_UPDATER_PUBKEY=key)
    env.pop('VITE_AURORA_P0_2_IPC', None)
    args = ['npm.cmd', 'run', 'tauri', '--', 'build', '--no-bundle', '--config', str(path)]
    if label == 'attribution': args += ['--features', 'performance-p0-2']
    log = PRIVATE / (label + '-build.log')
    with log.open('xb') as f:
        subprocess.run(['npm.cmd', 'run', 'prepare'], cwd=stage, env=env, stdout=f, stderr=subprocess.STDOUT, check=True)
        subprocess.run(args, cwd=stage, env=env, stdout=f, stderr=subprocess.STDOUT, check=True)
    canonical = []
    for row in rows:
        p = stage / row['path']
        assert p.is_file(), 'STOP: build removed source'
        if sha(p) != row['sha256']:
            assert row['path'] == 'src-tauri/Cargo.toml'
            assert p.read_bytes().replace(b'\r\n', b'\n') == (REPO / row['path']).read_bytes().replace(b'\r\n', b'\n')
            canonical.append(row['path'])
    out = root / 'artifacts' / label
    out.mkdir(parents=True)
    shutil.copy2(target / 'release/aurora-launcher.exe', out / 'aurora-launcher.exe')
    shutil.copytree(stage / 'build', out / 'frontend')
    frontend = [dict(path=p.relative_to(out / 'frontend').as_posix(), bytes=p.stat().st_size, sha256=sha(p))
                for p in sorted((out / 'frontend').rglob('*')) if p.is_file()]
    write(label + '-artifact.json', dict(cohort=label, sourceHead=head, sourceSnapshotSha256=snapshot,
          executableSha256=sha(out / 'aurora-launcher.exe'), executableBytes=(out / 'aurora-launcher.exe').stat().st_size,
          frontendMode=mode, researchFeature=label == 'attribution', separateIpc=False,
          updaterKeySha256=key_sha, frontendFiles=frontend,
          frontendTreeSha256=hashlib.sha256(json.dumps(frontend, sort_keys=True, separators=(',', ':')).encode()).hexdigest(),
          sourceInputsVerified=len(rows), canonicalLineEndingChecks=canonical,
          buildLogSha256=sha(log)))
    print(label, 'built and source verified', flush=True)

with ThreadPoolExecutor(max_workers=2) as pool:
    list(pool.map(build, ['attribution', 'control']))
