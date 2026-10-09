"""Start one exact source-built artifact; refuse competing Launcher/Java."""
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

root, cohort, number = Path(sys.argv[1]), sys.argv[2], int(sys.argv[3])
assert root.name.startswith('aurora-p0-2-p2-')
assert cohort in ['baseline', 'attribution', 'control']
assert root.resolve().is_relative_to(Path(os.environ['TEMP']).resolve())
# Process names only; never read command lines, credentials or logs.
guard = subprocess.run(['powershell', '-NoProfile', '-Command',
    "if (@(Get-Process -Name 'aurora-launcher','java','javaw' -ErrorAction SilentlyContinue).Count) { exit 2 }"],
    capture_output=True)
assert guard.returncode == 0, 'STOP: competing Launcher/Java; leave untouched'
manifest = json.loads((Path(__file__).parent / 'evidence' / (cohort + '-artifact.json')).read_text())
executable = root / 'artifacts' / cohort / 'aurora-launcher.exe'
with executable.open('rb') as f:
    assert hashlib.file_digest(f, 'sha256').hexdigest() == manifest['executableSha256']
folder = root / 'trials' / f'{cohort}-{number:02}'
folder.mkdir(parents=True, exist_ok=False)
requested = time.time_ns() // 1_000_000
process = subprocess.Popen([str(executable)], cwd=executable.parent,
                           env=dict(os.environ, AURORA_P0_2_CAPTURE_ROOT=str(folder)),
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
returned = time.time_ns() // 1_000_000
creation, exit_time, kernel, user = [wintypes.FILETIME() for _ in range(4)]
get_times = ctypes.windll.kernel32.GetProcessTimes
get_times.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
assert get_times(wintypes.HANDLE(int(process._handle)), ctypes.byref(creation), ctypes.byref(exit_time), ctypes.byref(kernel), ctypes.byref(user))
created = ((creation.dwHighDateTime << 32) + creation.dwLowDateTime) // 10_000 - 11_644_473_600_000
row = dict(cohort=cohort, trial=number, requestUnixMs=requested, startReturnUnixMs=returned,
           createdUnixMs=created, competingLauncherOrJava=0)
with (folder / 'start.json').open('x') as f: json.dump(row, f, indent=2)
with (folder / 'process-private.json').open('x') as f: json.dump({'processId': process.pid}, f)
print('started', cohort, number)
