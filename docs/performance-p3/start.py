"""Start one exact research Launcher, using fresh trial/temp directories."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

root, label = Path(sys.argv[1]).resolve(strict=True), sys.argv[2]
assert root.name.startswith('aurora-p0-2-p3-')
assert label.replace('-', '').isalnum()
for attempt in range(10):
    guard = subprocess.run(['powershell', '-NoProfile', '-Command',
        "if (@(Get-Process -Name 'aurora-launcher','java','javaw' -ErrorAction SilentlyContinue).Count) { exit 2 }"], capture_output=True)
    if guard.returncode == 0: break
    time.sleep(.5)
assert guard.returncode == 0, 'competing Launcher/Java; leave untouched'
baseline = json.loads((root / 'baseline.json').read_text())
for row in baseline['files']:
    if 'jna' in row['relative'].lower() or row['kind'] == 'installed':
        with Path(row['path']).open('rb') as f: assert hashlib.file_digest(f, 'sha256').hexdigest() == row['sha256']
artifact = json.loads((root / 'artifacts.json').read_text())
exe = root / 'target/release/aurora-launcher.exe'
with exe.open('rb') as f: assert hashlib.file_digest(f, 'sha256').hexdigest() == artifact['executableSha256']
for name, key in [('agent.jar','agentSha256'),('markers.jar','markerBridgeSha256'),('startup.jfc','jfrConfigSha256')]:
    with (root/name).open('rb') as f: assert hashlib.file_digest(f,'sha256').hexdigest()==artifact[key]
trial = root / 'trials' / label; trial.mkdir(parents=True, exist_ok=False)
env = dict(os.environ, AURORA_P3_ROOT=str(root), AURORA_P3_TRIAL_ROOT=str(trial),
           AURORA_P0_2_CAPTURE_ROOT=str(trial), AURORA_P3_JFR='0' if label.startswith('control') else '1',
           WEBVIEW2_USER_DATA_FOLDER=str(root / 'webview'))
begin = time.time_ns() // 1000
process = subprocess.Popen([str(exe)], cwd=exe.parent, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
(trial / 'start-private.json').write_text(json.dumps(dict(processId=process.pid, requestUnixUs=begin, returnUnixUs=time.time_ns() // 1000)))
with (trial / 'watch.log').open('xb') as log:
    subprocess.Popen([sys.executable, str(Path(__file__).with_name('watch.py')), str(trial)],
        stdout=log, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW)
print('started', label)
