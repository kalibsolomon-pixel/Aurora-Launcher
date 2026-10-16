"""Offline P1 analysis. Standard library only; no launch, capture, or network."""
import argparse
import csv
import hashlib
import json
import math
import random
import re
import statistics
from datetime import datetime
from pathlib import Path


class InvalidData(ValueError):
    pass


def number(value, *, positive=False):
    if isinstance(value, bool):
        raise InvalidData("boolean is not a measurement")
    try:
        result = float(value)
    except (TypeError, ValueError, OverflowError) as exc:
        raise InvalidData("invalid numeric measurement") from exc
    if not math.isfinite(result) or result < 0 or (positive and result == 0):
        raise InvalidData("measurement must be finite and nonnegative")
    return result


def percentile(values, probability):
    """Linear interpolation at (n-1)*p (Hyndman-Fan type 7)."""
    if not values or not 0 <= probability <= 1:
        raise InvalidData("percentile requires samples and p in [0,1]")
    ordered = sorted(number(v) for v in values)
    rank = (len(ordered) - 1) * probability
    lo, hi = math.floor(rank), math.ceil(rank)
    return ordered[lo] + (ordered[hi] - ordered[lo]) * (rank - lo)


def distribution(values):
    if not values:
        return {"count": 0, "mean": None, "median": None, "p95": None,
                "p99": None, "p999": None, "max": None, "variance": None}
    return {"count": len(values), "mean": statistics.mean(values),
            "median": percentile(values, .5), "p95": percentile(values, .95),
            "p99": percentile(values, .99),
            "p999": percentile(values, .999) if len(values) >= 10000 else None,
            "max": max(values), "variance": statistics.pvariance(values)}


def read_csv(path, required):
    with path.open(encoding="utf-8-sig", newline="") as stream:
        reader = csv.DictReader(stream)
        fields = reader.fieldnames or []
        if len(fields) != len(set(fields)) or not set(required) <= set(fields):
            raise InvalidData("CSV missing required or duplicate columns")
        rows = list(reader)
    if not rows or any(None in row or any(v is None for v in row.values()) for row in rows):
        raise InvalidData("empty or structurally malformed CSV")
    return rows


def frames(path, duration):
    rows = read_csv(path, ["timestamp_s", "frame_ms"])
    times = [number(r["timestamp_s"]) for r in rows]
    values = [number(r["frame_ms"], positive=True) for r in rows]
    if any(b <= a for a, b in zip(times, times[1:])) or times[-1] > duration:
        raise InvalidData("frame timestamps must increase inside the window")
    # Adapter timestamps are interval END times, relative to window start.
    previous = 0.0
    for end, interval in zip(times, values):
        if abs((end - previous) * 1000 - interval) > .05:
            raise InvalidData("frame intervals disagree with timestamps")
        previous = end
    if duration - times[-1] > .001:
        raise InvalidData("partial frame capture")
    result = distribution(values)
    result["sustained_fps"] = len(values) * 1000 / sum(values)
    # Fixed, predeclared absolute threshold; consecutive frames count separately.
    result["stutter_frames_over_50ms"] = sum(v > 50 for v in values)
    result["stutter_frames_per_minute"] = result["stutter_frames_over_50ms"] * 60 / duration
    result["stutter_events_over_50ms"] = sum(v > 50 and (i == 0 or values[i - 1] <= 50) for i, v in enumerate(values))
    result["stutter_events_per_minute"] = result["stutter_events_over_50ms"] * 60 / duration
    result["tail_warning"] = "P95/P99 descriptive; small tails are unstable" if len(values) < 1000 else None
    return result


UPTIME = re.compile(r"\[(\d+(?:\.\d+)?)s\]")
GC_TAG = re.compile(r"\[gc\s*\]")
PAUSE = re.compile(r"GC\((\d+)\) (Pause .+?) (\d+(?:\.\d+)?)(ms|us|s)$")
HEAP = re.compile(r"(\d+(?:\.\d+)?)([BKMG])->(\d+(?:\.\d+)?)([BKMG])\((\d+(?:\.\d+)?)([BKMG])\)")


def bytes_value(value, unit):
    return number(value) * 1024 ** "BKMG".index(unit)


