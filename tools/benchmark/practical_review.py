"""Small offline practical-protocol checks; never changes strict evidence/contracts.

CPU is machine-capacity share, not per-core speed or certified unrelated load.
Frame metrics retain the pinned collector's eligible HUD-callback semantics.
"""
import csv
import argparse
import json
import math
from pathlib import Path

import collector_review as strict


def environment(rows, heap_mib=2048):
    """Review synchronized process counters from retained exact-PID handles."""
    if type(heap_mib) is not int or heap_mib <= 0:
        raise ValueError("invalid heap size")
    if len(rows) < 3:
        raise ValueError("at least three environmental samples required")
    keys = ["monotonic_ns", "system_total_ns", "system_busy_ns",
            "game_cpu_ns", "launcher_cpu_ns", "sampler_cpu_ns"]
    for row in rows:
        for key in keys + ["available_bytes", "game_rss_bytes"]:
            if type(row.get(key)) is not int or row[key] < 0:
                raise ValueError("missing or invalid environmental counter: " + key)
    series = []
    for first, last in zip(rows, rows[1:]):
        delta = {k: last[k] - first[k] for k in keys}
        if any(v < 0 for v in delta.values()) or delta["monotonic_ns"] <= 0 or delta["system_total_ns"] <= 0:
            raise ValueError("reversed or stationary environmental counters")
        if delta["system_busy_ns"] > delta["system_total_ns"]:
            raise ValueError("busy CPU exceeds measured capacity")
        if any(delta[k + "_cpu_ns"] > delta["system_total_ns"] for k in ("game", "launcher", "sampler")):
            raise ValueError("process CPU exceeds measured capacity")
        share = lambda key: 100 * delta[key] / delta["system_total_ns"]
        measured = {k: share(k + "_cpu_ns") for k in ("game", "launcher", "sampler")}
        measured["total"] = share("system_busy_ns")
        measured["residual"] = measured["total"] - sum(measured[k] for k in ("game", "launcher", "sampler"))
        series.append(measured)
    first, last = rows[0], rows[-1]
    capacity = last["system_total_ns"] - first["system_total_ns"]
    means = {k: 100 * (last[k + "_cpu_ns"] - first[k + "_cpu_ns"]) / capacity
             for k in ("game", "launcher", "sampler")}
    means["total"] = 100 * (last["system_busy_ns"] - first["system_busy_ns"]) / capacity
    means["residual"] = means["total"] - sum(means[k] for k in ("game", "launcher", "sampler"))
    return {"protocol": "Practical Performance Protocol v1", "samples": len(rows),
            "elapsedSeconds": (last["monotonic_ns"] - first["monotonic_ns"]) / 1e9,
            "machineCpuMeanPercent": means,
            "intervalCpuRangePercent": {k: [min(x[k] for x in series), max(x[k] for x in series)] for k in means},
            "negativeResidualIntervals": sum(x["residual"] < 0 for x in series),
            "minimumAvailableBytes": min(x["available_bytes"] for x in rows),
            "memoryReservePassed": min(x["available_bytes"] for x in rows) >= (heap_mib + 4096) * 1024**2,
            "gameRssRangeBytes": [min(x["game_rss_bytes"] for x in rows), max(x["game_rss_bytes"] for x in rows)],
            "residualMeaning": "unattributed estimate: includes WebView, attach tools, OS work and sequential-read uncertainty",
            "unrelatedCpuCertified": False, "environmentComparable": None}


