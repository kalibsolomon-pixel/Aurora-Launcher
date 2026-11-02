"""Windows-only opt-in kill of an exact native runner during a real Forge tool.
Usage: python f3r_processor_kill.py RUNNER_EXE MARKED_F3R_ROOT
Retains the game, interrupted receipt, and all observations for inspection.
"""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

assert os.name == "nt"
runner, root = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
assert root.name.startswith("aurora-f3r-acceptance-")
assert (root / "f3r-owned-root").read_bytes() == b"F3R isolated native acceptance"
evidence = root / "native-evidence"
evidence.mkdir(exist_ok=True)
registry = json.loads((root / "managed/launcher/instances.json").read_text(encoding="utf-8"))
identity = next(r["id"] for r in registry["instances"] if r["displayName"] == "F1C 1.20.4 forge")
instance = root / "managed/instances" / identity
manifest = instance / "game/installed-game.json"
before = hashlib.sha256(manifest.read_bytes()).hexdigest()
with (evidence / "processor-interruption.log").open("w", encoding="utf-8") as log:
    process = subprocess.Popen([str(runner), str(root), "replace", identity], stdout=log, stderr=log,
                               env={**os.environ, "AURORA_INSTALL_DIAGNOSTICS": "1"})
    try:
        started = time.monotonic()
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        class ProcessEntry(ctypes.Structure):
            _fields_ = [("size", ctypes.c_uint32), ("usage", ctypes.c_uint32),
                        ("pid", ctypes.c_uint32), ("heap", ctypes.c_size_t),
                        ("module", ctypes.c_uint32), ("threads", ctypes.c_uint32),
                        ("parent", ctypes.c_uint32), ("priority", ctypes.c_int32),
                        ("flags", ctypes.c_uint32), ("name", ctypes.c_wchar * 260)]
        kernel.CreateToolhelp32Snapshot.argtypes = [ctypes.c_uint32, ctypes.c_uint32]
        kernel.CreateToolhelp32Snapshot.restype = ctypes.c_void_p
        kernel.Process32FirstW.argtypes = [ctypes.c_void_p, ctypes.POINTER(ProcessEntry)]
        kernel.Process32NextW.argtypes = [ctypes.c_void_p, ctypes.POINTER(ProcessEntry)]
        kernel.OpenProcess.argtypes = [ctypes.c_uint, ctypes.c_int, ctypes.c_uint]
        kernel.OpenProcess.restype = ctypes.c_void_p
        kernel.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_uint]
        kernel.CloseHandle.argtypes = [ctypes.c_void_p]
        handles = []
        while not handles:
            assert process.poll() is None, "runner exited before a Java processor was observed"
            assert time.monotonic() - started < 300, "no processor observed"
            snapshot = kernel.CreateToolhelp32Snapshot(2, 0)
            assert snapshot and snapshot != ctypes.c_void_p(-1).value
            entry = ProcessEntry()
            entry.size = ctypes.sizeof(entry)
            more = kernel.Process32FirstW(snapshot, ctypes.byref(entry))
            while more:
                if entry.parent == process.pid and entry.name.lower() == "java.exe":
                    handle = kernel.OpenProcess(0x00100000, False, entry.pid)
                    if handle:
                        # Keep a real handle and require the tool still to be
                        # executing after one second, avoiding short-step PID races.
                        if kernel.WaitForSingleObject(handle, 1000) == 258:
                            handles.append((entry.pid, handle))
                            break
                        kernel.CloseHandle(handle)
                more = kernel.Process32NextW(snapshot, ctypes.byref(entry))
            kernel.CloseHandle(snapshot)
            if not handles:
                time.sleep(0.05)
        children = [pid for pid, _ in handles]
        process.kill()
        process.wait(timeout=15)
        for pid, handle in handles:
            assert handle, f"could not retain observed Java process {pid}"
            assert kernel.WaitForSingleObject(handle, 20000) == 0, f"Java child {pid} survived launcher death"
            kernel.CloseHandle(handle)
        assert hashlib.sha256(manifest.read_bytes()).hexdigest() == before
        journal = json.loads((instance / ".install-operation.json").read_text(encoding="utf-8"))
        receipt = json.loads((instance / ".install-staging/transaction-receipt.json").read_text(encoding="utf-8"))
        assert journal["stage"] == "prepared"
        assert receipt["stages"][-1]["stage"] == "processing"
        assert not (instance / ".install-staging/game/installed-game.json").exists()
        result = {"verdict": "PASS", "launcherPid": process.pid, "javaPids": children,
                  "javaExitedAfterLauncherTermination": True, "journal": journal,
                  "lastReceiptStage": "processing", "oldManifestSha256": before}
        (evidence / "processor-result.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
        print(json.dumps(result), flush=True)
    finally:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=15)
with (evidence / "processor-recovery.log").open("w", encoding="utf-8") as log:
    subprocess.run([str(runner), str(root), "recover", identity], stdout=log, stderr=log, check=True, timeout=300)
