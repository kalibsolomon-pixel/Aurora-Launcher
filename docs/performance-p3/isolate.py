"""Exact-JNA positive/negative cleanup controls, with no owner-file writes."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent

def sha(path):
    with Path(path).open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()

def run(root):
    root = Path(root).resolve(strict=True)
    assert root.name.startswith('aurora-p0-2-p3-')
    baseline = json.loads((root / 'baseline.json').read_text())
    managed = Path(os.environ['LOCALAPPDATA']) / 'com.aurora.launcher'
    java = next((managed / 'runtimes').glob('java-runtime-delta/*/bin/java.exe'))
    jar = next((managed / 'instances').glob('*/game/libraries/net/java/dev/jna/jna/5.17.0/jna-5.17.0.jar'))
    controls = root / 'jna-proof'
    controls.mkdir(exist_ok=False)
    subprocess.run(['javac', '--release', '21', '-d', str(controls), str(HERE / 'JnaProbe.java')], check=True)
    receipts = []
    for index in range(3):
        trial = controls / str(index)
        inside, outside, tmp = [trial / n for n in ['jna', 'outside', 'tmp']]
        for folder in [inside, outside, tmp]: folder.mkdir(parents=True)
        for folder in [inside, outside]:
            (folder / 'jna-p3-fixture.dll').write_bytes(b'disposable cleanup control')
            (folder / 'jna-p3-fixture.dll.x').write_bytes(b'')
        outside_hashes = {p.name: sha(p) for p in outside.iterdir()}
        env = dict(os.environ, TEMP=str(tmp), TMP=str(tmp))
        for key in ['JAVA_TOOL_OPTIONS', '_JAVA_OPTIONS', 'JDK_JAVA_OPTIONS', 'CLASSPATH']:
            env.pop(key, None)
        result = subprocess.run([str(java), '-Djna.tmpdir=' + str(inside),
            '-Djava.io.tmpdir=' + str(tmp), '-cp', os.pathsep.join([str(controls), str(jar)]),
            'JnaProbe', str(inside)], env=env, capture_output=True, timeout=20)
        assert result.returncode == 0, 'JNA probe failed (private output retained)'
        assert {p.name: sha(p) for p in outside.iterdir()} == outside_hashes
        receipts.append(dict(trial=index, exitCode=0, cleanupInside=True,
                             extractionInside=True, outsideSentinelsUnchanged=True))
    protected = [r for r in baseline['files'] if 'jna' in r['relative'].lower()]
    assert all(Path(r['path']).is_file() and sha(r['path']) == r['sha256'] for r in protected)
    receipt = dict(javaSha256=sha(java), jnaSha256=sha(jar), trials=receipts,
                   protectedJnaFilesChecked=len(protected), protectedJnaChanged=0)
    (root / 'jna-isolation.json').write_text(json.dumps(receipt, indent=2))
    print(json.dumps(receipt, indent=2))

if __name__ == '__main__': run(sys.argv[1])
