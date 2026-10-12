"""Retained-handle controller for the actual debug Tauri application.

python f3ra_desktop_driver.py MARKED_TEMP_ROOT
One JSON command per stdin line: start/kill/status/snapshot/resume/exit.
Install/retry/recovery are performed by the real desktop UI, never this driver.
Evidence and process copies are retained. No production root is accepted.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import threading
import ctypes

root = Path(sys.argv[1]).resolve()
assert root.parent == Path(tempfile.gettempdir()).resolve()
assert root.name.startswith("aurora-f3ra-")
assert (root / "F3RA-ISOLATED.txt").read_bytes() == b"F3RA isolated desktop acceptance\n"
evidence = root / "evidence"
evidence.mkdir(exist_ok=True)
children = {}
records_path = evidence / "desktop-processes.json"
records = json.loads(records_path.read_text()) if records_path.exists() else []
record_lock = threading.Lock()


def emit(record):
    record["observedAtUnix"] = time.time()
    with record_lock:
        records.append(record)
        records_path.write_text(json.dumps(records, indent=2), encoding="utf-8")
        print(json.dumps(record), flush=True)


def watch_processor(label, process):
    """Observe a real child through a retained kernel handle before parent kill."""
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.restype = ctypes.c_void_p
    kernel.OpenProcess.argtypes = [ctypes.c_ulong, ctypes.c_bool, ctypes.c_ulong]
    kernel.GetExitCodeProcess.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
    kernel.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
    kernel.CloseHandle.argtypes = [ctypes.c_void_p]
    started = time.monotonic()
    try:
        while time.monotonic() - started < 150 and process.poll() is None:
            # The retained parent Popen is this controller's own exact child.
            query = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command",
                f"Get-CimInstance Win32_Process -Filter 'ParentProcessId={process.pid}' | Select-Object ProcessId,Name | ConvertTo-Json -Compress"],
                capture_output=True, text=True, timeout=5, creationflags=subprocess.CREATE_NO_WINDOW)
            if query.stdout.strip():
                rows = json.loads(query.stdout)
                if isinstance(rows, dict): rows = [rows]
                for row in rows:
                    if row["Name"].lower() != "java.exe": continue
                    handle = kernel.OpenProcess(0x00100000 | 0x1000, False, row["ProcessId"])
                    if not handle: continue
                    try:
                        code = ctypes.c_ulong()
                        assert kernel.GetExitCodeProcess(handle, ctypes.byref(code))
                        if code.value != 259: continue
                        time.sleep(0.2)
                        assert kernel.GetExitCodeProcess(handle, ctypes.byref(code))
                        if code.value != 259: continue
                        emit({"action": "processor-active", "label": label, "parentPid": process.pid,
                              "javaPid": row["ProcessId"], "retainedHandle": True, "exitCodeBeforeKill": code.value})
                        process.kill(); process.wait(timeout=15)
                        wait_result = kernel.WaitForSingleObject(handle, 10000)
                        assert kernel.GetExitCodeProcess(handle, ctypes.byref(code))
                        emit({"action": "processor-parent-killed", "label": label, "parentPid": process.pid,
                              "javaPid": row["ProcessId"], "childWaitResult": wait_result,
                              "childExitCode": code.value, "childTerminated": wait_result == 0 and code.value != 259})
                        return
                    finally: kernel.CloseHandle(handle)
            time.sleep(0.1)
        emit({"action": "processor-observation-failed", "label": label, "reason": "no retained live Java child within bounded watch"})
    except Exception as error:
        emit({"action": "processor-observation-failed", "label": label, "reason": str(error)})


def snapshot(identity):
    assert len(identity) == 32 and all(c in "0123456789abcdef" for c in identity)
    instance = root / "managed/instances" / identity
    result = {"instanceId": identity}
    for name in [".install-operation.json", ".install-staging/transaction-receipt.json"]:
        path = instance / name
        result[name] = json.loads(path.read_text(encoding="utf-8")) if path.exists() else None
    result["manifests"] = {}
    for path in instance.rglob("installed-game.json"):
        result["manifests"][str(path.relative_to(instance))] = {
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "installationId": json.loads(path.read_text(encoding="utf-8"))["installationId"],
        }
    registry = json.loads((root / "managed/launcher/instances.json").read_text(encoding="utf-8"))
    result["record"] = next(r for r in registry["instances"] if r["id"] == identity)
    return result


emit({"controllerPid": os.getpid(), "root": str(root)})
try:
    for line in sys.stdin:
        try:
            command = json.loads(line)
            action = command["action"]
            label = command.get("label", "")
            if action == "start":
                assert label and len(label) < 64 and all(c.isalnum() or c == "-" for c in label)
                assert label not in children
                exe = root / f"desktop-{label}.exe"
                assert not exe.exists()
                shutil.copy2(root / "aurora-f3ra.exe", exe)
                environment = {**os.environ,
                    "AURORA_DIAGNOSTIC_DATA_ROOT": str(root / "managed"),
                    "WEBVIEW2_USER_DATA_FOLDER": str(root / f"webview-{label}"),
                    "AURORA_INSTALL_DIAGNOSTICS": "1"}
                for variable in ["AURORA_F3RA_PAUSE", "AURORA_F3RA_INSTANCE"]:
                    environment.pop(variable, None)
                if command.get("point"):
                    environment["AURORA_F3RA_PAUSE"] = command["point"]
                    environment["AURORA_F3RA_INSTANCE"] = command["id"]
                with (evidence / f"{label}.log").open("wb") as log:
                    process = subprocess.Popen([str(exe)], env=environment, stdout=log, stderr=log)
                children[label] = process
                emit({"action": action, "label": label, "pid": process.pid, "exe": str(exe),
                      "point": command.get("point"), "sha256": hashlib.sha256(exe.read_bytes()).hexdigest()})
            elif action == "kill":
                process = children[label]
                assert process.poll() is None
                process.kill()
                process.wait(timeout=15)
                emit({"action": action, "label": label, "pid": process.pid, "exit": process.returncode})
            elif action == "resume":
                process = children[label]
                assert process.poll() is None
                (evidence / f"resume-{process.pid}").write_text("resume", encoding="utf-8")
                emit({"action": action, "label": label, "pid": process.pid})
            elif action == "watch-processor-and-kill":
                assert os.name == "nt"
                process = children[label]
                assert process.poll() is None
                threading.Thread(target=watch_processor, args=(label, process), daemon=True).start()
                emit({"action": action, "label": label, "pid": process.pid})
            elif action == "status":
                emit({"action": action, "processes": {k: {"pid": p.pid, "exit": p.poll()} for k, p in children.items()},
                      "checkpoints": [p.name for p in evidence.glob("checkpoint-*.json")]})
            elif action == "snapshot":
                value = snapshot(command["id"])
                path = evidence / f"snapshot-{len(records):03d}.json"
                path.write_text(json.dumps(value, indent=2), encoding="utf-8")
                emit({"action": action, "path": str(path), "state": value["record"]["state"],
                      "journal": value[".install-operation.json"], "manifests": value["manifests"]})
            elif action == "exit":
                break
            else:
                raise ValueError("unknown controller action")
        except Exception as error:
            emit({"error": str(error), "command": line.strip()})
finally:
    for label, process in children.items():
        if process.poll() is None:
            process.kill()
            process.wait(timeout=15)
            emit({"action": "controller-owned-shutdown", "label": label, "pid": process.pid})
