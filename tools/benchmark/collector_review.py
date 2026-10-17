"""Offline, diagnostic-only review of Performance Overlay 1.0.2 exports.

Separate from authoritative measured-run schema 1. Never captures or promotes a
metric. Exported row consistency cannot establish eligible-callback completeness.
"""
import argparse
import csv
import hashlib
import io
import json
import math
from pathlib import Path

FORMAT = "performanceoverlay-1.0.2-csv"
JAR_SHA256 = "46c9500f67824996cd81b00a4d3da2dc09d5c6d56495cff98276c961d74804c2"
ARCHIVE_SHA256 = "d385120835fd6b0ad0fb8757601de97b066d9e23023e32733f62e6bd56e13f4f"
WORKLOAD = "workload-b-a-scene-diagnostic-v1"
COLUMNS = "elapsed_ms,frame_ms,inst_fps,fps_smoothed,avg_fps,low1_fps,low01_fps,stutters,stutter_percent,max_spike_ms,gc_time_delta_ms,mem_used_mb,mem_max_mb".split(",")


def percentile(values, p):
    values = sorted(values)
    x = (len(values) - 1) * p
    i = int(x)
    return values[i] + (values[min(i + 1, len(values) - 1)] - values[i]) * (x - i)


def review(path, metadata):
    """Return consistency findings; all metrics describe HUD-callback intervals."""
    result = {"status": "INVALID", "metricRole": "DIAGNOSTIC ONLY",
              "semantic": "eligible-hud-callback-interval", "errors": [],
              "incomplete": [], "warnings": [], "metrics": None,
              "missingOrDroppedSamples": None, "independentCompleteness": False,
              "jfrPreciseAlignment": False, "observerOverheadBounded": False,
              "fullMeasuredRunQuality": "INCOMPLETE"}
    errors, incomplete = result["errors"], result["incomplete"]
    try:
        if not isinstance(metadata, dict):
            raise ValueError("missing source metadata")
        expected = {"format": FORMAT, "sourceJarSha256": JAR_SHA256,
                    "workloadId": WORKLOAD, "worldArchiveSha256": ARCHIVE_SHA256,
                    "timestampUnit": "ms", "frameUnit": "ms"}
        for key, value in expected.items():
            if metadata.get(key) != value:
                errors.append("missing or incorrect " + key)
        if metadata.get("requestedMetricRole", "diagnostic") != "diagnostic":
            errors.append("unjustified metric promotion")
        if metadata.get("jfrAlignment") is not None:
            errors.append("unsupported JFR alignment: export has no bounded epoch anchor")
        if metadata.get("recordingComplete") is not True:
            incomplete.append("recording completion unproved")
        if metadata.get("bufferOverflow") is not False:
            incomplete.append("writer overflow present or unknown")
        size = path.stat().st_size
        if size > 64 * 1024 * 1024:
            raise ValueError("short diagnostic exceeds 64 MiB input bound")
        raw = path.read_bytes()
        result["rawSha256"] = hashlib.sha256(raw).hexdigest()
        if metadata.get("rawSha256") != result["rawSha256"]:
            errors.append("raw hash mismatch")
        text = raw.decode("utf-8-sig")
        lines = text.splitlines()
        if not lines or lines[0] != "# PerformanceOverlay Benchmark":
            raise ValueError("unsupported recording format")
        fields = {}
        data = []
        footer_started = False
        for line in lines:
            if line == "# SUMMARY":
                footer_started = True
            if line.startswith("# ") and ": " in line:
                key, value = line[2:].split(": ", 1)
                if key in fields:
                    raise ValueError("duplicate metadata field")
                fields[key] = value
            elif line and not line.startswith("#"):
                if footer_started:
                    raise ValueError("rows after summary")
                data.append(line)
        for key, expected_value in {"ModVersion": "1.0.2", "Minecraft": "1.21.11"}.items():
            if fields.get(key) != expected_value:
                errors.append("missing or incorrect CSV " + key)
        if not fields.get("Date"):
            errors.append("missing CSV Date")
        if fields.get("EndReason") not in ("MANUAL", "AUTO_DURATION"):
            incomplete.append("missing or unsuccessful end reason")
        if not footer_started or "FramesLogged" not in fields or "FramesSummary" not in fields:
            incomplete.append("missing completion footer")
        reader = csv.DictReader(io.StringIO("\n".join(data)))
        if reader.fieldnames != COLUMNS:
            raise ValueError("unsupported recording columns")
        rows = list(reader)
        if len(rows) < 2:
            raise ValueError("insufficient frame rows")
        elapsed, frames = [], []
        for row in rows:
            if None in row or any(row[k] is None for k in COLUMNS):
                raise ValueError("missing sample value")
            t = int(row["elapsed_ms"])
            frame = float(row["frame_ms"])
            if t < 0 or not math.isfinite(frame) or not 0 < frame <= 30000:
                raise ValueError("invalid frame timestamp or interval")
            elapsed.append(t)
            frames.append(frame)
        for i in range(1, len(rows)):
            if elapsed[i] < elapsed[i - 1]:
                errors.append("nonmonotonic timestamp")
                break
            if abs(elapsed[i] - elapsed[i - 1] - frames[i]) > 1.001:
                errors.append("interval inconsistent with truncated timestamp")
                break
        for key in ("FramesLogged", "FramesSummary"):
            if key in fields and int(fields[key]) != len(rows):
                incomplete.append(key + " differs from exported count (loss or summary truncation)")
        window = metadata.get("expectedWindowMs")
        if not isinstance(window, list) or len(window) != 2 or any(type(x) is not int for x in window):
            errors.append("missing or invalid expected window")
        elif not 0 <= window[0] < window[1] <= 30000:
            errors.append("unsupported short diagnostic window")
        elif elapsed[0] > window[0] + 100 or elapsed[-1] < window[1] - 100:
            incomplete.append("incomplete recording window")
        if metadata.get("warmupEvidenceVerified") is not True:
            result["warnings"].append("warm-up not independently verified; compatibility only")
        ties = sum(a == b for a, b in zip(elapsed, elapsed[1:]))
        identical = sum(a == b for a, b in zip(rows, rows[1:]))
        result["timestampTies"] = ties
        result["indistinguishableAdjacentRows"] = identical
        result["warnings"].extend([
            "integer-ms timestamps may tie; identical rows cannot prove duplication",
            "0.001-ms rounded intervals; no absolute nanoTime or epoch anchor",
            "footer reconciles exported rows, not all Minecraft frames",
            "no independent observer-overhead or physical-display validation"])
        total = math.fsum(frames)
        result["metrics"] = {"frameCount": len(frames), "summedIntervalMs": total,
                             "elapsedTimestampSpanMs": elapsed[-1] - elapsed[0],
                             "internalIntervalRateHz": len(frames) * 1000 / total,
                             "medianMs": percentile(frames, .5),
                             "p95Ms": percentile(frames, .95),
                             "p99Ms": percentile(frames, .99),
                             "maximumMs": max(frames),
                             "intervalsAbove50Ms": sum(v > 50 for v in frames)}
    except (OSError, ValueError, TypeError, KeyError, UnicodeError, csv.Error) as error:
        errors.append(str(error))
    result["status"] = "INVALID" if errors else "INCOMPLETE" if incomplete else "CONSISTENT DIAGNOSTIC"
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("csv", type=Path)
    parser.add_argument("metadata", type=Path)
    args = parser.parse_args()
    result = review(args.csv, json.loads(args.metadata.read_text(encoding="utf-8")))
    print(json.dumps(result, indent=2, allow_nan=False))
    return 0 if result["status"] == "CONSISTENT DIAGNOSTIC" else 2


if __name__ == "__main__":
    raise SystemExit(main())