def gc_log(path, start, duration):
    """Java 21 HotSpot G1 unified gc summary lines only, not all collectors."""
    pauses, heap_after, seen = [], [], set()
    collector = None
    full = ignored = collections = 0
    for line in path.read_text(encoding="utf-8").splitlines():
        if "Using " in line and GC_TAG.search(line):
            found = line.split("Using ", 1)[1].strip()
            if found != "G1" or collector not in (None, found):
                raise InvalidData("GC parser supports only HotSpot G1")
            collector = found
        if "OutOfMemoryError" in line:
            # GC logs are not a reliable OOM channel. Do not silently infer absence.
            raise InvalidData("OOM marker requires failed-run classification")
        if not GC_TAG.search(line) or "GC(" not in line or "Pause " not in line:
            continue
        match, uptime = PAUSE.search(line.strip()), UPTIME.search(line)
        if not match or not uptime:
            raise InvalidData("malformed G1 pause summary or missing uptime")
        end = number(uptime[1])
        pause = number(match[3]) * {"ms": 1, "us": .001, "s": 1000}[match[4]]
        beginning = end - pause / 1000
        if beginning < start or end > start + duration:
            if end >= start and beginning <= start + duration:
                raise InvalidData("pause crosses measurement boundary; choose a clean window")
            ignored += 1
            continue
        # Remark/Cleanup can share a cycle ID; heap text is not event identity.
        key = (match[1], re.split(r"\s+\d+(?:\.\d+)?[BKMG]->", match[2])[0])
        if key in seen:
            raise InvalidData("duplicate GC pause summary")
        seen.add(key)
        heap = HEAP.search(match[2])
        if "Pause Young" in match[2] or "Pause Full" in match[2]:
            if not heap:
                raise InvalidData("collection pause lacks heap occupancy")
            collections += 1
        if heap:
            before, after, capacity = [bytes_value(heap[i], heap[i + 1]) for i in (1, 3, 5)]
            if max(before, after) > capacity:
                raise InvalidData("GC heap occupancy exceeds capacity")
            heap_after.append(after)
        pauses.append(pause)
        full += "Pause Full" in match[2]
    if collector is None:
        raise InvalidData("collector header missing; retain complete unrotated log")
    return {"collector": collector, "pause_ms": distribution(pauses),
            "pause_count_per_minute": len(pauses) * 60 / duration,
            "total_pause_ms": sum(pauses), "pause_wall_percent": sum(pauses) / (duration * 10),
            "full_gc_count": full, "max_post_gc_used_bytes": max(heap_after, default=None),
            "collection_count": collections, "collections_per_minute": collections * 60 / duration,
            "oom_count": None, "allocation_bytes_per_second": None,
            "gc_cpu_percent": None, "outside_window_pause_count": ignored}


SAMPLE_COLUMNS = ("cpu_percent_one_core", "rss_bytes", "peak_rss_bytes",
                  "heap_committed_bytes", "heap_used_bytes", "system_available_bytes",
                  "background_cpu_percent_one_core", "system_cpu_percent_one_core",
                  "paging_bytes_per_second", "gc_cpu_percent_one_core")


def process_samples(path, duration):
    rows = read_csv(path, ["timestamp_s", "cpu_percent_one_core", "rss_bytes"])
    times = [number(r["timestamp_s"]) for r in rows]
    if len(times) < 2 or any(b <= a for a, b in zip(times, times[1:])):
        raise InvalidData("process samples need increasing timestamps")
    if times[0] != 0 or abs(times[-1] - duration) > .01:
        raise InvalidData("partial process capture")
    result = {}
    for column in SAMPLE_COLUMNS:
        supplied = [r.get(column, "") for r in rows]
        if all(v == "" for v in supplied):
            if column in ("cpu_percent_one_core", "rss_bytes"):
                raise InvalidData("required process metric is empty")
            result[column] = None
            continue
        if any(v == "" for v in supplied):
            raise InvalidData("partially missing process metric")
        values = [number(v) for v in supplied]
        # Values at t[i] describe the following interval; last row is endpoint.
        mean = sum(v * (b - a) for v, a, b in zip(values, times, times[1:])) / duration
        result[column] = {"time_weighted_mean": mean, "max": max(values), "min": min(values)}
    for row in rows:
        if row.get("heap_used_bytes", "") != "" and row.get("heap_committed_bytes", "") != "":
            if number(row["heap_used_bytes"]) > number(row["heap_committed_bytes"]):
                raise InvalidData("used heap exceeds committed heap")
    return result


COHORT = ("sampleKind", "workloadId", "hardware", "os", "runtime", "minecraft",
          "loader", "mods", "graphics", "scene", "environment", "measurementMethods",
          "warmupSeconds", "measurementSeconds")


