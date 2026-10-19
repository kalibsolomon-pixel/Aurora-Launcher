"""Observe the exact managed child of one recorded disposable Launcher.
Optional fault injection uses its retained verified handle, never a process name.
"""
import argparse
import ctypes
from ctypes import wintypes as w
import csv
import datetime as dt
import hashlib
import json
from pathlib import Path
import subprocess
import time
from process_capture import Reader
from initiative2_startup import visible


def observe(root, launch, output, kill, timeout=900):
    assert 60 <= timeout <= 1800
    assert (root / 'DISPOSABLE-RESEARCH.txt').is_file()
    parent = json.loads((root / launch / 'process.json').read_text())
    assert 'exitCode' not in parent, 'Launcher must still be running'
    runtime_states = list((root / 'managed/runtimes/java-runtime-delta').glob('*/runtime-installed.json'))
    assert len(runtime_states) == 1
    runtime = json.loads(runtime_states[0].read_text())
    java = (runtime_states[0].parent / runtime['launchExecutable']).resolve()
    assert java.is_relative_to((root / 'managed/runtimes').resolve()) and java.is_file()
    expected = str(java).casefold()
    end = time.monotonic() + 120
    reader = None
    while reader is None:
        command = f'Get-CimInstance Win32_Process -Filter "ParentProcessId={int(parent["pid"])}" | Select-Object ProcessId,ParentProcessId,ExecutablePath | ConvertTo-Json -Compress'
        raw = subprocess.check_output(['powershell.exe', '-NoProfile', '-Command', command], timeout=15, text=True)
        values = json.loads(raw) if raw.strip() else []
        if isinstance(values, dict): values = [values]
        matched = [r for r in values if str(r.get('ExecutablePath', '')).casefold() == expected]
        assert len(matched) <= 1, 'Ambiguous exact managed child'
        if matched: reader = Reader(matched[0]['ProcessId'])
        else:
            assert time.monotonic() < end, 'No exact managed child observed'
            time.sleep(.5)
    record = dict(pid=reader.pid, creationId=reader.creation_id, parentPid=parent['pid'],
                  parentCreationId=parent['creationId'], matchedManagedExecutable=True,
                  observerQpcNs=time.perf_counter_ns(), faultRequested=kill,
                  utc=dt.datetime.now(dt.timezone.utc).isoformat())
    (output / 'process.json').write_text(json.dumps(record, indent=2))
    print(json.dumps(record), flush=True)
    terminate_handle = None
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    if kill:
        kernel.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
        kernel.OpenProcess.restype = w.HANDLE
        kernel.GetProcessTimes.argtypes = [w.HANDLE] + [ctypes.c_void_p] * 4
        kernel.TerminateProcess.argtypes = [w.HANDLE, w.UINT]
        kernel.CloseHandle.argtypes = [w.HANDLE]
        terminate_handle = kernel.OpenProcess(0x100000 | 0x1000 | 0x0001, False, reader.pid)
        assert terminate_handle, 'Cannot retain exact fault-injection handle'
        from process_capture import FILETIME, ticks
        created, exited, system, user = (FILETIME() for _ in range(4))
        assert kernel.GetProcessTimes(terminate_handle, ctypes.byref(created), ctypes.byref(exited), ctypes.byref(system), ctypes.byref(user))
        assert str(ticks(created)) == reader.creation_id, 'Identity changed before handle acquisition'
    samples = []
    log = Path(json.loads((root / 'integration-private.json').read_text())['instance']) / 'logs/latest.log'
    started = time.monotonic(); last = 0; window = None
    try:
        while True:
            now = time.perf_counter_ns()
            try:
                sample = reader.sample()
            except OSError: break
            if now - last >= 500_000_000:
                samples.append(sample); last = now
            if window is None:
                handles = visible(reader.pid)
                if handles: window = dict(observedQpcNs=time.perf_counter_ns(), handles=handles, pollPeriodMs=50)
            if kill and log.exists():
                # Refuse the previous launch's log: FILETIME is Windows UTC 100ns.
                birth_ns = (int(reader.creation_id) - 116444736000000000) * 100
                if log.stat().st_mtime_ns >= birth_ns:
                    text = log.read_text(errors='replace')
                    if 'Checkpoint main-menu-renderable' in text:
                        assert 'Checkpoint world-open-request' not in text, 'Expected a menu-only fault'
                        assert kernel.TerminateProcess(terminate_handle, 23)
                        record['inducedFaultAtMenu'] = True
                        kill = False
            assert time.monotonic() - started < timeout, 'Native child observation timed out'
            time.sleep(.05 if window is None or terminate_handle else .1)
    finally:
        exit_code = w.DWORD()
        if reader.kernel.GetExitCodeProcess(reader.handle, ctypes.byref(exit_code)):
            record['exitCode'] = int(exit_code.value)
        reader.close()
        if terminate_handle: kernel.CloseHandle(terminate_handle)
    record.update(firstVisibleWindow=window, endedQpcNs=time.perf_counter_ns(), samples=len(samples))
    (output / 'process.json').write_text(json.dumps(record, indent=2))
    if samples:
        with (output / 'process.csv').open('x', newline='') as stream:
            writer = csv.DictWriter(stream, fieldnames=list(samples[0])); writer.writeheader(); writer.writerows(samples)
    (output / 'native-game-latest.log').write_bytes(log.read_bytes())
    print(json.dumps(dict(ended=True, **record)), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('root', type=Path)
    parser.add_argument('launch'); parser.add_argument('output', type=Path)
    parser.add_argument('--kill-at-menu', action='store_true')
    parser.add_argument('--timeout', type=int, default=900)
    args = parser.parse_args()
    observe(args.root.resolve(), args.launch, args.output.resolve(), args.kill_at_menu, args.timeout)
