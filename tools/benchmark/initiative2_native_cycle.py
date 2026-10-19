"""Configure a marked disposable fixture between native, UI-driven launches.

No application commands, credentials, original instances or release manifests.
Each previous Launcher must have exited normally. Preserve logs and world hashes.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil


def world_hashes(instance):
    return {p.relative_to(instance / 'saves').as_posix():
            hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted((instance / 'saves').rglob('*')) if p.is_file()}


def configure(root, previous, output_name, mode):
    assert 'temp' in str(root).casefold() and (root / 'DISPOSABLE-RESEARCH.txt').is_file()
    prior = json.loads((root / previous / 'process.json').read_text())
    assert prior['exitCode'] == 0 and not prior['timeout'], 'Close the previous Launcher normally first'
    instance = Path(json.loads((root / 'integration-private.json').read_text())['instance']).resolve()
    assert instance.is_relative_to(root) and instance.is_dir()
    hashes = world_hashes(instance)
    baseline = root / 'world-after-normal-sha256.json'
    if baseline.exists():
        assert hashes == json.loads(baseline.read_text()), 'Menu-only cycle changed a world'
    else:
        baseline.write_text(json.dumps(hashes, indent=2) + '\n')
    logs = root / (previous + '-game-logs')
    shutil.copytree(instance / 'logs', logs)
    assert Path(output_name).name == output_name
    output = root / output_name
    output.mkdir(exist_ok=False)
    (output / 'DISPOSABLE-RESEARCH.txt').write_text('Menu-only native recovery diagnostic; retain\n')
    path = root / 'managed/launcher/instances.json'
    state = json.loads(path.read_text())
    assert len(state['instances']) == 1
    cfg = state['instances'][0]['configuration']
    assert cfg['memoryMib'] == 2048 and cfg['window'] is None
    args = ['-Daurora.research.output=' + output.as_posix(),
            '-Daurora.research.kind=startup', '-Daurora.research.menuOnly=true',
            '-Daurora.research.profile=false']
    if mode == 'normal': args.append('-Daurora.research.menuExitSeconds=10')
    cfg['additionalJvmArguments'] = ' '.join(args)
    path.write_text(json.dumps(state, indent=2) + '\n')
    print(json.dumps(dict(output=output_name, mode=mode, protectedWorldFiles=len(hashes),
                          worldHashesMatch=True)))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path); parser.add_argument('previous')
    parser.add_argument('output'); parser.add_argument('mode', choices=['fault', 'normal'])
    args = parser.parse_args()
    configure(args.root.resolve(), args.previous, args.output, args.mode)
