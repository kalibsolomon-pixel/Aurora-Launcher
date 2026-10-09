"""Validate numeric traces and regenerate the packaged-trial summary. No app I/O."""
import hashlib
import json
import math
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parent / "evidence"
NAMES = {
    1: "initialization", 10: "runtimeStatus", 11: "readiness", 12: "play",
    13: "unrelatedIpc", 14: "selection", 20: "gameIntegrity", 21: "gameMetadata",
    22: "runtimeMetadata", 23: "modInventory", 24: "runtimeIntegrity",
    25: "javaDiagnostic", 26: "session", 27: "ensureSession", 28: "sessionLock",
    29: "contentLock", 30: "lockedValidation", 31: "javaSpawn",
    32: "blockingQueue", 33: "blockingWorker", 34: "dispatch",
    35: "modInventoryCommand", 36: "mojangHttp", 37: "fabricHttp", 38: "runtimeHttp",
}
FIELDS = {"event", "edge", "id", "parent", "us", "hash_us", "hash_calls",
          "correlation", "instance_generation", "account_generation", "client_us"}


def stats(values):
    if not values:
        return {"n": 0}
    ordered = sorted(values)
    return {"n": len(values), "medianMs": statistics.median(values),
            "minMs": min(values), "worstMs": max(values),
            "p95Ms": ordered[math.ceil(.95 * len(values)) - 1] if len(values) >= 20 else None,
            "sampleStdDevMs": statistics.stdev(values) if len(values) > 1 else None}


def trace(path):
    raw = path.read_bytes()
    doc = json.loads(raw)
    assert set(doc) == {"records", "dropped"}
    assert doc["dropped"] == 0 and len(doc["records"]) <= 16384
    records = doc["records"]
    begins, spans, markers = {}, [], []
    for row in records:
        assert set(row) == FIELDS
        assert all(type(v) is int and 0 <= v < 2**64 for v in row.values())
        event, edge = row["event"], row["edge"]
        if event >= 100:
            assert 100 <= event <= 113 and edge == 0, "unknown event or frontend overflow"
            assert row["id"] == row["parent"] == row["hash_us"] == row["hash_calls"] == 0
            markers.append(row)
        else:
            assert event in NAMES and edge in (1, 2, 3) and row["id"] > 0
            if edge == 1:
                assert row["id"] not in begins
                begins[row["id"]] = row
            else:
                begin = begins.pop(row["id"])
                for key in ("event", "parent", "correlation", "instance_generation", "account_generation"):
                    assert begin[key] == row[key]
                assert row["us"] >= begin["us"]
                spans.append({**row, "start_us": begin["us"], "duration_us": row["us"] - begin["us"]})
    assert not begins, "unfinished spans: exclude incomplete trial"
    assert len(markers) <= 2048
    assert all(s["edge"] == 2 for s in spans), "cancelled spans: inspect before using counts"
    assert not any(s["event"] in (12, 31) for s in spans), "this cohort contains no Play"
    def selected(event):
        return [s for s in spans if s["event"] == event]
    counts = {NAMES[e]: len(selected(e)) for e in NAMES}
    timing = {NAMES[e]: stats([s["duration_us"] / 1000 for s in selected(e)]) for e in NAMES if selected(e)}
    hashing = {NAMES[e]: {"wallMs": sum(s["hash_us"] for s in selected(e)) / 1000,
                         "calls": sum(s["hash_calls"] for s in selected(e))}
               for e in NAMES if any(s["hash_calls"] for s in selected(e))}
    overlaps = []
    for runtime in selected(10):
        for readiness in selected(11):
            overlap = min(runtime["us"], readiness["us"]) - max(runtime["start_us"], readiness["start_us"])
            if overlap > 0:
                overlaps.append({"runtimeCorrelation": runtime["correlation"],
                                 "readinessCorrelation": readiness["correlation"], "wallMs": overlap / 1000})
    by_marker = {(m["event"], m["correlation"]): m for m in markers if m["correlation"]}
    requests = []
    for begin_event, end_event in ((101, 102), (103, 104)):
        for (event, corr), begin in by_marker.items():
            if event == begin_event:
                end = by_marker[(end_event, corr)]
                assert end["client_us"] >= begin["client_us"]
                requests.append((begin["client_us"], end["client_us"]))
    ipc, pending, settled = [], [], []
    for (event, corr), begin in by_marker.items():
        if event == 112:
            end = by_marker[(113, corr)]
            ms = (end["client_us"] - begin["client_us"]) / 1000
            assert ms >= 0
            ipc.append(ms)
            (pending if any(a <= begin["client_us"] <= b for a, b in requests) else settled).append(ms)
    frontend_init = next(m for m in markers if m["event"] == 100)
    transitions = [{"event": m["event"], "afterFrontendInitMs": (m["client_us"] - frontend_init["client_us"]) / 1000}
                   for m in markers if m["event"] in (105, 106)]
    ready = [m for m in markers if m["event"] == 106]
    assert ready
    flashes = [b["afterFrontendInitMs"] - a["afterFrontendInitMs"]
               for a, b in zip(transitions, transitions[1:]) if a["event"] == 106 and b["event"] == 105]
    return {"inputSha256": hashlib.sha256(raw).hexdigest(), "recordCount": len(records),
            "counts": counts, "spanWallMsInclusive": timing, "hashFileIoAndDigest": hashing,
            "durationsByEventCorrelationMs": {
                f"event{e}/correlation{c}": [s["duration_us"] / 1000 for s in spans if s["event"] == e and s["correlation"] == c]
                for e, c in sorted({(s["event"], s["correlation"]) for s in spans})},
            "gameAuditHashWallFractions": [s["hash_us"] / s["duration_us"] for s in selected(20)],
            "readyFlashMs": flashes,
            "frontendInitializeAfterNativeOriginMs": frontend_init["us"] / 1000,
            "runtimeReadinessOverlap": overlaps, "frontendTransitions": transitions,
            "firstReadyAfterNativeOriginMs": ready[0]["us"] / 1000,
            "lastReadyAfterNativeOriginMs": ready[-1]["us"] / 1000,
            "firstReadyAfterFrontendInitMs": (ready[0]["client_us"] - frontend_init["client_us"]) / 1000,
            "lastReadyAfterFrontendInitMs": (ready[-1]["client_us"] - frontend_init["client_us"]) / 1000,
            "unattributedGameAudits": sum(s["parent"] == 0 for s in selected(20)),
            "gameAuditsByCorrelation": {str(c): sum(s["correlation"] == c for s in selected(20))
                                       for c in sorted({s["correlation"] for s in selected(20)})},
            "discardedReadiness": sum(m["event"] == 107 for m in markers),
            "instanceGenerations": sorted({m["instance_generation"] for m in markers}),
            "accountGenerations": sorted({m["account_generation"] for m in markers}),
            "ipcRtt": stats(ipc), "ipcWhileValidationRequestPending": stats(pending),
            "ipcWhileNoValidationRequestPending": stats(settled)}