def analyze(path):
    manifest = json.loads(path.read_text(encoding="utf-8"))
    required = (*COHORT, "schemaVersion", "benchmarkId", "configId", "blockId",
                "timestamp", "status", "jvmArguments", "sources", "measurementStartUptimeSeconds")
    if not isinstance(manifest, dict) or any(k not in manifest for k in required):
        raise InvalidData("manifest missing required fields")
    if type(manifest["schemaVersion"]) is not int or manifest["schemaVersion"] != 1:
        raise InvalidData("unsupported manifest schema")
    for key in ("benchmarkId", "configId", "blockId", "workloadId"):
        if not isinstance(manifest[key], str) or not re.fullmatch(r"[a-zA-Z0-9_-]{1,80}", manifest[key]):
            raise InvalidData("invalid benchmark identifier")
    if manifest["sampleKind"] not in ("synthetic", "measured") or manifest["status"] not in ("complete", "failed", "interrupted"):
        raise InvalidData("invalid run classification")
    if not isinstance(manifest["timestamp"], str):
        raise InvalidData("timestamp must be ISO-8601 text")
    timestamp = datetime.fromisoformat(manifest["timestamp"].replace("Z", "+00:00"))
    if timestamp.tzinfo is None:
        raise InvalidData("timestamp requires timezone")
    for key in ("hardware", "os", "runtime", "minecraft", "loader", "graphics", "scene", "environment", "measurementMethods", "sources"):
        if not isinstance(manifest[key], dict) or not manifest[key]:
            raise InvalidData("manifest requires nonempty condition objects")
    if not isinstance(manifest["mods"], list) or not isinstance(manifest["jvmArguments"], list) or any(not isinstance(a, str) for a in manifest["jvmArguments"]):
        raise InvalidData("manifest inventories must be arrays")
    for key in ("vendor", "version", "build", "manifestSha1"):
        if not isinstance(manifest["runtime"].get(key), str) or not manifest["runtime"][key]:
            raise InvalidData("runtime identity is incomplete")
    duration = number(manifest["measurementSeconds"], positive=True)
    start = number(manifest["measurementStartUptimeSeconds"])
    number(manifest["warmupSeconds"])
    cohort = hashlib.sha256(json.dumps({k: manifest[k] for k in COHORT}, sort_keys=True, allow_nan=False).encode()).hexdigest()
    result = {k: manifest[k] for k in ("benchmarkId", "configId", "blockId", "sampleKind", "status")}
    signature = hashlib.sha256(json.dumps(manifest["jvmArguments"], allow_nan=False).encode()).hexdigest()
    result.update(cohort=cohort, configuration_signature=signature,
                  manifest_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                  source_sha256={}, metrics=None)
    if manifest["status"] != "complete":
        if not isinstance(manifest.get("failureReason"), str) or not manifest["failureReason"]:
            raise InvalidData("failed/interrupted run needs a reason")
        return result
    sources = {}
    for key in ("frames", "gc", "process"):
        name = manifest["sources"].get(key)
        if not isinstance(name, str) or not name or "\\" in name or ":" in name:
            raise InvalidData("source needs a safe run-relative path")
        relative = Path(name)
        if relative.is_absolute() or any(p in (".", "..") for p in name.split("/")):
            raise InvalidData("source escapes run directory")
        source = path.parent / relative
        if not source.resolve().is_relative_to(path.parent.resolve()):
            raise InvalidData("source escapes run directory through a link")
        sources[key] = source
        result["source_sha256"][key] = hashlib.sha256(source.read_bytes()).hexdigest()
    startup = manifest.get("startupSeconds")
    if startup is not None:
        keys = ("jvm", "minecraftInitialization", "mainMenu", "worldLoad")
        if not isinstance(startup, dict) or set(startup) != set(keys):
            raise InvalidData("startup timing fields incomplete")
        startup = {k: None if v is None else number(v) for k, v in startup.items()}
    result["metrics"] = {"frames_ms": frames(sources["frames"], duration),
                         "gc": gc_log(sources["gc"], start, duration),
                         "process": process_samples(sources["process"], duration),
                         "startup_seconds": startup}
    return result


def paired_change_ci(changes):
    if len(changes) < 5:
        return None
    rng = random.Random(20261016)
    estimates = sorted(statistics.median(rng.choices(changes, k=len(changes))) for _ in range(2000))
    # Changes can be negative; percentile() intentionally accepts measurements only.
    def at(p):
        x = (len(estimates) - 1) * p
        return estimates[math.floor(x)] + (estimates[math.ceil(x)] - estimates[math.floor(x)]) * (x % 1)
    return [at(.025), at(.975)]


