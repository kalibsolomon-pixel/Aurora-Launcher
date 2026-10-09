"""Run bounded verification, keep full logs private and export receipts."""
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]

def run(label, args):
    log = HERE / 'private' / (label + '.log')
    with log.open('xb') as f:
        code = subprocess.run(args, cwd=ROOT, stdout=f, stderr=subprocess.STDOUT).returncode
    body = log.read_bytes()
    lines = body.decode('utf-8', errors='replace').splitlines()
    result = dict(exitCode=code, logSha256=hashlib.sha256(body).hexdigest(),
                  results=[line.strip() for line in lines if any(x in line for x in
                           ['test result:', 'svelte-check found', 'pass 252', 'fail 0'])])
    print(label, code, flush=True)
    assert code == 0, label + ' failed; inspect private log'
    return label, result

def frontend():
    return [run('frontend-check', ['npm.cmd', 'run', 'check']),
            run('frontend-test', ['npm.cmd', 'test']),
            run('frontend-build', ['npm.cmd', 'run', 'build'])]

def native():
    base = ['--manifest-path', 'src-tauri/Cargo.toml']
    rows = [run('rust-format', ['cargo', 'fmt', *base, '--all', '--', '--check'])]
    for label, features in [('ordinary', []), ('research', ['--features', 'performance-p0-2'])]:
        rows.append(run('rust-check-' + label, ['cargo', 'check', *base, '--all-targets', *features]))
        rows.append(run('rust-test-' + label, ['cargo', 'test', *base, *features]))
    return rows

with ThreadPoolExecutor(max_workers=2) as pool:
    groups = list(pool.map(lambda f: f(), [frontend, native]))
result = dict(row for group in groups for row in group)
with (HERE / 'evidence/verification-checks.json').open('x') as f:
    json.dump(result, f, indent=2)