def main():
    trials = json.loads((ROOT / "trials.json").read_text())
    assert len({(r["cohort"], r["trial"]) for r in trials}) == len(trials)
    summaries = {}
    for cohort in ("profile", "control", "ipc"):
        rows = [r for r in trials if r["cohort"] == cohort and r["phase"] == "matched"]
        assert all(r["success"] and r["closedNormally"] and r["competingLauncherOrJava"] == 0 for r in rows)
        summaries[cohort] = {"firstReadyUpper": stats([r["firstReadyUpperMs"] for r in rows]),
                             "stableReadyUpper": stats([r["stableReadyUpperMs"] for r in rows]),
                             "worstObserverCaptureMs": max((r["maxCaptureMs"] for r in rows), default=None)}
    pairs = []
    stable_pairs = []
    for control in [r for r in trials if r["cohort"] == "control" and r["phase"] == "matched"]:
        profile = next(r for r in trials if r["cohort"] == "profile" and r["trial"] == control["trial"])
        pairs.append(profile["firstReadyUpperMs"] - control["firstReadyUpperMs"])
        stable_pairs.append(profile["stableReadyUpperMs"] - control["stableReadyUpperMs"])
    traces = {p.name: trace(p) for p in sorted(ROOT.glob("trace-*.json"))}
    primary_names = [f"trace-profile-{r['trial']:02}.json" for r in trials
                     if r["cohort"] == "profile" and r["phase"] == "matched"]
    groups, primary_counts, all_counts = {}, {}, {}
    for name, t in traces.items():
        for key, value in t["counts"].items():
            all_counts[key] = all_counts.get(key, 0) + value
            if name in primary_names:
                primary_counts[key] = primary_counts.get(key, 0) + value
        if name in primary_names:
            for key, values in t["durationsByEventCorrelationMs"].items():
                groups.setdefault(key, []).extend(values)
    result = {"cohorts": summaries, "pairedProfileMinusControlFirstReady": stats(pairs),
              "pairedProfileMinusControlStableReady": stats(stable_pairs),
              "totalRealStartupTrials": len(trials),
              "excludedUiPilots": sum(r["phase"] == "pilot" for r in trials),
              "primaryNativeCounts": primary_counts, "allTracedNativeCounts": all_counts,
              "primaryEventCorrelationTimings": {key: stats(value) for key, value in groups.items()},
              "primaryRunsWithReadyCheckingReady": sum(bool(traces[n]["readyFlashMs"]) for n in primary_names),
              "primaryNativeFinalReady": stats([traces[n]["lastReadyAfterNativeOriginMs"] for n in primary_names]),
              "traces": traces, "newPlayTrials": 0,
              "notes": ["No p95 for cohorts smaller than 20.",
                        "UI values are observation upper bounds, not exact paint times.",
                        "Span durations are inclusive and overlapping; never sum as independent cost.",
                        "Hash counters include file I/O, not pure hashing CPU time.",
                        "IPC pending classification uses one frontend clock and outstanding validation requests.",
                        "No matched Play/JFR overhead claim."]}
    (ROOT / "summary.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"cohorts": summaries, "pairedFirstReady": stats(pairs),
                      "pairedStableReady": stats(stable_pairs), "traces": len(traces)}, indent=2))


if __name__ == "__main__":
    main()
