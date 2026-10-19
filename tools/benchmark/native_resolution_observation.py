"""Observe only numeric resolution/heap arguments of an exact disposable child.

The complete Windows command line stays in memory and is never emitted or saved.
Actual framebuffer dimensions must be observed independently at runtime.
"""
import argparse
import ctypes
from ctypes import wintypes as w
import json
from pathlib import Path
import subprocess
import time
from process_capture import Reader


def observe(output):
    assert 'temp' in str(output).casefold() and (output / 'DISPOSABLE-RESEARCH.txt').is_file()
    deadline = time.monotonic() + 90
    while not (output / 'process.json').exists():
        assert time.monotonic() < deadline, 'No exact child observed'
        time.sleep(.1)
    record = json.loads((output / 'process.json').read_text())
    reader = Reader(record['pid'])
    try:
        assert reader.creation_id == record['creationId'], 'Child identity changed'
        query = f'Get-CimInstance Win32_Process -Filter "ProcessId={int(record["pid"])}" | Select-Object -ExpandProperty CommandLine | ConvertTo-Json -Compress'
        command_line = json.loads(subprocess.check_output(['powershell.exe', '-NoProfile', '-Command', query],
                                                          text=True, timeout=15))
        assert isinstance(command_line, str) and command_line
        shell, kernel = ctypes.WinDLL('shell32'), ctypes.WinDLL('kernel32')
        shell.CommandLineToArgvW.argtypes = [w.LPCWSTR, ctypes.POINTER(ctypes.c_int)]
        shell.CommandLineToArgvW.restype = ctypes.POINTER(w.LPWSTR)
        kernel.LocalFree.argtypes = [w.HLOCAL]; kernel.LocalFree.restype = w.HLOCAL
        count = ctypes.c_int()
        pointer = shell.CommandLineToArgvW(command_line, ctypes.byref(count))
        assert pointer
        try: args = [pointer[i] for i in range(count.value)]
        finally: kernel.LocalFree(ctypes.cast(pointer, w.HLOCAL))
        def dimension(flag):
            indices = [i for i,arg in enumerate(args) if arg == flag]
            assert len(indices) <= 1 and all(i + 1 < len(args) for i in indices)
            value = int(args[indices[0] + 1]) if indices else None
            return value
        result = dict(width=dimension('--width'), height=dimension('--height'),
                      unresolvedResolution=any('${resolution_' in arg for arg in args),
                      managedHeap2048MiB=args.count('-Xmx2048m') == 1,
                      exactChildIdentityMatched=True,
                      limitation='Arguments only; framebuffer is observed separately')
        assert not result['unresolvedResolution'] and result['managedHeap2048MiB']
        with (output / 'numeric-argument-observation.json').open('x') as stream:
            json.dump(result, stream, indent=2)
        print(json.dumps(result))
    finally: reader.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    observe(parser.parse_args().output.resolve())
