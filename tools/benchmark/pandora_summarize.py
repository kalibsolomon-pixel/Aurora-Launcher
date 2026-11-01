"""Summarize only recorded PANDORA probe outputs; preserve every run/outlier."""
import argparse
import json
from pathlib import Path
import statistics


def summarize(root):
    result = {"machine": json.loads((root / "machine.json").read_text(encoding="utf-8-sig")),
        "baselineSource": "0e2c8dd9e13ea1b1d548505d575674d9114ee386",
        "pandoraSource": "2d83d6f22c43e804d9b544e8dc98d49d4874e3af",
        "toolchain": "rustc 1.98.1 (48a229cea 2026-09-01)", "groups": {}}
    for mode in ["idle", "archive", "hosts"]:
        for variant in ["before", "after"]:
            rows = []
            paths = sorted(root.glob(f"pandora-{mode}-{variant}-*"))
            if not paths:
                raise RuntimeError(f"missing {mode}/{variant} evidence")
            for path in paths:
                data = json.loads((path / "resources.json").read_text())
                samples = data["samples"]
                if not samples:
                    raise RuntimeError(f"missing process samples: {path.name}")
                row = {"run": path.name, "exeSha256": data["exeSha256"],
                    "peakWorkingSetMiB": data["peakWorkingSet"] / 1048576,
                    "peakSampledPrivateMiB": data["peakSampledPrivate"] / 1048576,
                    "observedDescendants": data["descendantsObserved"],
                    "cpuSeconds": samples[-1]["cpuSeconds"],
                    "logicalReadMiB": samples[-1]["logicalReadBytes"] / 1048576,
                    "logicalWriteMiB": samples[-1]["logicalWriteBytes"] / 1048576}
                if mode == "idle":
                    idle = data["idle"]
                    row.update(idleWorkingSetMiB=idle["workingSet"] / 1048576,
                        idlePrivateCommittedMiB=idle["privateCommitted"] / 1048576,
                        idleCpuSeconds=idle["cpuSeconds"], idleSampleSeconds=idle["sampleSeconds"],
                        idleOneCorePercent=100 * idle["cpuSeconds"] / idle["sampleSeconds"])
                    events = []
                    for line in (path / "stderr.txt").read_text(errors="replace").splitlines():
                        if line.startswith("[borealis] "):
                            event = json.loads(line.removeprefix("[borealis] "))
                            if event["event"] == "home-state-loaded":
                                events.append(event)
                    row["homeLoadedInternalMs"] = events[0]["ms"] if events else None
                    row["homeCheckpoint"] = events[0] if events else None
                else:
                    row.update(json.loads((path / "stdout.txt").read_text()))
                    if data["exitCode"] != 0:
                        raise RuntimeError(f"probe failed: {path.name}")
                rows.append(row)
            numeric = [key for key, value in rows[0].items() if isinstance(value, (int, float))]
            result["groups"][f"{mode}-{variant}"] = {"runs": rows,
                "median": {key: statistics.median(row[key] for row in rows) for key in numeric},
                "range": {key: [min(row[key] for row in rows), max(row[key] for row in rows)] for key in numeric}}
    scheduler = [json.loads(line) for line in (root / "scheduler-results.jsonl").read_text(encoding="utf-8-sig").splitlines() if line]
    result["scheduler"] = {"runs": scheduler, "medianOrderedMs": statistics.median(row["elapsedMs"] for row in scheduler if not row["completionOrder"]),
        "medianCompletionMs": statistics.median(row["elapsedMs"] for row in scheduler if row["completionOrder"])}
    result["limitations"] = ["Logical I/O is not physical disk I/O.", "Delay-only scheduling is not internet throughput.",
        "Idle is empty-root Simple, not populated/Borealis or a matched Pandora comparison.",
        "The first after-build working-set outlier is retained; causal attribution is unproven."]
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    print(json.dumps(summarize(parser.parse_args().root), indent=2))