def compare(runs, baseline):
    complete = [r for r in runs if r["status"] == "complete"]
    if not complete or len({r["cohort"] for r in complete}) != 1:
        raise InvalidData("comparison requires one identical measurement cohort")
    groups = {}
    for run in complete:
        group = groups.setdefault(run["configId"], {})
        if group and next(iter(group.values()))["configuration_signature"] != run["configuration_signature"]:
            raise InvalidData("configuration ID has inconsistent JVM arguments")
        if run["blockId"] in group:
            raise InvalidData("duplicate configuration/block")
        group[run["blockId"]] = run
    if baseline not in groups:
        raise InvalidData("baseline absent")
    output = {"baseline": baseline, "excluded_runs": [r["benchmarkId"] for r in runs if r["status"] != "complete"], "configurations": {}}
    for config, blocks in groups.items():
        pairs = sorted(set(blocks) & set(groups[baseline]))
        values = [r["metrics"]["frames_ms"]["p99"] for r in blocks.values()]
        changes = []
        for block in pairs:
            base = groups[baseline][block]["metrics"]["frames_ms"]["p99"]
            value = blocks[block]["metrics"]["frames_ms"]["p99"]
            changes.append((value / base - 1) * 100)
        metric_paths = {
            "frame_mean_ms": ("frames_ms", "mean"),
            "frame_p95_ms": ("frames_ms", "p95"),
            "frame_p999_ms": ("frames_ms", "p999"),
            "gc_pause_wall_percent": ("gc", "pause_wall_percent"),
            "gc_total_pause_ms": ("gc", "total_pause_ms"),
            "gc_p99_pause_ms": ("gc", "pause_ms", "p99"),
            "cpu_mean_percent_one_core": ("process", "cpu_percent_one_core", "time_weighted_mean"),
            "rss_mean_bytes": ("process", "rss_bytes", "time_weighted_mean"),
            "rss_sampled_peak_bytes": ("process", "rss_bytes", "max"),
            "heap_used_mean_bytes": ("process", "heap_used_bytes", "time_weighted_mean"),
            "heap_committed_mean_bytes": ("process", "heap_committed_bytes", "time_weighted_mean"),
            "main_menu_seconds": ("startup_seconds", "mainMenu"),
            "world_load_seconds": ("startup_seconds", "worldLoad"),
        }
        def metric(run, path):
            value = run["metrics"]
            for key in path:
                if value is None:
                    return None
                value = value[key]
            return value
        other_metrics = {}
        for name, path in metric_paths.items():
            observations = [metric(r, path) for r in blocks.values()]
            observations = [v for v in observations if v is not None]
            relative = []
            for block in pairs:
                b, c = metric(groups[baseline][block], path), metric(blocks[block], path)
                if b is not None and b > 0 and c is not None:
                    relative.append((c / b - 1) * 100)
            other_metrics[name] = {
                "available_runs": len(observations),
                "median_across_runs": statistics.median(observations) if observations else None,
                "run_sd": statistics.stdev(observations) if len(observations) > 1 else None,
                "paired_percent_change_median": statistics.median(relative) if relative else None,
                "paired_bootstrap_95_percent_interval": paired_change_ci(relative),
            }
        output["configurations"][config] = {"usable_runs": len(blocks), "paired_blocks": len(pairs),
            "p99_ms_median_across_runs": statistics.median(values),
            "p99_ms_run_sd": statistics.stdev(values) if len(values) > 1 else None,
            "paired_p99_percent_change_median": statistics.median(changes) if changes else None,
            "paired_bootstrap_95_percent_interval": paired_change_ci(changes),
            "other_metrics": other_metrics,
            "warning": "Exploratory run-level interval; no automatic performance verdict"}
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifests", nargs="+", type=Path)
    parser.add_argument("--baseline", help="compare complete runs by paired block ID")
    args = parser.parse_args()
    try:
        runs = [analyze(p) for p in args.manifests]
        output = compare(runs, args.baseline) if args.baseline else runs
        print(json.dumps(output, indent=2, allow_nan=False))
    except (InvalidData, OSError, ValueError, TypeError, KeyError) as exc:
        # Do not echo raw input, filesystem paths, or manifest contents.
        parser.exit(2, "Invalid benchmark input (" + type(exc).__name__ + "). See the documented schema.\n")


if __name__ == "__main__":
    main()
