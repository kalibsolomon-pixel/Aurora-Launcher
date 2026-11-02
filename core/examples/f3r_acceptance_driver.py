"""Opt-in Windows/native engine fault acceptance; leaves all evidence intact.

Usage: python f3r_acceptance_driver.py RUNNER_EXE MARKED_F3R_ROOT
The root must already contain isolated installations and its ownership marker.
Never pass the production application-data directory.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

runner, root = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
assert root.name.startswith("aurora-f3r-acceptance-")
assert (root / "f3r-owned-root").read_bytes() == b"F3R isolated native acceptance"
evidence = root / "native-evidence"
evidence.mkdir(exist_ok=True)
environment = {**os.environ, "AURORA_INSTALL_DIAGNOSTICS": "1"}
records = json.loads((root / "managed/launcher/instances.json").read_text(encoding="utf-8"))["instances"]
forge = next(r["id"] for r in records if r["displayName"] == "F1C 1.20.1 forge")
vanilla = next(r["id"] for r in records if r["displayName"] == "F1C 1.21.11 vanilla")
results = []
children = []


def emit(value):
    results.append(value)
    (evidence / "results.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
    print(json.dumps(value), flush=True)


def invoke(label, command, identity="", point="", success=True):
    started = time.monotonic()
    with (evidence / (label + ".log")).open("w", encoding="utf-8") as log:
        process = subprocess.Popen([str(runner), str(root), command, identity, point], stdout=log, stderr=log, env=environment)
        children.append(process)
        code = process.wait(timeout=300)
    emit({"case": label, "pid": process.pid, "exit": code, "seconds": time.monotonic() - started})
    assert (code == 0) == success, label
    return (evidence / (label + ".log")).read_text(encoding="utf-8")


def pause(label, identity, point, command="replace"):
    log = (evidence / (label + ".log")).open("w", encoding="utf-8")
    process = subprocess.Popen([str(runner), str(root), command, identity, point], stdout=log, stderr=log, env=environment)
    children.append(process)
    log.close()
    marker = root / f"transition-marker-{process.pid}.json"
    started = time.monotonic()
    while not marker.exists():
        assert process.poll() is None, (label, "exited before checkpoint")
        assert time.monotonic() - started < 300, (label, "checkpoint timeout")
        time.sleep(0.1)
    journal = root / "managed/instances" / identity / ".install-operation.json"
    document = json.loads(journal.read_text(encoding="utf-8"))
    emit({"case": label, "pid": process.pid, "checkpoint": point, "secondsToCheckpoint": time.monotonic() - started,
          "journal": document, "gameExists": (journal.parent / "game").exists(),
          "backupExists": (journal.parent / ".install-history" / document["transactionId"] / "previous-game").exists()})
    return process


def terminate(process):
    # Popen.kill uses TerminateProcess on Windows and SIGKILL on Unix.
    # Only handles created above are eligible; never kill by executable name.
    process.kill()
    process.wait(timeout=15)
    emit({"case": "forced-termination", "pid": process.pid, "exit": process.returncode})


try:
    invoke("setup-recovery", "recover", forge)
    invoke("baseline-all-loaders", "validate-all")
    held = pause("same-target-holder", forge, "Prepared")
    output = invoke("same-target-contender", "replace", forge, success=False)
    assert "AlreadyInProgress" in output, output
    other = pause("independent-target-holder", vanilla, "Prepared")
    assert held.poll() is None
    terminate(other)
    terminate(held)
    invoke("independent-target-recovery", "recover", vanilla)
    invoke("same-target-recovery", "recover", forge)

    for point in ["AfterBackup", "AfterPromotion", "AfterCommit"]:
        held = pause("replace-" + point, forge, point)
        terminate(held)
        invoke("recover-" + point, "recover", forge)
        invoke("repeat-recover-" + point, "recover", forge)

    for point in ["Restoring", "AfterQuarantine", "AfterRestore"]:
        held = pause("prepare-recovery-" + point, forge, "AfterPromotion")
        terminate(held)
        replay = pause("recovery-" + point, forge, point, command="recover")
        terminate(replay)
        invoke("finish-recovery-" + point, "recover", forge)
        invoke("repeat-recovery-" + point, "recover", forge)

    invoke("final-all-loaders", "validate-all")
    emit({"verdict": "PASS", "scope": "native engine processes, official installed bytes; no authenticated gameplay"})
finally:
    for process in children:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=15)
