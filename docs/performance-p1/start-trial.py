"""Exact packaged process startup; no logs, arguments or credentials retained."""
import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import subprocess
import sys
import time

root, cohort, number = Path(sys.argv[1]), sys.argv[2], int(sys.argv[3])
assert root.name.startswith('aurora-p0-2-p1-')
assert cohort in ['baseline', 'candidate', 'baseline-control', 'candidate-control', 'baseline-ipc', 'candidate-ipc']
folder = root / 'trials' / f'{cohort}-{number:02}'
folder.mkdir(exist_ok=False)
executable = root / 'artifacts' / cohort / 'aurora-launcher.exe'
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
row = dict(cohort=cohort, trial=number, requestUnixMs=requested, startReturnUnixMs=returned, createdUnixMs=created, competingLauncherOrJava=0)
with (folder / 'start.json').open('x') as stream:
    json.dump(row, stream, indent=2)
with (folder / 'process-private.json').open('x') as stream:
    json.dump({'processId': process.pid}, stream)
print('started', cohort, number)
