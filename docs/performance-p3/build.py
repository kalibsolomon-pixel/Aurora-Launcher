"""Build only in the new private source/output tree. Never install a bundle."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zipfile

HERE = Path(__file__).resolve().parent

def sha(p):
    with Path(p).open('rb') as f: return hashlib.file_digest(f, 'sha256').hexdigest()

def build(root):
    root = Path(root).resolve(strict=True)
    assert root.name.startswith('aurora-p0-2-p3-')
    stage = root / 'source'
    java = next((root / 'managed/runtimes').glob('java-runtime-delta/*/bin/java.exe'))
    asm = next((root / 'managed/instances').glob('*/game/libraries/org/ow2/asm/asm/*/*.jar'))
    classes = Path(tempfile.mkdtemp(prefix='agent-classes-', dir=root))
    subprocess.run(['javac', '--release', '21', '-cp', str(asm), '-d', str(classes),
        str(HERE / 'ResearchAgent.java'), str(HERE / 'Markers.java'), str(HERE / 'BuildConfig.java')], check=True)
    # Use the exact ASM already on the validated game classpath. Bundling a
    # second copy correctly triggers Fabric's duplicate-class integrity guard.
    manifest = root / 'agent-manifest.txt'
    manifest.write_text('Manifest-Version: 1.0\nPremain-Class: p3.ResearchAgent\n\n')
    bridge = root / 'markers.jar'
    subprocess.run(['jar', '--create', '--file', str(bridge), '-C', str(classes), 'p3/Markers.class',
        '-C', str(classes), 'p3/Markers$Phase.class'], check=True)
    agent_only = Path(tempfile.mkdtemp(prefix='agent-only-', dir=root)) / 'p3'
    agent_only.mkdir()
    for p in (classes / 'p3').glob('ResearchAgent*.class'): shutil.copy2(p, agent_only / p.name)
    subprocess.run(['jar', '--create', '--file', str(root / 'agent.jar'), '--manifest', str(manifest), '-C', str(agent_only.parent), '.'], check=True)
    if not (root / 'startup.jfc').exists():
        subprocess.run([str(java), '-cp', str(classes), 'BuildConfig', str(root / 'startup.jfc')], check=True)
    # Keep the reviewed upstream ASM and safe configuration identities explicit.
    config = json.loads((root / 'build-config.json').read_text())
    key = config['plugins']['updater']['pubkey']
    assert hashlib.sha256(key.encode()).hexdigest() == 'bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc'
    env = dict(os.environ, CARGO_TARGET_DIR=str(root / 'target'), AURORA_UPDATER_PUBKEY=key)
    build_tmp = root / 'build-tmp'; build_tmp.mkdir(exist_ok=True)
    env.update(TEMP=str(build_tmp), TMP=str(build_tmp))
    env.pop('VITE_AURORA_P0_2_IPC', None)
    checks = [('frontend-check', ['npm.cmd', 'run', 'check']), ('frontend-test', ['npm.cmd', 'test']),
              ('frontend-build', ['npm.cmd', 'run', 'build']),
              ('rust-format', ['cargo', 'fmt', '--manifest-path', 'src-tauri/Cargo.toml', '--all']),
              ('rust-check', ['cargo', 'check', '--manifest-path', 'src-tauri/Cargo.toml', '--all-targets', '--features', 'performance-p3']),
              ('rust-test', ['cargo', 'test', '--manifest-path', 'src-tauri/Cargo.toml', '--features', 'performance-p3']),
              ('release-build', ['npm.cmd', 'run', 'tauri', '--', 'build', '--no-bundle', '--features', 'performance-p3', '--config', str(root / 'build-config.json')])]
    receipts = json.loads((root / 'checks.json').read_text()) if (root / 'checks.json').exists() else []
    for label, command in checks:
        if any(row['check'] == label and row['exitCode'] == 0 for row in receipts): continue
        print('running', label, flush=True)
        log = root / (label + '.log')
        index = 1
        while log.exists():
            log = root / (label + '.' + str(index) + '.log'); index += 1
        with log.open('xb') as f: result = subprocess.run(command, cwd=stage, env=env, stdout=f, stderr=subprocess.STDOUT)
        receipts.append(dict(check=label, exitCode=result.returncode, logSha256=sha(log)))
        (root / 'checks.json').write_text(json.dumps(receipts, indent=2))
        if result.returncode: raise RuntimeError(label + ' failed; inspect private log')
    exe = root / 'target/release/aurora-launcher.exe'
    (root / 'artifacts.json').write_text(json.dumps(dict(executableSha256=sha(exe),
        agentSha256=sha(root / 'agent.jar'), markerBridgeSha256=sha(bridge), asmSha256=sha(asm), jfrConfigSha256=sha(root / 'startup.jfc'),
        javaSha256=sha(java), compiledResearchSources={p.name:sha(p) for p in HERE.glob('*.java')}), indent=2))
    print('research build complete', flush=True)

if __name__ == '__main__': build(sys.argv[1])
