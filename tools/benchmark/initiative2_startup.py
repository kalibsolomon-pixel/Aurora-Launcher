"""Explicit isolated native startup experiment; no production root or credentials read.

The source is the previous initiative's already disposable integration fixture.
Browser cache is isolated; no browser debugging or polling is required for probes.
Close each window normally with Computer Use to flush its bounded trace.
"""
import argparse
import ctypes
from ctypes import wintypes as w
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

from process_capture import Reader


def prepare(root, source, executable):
    assert not root.exists() and root.name.startswith('aurora-p0-2-')
    assert source.is_dir() and 'temp' in str(source).lower(), 'Use an explicit disposable source'
    root.mkdir()
    (root / 'DISPOSABLE-RESEARCH.txt').write_text('Initiative II disposable managed fixture; retain\n')
    shutil.copytree(source, root / 'managed', ignore=shutil.ignore_patterns('logs', 'crash-reports', 'session.lock'))
    shutil.copy2(executable, root / 'reference.exe')
    with executable.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    (root / 'preparation-private.json').write_text(json.dumps(dict(source=str(source),
        executable=str(executable), sha256=digest, createdUtc=dt.datetime.now(dt.timezone.utc).isoformat()), indent=2))
    print(json.dumps(dict(root=str(root), sha256=digest)))


def visible(pid):
    user = ctypes.WinDLL('user32', use_last_error=True)
    callback_type = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
    user.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]
    user.IsWindowVisible.argtypes = [w.HWND]
    found = []
    @callback_type
    def visit(hwnd, unused):
        owner = w.DWORD()
        user.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid and user.IsWindowVisible(hwnd):
            found.append(int(hwnd))
        return True
    user.EnumWindows.argtypes = [callback_type, w.LPARAM]
    user.EnumWindows(visit, 0)
    return found


def launch(root, name, variant, timeout=240):
    assert (root / 'DISPOSABLE-RESEARCH.txt').is_file()
    assert variant in ['reference', 'candidate', 'final']
    assert 30 <= timeout <= 1800
    directory = root / name
    directory.mkdir(exist_ok=False)
    env = dict(os.environ, AURORA_DIAGNOSTIC_DATA_ROOT=str(root / 'managed'),
               WEBVIEW2_USER_DATA_FOLDER=str(root / 'webview'),
               AURORA_P0_2_CAPTURE_ROOT=str(directory))
    with (directory / 'stdout.log').open('xb') as out, (directory / 'stderr.log').open('xb') as err:
        before = time.perf_counter_ns()
        process = subprocess.Popen([str(root / (variant + '.exe'))], cwd=root, env=env,
                                   stdout=out, stderr=err, creationflags=subprocess.CREATE_NO_WINDOW)
        after = time.perf_counter_ns()
        reader = Reader(process.pid)
        record = dict(pid=process.pid, creationId=reader.creation_id, beforeSpawnQpcNs=before,
                      afterSpawnQpcNs=after, variant=variant, cacheState='Warm OS caches; shared isolated WebView profile',
                      utc=dt.datetime.now(dt.timezone.utc).isoformat())
        (directory / 'process.json').write_text(json.dumps(record, indent=2))
        print(json.dumps(record), flush=True)
        window, timedout = None, False
        while process.poll() is None:
            now = time.perf_counter_ns()
            if window is None:
                handles = visible(process.pid)
                if handles:
                    window = dict(beforePollQpcNs=now, observedQpcNs=time.perf_counter_ns(), handles=handles,
                                  pollingPeriodMs=20)
                    print(json.dumps(dict(firstVisibleWindow=window)), flush=True)
            if now - before > timeout * 1e9:
                timedout = True
                process.terminate()  # exact disposable process handle only
                break
            time.sleep(.02 if window is None else .2)
        process.wait(timeout=30)
        reader.close()
        record.update(exitCode=process.returncode, firstVisibleWindow=window, timeout=timedout,
                      endQpcNs=time.perf_counter_ns())
        (directory / 'process.json').write_text(json.dumps(record, indent=2))
        print(json.dumps(dict(exitCode=process.returncode, traceBytes=(directory / 'native-trace.json').stat().st_size)), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest='action', required=True)
    prep = sub.add_parser('prepare')
    prep.add_argument('root', type=Path); prep.add_argument('source', type=Path); prep.add_argument('executable', type=Path)
    run = sub.add_parser('run')
    run.add_argument('root', type=Path); run.add_argument('name'); run.add_argument('--variant', default='reference')
    run.add_argument('--timeout', type=int, default=240)
    args = parser.parse_args()
    if args.action == 'prepare': prepare(args.root.resolve(), args.source.resolve(), args.executable.resolve())
    else: launch(args.root.resolve(), args.name, args.variant, args.timeout)
