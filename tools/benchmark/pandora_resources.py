"""Windows process-resource probe; explicit owned processes and disposable roots only.

No third-party Python dependency, UI automation, credentials, or process-name
termination. I/O counters are logical process I/O, not physical disk traffic.
Native idle runs terminate only the exact subprocess this probe created.
"""
import argparse
import ctypes as c
from ctypes import wintypes as w
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import time


class Memory(c.Structure):
    _fields_ = [("cb", w.DWORD), ("PageFaultCount", w.DWORD)] + [
        (name, c.c_size_t) for name in ["PeakWorkingSetSize", "WorkingSetSize",
        "QuotaPeakPagedPoolUsage", "QuotaPagedPoolUsage", "QuotaPeakNonPagedPoolUsage",
        "QuotaNonPagedPoolUsage", "PagefileUsage", "PeakPagefileUsage", "PrivateUsage"]]


class Io(c.Structure):
    _fields_ = [(name, c.c_ulonglong) for name in ["ReadOperationCount", "WriteOperationCount",
        "OtherOperationCount", "ReadTransferCount", "WriteTransferCount", "OtherTransferCount"]]


class ProcessEntry(c.Structure):
    _fields_ = [("dwSize", w.DWORD), ("cntUsage", w.DWORD), ("pid", w.DWORD),
        ("heap", c.c_size_t), ("module", w.DWORD), ("threads", w.DWORD),
        ("parent", w.DWORD), ("priority", w.LONG), ("flags", w.DWORD), ("exe", w.WCHAR * 260)]


def setup():
    kernel = c.WinDLL("kernel32", use_last_error=True)
    psapi = c.WinDLL("psapi", use_last_error=True)
    kernel.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
    kernel.OpenProcess.restype = w.HANDLE
    kernel.CloseHandle.argtypes = [w.HANDLE]
    kernel.GetProcessTimes.argtypes = [w.HANDLE] + [c.POINTER(w.FILETIME)] * 4
    kernel.GetProcessIoCounters.argtypes = [w.HANDLE, c.POINTER(Io)]
    kernel.CreateToolhelp32Snapshot.argtypes = [w.DWORD, w.DWORD]
    kernel.CreateToolhelp32Snapshot.restype = w.HANDLE
    kernel.Process32FirstW.argtypes = [w.HANDLE, c.POINTER(ProcessEntry)]
    kernel.Process32NextW.argtypes = [w.HANDLE, c.POINTER(ProcessEntry)]
    psapi.GetProcessMemoryInfo.argtypes = [w.HANDLE, c.POINTER(Memory), w.DWORD]
    return kernel, psapi


def metrics(pid, kernel, psapi):
    handle = kernel.OpenProcess(0x1010, False, pid)
    if not handle:
        return None
    try:
        memory, io = Memory(), Io()
        memory.cb = c.sizeof(memory)
        times = [w.FILETIME() for _ in range(4)]
        if not psapi.GetProcessMemoryInfo(handle, c.byref(memory), c.sizeof(memory)):
            return None
        if not kernel.GetProcessTimes(handle, *[c.byref(t) for t in times]):
            return None
        if not kernel.GetProcessIoCounters(handle, c.byref(io)):
            raise c.WinError(c.get_last_error())
        ticks = lambda t: (t.dwHighDateTime << 32) | t.dwLowDateTime
        return dict(workingSet=memory.WorkingSetSize, peakWorkingSet=memory.PeakWorkingSetSize,
            privateCommitted=memory.PrivateUsage, cpuSeconds=(ticks(times[2]) + ticks(times[3])) / 1e7,
            logicalReadBytes=io.ReadTransferCount, logicalWriteBytes=io.WriteTransferCount)
    finally:
        kernel.CloseHandle(handle)


