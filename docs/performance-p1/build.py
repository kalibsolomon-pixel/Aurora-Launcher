"""Build task-owned artifacts; preserve old evidence and approved updater trust."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import os
import sys
from concurrent.futures import ThreadPoolExecutor

root, source, previous, compiler_cache = map(Path, sys.argv[1:])
repo = Path(__file__).resolve().parents[2]
private = repo / 'docs/performance-p1/private/final'
private.mkdir(exist_ok=False)
def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

source_head = subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip()
assert source_head == '0d68340f0e280978f675c6f83330a10fb97bf678'
assert subprocess.run(['git', '-C', str(source), 'diff', '--quiet']).returncode == 0
tracked = subprocess.check_output(['git', '-C', str(source), 'ls-files', '-z']).decode().split('\0')
tracked = [p for p in tracked if p]
source_rows = [dict(path=p, bytes=(source / p).stat().st_size, sha256=sha(source / p)) for p in tracked]
snapshot_sha = hashlib.sha256(json.dumps(source_rows, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
with (private / 'source-snapshot.json').open('x') as stream:
    json.dump(dict(sourceHead=source_head, sourceFiles=source_rows, sourceSnapshotSha256=snapshot_sha), stream, indent=2)
for label, old_label in [('baseline', 'profile'), ('baseline-control', 'control'), ('baseline-ipc', 'ipc')]:
    manifest = json.loads((previous / (old_label + '-artifact.json')).read_text())
    assert sha(previous / 'artifacts' / old_label / 'aurora-launcher.exe') == manifest['executableSha256']
    shutil.copytree(previous / 'artifacts' / old_label, root / 'artifacts' / label)
    manifest['cohort'] = label
    with (private / (label + '-artifact.json')).open('x') as stream:
        json.dump(manifest, stream, indent=2)

config = json.loads((previous / 'production-build-config.json').read_text())
key = config['plugins']['updater']['pubkey']
assert hashlib.sha256(key.encode()).hexdigest() == 'bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc'
def build(settings):
    label, mode, feature, ipc = settings
    stage = root / 'sources' / label
    stage.mkdir(parents=True)
    for p in tracked:
        destination = stage / p
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source / p, destination)
        assert sha(destination) == sha(source / p)
    # Independent frontend and compiler outputs permit two bounded concurrent
    # builds. Old cache/source/artifacts stay untouched, including rejected trials.
    shutil.copytree(source / 'node_modules', stage / 'node_modules')
    target = root / 'targets' / label
    shutil.copytree(compiler_cache, target)
    build_config = dict(config, build={'beforeBuildCommand': 'npm exec vite build -- --mode ' + mode})
    config_path = root / (label + '-config.json')
    with config_path.open('x') as stream:
        json.dump(build_config, stream)
    build_env = dict(os.environ, CARGO_TARGET_DIR=str(target), AURORA_UPDATER_PUBKEY=key)
    if ipc: build_env['VITE_AURORA_P0_2_IPC'] = '1'
    else: build_env.pop('VITE_AURORA_P0_2_IPC', None)
    args = ['npm.cmd', 'run', 'tauri', '--', 'build', '--no-bundle', '--config', str(config_path)]
    if feature: args += ['--features', 'performance-p0-2']
    with (private / (label + '-build.log')).open('x') as log:
        subprocess.run(['npm.cmd', 'run', 'prepare'], cwd=stage, env=build_env, stdout=log, stderr=subprocess.STDOUT, check=True)
        subprocess.run(args, cwd=stage, env=build_env, stdout=log, stderr=subprocess.STDOUT, check=True)
    out = root / 'artifacts' / label
    out.mkdir()
    shutil.copy2(target / 'release/aurora-launcher.exe', out / 'aurora-launcher.exe')
    shutil.copytree(stage / 'build', out / 'frontend')
    for row in source_rows:
        p = stage / row['path']
        assert p.is_file(), 'build removed a source file'
        if row['path'] == 'src-tauri/Cargo.toml':
            assert p.read_text() == (source / row['path']).read_text(), 'manifest changed beyond CLI line endings'
        else: assert sha(p) == row['sha256'], 'build changed a source file'
    rows = [dict(path=p.relative_to(out / 'frontend').as_posix(), bytes=p.stat().st_size, sha256=sha(p))
            for p in sorted((out / 'frontend').rglob('*')) if p.is_file()]
    manifest = dict(cohort=label, sourceHead=source_head, sourceSnapshotSha256=snapshot_sha,
                    executableSha256=sha(out / 'aurora-launcher.exe'), executableBytes=(out / 'aurora-launcher.exe').stat().st_size,
                    frontendFiles=rows, frontendTreeSha256=hashlib.sha256(json.dumps(rows, sort_keys=True, separators=(',', ':')).encode()).hexdigest(),
                    frontendMode=mode, researchFeature=feature, separateIpc=ipc, updaterKeySha256=hashlib.sha256(key.encode()).hexdigest())
    with (private / (label + '-artifact.json')).open('x') as stream:
        json.dump(manifest, stream, indent=2)
    print(label, manifest['executableSha256'], flush=True)

with ThreadPoolExecutor(max_workers=2) as pool:
    list(pool.map(build, [('candidate', 'performance-p0-2', True, False),
                          ('candidate-control', 'production', False, False),
                          ('candidate-ipc', 'performance-p0-2', True, True)]))
