"""Observe only the child PID returned by the research Rust spawn helper."""
import ctypes
from ctypes import wintypes as w
import json
from pathlib import Path
import sys
import time

trial = Path(sys.argv[1]).resolve(strict=True)
assert trial.parent.name == 'trials'
deadline = time.monotonic() + 120
while not (trial / 'spawn-private.json').exists():
    assert time.monotonic() < deadline, 'no spawn within 120 seconds'
    time.sleep(.05)
pid = json.loads((trial / 'spawn-private.json').read_text())['processId']
k = ctypes.WinDLL('kernel32', use_last_error=True)
k.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
k.OpenProcess.restype = w.HANDLE
k.GetProcessTimes.argtypes = [w.HANDLE] + [ctypes.POINTER(w.FILETIME)] * 4
k.WaitForSingleObject.argtypes = [w.HANDLE, w.DWORD]
k.GetExitCodeProcess.argtypes = [w.HANDLE, ctypes.POINTER(w.DWORD)]
k.CloseHandle.argtypes = [w.HANDLE]
handle = k.OpenProcess(0x100000 | 0x1000, False, pid)
assert handle, 'exact child handle unavailable'
times = [w.FILETIME() for _ in range(4)]
assert k.GetProcessTimes(handle, *(ctypes.byref(t) for t in times))
creation = ((times[0].dwHighDateTime << 32) | times[0].dwLowDateTime) // 10 - 11644473600000000
(trial / 'os-private.json').write_text(json.dumps(dict(processId=pid, creationUnixUs=creation)))
while k.WaitForSingleObject(handle, 1000) == 258:
    assert time.monotonic() < deadline + 600, 'child still active; do not start another trial'
code = w.DWORD()
assert k.GetExitCodeProcess(handle, ctypes.byref(code))
k.CloseHandle(handle)
(trial / 'exit.json').write_text(json.dumps(dict(exitCode=code.value, observedUnixUs=time.time_ns() // 1000)))
print('child exit', code.value)