def descendants(pid, kernel):
    snapshot = kernel.CreateToolhelp32Snapshot(2, 0)
    if snapshot == c.c_void_p(-1).value:
        raise c.WinError(c.get_last_error())
    try:
        entry = ProcessEntry()
        entry.dwSize = c.sizeof(entry)
        parents = {}
        ok = kernel.Process32FirstW(snapshot, c.byref(entry))
        while ok:
            parents[entry.pid] = entry.parent
            ok = kernel.Process32NextW(snapshot, c.byref(entry))
        owned = {pid}
        while True:
            expanded = owned | {child for child, parent in parents.items() if parent in owned}
            if expanded == owned:
                return owned
            owned = expanded
    finally:
        kernel.CloseHandle(snapshot)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("exe", type=Path)
    parser.add_argument("root", type=Path, help="new, absolute, disposable evidence directory")
    parser.add_argument("--idle", action="store_true")
    parser.add_argument("--seconds", type=float, default=20)
    parser.add_argument("--sync-fixture", type=Path,
        help="owned synthetic pandora-ui-* fixture; seeds only registry/options and automatic policy")
    parser.add_argument("args", nargs="*")
    args = parser.parse_args()
    if os.name != "nt" or not args.root.is_absolute() or "pandora" not in args.root.name.lower():
        parser.error("requires Windows and an explicitly named absolute Pandora disposable directory")
    args.root.mkdir(parents=True, exist_ok=False)
    (args.root / "PANDORA-DISPOSABLE.txt").write_text("Owned performance fixture; no real accounts or worlds.\n")
    if args.sync_fixture:
        seed = args.sync_fixture
        if not args.idle or not seed.is_absolute() or not seed.name.startswith("pandora-ui-"):
            parser.error("sync sampling requires an absolute synthetic pandora-ui-* root and --idle")
        marker = seed / "PANDORA-DISPOSABLE.txt"
        if marker.read_text().strip() != "Synthetic UI fixture: no installed game or accounts.":
            parser.error("sync fixture is not the explicit owned synthetic fixture")
        registry_bytes = (seed / "managed/launcher/instances.json").read_bytes()
        registry = json.loads(registry_bytes)
        ids = [row["id"] for row in registry["instances"]]
        if len(ids) != 2 or any(not re.fullmatch(r"[a-f0-9]{32}", id) for id in ids):
            parser.error("sync sample expects exactly two synthetic instance IDs")
        launcher = args.root / "managed/launcher"
        launcher.mkdir(parents=True)
        (launcher / "instances.json").write_bytes(registry_bytes)
        for id in ids:
            options = (seed / f"managed/instances/{id}/options.txt").read_bytes()
            if len(options) > 1048576:
                parser.error("synthetic options exceed the limit")
            destination = args.root / f"managed/instances/{id}"
            destination.mkdir(parents=True)
            (destination / "options.txt").write_bytes(options)
        (launcher / "instance-sync.json").write_text(json.dumps(dict(schemaVersion=2,
            enabled=True, instances=ids, categories=["settings", "controls", "resourcePacks",
            "shaderPacks", "screenshots"], exclusions=[], selectedPaths=[])))
    env = os.environ.copy()
    if args.idle:
        if b"AURORA_DIAGNOSTIC_DATA_ROOT" not in args.exe.read_bytes():
            parser.error("idle probe refuses a binary without disposable-root support")
        env["AURORA_DIAGNOSTIC_DATA_ROOT"] = str(args.root / "managed")
        env["AURORA_DIAGNOSTIC_BOREALIS_TRACE"] = "1"
    kernel, psapi = setup()
    with (args.root / "stdout.txt").open("wb") as stdout, (args.root / "stderr.txt").open("wb") as stderr:
        started = time.perf_counter()
        process = subprocess.Popen([str(args.exe), *args.args], env=env, stdout=stdout, stderr=stderr)
        samples, children = [], set()
        try:
            while process.poll() is None:
                now = time.perf_counter() - started
                owned = descendants(process.pid, kernel)
                children |= owned - {process.pid}
                values = [metrics(pid, kernel, psapi) for pid in owned]
                values = [value for value in values if value is not None]
                if values:
                    samples.append(dict(seconds=now, **{key: sum(v[key] for v in values) for key in values[0]}))
                if args.idle and now >= args.seconds:
                    break
                time.sleep(.05)
        finally:
            if process.poll() is None:
                process.terminate()  # exact Popen handle; no process-name matching
            process.wait(timeout=15)
    idle = [sample for sample in samples if sample["seconds"] >= 8]
    if args.idle:
        reported = (args.root / "stderr.txt").read_text(errors="replace").replace("\\", "/").lower()
        expected = str(args.root / "managed").replace("\\", "/").lower()
        if "managed data root: " + expected not in reported:
            raise RuntimeError("the owned native process did not confirm its disposable managed root")
    result = dict(exeSha256=hashlib.sha256(args.exe.read_bytes()).hexdigest(), pid=process.pid,
        syntheticAutomaticPool=bool(args.sync_fixture),
        descendantsObserved=sorted(children), samplePeriodMs=50, samples=samples,
        peakWorkingSet=max((s["peakWorkingSet"] for s in samples), default=0),
        peakSampledPrivate=max((s["privateCommitted"] for s in samples), default=0),
        exitCode=process.returncode, idleTermination="exact subprocess handle" if args.idle else None)
    if len(idle) > 1:
        import statistics
        result["idle"] = dict(workingSet=statistics.median(s["workingSet"] for s in idle),
            privateCommitted=statistics.median(s["privateCommitted"] for s in idle),
            cpuSeconds=idle[-1]["cpuSeconds"] - idle[0]["cpuSeconds"],
            sampleSeconds=idle[-1]["seconds"] - idle[0]["seconds"])
    (args.root / "resources.json").write_text(json.dumps(result, indent=2))
    print(json.dumps({key: value for key, value in result.items() if key != "samples"}))


if __name__ == "__main__":
    main()