def overlay(path, metadata):
    """Add producer-summary reconciliation to the unchanged diagnostic adapter."""
    base = strict.review(path, metadata)
    result = {"protocol": "Practical Performance Protocol v1", "strictReview": base,
              "status": "INVALID", "producerSummaryConsistent": False,
              "practicalTimingOverheadKnown": False, "allRenderedFramesProven": False}
    if base["status"] != "CONSISTENT DIAGNOSTIC":
        return result
    lines = path.read_text(encoding="utf-8-sig").splitlines()
    fields = dict(line[2:].split(": ", 1) for line in lines if line.startswith("# ") and ": " in line)
    rows = list(csv.DictReader(line for line in lines if line and not line.startswith("#")))
    frames = sorted(float(row["frame_ms"]) for row in rows)
    count = len(frames)
    try:
        for row in rows:
            for key in strict.COLUMNS[2:]:
                if key == "gc_time_delta_ms" and row[key] == "":
                    continue
                number = float(row[key])
                if not math.isfinite(number) or number < 0:
                    raise ValueError("invalid exported numeric column: " + key)
            if float(row['inst_fps']) <= 0 or float(row['mem_max_mb']) <= 0 or float(row['mem_used_mb']) > float(row['mem_max_mb']):
                raise ValueError("implausible exported rate or heap values")
            if not 0 <= int(row['stutter_percent']) <= 100:
                raise ValueError("invalid exported stutter percentage")
        if fields["LowMethod"] != "PERCENTILE":
            raise ValueError("unreviewed producer low method")
        duration = int(fields['DurationSec'])
        if not 0 <= duration <= 30:
            raise ValueError("unsupported recording duration")
        if fields['EndReason'] == 'AUTO_DURATION' and metadata['expectedWindowMs'] != [0, duration * 1000]:
            raise ValueError("automatic recording duration differs from declared window")
        if any(not math.isfinite(float(fields[k])) for k in ("AvgFPS", "Low1FPS", "Low01FPS", "MaxSpikeMs")):
            raise ValueError("nonfinite producer summary")
        # Each exported interval is rounded by <=0.0005 ms. Propagate that bound
        # through reciprocals instead of imposing an arbitrary FPS tolerance.
        bounds = {}
        mean = math.fsum(frames) / count
        for key, duration in (("AvgFPS", mean), ("Low1FPS", frames[math.ceil(count * .99) - 1]),
                              ("Low01FPS", frames[math.ceil(count * .999) - 1])):
            if duration <= .0005:
                raise ValueError("interval precision insufficient for reciprocal reconciliation")
            bounds[key] = [1000 / (duration + .0005) - .050001,
                           1000 / (duration - .0005) + .050001]
            if not bounds[key][0] <= float(fields[key]) <= bounds[key][1]:
                raise ValueError("producer summary differs: " + key)
        if abs(float(fields["MaxSpikeMs"]) - max(frames)) > .050501:
            raise ValueError("producer maximum differs")
        threshold = int(fields["StutterThresholdMs"])
        # The original ns value can lie on either side of a rounded threshold.
        lower = sum(v - .0005 >= threshold for v in frames)
        upper = sum(v + .0005 >= threshold for v in frames)
        stutters = int(fields["Stutters"])
        if not lower <= stutters <= upper:
            raise ValueError("producer stutter count differs")
        if int(fields["StutterPercent"]) != math.floor(stutters * 100 / count + .5):
            raise ValueError("producer stutter percentage differs")
        result.update(status="COHERENT INTERNAL DIAGNOSTIC", producerSummaryConsistent=True,
                      producerReciprocalBounds=bounds, metrics=base["metrics"],
                      stuttersAbove50PerMinute=base["metrics"]["intervalsAbove50Ms"] * 60000 / base["metrics"]["summedIntervalMs"],
                      intervalsAmbiguousAt50Ms=sum(abs(v - 50) <= .0005 for v in frames))
    except (ValueError, KeyError, OverflowError) as error:
        result["summaryError"] = str(error)
    return result


def variation(values):
    if len(values) < 3 or any(not math.isfinite(x) or x <= 0 for x in values):
        raise ValueError("three positive finite baseline observations required")
    median = strict.percentile(values, .5)
    return {"runs": len(values), "minimum": min(values), "maximum": max(values),
            "median": median, "rangePercentOfMedian": 100 * (max(values) - min(values)) / median,
            "scope": "observed variation only; not a population confidence interval"}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('csv', type=Path)
    parser.add_argument('metadata', type=Path)
    args = parser.parse_args()
    result = overlay(args.csv, json.loads(args.metadata.read_text(encoding='utf-8')))
    print(json.dumps(result, indent=2, allow_nan=False))
    raise SystemExit(0 if result['status'] == 'COHERENT INTERNAL DIAGNOSTIC' else 2)
